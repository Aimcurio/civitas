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

    // Step 1: Adjust job openings and wages in settlements based on population and inventory scarcity
    for settlement in settlements.settlements.values_mut() {
        let pop = settlement.population;
        let target_farmer = (pop / 3).max(4);
        let target_forester = (pop / 8).max(2);
        let target_miner = (pop / 8).max(2);
        let target_artisan = (pop / 10).max(1);
        let target_merchant = (pop / 15).max(1);
        let target_laborer = (pop / 6).max(2);

        let targets = [
            (OccupationType::Farmer, target_farmer),
            (OccupationType::Forester, target_forester),
            (OccupationType::Miner, target_miner),
            (OccupationType::Artisan, target_artisan),
            (OccupationType::Merchant, target_merchant),
            (OccupationType::Laborer, target_laborer),
        ];

        // Expire stale vacancies and set fresh bounded openings
        settlement.job_openings.clear();
        for (occ, target) in targets {
            let current = *settlement.job_headcounts.get(&occ).unwrap_or(&0);
            let needed = target.saturating_sub(current).min(25);
            settlement.job_openings.insert(occ, needed);
        }

        // Wage adjustments based on inventory scarcity
        let food_inv = *settlement
            .inventories
            .get(&ResourceType::Food)
            .unwrap_or(&0.0);
        if food_inv < (pop as f64) * 4.0 {
            let wage = settlement
                .wages
                .entry(OccupationType::Farmer)
                .or_insert(3.0);
            *wage = (*wage * 1.05).min(15.0);
        } else if food_inv > (pop as f64) * 15.0 {
            let wage = settlement
                .wages
                .entry(OccupationType::Farmer)
                .or_insert(3.0);
            *wage = (*wage * 0.95).max(1.5);
        }

        // Reset headcounts for authoritative recount
        for count in settlement.job_headcounts.values_mut() {
            *count = 0;
        }
    }

    // Step 2: Layoffs if settlement treasury is exhausted or industry over-saturated
    for (meta, mut occ_profile, mut audit, settlement_ref) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
            // Check for layoffs when treasury is broke
            if settlement.treasury < 5.0
                && occ_profile.occupation != OccupationType::Unemployed
                && occ_profile.occupation != OccupationType::Farmer
            {
                occ_profile.occupation = OccupationType::Unemployed;

                let trace = DecisionTrace::new(
                    ReasonCode::JobLaidOff,
                    tick,
                    settlement.treasury as f32,
                    0.0,
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

            // Step 3: Match unemployed citizens to available openings
            if occ_profile.occupation == OccupationType::Unemployed {
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
                        *openings = openings.saturating_sub(1);
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

            // Authoritative headcount tally
            *settlement
                .job_headcounts
                .entry(occ_profile.occupation)
                .or_insert(0) += 1;
        }
    }
}
