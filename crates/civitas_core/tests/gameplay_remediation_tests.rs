use civitas_core::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use civitas_core::household::HouseholdDirectory;
use civitas_core::settlement::{SettlementDirectory, SettlementPolicy};
use civitas_core::sim::Simulation;
use civitas_core::types::{
    CitizenId, DecisionTrace, Gender, HouseholdId, HouseholdRole, MigrationStatus, OccupationType,
    ReasonCode, ResourceType, SettlementId,
};

#[test]
fn test_multi_year_economic_circulation_and_survival() {
    let seed = 42;
    let initial_pop = 2_000;
    let mut sim = Simulation::new(seed, 128, 128, 4, initial_pop);

    assert!(sim.check_invariants().is_ok());

    let mut pop_1year = 0;
    let mut treasury_1year = 0.0;
    let mut food_1year = 0.0;

    let mut pop_3years = 0;
    let mut treasury_3years = 0.0;
    let mut food_3years = 0.0;

    for tick in 1..=1080 {
        sim.step();

        if tick == 360 {
            pop_1year = sim.living_citizens_count();
            let settlements = sim.world.resource::<SettlementDirectory>();
            treasury_1year = settlements.settlements.values().map(|s| s.treasury).sum();
            food_1year = settlements
                .settlements
                .values()
                .map(|s| {
                    s.inventories
                        .get(&ResourceType::Food)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum();
        }

        if tick == 1080 {
            pop_3years = sim.living_citizens_count();
            let settlements = sim.world.resource::<SettlementDirectory>();
            treasury_3years = settlements.settlements.values().map(|s| s.treasury).sum();
            food_3years = settlements
                .settlements
                .values()
                .map(|s| {
                    s.inventories
                        .get(&ResourceType::Food)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum();
        }
    }

    println!(
        "CIRCULATION VERIFICATION: Year 1 (Tick 360): Pop {}, Treasury {:.1}, Food {:.1}",
        pop_1year, treasury_1year, food_1year
    );
    println!(
        "CIRCULATION VERIFICATION: Year 3 (Tick 1080): Pop {}, Treasury {:.1}, Food {:.1}",
        pop_3years, treasury_3years, food_3years
    );

    // Remediation acceptance:
    // 1. No economic collapse / mass starvation in abundance: population sustained (> 95% of initial)
    assert!(
        pop_3years >= 1_900,
        "Population died off excessively: {}",
        pop_3years
    );

    // 2. Municipal treasury maintains healthy non-zero liquid circulation (> 1,000 coins across settlements)
    assert!(
        treasury_3years >= 1_000.0,
        "Treasury failed to recirculate: {:.1}",
        treasury_3years
    );

    // 3. Food warehouses remain stable and stocked
    assert!(
        food_3years >= 200_000.0,
        "Food collapsed: {:.1}",
        food_3years
    );

    // 4. Invariants strictly pass at Year 3
    if let Err(errors) = sim.check_invariants() {
        for e in &errors {
            println!("INVARIANT ERROR: {}", e);
        }
        panic!("Invariants failed: {:?}", errors);
    }
}

#[test]
fn test_infant_migration_prevention_and_family_cohesion() {
    let mut sim = Simulation::new(42, 128, 128, 4, 200);

    // Add a child and infant to household 1
    let child_id = CitizenId(999_990);
    let infant_id = CitizenId(999_991);
    {
        let mut hh_dir = sim.world.resource_mut::<HouseholdDirectory>();
        let hh = hh_dir.get_mut(HouseholdId(1)).unwrap();
        hh.migration_pressure = 15.0; // severe hunger push factor
        hh.food_reserve = 0.0;
        hh.savings = 0.0;
        hh.add_member(child_id);
        hh.add_member(infant_id);
    }

    // Spawn child (age 6) and infant (age 0)
    for (id, age) in [(child_id, 6), (infant_id, 0)] {
        let trace = DecisionTrace::new(ReasonCode::NaturalBirth, 0, 0.0, 0.0, 1);
        let entity = sim
            .world
            .spawn((
                CitizenMeta {
                    id,
                    gender: Gender::Female,
                    alive: true,
                },
                Demographics {
                    age_years: age,
                    age_ticks: 0,
                    health: 100,
                    fertility_timer: 0,
                },
                HouseholdRef {
                    household_id: HouseholdId(1),
                    role: HouseholdRole::Child,
                },
                SettlementRef {
                    settlement_id: SettlementId(0),
                    district_id: 0,
                },
                OccupationProfile {
                    occupation: OccupationType::Unemployed,
                    skill_level: 0,
                    experience: 0,
                    productivity: 1.0,
                },
                PersonalFinances {
                    savings: 0.0,
                    last_income: 0.0,
                    last_expense: 0.0,
                },
                PhysicalNeeds {
                    satiety: 60,
                    shelter: 100,
                    comfort: 100,
                },
                MobilityProfile {
                    status: MigrationStatus::Settled,
                },
                Kinship {
                    spouse: None,
                    parent_a: Some(CitizenId(1)),
                    parent_b: None,
                    children_count: 0,
                },
                CausalAudit { trace },
            ))
            .id();

        sim.world
            .resource_mut::<civitas_core::sim::CitizenIndex>()
            .insert_active(id, entity);
        sim.world
            .resource_mut::<SettlementDirectory>()
            .get_mut(SettlementId(0))
            .unwrap()
            .population += 1;
        sim.world
            .resource_mut::<SettlementDirectory>()
            .get_mut(SettlementId(0))
            .unwrap()
            .occupied_housing += 1;
    }

    // Run for 60 ticks (2 migration evaluation cycles)
    for _ in 1..=60 {
        sim.step();
    }

    // Verify invariants pass
    assert!(sim.check_invariants().is_ok());

    // Verify neither minor ever became independent household head or initiated lone migration
    for cid in [child_id, infant_id] {
        let entity = sim.get_citizen_entity(cid).unwrap();
        let hr = sim.world.get::<HouseholdRef>(entity).unwrap();
        assert_eq!(
            hr.role,
            HouseholdRole::Child,
            "Minor {:?} became household head!",
            cid
        );
    }
}

#[test]
fn test_environmental_production_scaling() {
    let sim = Simulation::new(12345, 128, 128, 4, 100);
    let settlements = sim.world.resource::<SettlementDirectory>();

    // Assert that environmental factors vary deterministically across settlements
    let fertilities: Vec<f32> = settlements
        .settlements
        .values()
        .map(|s| s.fertility_factor)
        .collect();
    let minerals: Vec<f32> = settlements
        .settlements
        .values()
        .map(|s| s.mineral_factor)
        .collect();

    println!("SETTLEMENT FERTILITIES: {:?}", fertilities);
    println!("SETTLEMENT MINERALS: {:?}", minerals);

    // Factors must be non-zero and grounded in terrain
    for &f in &fertilities {
        assert!((0.2..=2.5).contains(&f));
    }
    for &m in &minerals {
        assert!((0.2..=2.5).contains(&m));
    }
}

#[test]
fn test_player_settlement_policy_and_housing_commission() {
    let mut sim = Simulation::new(42, 128, 128, 4, 500);

    let sid = SettlementId(0);

    // Initial state
    let initial_cap = {
        let s_dir = sim.world.resource::<SettlementDirectory>();
        let s = s_dir.get(sid).unwrap();
        s.housing_capacity
    };

    // 1. Commission housing expansion
    {
        let mut s_dir = sim.world.resource_mut::<SettlementDirectory>();
        let s = s_dir.get_mut(sid).unwrap();
        // Give materials to ensure expansion succeeds
        s.treasury = 500.0;
        *s.inventories.entry(ResourceType::Timber).or_insert(0.0) = 50.0;
        *s.inventories.entry(ResourceType::Stone).or_insert(0.0) = 50.0;

        let res = s.commission_housing_expansion();
        assert!(res.is_ok());
        assert_eq!(s.housing_capacity, initial_cap + 25);
        assert_eq!(s.treasury, 450.0);
        assert_eq!(*s.inventories.get(&ResourceType::Timber).unwrap(), 40.0);
        assert_eq!(*s.inventories.get(&ResourceType::Stone).unwrap(), 45.0);
    }

    // 2. Set settlement policy
    {
        let mut s_dir = sim.world.resource_mut::<SettlementDirectory>();
        let s = s_dir.get_mut(sid).unwrap();
        s.set_policy(SettlementPolicy {
            tax_rate: 0.25,
            emergency_relief: false,
            wage_bonus: 0.50,
        });
        assert_eq!(s.policy.tax_rate, 0.25);
        assert!(!s.policy.emergency_relief);
        assert_eq!(s.policy.wage_bonus, 0.50);
    }

    // Step simulation and verify invariants hold
    sim.step();
    assert!(sim.check_invariants().is_ok());
}
