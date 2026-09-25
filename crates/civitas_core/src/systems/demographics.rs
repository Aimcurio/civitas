use bevy_ecs::prelude::*;

use crate::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::types::{
    CitizenId, DecisionTrace, Gender, HouseholdRole, MigrationStatus, OccupationType, ReasonCode,
    SimClock,
};

#[derive(Resource, Debug, Clone, Copy)]
pub struct NextCitizenId(pub u64);

pub fn demographics_aging_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &mut CitizenMeta,
        &mut Demographics,
        &mut CausalAudit,
        &SettlementRef,
    )>,
) {
    let tick = clock.tick;

    for (mut meta, mut demo, mut audit, settlement_ref) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        demo.age_ticks = demo.age_ticks.saturating_add(1);
        if demo.age_ticks >= 360 {
            demo.age_ticks = 0;
            demo.age_years += 1;
        }

        // Senescence mortality roll for older citizens
        if demo.age_years > 50 {
            let risk = (demo.age_years as u64 - 50) * 3;
            // Deterministic hash check based on ID, tick, and age
            let roll = (meta.id.0.wrapping_mul(6364136223846793005)
                ^ tick.wrapping_mul(1442695040888963407))
                % 100_000;

            if roll < risk {
                meta.alive = false;
                let trace = DecisionTrace::new(
                    ReasonCode::OldAgeSenescence,
                    tick,
                    demo.age_years as f32,
                    0.0,
                    0,
                );
                audit.trace = trace.clone();

                if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                    settlement.total_deaths += 1;
                    if settlement.population > 0 {
                        settlement.population -= 1;
                    }
                }

                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::Death,
                    entity: Some(meta.id),
                    settlement: Some(settlement_ref.settlement_id),
                    reason: trace,
                });
            }
        }
    }
}

pub fn reproduction_system(
    clock: Res<SimClock>,
    mut commands: Commands,
    mut next_id: ResMut<NextCitizenId>,
    mut events: ResMut<EventRing>,
    mut households: ResMut<HouseholdDirectory>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut Demographics,
        &HouseholdRef,
        &SettlementRef,
        &mut Kinship,
    )>,
) {
    let tick = clock.tick;

    for (meta, mut demo, hh_ref, settlement_ref, mut kinship) in query.iter_mut() {
        if !meta.alive || meta.gender != Gender::Female {
            continue;
        }

        // Sliced reproduction check
        if !(meta.id.0 + tick).is_multiple_of(30) {
            continue;
        }

        if demo.age_years < 18 || demo.age_years > 45 {
            continue;
        }

        if demo.fertility_timer > 0 {
            demo.fertility_timer -= 1;
            continue;
        }

        if let Some(hh) = households.get_mut(hh_ref.household_id) {
            if hh.savings < 20.0 || hh.food_reserve < 5.0 {
                continue;
            }

            // Reproduction roll
            let roll = (meta.id.0.wrapping_mul(2862933555777941757) ^ tick) % 100;
            if roll < 5 {
                // Birth happens!
                let baby_id = CitizenId(next_id.0);
                next_id.0 += 1;

                demo.fertility_timer = 12; // 1 year cooldown
                kinship.children_count += 1;
                hh.add_member(baby_id);

                let baby_gender = if baby_id.0.is_multiple_of(2) {
                    Gender::Female
                } else {
                    Gender::Male
                };

                let trace = DecisionTrace::new(
                    ReasonCode::NaturalBirth,
                    tick,
                    demo.age_years as f32,
                    0.0,
                    meta.id.0 as u32,
                );

                commands.spawn((
                    CitizenMeta {
                        id: baby_id,
                        gender: baby_gender,
                        alive: true,
                    },
                    Demographics {
                        age_years: 0,
                        age_ticks: 0,
                        health: 100,
                        fertility_timer: 0,
                    },
                    HouseholdRef {
                        household_id: hh_ref.household_id,
                        role: HouseholdRole::Child,
                    },
                    SettlementRef {
                        settlement_id: settlement_ref.settlement_id,
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
                        satiety: 100,
                        shelter: 100,
                        comfort: 100,
                    },
                    MobilityProfile {
                        status: MigrationStatus::Settled,
                    },
                    Kinship {
                        spouse: None,
                        parent_a: Some(meta.id),
                        parent_b: kinship.spouse,
                        children_count: 0,
                    },
                    CausalAudit {
                        trace: trace.clone(),
                    },
                ));

                if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                    settlement.population += 1;
                    settlement.total_births += 1;
                }

                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::Birth,
                    entity: Some(baby_id),
                    settlement: Some(settlement_ref.settlement_id),
                    reason: trace,
                });
            }
        }
    }
}
