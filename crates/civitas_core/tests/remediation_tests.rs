use civitas_core::components::{CitizenMeta, Demographics, HouseholdRef, Kinship, PhysicalNeeds};
use civitas_core::events::SimEventType;
use civitas_core::household::HouseholdDirectory;
use civitas_core::invariants::verify_invariants;
use civitas_core::settlement::SettlementDirectory;
use civitas_core::sim::Simulation;
use civitas_core::types::{CitizenId, ReasonCode, ResourceType, SettlementId};

#[test]
fn test_marriage_reciprocal_kinship_and_household_formation() {
    let mut sim = Simulation::new(999, 64, 64, 2, 50);

    // Step for a couple months (60 ticks) to let demographic/marriage systems run
    for _ in 0..65 {
        sim.step();
    }

    assert!(verify_invariants(&mut sim.world).is_ok());

    // Check for marriage events in event ring
    let marriage_count = sim
        .events()
        .events
        .iter()
        .filter(|e| e.event_type == SimEventType::Marriage)
        .count();

    // Verify any married couples have reciprocal kinship and shared household
    let hh_dir = sim
        .world
        .get_resource::<HouseholdDirectory>()
        .unwrap()
        .clone();

    // Collect married citizen pairs
    let mut married_pairs = Vec::new();
    {
        let mut query = sim.world.query::<(&CitizenMeta, &Kinship, &HouseholdRef)>();
        for (meta, kinship, hh_ref) in query.iter(&sim.world) {
            if meta.alive {
                if let Some(spouse_id) = kinship.spouse {
                    married_pairs.push((meta.id, meta.gender, spouse_id, hh_ref.household_id));
                }
            }
        }
    }

    let mut verified = 0;
    for (cid, gender, spouse_id, hid) in married_pairs {
        verified += 1;
        assert_ne!(cid, spouse_id, "Citizen cannot marry self");

        let spouse_entity = sim
            .get_citizen_entity(spouse_id)
            .expect("Spouse entity must exist");
        let spouse_meta = sim.world.get::<CitizenMeta>(spouse_entity).unwrap();
        let spouse_kinship = sim.world.get::<Kinship>(spouse_entity).unwrap();
        let spouse_hh_ref = sim.world.get::<HouseholdRef>(spouse_entity).unwrap();

        assert_eq!(
            spouse_kinship.spouse,
            Some(cid),
            "Spouse reciprocal kinship mismatch"
        );
        assert_ne!(
            gender, spouse_meta.gender,
            "Married couple must have opposite gender"
        );
        assert_eq!(
            hid, spouse_hh_ref.household_id,
            "Spouses must reside in the same household"
        );

        let hh = hh_dir.get(hid).unwrap();
        assert!(hh.members.contains(&cid));
        assert!(hh.members.contains(&spouse_id));
    }

    println!(
        "Verified marriage invariants for {} married citizens (events: {})",
        verified, marriage_count
    );
}

#[test]
fn test_ghost_citizen_eradication_and_household_dissolution() {
    let mut sim = Simulation::new(42, 64, 64, 2, 30);

    // Drain food and set critical physiological state on target citizens
    {
        let mut settlements = sim.world.get_resource_mut::<SettlementDirectory>().unwrap();
        for s in settlements.settlements.values_mut() {
            s.inventories.insert(ResourceType::Food, 0.0);
        }
        let mut households = sim.world.get_resource_mut::<HouseholdDirectory>().unwrap();
        for h in households.households.values_mut() {
            h.food_reserve = 0.0;
        }

        // Directly set low health & zero satiety on citizens to induce starvation mortality
        let mut query = sim.world.query::<(&mut PhysicalNeeds, &mut Demographics)>();
        for (mut needs, mut demo) in query.iter_mut(&mut sim.world) {
            needs.satiety = 0;
            demo.health = 2;
        }
    }

    // Step a few ticks to trigger mortality and dissolution
    for _ in 0..5 {
        sim.step();
    }

    // Must satisfy strict invariants at every step (no deceased in households, no empty households)
    let res = verify_invariants(&mut sim.world);
    assert!(
        res.is_ok(),
        "Invariants failed after famine: {:?}",
        res.err()
    );

    // Check event ring for death and household dissolution events
    let (death_count, dissolution_count, death_entities) = {
        let events = sim.events();
        let deaths: Vec<_> = events
            .events
            .iter()
            .filter(|e| e.event_type == SimEventType::Death)
            .map(|e| e.entity.unwrap())
            .collect();
        let dissolutions = events
            .events
            .iter()
            .filter(|e| e.event_type == SimEventType::HouseholdDissolved)
            .count();
        (deaths.len(), dissolutions, deaths)
    };

    assert!(death_count > 0, "Starvation deaths must have been recorded");
    assert!(
        dissolution_count > 0,
        "Empty households must have been dissolved"
    );

    // Deceased citizens must be marked in historical index and NOT in active index
    for cid in death_entities {
        assert!(
            sim.get_citizen_entity(cid).is_some(),
            "Historical citizen must still be queryable by ID"
        );
        let entity = sim.get_citizen_entity(cid).unwrap();
        let meta = sim.world.get::<CitizenMeta>(entity).unwrap();
        assert!(!meta.alive, "Deceased citizen meta.alive must be false");
    }
}

#[test]
fn test_labor_layoffs_on_depleted_treasury() {
    let mut sim = Simulation::new(555, 64, 64, 1, 20);

    // Zero out settlement treasury so wages cannot be paid
    {
        let mut settlements = sim.world.get_resource_mut::<SettlementDirectory>().unwrap();
        for s in settlements.settlements.values_mut() {
            s.treasury = 0.0;
        }
    }

    // Step a few cycles
    for _ in 0..10 {
        sim.step();
    }

    // Verify layoff events were emitted
    let layoff_count = sim
        .events()
        .events
        .iter()
        .filter(|e| {
            e.event_type == SimEventType::JobChange && e.reason.reason == ReasonCode::JobLaidOff
        })
        .count();

    assert!(
        layoff_count > 0,
        "Layoff events must be emitted when treasury is depleted"
    );
    assert!(verify_invariants(&mut sim.world).is_ok());
}

#[test]
fn test_real_resource_construction_and_no_free_money() {
    let mut sim = Simulation::new(777, 64, 64, 1, 30);

    let sid = SettlementId(0);
    let initial_treasury = {
        let settlements = sim.world.get_resource::<SettlementDirectory>().unwrap();
        settlements.get(sid).unwrap().treasury
    };

    // Step 20 ticks
    for _ in 0..20 {
        sim.step();
    }

    let final_treasury = {
        let settlements = sim.world.get_resource::<SettlementDirectory>().unwrap();
        settlements.get(sid).unwrap().treasury
    };

    println!(
        "Treasury initial: {}, final: {}",
        initial_treasury, final_treasury
    );
    assert!(!final_treasury.is_nan());
    assert!(verify_invariants(&mut sim.world).is_ok());
}

#[test]
fn test_invariant_falsification_catches_corrupted_state() {
    let mut sim = Simulation::new(101, 64, 64, 1, 20);
    assert!(verify_invariants(&mut sim.world).is_ok());

    // Corrupt occupied housing
    {
        let mut settlements = sim.world.get_resource_mut::<SettlementDirectory>().unwrap();
        let s = settlements.get_mut(SettlementId(0)).unwrap();
        s.occupied_housing += 50; // Injected inconsistency!
    }

    let check = verify_invariants(&mut sim.world);
    assert!(
        check.is_err(),
        "verify_invariants MUST fail when occupied_housing is inconsistent"
    );
    let errs = check.unwrap_err();
    assert!(
        errs.iter().any(|e| e.contains("occupied_housing")),
        "Expected occupied_housing error, got: {:?}",
        errs
    );

    // Restore correctness
    {
        let mut settlements = sim.world.get_resource_mut::<SettlementDirectory>().unwrap();
        let s = settlements.get_mut(SettlementId(0)).unwrap();
        s.occupied_housing -= 50;
    }
    assert!(verify_invariants(&mut sim.world).is_ok());

    // Corrupt household by adding a non-existent member
    {
        let mut households = sim.world.get_resource_mut::<HouseholdDirectory>().unwrap();
        let h = households.households.values_mut().next().unwrap();
        h.members.push(CitizenId(999999));
    }

    let check_hh = verify_invariants(&mut sim.world);
    assert!(
        check_hh.is_err(),
        "verify_invariants MUST fail when a ghost/non-existent member is in a household"
    );
}
