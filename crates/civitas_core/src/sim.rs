use bevy_ecs::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use crate::events::EventRing;
use crate::household::{Household, HouseholdDirectory};
use crate::invariants::verify_invariants;
use crate::persistence::SimulationSnapshot;
use crate::replay::compute_authoritative_state_hash;
use crate::settlement::{Settlement, SettlementDirectory};
use crate::systems::demographics::{demographics_aging_system, reproduction_system, NextCitizenId};
use crate::systems::labor::labor_market_system;
use crate::systems::market::{household_consumption_system, market_price_update_system};
use crate::systems::migration::{migration_evaluation_system, migration_transit_system};
use crate::systems::physiology::physiology_system;
use crate::systems::production::production_system;
use crate::types::{
    CitizenId, DecisionTrace, Gender, HouseholdId, HouseholdRole, MigrationStatus, OccupationType,
    ReasonCode, SimClock,
};
use crate::world::WorldMap;

pub struct Simulation {
    pub world: World,
    pub daily_schedule: Schedule,
    pub weekly_schedule: Schedule,
    pub monthly_schedule: Schedule,
    pub seed: u64,
    pub initial_population: usize,
}

impl Simulation {
    pub fn new(
        seed: u64,
        map_width: u32,
        map_height: u32,
        settlement_count: usize,
        initial_population: usize,
    ) -> Self {
        let mut world = World::new();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        // 1. Generate World Map
        let (world_map, settlement_nodes) =
            WorldMap::generate(map_width, map_height, settlement_count, &mut rng);

        // 2. Initialize Settlement Directory
        let mut settlements = SettlementDirectory::new();
        let per_settlement_pop = (initial_population / settlement_nodes.len().max(1)).max(1);
        let capacity_per_settlement = (per_settlement_pop as u32) * 2 + 500;

        for (sid, x, y) in &settlement_nodes {
            let name = format!("Settlement-{}", sid.0);
            let s = Settlement::new(*sid, name, *x, *y, capacity_per_settlement);
            settlements.settlements.insert(*sid, s);
        }

        // 3. Initialize Households and Citizens
        let households = HouseholdDirectory::new();
        let events = EventRing::new(10_000);
        let clock = SimClock::new();

        // Register Resources
        world.insert_resource(clock);
        world.insert_resource(world_map);
        world.insert_resource(settlements);
        world.insert_resource(households);
        world.insert_resource(events);
        world.insert_resource(NextCitizenId(initial_population as u64 + 1));

        // Spawn Initial Citizens in batch
        let mut current_hh_id = HouseholdId(1);
        let mut current_hh_members = 0;
        let mut target_settlement_idx = 0;

        let num_nodes = settlement_nodes.len().max(1);

        for i in 1..=(initial_population as u64) {
            let cid = CitizenId(i);
            let sid = settlement_nodes[target_settlement_idx % num_nodes].0;
            let gender = if rng.gen_bool(0.5) {
                Gender::Female
            } else {
                Gender::Male
            };

            let assigned_hh_id = current_hh_id;
            let role = if current_hh_members == 0 {
                HouseholdRole::Head
            } else if current_hh_members == 1 {
                HouseholdRole::Spouse
            } else {
                HouseholdRole::Child
            };

            // Household grouping: 3-4 citizens per household
            if current_hh_members == 0 {
                let mut hh = Household::new(assigned_hh_id, sid, cid);
                hh.savings = 100.0;
                hh.food_reserve = 25.0;
                world.resource_mut::<HouseholdDirectory>().insert(hh);
            } else {
                if let Some(hh) = world
                    .resource_mut::<HouseholdDirectory>()
                    .get_mut(assigned_hh_id)
                {
                    hh.add_member(cid);
                }
            }

            current_hh_members += 1;
            if current_hh_members >= 4 {
                current_hh_members = 0;
                current_hh_id = HouseholdId(current_hh_id.0 + 1);
                target_settlement_idx += 1;
            }

            let occ = match i % 6 {
                0 => OccupationType::Farmer,
                1 => OccupationType::Farmer,
                2 => OccupationType::Forester,
                3 => OccupationType::Miner,
                4 => OccupationType::Artisan,
                _ => OccupationType::Laborer,
            };

            let initial_age = rng.gen_range(18..55);
            let initial_savings = rng.gen_range(10.0..50.0);
            let trace = DecisionTrace::new(ReasonCode::InitialSpawn, 0, 0.0, 0.0, 0);

            world.spawn((
                CitizenMeta {
                    id: cid,
                    gender,
                    alive: true,
                },
                Demographics {
                    age_years: initial_age,
                    age_ticks: 0,
                    health: 100,
                    fertility_timer: 0,
                },
                HouseholdRef {
                    household_id: assigned_hh_id,
                    role,
                },
                SettlementRef {
                    settlement_id: sid,
                    district_id: 0,
                },
                OccupationProfile {
                    occupation: occ,
                    skill_level: 1,
                    experience: 0,
                    productivity: 1.1,
                },
                PersonalFinances {
                    savings: initial_savings,
                    last_income: 3.0,
                    last_expense: 0.0,
                },
                PhysicalNeeds {
                    satiety: 100,
                    shelter: 100,
                    comfort: 100,
                },
                MobilityProfile {
                    status: MigrationStatus::Settled,
                },
                Kinship {
                    spouse: None,
                    parent_a: None,
                    parent_b: None,
                    children_count: 0,
                },
                CausalAudit { trace },
            ));

            if let Some(settlement) = world.resource_mut::<SettlementDirectory>().get_mut(sid) {
                settlement.population += 1;
                settlement.occupied_housing += 1;
                *settlement.job_headcounts.entry(occ).or_insert(0) += 1;
            }
        }

        world.resource_mut::<HouseholdDirectory>().next_id = current_hh_id.0 + 1;

        // Initialize Bevy Schedules
        let mut daily = Schedule::default();
        daily.add_systems((
            migration_transit_system,
            production_system,
            physiology_system,
            demographics_aging_system,
        ));

        let mut weekly = Schedule::default();
        weekly.add_systems((
            market_price_update_system,
            household_consumption_system,
            labor_market_system,
        ));

        let mut monthly = Schedule::default();
        monthly.add_systems((migration_evaluation_system, reproduction_system));

        Self {
            world,
            daily_schedule: daily,
            weekly_schedule: weekly,
            monthly_schedule: monthly,
            seed,
            initial_population,
        }
    }

    pub fn from_snapshot(snapshot: SimulationSnapshot) -> Self {
        let mut world = World::new();

        world.insert_resource(snapshot.clock);
        world.insert_resource(snapshot.world_map);
        world.insert_resource(snapshot.settlements);
        world.insert_resource(snapshot.households);
        world.insert_resource(snapshot.events);
        world.insert_resource(NextCitizenId(snapshot.next_citizen_id));

        for c in snapshot.citizens {
            world.spawn((
                c.meta,
                c.demographics,
                c.household_ref,
                c.settlement_ref,
                c.occupation,
                c.finances,
                c.needs,
                c.mobility,
                c.kinship,
                c.causal_audit,
            ));
        }

        let mut daily = Schedule::default();
        daily.add_systems((
            migration_transit_system,
            production_system,
            physiology_system,
            demographics_aging_system,
        ));

        let mut weekly = Schedule::default();
        weekly.add_systems((
            market_price_update_system,
            household_consumption_system,
            labor_market_system,
        ));

        let mut monthly = Schedule::default();
        monthly.add_systems((migration_evaluation_system, reproduction_system));

        Self {
            world,
            daily_schedule: daily,
            weekly_schedule: weekly,
            monthly_schedule: monthly,
            seed: snapshot.seed,
            initial_population: 0,
        }
    }

    pub fn step(&mut self) -> u64 {
        // Increment clock
        let tick = {
            let mut clock = self.world.resource_mut::<SimClock>();
            clock.tick += 1;
            clock.tick
        };

        // Execute Daily schedule
        self.daily_schedule.run(&mut self.world);

        // Execute Weekly schedule every 7 ticks
        if tick % 7 == 0 {
            self.weekly_schedule.run(&mut self.world);
        }

        // Execute Monthly schedule every 30 ticks
        if tick % 30 == 0 {
            self.monthly_schedule.run(&mut self.world);
        }

        tick
    }

    pub fn run_ticks(&mut self, count: u64) {
        for _ in 0..count {
            self.step();
        }
    }

    pub fn living_citizens_count(&mut self) -> usize {
        let mut query = self.world.query::<&CitizenMeta>();
        query.iter(&self.world).filter(|m| m.alive).count()
    }

    pub fn total_citizens_count(&mut self) -> usize {
        let mut query = self.world.query::<&CitizenMeta>();
        query.iter(&self.world).count()
    }

    pub fn state_hash(&mut self) -> u64 {
        compute_authoritative_state_hash(&mut self.world)
    }

    pub fn check_invariants(&mut self) -> Result<(), Vec<String>> {
        verify_invariants(&mut self.world)
    }
}
