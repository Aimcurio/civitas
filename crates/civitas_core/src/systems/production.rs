use bevy_ecs::prelude::*;

use crate::components::{
    CitizenMeta, MobilityProfile, OccupationProfile, PersonalFinances, SettlementRef,
};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::settlement::SettlementDirectory;
use crate::types::{
    DecisionTrace, MigrationStatus, OccupationType, ReasonCode, ResourceType, SimClock,
};

pub fn production_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut OccupationProfile,
        &mut PersonalFinances,
        &SettlementRef,
        &MobilityProfile,
    )>,
) {
    let tick = clock.tick;

    for (meta, mut occ_profile, mut finances, settlement_ref, mobility) in query.iter_mut() {
        if !meta.alive
            || occ_profile.occupation == OccupationType::Unemployed
            || mobility.status != MigrationStatus::Settled
        {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
            let gross_wage = settlement.effective_wage(occ_profile.occupation);
            let tax_rate = settlement.policy.tax_rate.clamp(0.0, 0.50);
            let net_wage = (gross_wage * (1.0 - tax_rate)).max(0.1);

            // Pay net wage if settlement has funds (tax withholding remains in municipal treasury)
            if settlement.treasury >= net_wage as f64 {
                settlement.treasury -= net_wage as f64;
                finances.savings += net_wage as f64;
                finances.last_income = net_wage;
            } else {
                finances.last_income = 0.0;
            }

            // Experience and skill progression
            occ_profile.experience = occ_profile.experience.saturating_add(1);
            if occ_profile.experience >= 100 && occ_profile.skill_level < 10 {
                occ_profile.skill_level += 1;
                occ_profile.experience = 0;
                occ_profile.productivity = 1.0 + (occ_profile.skill_level as f32) * 0.1;
            }

            // Produce output based on occupation scaled by local environmental factors
            let output = occ_profile.productivity;
            match occ_profile.occupation {
                OccupationType::Farmer => {
                    let yield_amount = (output * 0.8 * settlement.fertility_factor) as f64;
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Food)
                        .or_insert(0.0);
                    *inv += yield_amount;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Food)
                        .or_insert(0.0);
                    *sup += yield_amount as f32;
                }
                OccupationType::Forester => {
                    let yield_amount = (output * 0.9 * settlement.timber_factor) as f64;
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    *inv += yield_amount;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    *sup += yield_amount as f32;
                }
                OccupationType::Miner => {
                    let yield_amount = (output * 0.9 * settlement.mineral_factor) as f64;
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Stone)
                        .or_insert(0.0);
                    *inv += yield_amount;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Stone)
                        .or_insert(0.0);
                    *sup += yield_amount as f32;
                }
                OccupationType::Artisan => {
                    let timber = settlement
                        .inventories
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    if *timber >= 0.5 {
                        *timber -= 0.5;
                        let yield_amount = (output * 0.5) as f64;
                        let tools = settlement
                            .inventories
                            .entry(ResourceType::Tools)
                            .or_insert(0.0);
                        *tools += yield_amount;
                        let sup = settlement
                            .supply_accumulators
                            .entry(ResourceType::Tools)
                            .or_insert(0.0);
                        *sup += yield_amount as f32;
                    }
                }
                OccupationType::Merchant => {
                    // Merchants craft and trade luxury goods from raw materials (no money printing!)
                    let timber = settlement
                        .inventories
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    if *timber >= 0.5 {
                        *timber -= 0.5;
                        let yield_amount = (output * 0.25) as f64;
                        let lux = settlement
                            .inventories
                            .entry(ResourceType::Luxury)
                            .or_insert(0.0);
                        *lux += yield_amount;
                        let sup = settlement
                            .supply_accumulators
                            .entry(ResourceType::Luxury)
                            .or_insert(0.0);
                        *sup += yield_amount as f32;
                    }
                }
                OccupationType::Laborer => {
                    // Laborers construct housing infrastructure when materials and demand exist
                    let timber = *settlement
                        .inventories
                        .get(&ResourceType::Timber)
                        .unwrap_or(&0.0);
                    let stone = *settlement
                        .inventories
                        .get(&ResourceType::Stone)
                        .unwrap_or(&0.0);

                    if timber >= 2.0 && stone >= 1.0 && settlement.housing_utilization() > 0.85 {
                        if let Some(t) = settlement.inventories.get_mut(&ResourceType::Timber) {
                            *t -= 2.0;
                        }
                        if let Some(s) = settlement.inventories.get_mut(&ResourceType::Stone) {
                            *s -= 1.0;
                        }
                        settlement.housing_capacity += 5;

                        let trace = DecisionTrace::new(
                            ReasonCode::SettlementInfrastructureExpansion,
                            tick,
                            settlement.housing_capacity as f32,
                            0.0,
                            settlement_ref.settlement_id.0 as u32,
                        );

                        events.push(SimEvent {
                            tick,
                            event_type: SimEventType::SettlementExpansion,
                            entity: Some(meta.id),
                            settlement: Some(settlement_ref.settlement_id),
                            reason: trace,
                        });
                    }
                }
                OccupationType::Unemployed => {}
            }
        }
    }
}
