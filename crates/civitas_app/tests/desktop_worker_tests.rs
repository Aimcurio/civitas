use civitas_app::worker::{SimCommand, WorkerHandle, WorkerResponse};
use civitas_core::types::CitizenId;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn test_worker_lifecycle_and_presentation_snapshot() {
    let worker = WorkerHandle::new(42, 1000);

    // Initial snapshot check
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(snap.living_population, 1000);
        assert!(!snap.is_running);
        assert_eq!(snap.speed, 1);
        assert_eq!(snap.settlements.settlements.len(), 8);
        assert!(snap.total_households_count > 0);
    }

    // Play simulation asynchronously
    assert!(worker.send_command(SimCommand::Play));

    // Wait briefly for worker thread to advance
    let start = Instant::now();
    let mut advanced = false;
    while start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(50));
        let snap = worker.snapshot.lock().unwrap().clone();
        if snap.tick > 0 {
            advanced = true;
            break;
        }
    }
    assert!(advanced, "Worker thread failed to advance simulation ticks");

    // Change speed
    assert!(worker.send_command(SimCommand::SetSpeed(5)));
    std::thread::sleep(Duration::from_millis(100));
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(snap.speed, 5);
    }

    // Pause simulation
    assert!(worker.send_command(SimCommand::Pause));
    std::thread::sleep(Duration::from_millis(100));

    let paused_tick = {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert!(!snap.is_running);
        snap.tick
    };

    std::thread::sleep(Duration::from_millis(100));
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(
            snap.tick, paused_tick,
            "Simulation should not advance while paused"
        );
    }
}

#[test]
fn test_worker_step_and_citizen_detail_query() {
    let worker = WorkerHandle::new(123, 500);

    // Step 5 ticks
    assert!(worker.send_command(SimCommand::Step(5)));

    // Wait for step response
    let mut step_done = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::StatusMessage(msg)) = worker.try_recv_response() {
            if msg.contains("Stepped 5 ticks") {
                step_done = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(step_done, "Worker failed to step requested ticks");

    // Query valid citizen details
    assert!(worker.send_command(SimCommand::RequestCitizenDetails(CitizenId(1))));
    let mut details_received = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::CitizenDetails(maybe_view)) = worker.try_recv_response() {
            let view = maybe_view.expect("Citizen #1 details must be present");
            assert_eq!(view.meta.id, CitizenId(1));
            assert!(view.meta.alive);
            assert!(view.needs.satiety > 0 && view.needs.satiety <= 100);
            assert!(!view.causal_audit.trace.to_human_explanation().is_empty());
            details_received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(details_received, "Failed to receive citizen detail view");

    // Query non-existent citizen
    assert!(worker.send_command(SimCommand::RequestCitizenDetails(CitizenId(999999))));
    let mut none_received = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::CitizenDetails(maybe_view)) = worker.try_recv_response() {
            assert!(maybe_view.is_none());
            none_received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(none_received, "Expected None for non-existent citizen");
}

#[test]
fn test_worker_save_load_roundtrip_and_error_recovery() {
    let worker = WorkerHandle::new(777, 400);

    let temp_save = std::env::temp_dir().join("civitas_worker_test.civ");
    let _ = std::fs::remove_file(&temp_save);

    // Step 10 ticks
    assert!(worker.send_command(SimCommand::Step(10)));
    std::thread::sleep(Duration::from_millis(100));

    let saved_tick = {
        let snap = worker.snapshot.lock().unwrap().clone();
        snap.tick
    };

    // Save state
    assert!(worker.send_command(SimCommand::Save(temp_save.clone())));
    let mut save_confirmed = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::StatusMessage(msg)) = worker.try_recv_response() {
            if msg.contains("Saved snapshot") {
                save_confirmed = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(save_confirmed, "Failed to confirm worker save");
    assert!(temp_save.exists(), "Save file must exist on disk");

    // Step 15 more ticks
    assert!(worker.send_command(SimCommand::Step(15)));
    std::thread::sleep(Duration::from_millis(100));
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert!(snap.tick > saved_tick);
    }

    // Load state back
    assert!(worker.send_command(SimCommand::Load(temp_save.clone())));
    let mut load_confirmed = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::StatusMessage(msg)) = worker.try_recv_response() {
            if msg.contains("Loaded snapshot") {
                load_confirmed = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(load_confirmed, "Failed to confirm worker load");

    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(
            snap.tick, saved_tick,
            "Loaded snapshot tick must match saved tick"
        );
    }

    // Test error recovery on invalid path
    let invalid_path = PathBuf::from("Z:\\non_existent_drive_987\\invalid.civ");
    assert!(worker.send_command(SimCommand::Save(invalid_path)));
    let mut error_reported = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::StatusMessage(msg)) = worker.try_recv_response() {
            if msg.contains("Save error") {
                error_reported = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        error_reported,
        "Worker must report error when saving to invalid path"
    );

    // Verify worker thread is still alive and responsive after error
    assert!(worker.send_command(SimCommand::Step(1)));
    std::thread::sleep(Duration::from_millis(50));
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(snap.tick, saved_tick + 1);
    }

    let _ = std::fs::remove_file(temp_save);
}

#[test]
fn test_worker_world_generation_and_invariants() {
    let worker = WorkerHandle::new(555, 300);

    // Verify initial invariants
    assert!(worker.send_command(SimCommand::VerifyInvariants));
    let mut inv_ok = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::VerificationResult(res)) = worker.try_recv_response() {
            assert!(res.is_ok(), "Initial invariants failed: {:?}", res.err());
            inv_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(inv_ok, "Did not receive initial invariant verification");

    // Generate new world
    assert!(worker.send_command(SimCommand::GenerateWorld {
        seed: 9999,
        population: 800,
    }));
    let mut gen_confirmed = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::StatusMessage(msg)) = worker.try_recv_response() {
            if msg.contains("Generated new world") {
                gen_confirmed = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(gen_confirmed, "Failed to confirm world generation");

    // Verify updated population and invariant satisfaction
    {
        let snap = worker.snapshot.lock().unwrap().clone();
        assert_eq!(snap.living_population, 800);
        assert_eq!(snap.tick, 0);
    }

    assert!(worker.send_command(SimCommand::VerifyInvariants));
    let mut post_gen_inv_ok = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(WorkerResponse::VerificationResult(res)) = worker.try_recv_response() {
            assert!(
                res.is_ok(),
                "Post-generation invariants failed: {:?}",
                res.err()
            );
            post_gen_inv_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(post_gen_inv_ok, "Did not receive post-generation invariant check");
}
