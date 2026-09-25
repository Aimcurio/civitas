use bevy_ecs::prelude::*;

use crate::components::{CausalAudit, CitizenMeta, OccupationProfile, SettlementRef};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::settlement::SettlementDirectory;
use crate::types::{DecisionTrace, OccupationType, ReasonCode, ResourceType, SimClock};

pub fn labor_market_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut OccupationProfile,
        &mut CausalAudit,
        &SettlementRef,
    )>,
) {
    let tick = clock.tick;

    // Step 1: Adjust job openings and wages in settlements based on inventory scarcity
    for settlement in settlements.settlements.values_mut() {
        let food_inv = *settlement
            .inventories
            .get(&ResourceType::Food)
            .unwrap_or(&0.0);
        let timber_inv = *settlement
            .inventories
            .get(&ResourceType::Timber)
            .unwrap_or(&0.0);
        let stone_inv = *settlement
            .inventories
            .get(&ResourceType::Stone)
            .unwrap_or(&0.0);

        // If food is low, open up more farming positions and raise farm wages
        if food_inv < (settlement.population as f64) * 5.0 {
            *settlement
                .job_openings
                .entry(OccupationType::Farmer)
                .or_insert(0) += 20;
            let wage = settlement
                .wages
                .entry(OccupationType::Farmer)
                .or_insert(3.0);
            *wage = (*wage * 1.05).min(15.0);
        }

        // If timber or stone are low, incentivize extraction
        if timber_inv < 100.0 {
            *settlement
                .job_openings
                .entry(OccupationType::Forester)
                .or_insert(0) += 10;
        }
        if stone_inv < 100.0 {
            *settlement
                .job_openings
                .entry(OccupationType::Miner)
                .or_insert(0) += 10;
        }

        // Reset headcounts for recount
        for count in settlement.job_headcounts.values_mut() {
            *count = 0;
        }
    }

    // Step 2: Match unemployed or seeking citizens to open positions
    for (meta, mut occ_profile, mut audit, settlement_ref) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
            if occ_profile.occupation == OccupationType::Unemployed {
                // Find highest paying occupation with available openings
                let mut best_occ = None;
                let mut highest_wage = 0.0;

                for occ in [
                    OccupationType::Farmer,
                    OccupationType::Forester,
                    OccupationType::Miner,
                    OccupationType::Artisan,
                    OccupationType::Merchant,
                    OccupationType::Laborer,
                ] {
                    let openings = *settlement.job_openings.get(&occ).unwrap_or(&0);
                    let wage = settlement.get_wage(occ);
                    if openings > 0 && wage > highest_wage {
                        highest_wage = wage;
                        best_occ = Some(occ);
                    }
                }

                if let Some(chosen_occ) = best_occ {
                    occ_profile.occupation = chosen_occ;
                    if let Some(openings) = settlement.job_openings.get_mut(&chosen_occ) {
                        if *openings > 0 {
                            *openings -= 1;
                        }
                    }

                    let trace = DecisionTrace::new(
                        ReasonCode::JobOpportunityFound,
                        tick,
                        highest_wage,
                        settlement.population as f32,
                        settlement_ref.settlement_id.0 as u32,
                    );
                    audit.trace = trace.clone();

                    events.push(SimEvent {
                        tick,
                        event_type: SimEventType::JobChange,
                        entity: Some(meta.id),
                        settlement: Some(settlement_ref.settlement_id),
                        reason: trace,
                    });
                }
            }

            // Tally headcount
            *settlement
                .job_headcounts
                .entry(occ_profile.occupation)
                .or_insert(0) += 1;
        }
    }
}
