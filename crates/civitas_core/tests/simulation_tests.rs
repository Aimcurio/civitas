use civitas_core::invariants::verify_invariants;
use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::sim::Simulation;
use civitas_core::types::ResourceType;
use std::fs::File;
use std::io::Write;

#[test]
fn test_world_generation_and_initial_invariants() {
    let mut sim = Simulation::new(42, 64, 64, 4, 100);
    assert_eq!(sim.total_citizens_count(), 100);
    assert_eq!(sim.living_citizens_count(), 100);

    // Initial invariants must be 100% satisfied
    let res = verify_invariants(&mut sim.world);
    assert!(
        res.is_ok(),
        "Invariants failed on initialization: {:?}",
        res.err()
    );
}

#[test]
fn test_determinism_same_seed_produces_identical_hash() {
    let mut sim_a = Simulation::new(12345, 64, 64, 4, 200);
    let mut sim_b = Simulation::new(12345, 64, 64, 4, 200);

    assert_eq!(sim_a.state_hash(), sim_b.state_hash());

    for _ in 0..50 {
        sim_a.step();
        sim_b.step();
    }

    assert_eq!(
        sim_a.state_hash(),
        sim_b.state_hash(),
        "State hashes diverged after 50 ticks despite identical seed!"
    );
}

#[test]
fn test_determinism_different_seed_produces_different_hash() {
    let mut sim_a = Simulation::new(111, 64, 64, 4, 200);
    let mut sim_b = Simulation::new(222, 64, 64, 4, 200);

    for _ in 0..20 {
        sim_a.step();
        sim_b.step();
    }

    assert_ne!(
        sim_a.state_hash(),
        sim_b.state_hash(),
        "Different seeds unexpectedly produced identical state hash!"
    );
}

#[test]
fn test_persistence_roundtrip_and_continuation_parity() {
    let mut sim_cont = Simulation::new(999, 64, 64, 4, 200);
    sim_cont.run_ticks(30);

    let temp_save_path = std::env::temp_dir().join("civitas_test_save.civ");

    // Save to disk
    save_to_file(&mut sim_cont.world, 999, &temp_save_path).expect("Failed to save state");

    // Load from disk
    let snapshot = load_from_file(&temp_save_path).expect("Failed to load snapshot");
    let mut sim_loaded = Simulation::from_snapshot(snapshot);

    assert_eq!(
        sim_cont.state_hash(),
        sim_loaded.state_hash(),
        "Loaded state hash does not match saved state hash!"
    );

    // Continue running both for 30 more ticks
    sim_cont.run_ticks(30);
    sim_loaded.run_ticks(30);

    assert_eq!(
        sim_cont.state_hash(),
        sim_loaded.state_hash(),
        "Loaded simulation diverged from continuous simulation after 30 ticks!"
    );

    let _ = std::fs::remove_file(temp_save_path);
}

#[test]
fn test_corrupted_save_rejection() {
    let temp_corrupted_path = std::env::temp_dir().join("civitas_corrupted.civ");
    {
        let mut file = File::create(&temp_corrupted_path).unwrap();
        // Write bogus data
        file.write_all(b"CIVITAS1garbagepayload").unwrap();
    }

    let load_res = load_from_file(&temp_corrupted_path);
    assert!(load_res.is_err(), "Corrupted save file must be rejected!");

    let _ = std::fs::remove_file(temp_corrupted_path);
}

#[test]
fn test_adversarial_zero_population() {
    let mut sim = Simulation::new(777, 64, 64, 4, 0);
    assert_eq!(sim.total_citizens_count(), 0);

    // Must run smoothly without panics or crashes
    for _ in 0..50 {
        sim.step();
    }

    let inv_res = sim.check_invariants();
    assert!(
        inv_res.is_ok(),
        "Invariants failed for 0-population run: {:?}",
        inv_res.err()
    );
}

#[test]
fn test_adversarial_single_citizen() {
    let mut sim = Simulation::new(888, 64, 64, 4, 1);
    assert_eq!(sim.total_citizens_count(), 1);

    for _ in 0..50 {
        sim.step();
    }

    let inv_res = sim.check_invariants();
    assert!(
        inv_res.is_ok(),
        "Invariants failed for 1-citizen run: {:?}",
        inv_res.err()
    );
}

#[test]
fn test_economic_price_discovery_response() {
    let mut sim = Simulation::new(555, 64, 64, 4, 100);

    // Artificially deplete food inventory in settlement 0
    {
        let mut s_dir = sim
            .world
            .resource_mut::<civitas_core::settlement::SettlementDirectory>();
        let s0 = s_dir.get_mut(civitas_core::types::SettlementId(0)).unwrap();
        s0.inventories.insert(ResourceType::Food, 0.0);
        s0.demand_accumulators.insert(ResourceType::Food, 500.0);
        s0.supply_accumulators.insert(ResourceType::Food, 0.0);
    }

    let initial_price = {
        let s_dir = sim
            .world
            .resource::<civitas_core::settlement::SettlementDirectory>();
        s_dir
            .get(civitas_core::types::SettlementId(0))
            .unwrap()
            .get_price(ResourceType::Food)
    };

    // Run until weekly market price update (tick 7)
    sim.run_ticks(7);

    let updated_price = {
        let s_dir = sim
            .world
            .resource::<civitas_core::settlement::SettlementDirectory>();
        s_dir
            .get(civitas_core::types::SettlementId(0))
            .unwrap()
            .get_price(ResourceType::Food)
    };

    assert!(
        updated_price > initial_price,
        "Food price did not increase under extreme scarcity! Initial: {}, Updated: {}",
        initial_price,
        updated_price
    );
}

#[test]
fn test_demographic_aging_and_death_event_tracing() {
    let mut sim = Simulation::new(333, 64, 64, 4, 50);

    // Run for a full year (360 ticks)
    sim.run_ticks(360);

    let events = sim.world.resource::<civitas_core::events::EventRing>();
    assert!(
        events.total_emitted > 0,
        "No simulation events were emitted after a full year!"
    );

    // Check invariants
    let inv_res = sim.check_invariants();
    assert!(
        inv_res.is_ok(),
        "Invariants violated after 360 ticks: {:?}",
        inv_res.err()
    );
}

#[test]
fn test_migration_wave_under_localized_famine() {
    let mut sim = Simulation::new(404, 64, 64, 4, 200);

    // Starve settlement 0 completely
    {
        let mut s_dir = sim
            .world
            .resource_mut::<civitas_core::settlement::SettlementDirectory>();
        let s0 = s_dir.get_mut(civitas_core::types::SettlementId(0)).unwrap();
        s0.inventories.insert(ResourceType::Food, 0.0);
        s0.prices.insert(ResourceType::Food, 50.0);
        s0.wages
            .insert(civitas_core::types::OccupationType::Farmer, 0.5);

        // Make settlement 1 a booming paradise
        let s1 = s_dir.get_mut(civitas_core::types::SettlementId(1)).unwrap();
        s1.inventories.insert(ResourceType::Food, 50_000.0);
        s1.prices.insert(ResourceType::Food, 1.0);
        s1.wages
            .insert(civitas_core::types::OccupationType::Farmer, 20.0);
    }

    // Set household migration pressure high in settlement 0
    {
        let mut hh_dir = sim
            .world
            .resource_mut::<civitas_core::household::HouseholdDirectory>();
        for hh in hh_dir.households.values_mut() {
            if hh.settlement_id == civitas_core::types::SettlementId(0) {
                hh.food_reserve = 0.0;
                hh.savings = 0.0;
                hh.migration_pressure = 10.0;
            }
        }
    }

    // Run 60 ticks (2 months)
    sim.run_ticks(60);

    let events = sim.world.resource::<civitas_core::events::EventRing>();
    let has_migration = events.events.iter().any(|e| {
        e.event_type == civitas_core::events::SimEventType::MigrationStart
            || e.event_type == civitas_core::events::SimEventType::MigrationArrival
    });

    assert!(
        has_migration,
        "Migration was not triggered under localized famine conditions!"
    );

    let inv_res = sim.check_invariants();
    assert!(
        inv_res.is_ok(),
        "Invariants violated during migration test: {:?}",
        inv_res.err()
    );
}

#[test]
fn test_replay_verification_matches_checkpoints() {
    let seed = 7777;
    let mut sim = Simulation::new(seed, 64, 64, 4, 150);

    let mut checkpoints = std::collections::BTreeMap::new();
    for tick in 1..=100 {
        sim.step();
        if tick % 25 == 0 {
            checkpoints.insert(tick, sim.state_hash());
        }
    }

    // Replay from scratch with same seed
    let mut replay_sim = Simulation::new(seed, 64, 64, 4, 150);
    for tick in 1..=100 {
        replay_sim.step();
        if let Some(&expected_hash) = checkpoints.get(&tick) {
            let actual_hash = replay_sim.state_hash();
            assert_eq!(
                expected_hash, actual_hash,
                "Replay checkpoint hash diverged at tick {}!",
                tick
            );
        }
    }
}

#[test]
fn test_scale_10k_population_invariant_check() {
    let mut sim = Simulation::new(10001, 128, 128, 8, 10_000);
    assert_eq!(sim.living_citizens_count(), 10_000);

    // Run 10 ticks
    sim.run_ticks(10);

    let inv_res = sim.check_invariants();
    assert!(
        inv_res.is_ok(),
        "Invariants violated in 10K population test: {:?}",
        inv_res.err()
    );
}
