use bevy_ecs::prelude::*;

use crate::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::household::{Household, HouseholdDirectory};
use crate::settlement::SettlementDirectory;
use crate::sim::CitizenIndex;
use crate::types::{
    CitizenId, DecisionTrace, Gender, HouseholdId, HouseholdRole, MigrationStatus, OccupationType,
    ReasonCode, ResourceType, SettlementId, SimClock,
};

#[derive(Resource, Debug, Clone, Copy)]
pub struct NextCitizenId(pub u64);

#[allow(clippy::type_complexity)]
pub fn demographics_aging_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut households: ResMut<HouseholdDirectory>,
    mut citizen_index: ResMut<CitizenIndex>,
    mut query: Query<(
        &mut CitizenMeta,
        &mut Demographics,
        &mut CausalAudit,
        &SettlementRef,
        &HouseholdRef,
        &OccupationProfile,
        &mut PersonalFinances,
        &MobilityProfile,
    )>,
) {
    let tick = clock.tick;

    for (
        mut meta,
        mut demo,
        mut audit,
        settlement_ref,
        hh_ref,
        occ_profile,
        mut finances,
        mobility,
    ) in query.iter_mut()
    {
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
                citizen_index.mark_deceased(meta.id);

                let trace = DecisionTrace::new(
                    ReasonCode::OldAgeSenescence,
                    tick,
                    demo.age_years as f32,
                    0.0,
                    0,
                );
                audit.trace = trace.clone();

                let is_settled = matches!(mobility.status, MigrationStatus::Settled);
                if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                    settlement.total_deaths += 1;
                    if is_settled {
                        settlement.population = settlement.population.saturating_sub(1);
                        settlement.occupied_housing = settlement.occupied_housing.saturating_sub(1);
                        if let Some(count) =
                            settlement.job_headcounts.get_mut(&occ_profile.occupation)
                        {
                            *count = count.saturating_sub(1);
                        }
                    }
                }

                let hid = hh_ref.household_id;
                let mut dissolved = false;
                let mut reclaimed_savings = 0.0;
                let mut reclaimed_food = 0.0;
                if let Some(hh) = households.get_mut(hid) {
                    hh.remove_member(meta.id);
                    if hh.is_empty() {
                        dissolved = true;
                        reclaimed_savings = hh.savings + finances.savings;
                        reclaimed_food = hh.food_reserve;
                        finances.savings = 0.0;
                    }
                }

                if dissolved {
                    households.remove(hid);
                    if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                        settlement.treasury += reclaimed_savings;
                        *settlement
                            .inventories
                            .entry(ResourceType::Food)
                            .or_insert(0.0) += reclaimed_food as f64;
                    }
                    events.push(SimEvent {
                        tick,
                        event_type: SimEventType::HouseholdDissolved,
                        entity: Some(meta.id),
                        settlement: Some(settlement_ref.settlement_id),
                        reason: DecisionTrace::new(
                            ReasonCode::HouseholdDissolved,
                            tick,
                            0.0,
                            0.0,
                            hid.0,
                        ),
                    });
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn reproduction_system(
    clock: Res<SimClock>,
    mut commands: Commands,
    mut next_id: ResMut<NextCitizenId>,
    mut events: ResMut<EventRing>,
    mut households: ResMut<HouseholdDirectory>,
    mut settlements: ResMut<SettlementDirectory>,
    mut citizen_index: ResMut<CitizenIndex>,
    mut query: Query<(
        &CitizenMeta,
        &mut Demographics,
        &HouseholdRef,
        &SettlementRef,
        &mut Kinship,
        &MobilityProfile,
    )>,
) {
    let tick = clock.tick;

    for (meta, mut demo, hh_ref, settlement_ref, mut kinship, mobility) in query.iter_mut() {
        if !meta.alive
            || meta.gender != Gender::Female
            || mobility.status != MigrationStatus::Settled
        {
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

                let baby_entity = commands
                    .spawn((
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
                    ))
                    .id();

                citizen_index.insert_active(baby_id, baby_entity);

                if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                    settlement.population += 1;
                    settlement.occupied_housing += 1;
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

pub fn marriage_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut households: ResMut<HouseholdDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &Demographics,
        &mut Kinship,
        &mut HouseholdRef,
        &SettlementRef,
        &MobilityProfile,
        &mut PersonalFinances,
    )>,
) {
    let tick = clock.tick;
    if !tick.is_multiple_of(30) {
        return;
    }

    // Partition eligible unmarried candidates by settlement
    let mut females_by_settlement: std::collections::BTreeMap<SettlementId, Vec<CitizenId>> =
        std::collections::BTreeMap::new();
    let mut males_by_settlement: std::collections::BTreeMap<SettlementId, Vec<CitizenId>> =
        std::collections::BTreeMap::new();

    for (meta, demo, kinship, _, settlement_ref, mobility, _) in query.iter() {
        if !meta.alive || mobility.status != MigrationStatus::Settled {
            continue;
        }
        if demo.age_years < 18 || demo.age_years > 55 || kinship.spouse.is_some() {
            continue;
        }
        match meta.gender {
            Gender::Female => {
                females_by_settlement
                    .entry(settlement_ref.settlement_id)
                    .or_default()
                    .push(meta.id);
            }
            Gender::Male => {
                males_by_settlement
                    .entry(settlement_ref.settlement_id)
                    .or_default()
                    .push(meta.id);
            }
        }
    }

    // Match pairs deterministically
    let mut pairings: std::collections::BTreeMap<
        CitizenId,
        (CitizenId, HouseholdId, SettlementId),
    > = std::collections::BTreeMap::new();

    for (sid, f_list) in females_by_settlement {
        if let Some(m_list) = males_by_settlement.get_mut(&sid) {
            let pair_count = f_list.len().min(m_list.len()).min(10);
            for i in 0..pair_count {
                let f_id = f_list[i];
                let m_id = m_list[i];

                let new_hh_id = households.allocate_id();
                let mut new_hh = Household::new_with_reserves(new_hh_id, sid, m_id, 60.0, 15.0);
                new_hh.add_member(f_id);
                households.insert(new_hh);

                pairings.insert(f_id, (m_id, new_hh_id, sid));
                pairings.insert(m_id, (f_id, new_hh_id, sid));

                let trace_f = DecisionTrace::new(
                    ReasonCode::HouseholdFormedMarriage,
                    tick,
                    0.0,
                    0.0,
                    m_id.0 as u32,
                );
                let trace_m = DecisionTrace::new(
                    ReasonCode::HouseholdFormedMarriage,
                    tick,
                    0.0,
                    0.0,
                    f_id.0 as u32,
                );

                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::Marriage,
                    entity: Some(f_id),
                    settlement: Some(sid),
                    reason: trace_f,
                });
                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::Marriage,
                    entity: Some(m_id),
                    settlement: Some(sid),
                    reason: trace_m,
                });
            }
        }
    }

    if pairings.is_empty() {
        return;
    }

    // Apply pairing updates to entity components
    let mut old_hhs_to_check = std::collections::BTreeSet::new();

    for (meta, _, mut kinship, mut hh_ref, _, _, _) in query.iter_mut() {
        if let Some(&(partner_id, new_hid, _)) = pairings.get(&meta.id) {
            kinship.spouse = Some(partner_id);
            old_hhs_to_check.insert(hh_ref.household_id);

            hh_ref.household_id = new_hid;
            hh_ref.role = if meta.gender == Gender::Male {
                HouseholdRole::Head
            } else {
                HouseholdRole::Spouse
            };
        }
    }

    // Detach from old households and dissolve if empty
    for old_hid in old_hhs_to_check {
        if let Some(hh) = households.get_mut(old_hid) {
            for cid in pairings.keys() {
                hh.remove_member(*cid);
            }
            if hh.is_empty() {
                households.remove(old_hid);
            }
        }
    }
}
