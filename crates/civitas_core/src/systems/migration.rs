use bevy_ecs::prelude::*;

use crate::components::{
    CausalAudit, CitizenMeta, HouseholdRef, Kinship, MobilityProfile, OccupationProfile,
    PersonalFinances, SettlementRef,
};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::household::{Household, HouseholdDirectory};
use crate::settlement::SettlementDirectory;
use crate::types::{
    DecisionTrace, HouseholdRole, MigrationStatus, OccupationType, ReasonCode, ResourceType,
    SettlementId, SimClock,
};
use crate::world::WorldMap;

pub fn migration_transit_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut MobilityProfile,
        &mut SettlementRef,
        &mut CausalAudit,
    )>,
) {
    let tick = clock.tick;

    for (meta, mut mobility, mut settlement_ref, mut audit) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        if let MigrationStatus::InTransit {
            origin,
            destination,
            ticks_remaining,
            total_ticks,
        } = mobility.status
        {
            if ticks_remaining > 1 {
                mobility.status = MigrationStatus::InTransit {
                    origin,
                    destination,
                    ticks_remaining: ticks_remaining - 1,
                    total_ticks,
                };
            } else {
                // Arrived at destination
                mobility.status = MigrationStatus::Settled;
                settlement_ref.settlement_id = destination;

                if let Some(dest_settlement) = settlements.get_mut(destination) {
                    dest_settlement.population += 1;
                    dest_settlement.occupied_housing += 1;
                    dest_settlement.net_migration += 1;
                }

                let trace = DecisionTrace::new(
                    ReasonCode::MigrationBetterWages,
                    tick,
                    total_ticks as f32,
                    origin.0 as f32,
                    destination.0 as u32,
                );
                audit.trace = trace.clone();

                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::MigrationArrival,
                    entity: Some(meta.id),
                    settlement: Some(destination),
                    reason: trace,
                });
            }
        }
    }
}

pub fn migration_evaluation_system(
    clock: Res<SimClock>,
    world_map: Res<WorldMap>,
    mut events: ResMut<EventRing>,
    mut households: ResMut<HouseholdDirectory>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut MobilityProfile,
        &SettlementRef,
        &mut HouseholdRef,
        &mut OccupationProfile,
        &Kinship,
        &mut PersonalFinances,
        &mut CausalAudit,
    )>,
) {
    let tick = clock.tick;

    for (
        meta,
        mut mobility,
        settlement_ref,
        mut hh_ref,
        mut occ_profile,
        kinship,
        mut finances,
        mut audit,
    ) in query.iter_mut()
    {
        if !meta.alive || mobility.status != MigrationStatus::Settled {
            continue;
        }

        // Sliced evaluation: only a fraction evaluate migration on any single tick
        if !(meta.id.0 + tick).is_multiple_of(30) {
            continue;
        }

        let origin_id = settlement_ref.settlement_id;
        let origin = match settlements.get(origin_id) {
            Some(s) => s,
            None => continue,
        };

        let hh = match households.get(hh_ref.household_id) {
            Some(h) => h,
            None => continue,
        };

        // Determine push/pull factors
        let local_food_price = origin.get_price(ResourceType::Food);
        let local_real_wage = origin.average_wage() / local_food_price.max(0.5);
        let local_hunger = hh.migration_pressure > 2.0;
        let local_housing_shortage = origin.housing_utilization() > 0.95;

        let mut best_dest: Option<(SettlementId, f32, u16, ReasonCode)> = None;

        for (dest_id, dest) in &settlements.settlements {
            if *dest_id == origin_id {
                continue;
            }

            let dist = world_map.distance(origin_id, *dest_id);
            let dest_food_price = dest.get_price(ResourceType::Food);
            let dest_real_wage = dest.average_wage() / dest_food_price.max(0.5);
            let housing_space = (1.0 - dest.housing_utilization()).max(0.05);

            let pull_utility = (dest_real_wage * housing_space) / (1.0 + (dist as f32) * 0.03);

            let mut candidate_reason = None;

            if pull_utility > local_real_wage * 1.3 {
                candidate_reason = Some(ReasonCode::MigrationBetterWages);
            } else if local_hunger && pull_utility > local_real_wage {
                candidate_reason = Some(ReasonCode::MigrationFleeingHunger);
            } else if local_housing_shortage && dest.housing_utilization() < 0.65 {
                candidate_reason = Some(ReasonCode::MigrationCheaperHousing);
            } else if kinship.spouse.is_some() || kinship.parent_a.is_some() {
                // Family reunion pull if distant kin resides in destination
                candidate_reason = Some(ReasonCode::MigrationFamilyReunion);
            }

            if let Some(reason) = candidate_reason {
                if let Some((_, best_util, _, _)) = best_dest {
                    if pull_utility > best_util {
                        best_dest = Some((*dest_id, pull_utility, dist, reason));
                    }
                } else {
                    best_dest = Some((*dest_id, pull_utility, dist, reason));
                }
            }
        }

        if let Some((target_settlement, util_score, dist, chosen_reason)) = best_dest {
            // Initiate migration
            let travel_ticks = (dist / 2).max(2);

            // Detach from current household
            let old_hid = hh_ref.household_id;
            let mut dissolved = false;
            let mut carried_savings = finances.savings;
            let mut carried_food = 0.0;

            if let Some(h) = households.get_mut(old_hid) {
                h.remove_member(meta.id);
                if h.is_empty() {
                    dissolved = true;
                    carried_savings += h.savings;
                    carried_food += h.food_reserve;
                } else {
                    let share = (h.savings / (h.members.len() + 1) as f64).min(h.savings);
                    h.savings -= share;
                    carried_savings += share;
                }
            }

            if dissolved {
                households.remove(old_hid);
                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::HouseholdDissolved,
                    entity: Some(meta.id),
                    settlement: Some(origin_id),
                    reason: DecisionTrace::new(
                        ReasonCode::HouseholdDissolved,
                        tick,
                        0.0,
                        0.0,
                        old_hid.0,
                    ),
                });
            }

            // Create new household for the traveling citizen right away so citizen-household invariants are strictly maintained
            let new_hh_id = households.allocate_id();
            let new_hh = Household::new_with_reserves(
                new_hh_id,
                target_settlement,
                meta.id,
                carried_savings,
                carried_food,
            );
            households.insert(new_hh);
            hh_ref.household_id = new_hh_id;
            hh_ref.role = HouseholdRole::Head;
            finances.savings = 0.0;

            // Update origin settlement population & employment
            if let Some(orig_settlement) = settlements.get_mut(origin_id) {
                orig_settlement.population = orig_settlement.population.saturating_sub(1);
                orig_settlement.occupied_housing =
                    orig_settlement.occupied_housing.saturating_sub(1);
                orig_settlement.net_migration -= 1;
                if let Some(count) = orig_settlement
                    .job_headcounts
                    .get_mut(&occ_profile.occupation)
                {
                    *count = count.saturating_sub(1);
                }
            }

            // Migrant resigns from previous job while in transit
            occ_profile.occupation = OccupationType::Unemployed;

            mobility.status = MigrationStatus::InTransit {
                origin: origin_id,
                destination: target_settlement,
                ticks_remaining: travel_ticks,
                total_ticks: travel_ticks,
            };

            let trace = DecisionTrace::new(
                chosen_reason,
                tick,
                util_score,
                dist as f32,
                target_settlement.0 as u32,
            );
            audit.trace = trace.clone();

            events.push(SimEvent {
                tick,
                event_type: SimEventType::MigrationStart,
                entity: Some(meta.id),
                settlement: Some(origin_id),
                reason: trace,
            });
        }
    }
}
