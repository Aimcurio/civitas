use bevy_ecs::prelude::*;

use crate::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, MobilityProfile, OccupationProfile,
    PersonalFinances, PhysicalNeeds, SettlementRef,
};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::sim::CitizenIndex;
use crate::types::{DecisionTrace, MigrationStatus, ReasonCode, ResourceType, SimClock};

pub fn physiology_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut households: ResMut<HouseholdDirectory>,
    mut citizen_index: ResMut<CitizenIndex>,
    mut query: Query<(
        &mut CitizenMeta,
        &mut PhysicalNeeds,
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
        mut needs,
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

        // Daily hunger & sustenance consumption from household reserve
        if needs.satiety < 95 {
            if let Some(hh) = households.get_mut(hh_ref.household_id) {
                if hh.food_reserve >= 0.2 {
                    hh.food_reserve -= 0.2;
                    needs.satiety = (needs.satiety + 5).min(100);
                }
            }
        }

        // Daily hunger degradation if no food consumed
        if needs.satiety > 0 {
            needs.satiety = needs.satiety.saturating_sub(1);
        }

        // Shelter evaluation based on settlement housing utilization
        if let Some(settlement) = settlements.get(settlement_ref.settlement_id) {
            if settlement.housing_utilization() <= 1.0 {
                needs.shelter = (needs.shelter + 2).min(100);
            } else {
                needs.shelter = needs.shelter.saturating_sub(2);
            }
        }

        // Comfort evaluation (balance of satiety and shelter)
        needs.comfort = ((needs.satiety as u16 + needs.shelter as u16) / 2) as u8;

        // Health impact of starvation or nourishment
        if needs.satiety == 0 {
            demo.health = demo.health.saturating_sub(5);
        } else if needs.satiety >= 70 && needs.shelter >= 50 && demo.health < 100 {
            demo.health = (demo.health + 1).min(100);
        }

        // Check for mortality
        if demo.health == 0 {
            meta.alive = false;
            citizen_index.mark_deceased(meta.id);

            let trace = DecisionTrace::new(ReasonCode::StarvationDeath, tick, 0.0, 0.0, 0);
            audit.trace = trace.clone();

            let is_settled = matches!(mobility.status, MigrationStatus::Settled);
            if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                settlement.total_deaths += 1;
                if is_settled {
                    settlement.population = settlement.population.saturating_sub(1);
                    settlement.occupied_housing = settlement.occupied_housing.saturating_sub(1);
                    if let Some(count) = settlement.job_headcounts.get_mut(&occ_profile.occupation) {
                        *count = count.saturating_sub(1);
                    }
                }
            }

            // Remove from active household
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
