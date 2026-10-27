use civitas_core::components::{CitizenMeta, Demographics, MobilityProfile, PhysicalNeeds};
use civitas_core::household::HouseholdDirectory;
use civitas_core::invariants::verify_invariants;
use civitas_core::persistence::{create_snapshot, load_from_file, save_to_file, SaveError};
use civitas_core::settlement::SettlementDirectory;
use civitas_core::sim::Simulation;
use civitas_core::types::{MigrationStatus, ResourceType, SettlementId};
use std::fs::File;
use std::io::{Read, Write};

#[test]
fn test_windows_existing_file_replacement_and_repeated_saves() {
    let temp_dir = std::env::temp_dir();
    let save_path = temp_dir.join("civitas_win_replace_test.civ");

    // Clean up if leftover from previous test run
    let _ = std::fs::remove_file(&save_path);

    let mut sim = Simulation::new(1234, 64, 64, 2, 60);

    // Initial save to new file
    let res1 = save_to_file(&mut sim.world, sim.seed, &save_path);
    assert!(res1.is_ok(), "Initial save failed: {:?}", res1.err());
    assert!(save_path.exists(), "Save file must exist on disk");

    let initial_hash = sim.state_hash();

    // Step simulation forward and overwrite the existing file on Windows repeatedly
    for iteration in 1..=5 {
        sim.run_ticks(10);
        let current_hash = sim.state_hash();
        assert_ne!(
            current_hash, initial_hash,
            "State should have changed after ticks"
        );

        let res = save_to_file(&mut sim.world, sim.seed, &save_path);
        assert!(
            res.is_ok(),
            "Repeated overwrite save failed at iteration {}: {:?}",
            iteration,
            res.err()
        );

        // Verify loaded state matches current in-memory state
        let loaded_snapshot = load_from_file(&save_path).expect("Failed to load overwritten save");
        assert_eq!(
            loaded_snapshot.clock.tick,
            sim.world.resource::<civitas_core::types::SimClock>().tick
        );
        let mut loaded_sim = Simulation::from_snapshot(loaded_snapshot);
        assert_eq!(
            loaded_sim.state_hash(),
            current_hash,
            "Loaded state diverged after overwrite iteration {}",
            iteration
        );
    }

    let _ = std::fs::remove_file(&save_path);
}

#[test]
fn test_failed_save_preserves_previous_valid_state() {
    let temp_dir = std::env::temp_dir();
    let valid_save_path = temp_dir.join("civitas_preserved_valid.civ");
    let _ = std::fs::remove_file(&valid_save_path);

    let mut sim = Simulation::new(4321, 64, 64, 2, 50);
    sim.run_ticks(15);
    let original_hash = sim.state_hash();

    // Save initial valid state
    save_to_file(&mut sim.world, sim.seed, &valid_save_path).expect("Initial valid save failed");

    // Read exact original bytes
    let mut original_bytes = Vec::new();
    File::open(&valid_save_path)
        .unwrap()
        .read_to_end(&mut original_bytes)
        .unwrap();

    // Attempt to save to an invalid non-existent directory
    let invalid_path = temp_dir
        .join("non_existent_subdir_xyz_98765")
        .join("file.civ");
    let fail_res = save_to_file(&mut sim.world, sim.seed, &invalid_path);
    assert!(fail_res.is_err(), "Saving to invalid path MUST fail");

    // Verify original save was completely untouched
    let mut current_bytes = Vec::new();
    File::open(&valid_save_path)
        .unwrap()
        .read_to_end(&mut current_bytes)
        .unwrap();
    assert_eq!(
        original_bytes, current_bytes,
        "Original save file bytes were corrupted or modified during failed operation!"
    );

    // Verify loaded state from original file still matches original hash
    let loaded = load_from_file(&valid_save_path).expect("Original save must load cleanly");
    let mut loaded_sim = Simulation::from_snapshot(loaded);
    assert_eq!(loaded_sim.state_hash(), original_hash);

    let _ = std::fs::remove_file(&valid_save_path);
}

#[test]
fn test_corruption_detection_and_checksum_verification() {
    let temp_dir = std::env::temp_dir();
    let base_path = temp_dir.join("civitas_base_valid.civ");
    let _ = std::fs::remove_file(&base_path);

    let mut sim = Simulation::new(8888, 64, 64, 2, 40);
    sim.run_ticks(10);
    save_to_file(&mut sim.world, sim.seed, &base_path).expect("Save failed");

    let mut valid_bytes = Vec::new();
    File::open(&base_path)
        .unwrap()
        .read_to_end(&mut valid_bytes)
        .unwrap();

    // 1. Invalid Magic Bytes
    {
        let corrupted_magic_path = temp_dir.join("civitas_corrupt_magic.civ");
        let mut bytes = valid_bytes.clone();
        bytes[0..8].copy_from_slice(b"BOGUS123");
        File::create(&corrupted_magic_path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();

        let load_res = load_from_file(&corrupted_magic_path);
        match load_res {
            Err(SaveError::InvalidMagic) => {} // Expected!
            other => panic!("Expected InvalidMagic, got: {:?}", other),
        }
        let _ = std::fs::remove_file(corrupted_magic_path);
    }

    // 2. CRC32 Checksum Mismatch (Corrupt 1 byte in payload)
    {
        let corrupted_payload_path = temp_dir.join("civitas_corrupt_payload.civ");
        let mut bytes = valid_bytes.clone();
        let payload_len = bytes.len();
        bytes[payload_len - 10] ^= 0xFF; // Flip bits in payload
        File::create(&corrupted_payload_path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();

        let load_res = load_from_file(&corrupted_payload_path);
        match load_res {
            Err(SaveError::ChecksumMismatch { .. }) => {} // Expected!
            other => panic!("Expected ChecksumMismatch, got: {:?}", other),
        }
        let _ = std::fs::remove_file(corrupted_payload_path);
    }

    // 3. Truncated Files at various cutoff boundaries
    for &cutoff in &[4, 8, 12, 20, 50, valid_bytes.len() / 2] {
        if cutoff < valid_bytes.len() {
            let truncated_path = temp_dir.join(format!("civitas_trunc_{}.civ", cutoff));
            File::create(&truncated_path)
                .unwrap()
                .write_all(&valid_bytes[..cutoff])
                .unwrap();

            let load_res = load_from_file(&truncated_path);
            assert!(
                load_res.is_err(),
                "Truncated file at {} bytes MUST be rejected without panicking",
                cutoff
            );
            let _ = std::fs::remove_file(truncated_path);
        }
    }

    let _ = std::fs::remove_file(&base_path);
}

#[test]
fn test_deterministic_replay_hash_vs_serialized_equality() {
    let seed = 54321;
    let pop = 100;
    let ticks = 60;

    // Run A
    let mut sim_a = Simulation::new(seed, 64, 64, 4, pop);
    sim_a.run_ticks(ticks);
    let hash_a = sim_a.state_hash();
    let snap_a = create_snapshot(&mut sim_a.world, sim_a.seed);
    let bytes_a = bincode::serialize(&snap_a).unwrap();

    // Run B (Independent fresh start with identical seed)
    let mut sim_b = Simulation::new(seed, 64, 64, 4, pop);
    sim_b.run_ticks(ticks);
    let hash_b = sim_b.state_hash();
    let snap_b = create_snapshot(&mut sim_b.world, sim_b.seed);
    let bytes_b = bincode::serialize(&snap_b).unwrap();

    // Assert both checkpoint-hash equality AND exact serialized bit equality
    assert_eq!(hash_a, hash_b, "Checkpoint state hash must match");
    assert_eq!(
        bytes_a, bytes_b,
        "Exact serialized state bytes must be bit-for-bit identical"
    );

    // Save/Load boundary equality check
    let temp_save = std::env::temp_dir().join("civitas_boundary_test.civ");
    save_to_file(&mut sim_a.world, sim_a.seed, &temp_save).unwrap();

    let snap_loaded = load_from_file(&temp_save).unwrap();
    let mut sim_loaded = Simulation::from_snapshot(snap_loaded);

    // State immediately upon loading must match
    assert_eq!(sim_a.state_hash(), sim_loaded.state_hash());

    // Both continue for 30 more ticks
    sim_a.run_ticks(30);
    sim_loaded.run_ticks(30);

    assert_eq!(
        sim_a.state_hash(),
        sim_loaded.state_hash(),
        "Loaded simulation diverged from continuous simulation after 30 ticks!"
    );

    let snap_cont = create_snapshot(&mut sim_a.world, sim_a.seed);
    let snap_res = create_snapshot(&mut sim_loaded.world, sim_loaded.seed);
    let cont_bytes = bincode::serialize(&snap_cont).unwrap();
    let res_bytes = bincode::serialize(&snap_res).unwrap();

    assert_eq!(
        cont_bytes, res_bytes,
        "Exact serialized bytes diverged across save/load boundary!"
    );

    let _ = std::fs::remove_file(temp_save);
}

#[test]
fn test_adversarial_in_transit_mortality_and_population_conservation() {
    let mut sim = Simulation::new(777, 64, 64, 2, 40);

    // Trigger migration: set up extreme wage and famine imbalance
    {
        let mut s_dir = sim.world.resource_mut::<SettlementDirectory>();
        let s0 = s_dir.get_mut(SettlementId(0)).unwrap();
        s0.inventories.insert(ResourceType::Food, 0.0);
        s0.prices.insert(ResourceType::Food, 100.0);
        let s1 = s_dir.get_mut(SettlementId(1)).unwrap();
        s1.inventories.insert(ResourceType::Food, 20_000.0);
        s1.prices.insert(ResourceType::Food, 1.0);
    }
    {
        let mut h_dir = sim.world.resource_mut::<HouseholdDirectory>();
        for h in h_dir.households.values_mut() {
            if h.settlement_id == SettlementId(0) {
                h.migration_pressure = 20.0;
                h.food_reserve = 0.0;
            }
        }
    }

    // Run until monthly migration evaluation triggers (tick 30)
    sim.run_ticks(30);

    // Verify some citizens are now InTransit
    let mut transit_cids = Vec::new();
    {
        let mut query = sim.world.query::<(&CitizenMeta, &MobilityProfile)>();
        for (meta, mob) in query.iter(&sim.world) {
            if meta.alive && matches!(mob.status, MigrationStatus::InTransit { .. }) {
                transit_cids.push(meta.id);
            }
        }
    }

    assert!(
        !transit_cids.is_empty(),
        "Expected at least 1 citizen in transit under famine"
    );

    // Adversarially induce starvation on the in-transit citizens directly
    {
        let mut query = sim
            .world
            .query::<(&CitizenMeta, &mut PhysicalNeeds, &mut Demographics)>();
        for (meta, mut needs, mut demo) in query.iter_mut(&mut sim.world) {
            if transit_cids.contains(&meta.id) {
                needs.satiety = 0;
                demo.health = 1; // Critical health
            }
        }
    }

    // Step 2 ticks to trigger death while in transit
    sim.run_ticks(2);

    // Verify invariants: NO double-decrement, NO phantom population, NO ghost citizens
    let inv = verify_invariants(&mut sim.world);
    assert!(
        inv.is_ok(),
        "Invariants violated after in-transit mortality: {:?}",
        inv.err()
    );

    // Deceased in-transit citizens must be marked dead
    for &cid in &transit_cids {
        let entity = sim.get_citizen_entity(cid).unwrap();
        let meta = sim.world.get::<CitizenMeta>(entity).unwrap();
        if !meta.alive {
            // Correctly died
            let mob = sim.world.get::<MobilityProfile>(entity).unwrap();
            println!(
                "Verified in-transit citizen #{:?} safely died and satisfied invariants (status: {:?})",
                cid, mob.status
            );
        }
    }
}

#[test]
fn test_adversarial_multi_system_stress_and_resource_conservation() {
    let mut sim = Simulation::new(99999, 128, 128, 4, 300);

    // Run across multiple weekly and monthly boundaries (120 ticks = 4 months)
    for tick in 1..=120 {
        sim.step();

        // Invariant check at every weekly and monthly boundary
        if tick % 7 == 0 || tick % 30 == 0 {
            let inv = verify_invariants(&mut sim.world);
            assert!(
                inv.is_ok(),
                "Invariant failure at tick {}: {:?}",
                tick,
                inv.err()
            );
        }
    }

    let events = sim.events();
    println!(
        "Multi-system stress test completed successfully. Total events emitted: {}",
        events.total_emitted
    );
    assert!(
        events.total_emitted > 0,
        "Simulation should have emitted events"
    );
}
