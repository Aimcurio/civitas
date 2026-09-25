use bevy_ecs::prelude::*;

use crate::components::{CitizenMeta, OccupationProfile, PersonalFinances, SettlementRef};
use crate::settlement::SettlementDirectory;
use crate::types::{OccupationType, ResourceType};

pub fn production_system(
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &mut OccupationProfile,
        &mut PersonalFinances,
        &SettlementRef,
    )>,
) {
    for (meta, mut occ_profile, mut finances, settlement_ref) in query.iter_mut() {
        if !meta.alive || occ_profile.occupation == OccupationType::Unemployed {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
            let wage = settlement.get_wage(occ_profile.occupation);

            // Pay wage if settlement has funds
            if settlement.treasury >= wage as f64 {
                settlement.treasury -= wage as f64;
                finances.savings += wage as f64;
                finances.last_income = wage;
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

            // Produce output based on occupation
            let output = occ_profile.productivity;
            match occ_profile.occupation {
                OccupationType::Farmer => {
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Food)
                        .or_insert(0.0);
                    *inv += (output * 1.5) as f64;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Food)
                        .or_insert(0.0);
                    *sup += output * 1.5;
                }
                OccupationType::Forester => {
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    *inv += output as f64;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    *sup += output;
                }
                OccupationType::Miner => {
                    let inv = settlement
                        .inventories
                        .entry(ResourceType::Stone)
                        .or_insert(0.0);
                    *inv += output as f64;
                    let sup = settlement
                        .supply_accumulators
                        .entry(ResourceType::Stone)
                        .or_insert(0.0);
                    *sup += output;
                }
                OccupationType::Artisan => {
                    let timber = settlement
                        .inventories
                        .entry(ResourceType::Timber)
                        .or_insert(0.0);
                    if *timber >= 0.5 {
                        *timber -= 0.5;
                        let tools = settlement
                            .inventories
                            .entry(ResourceType::Tools)
                            .or_insert(0.0);
                        *tools += (output * 0.5) as f64;
                        let sup = settlement
                            .supply_accumulators
                            .entry(ResourceType::Tools)
                            .or_insert(0.0);
                        *sup += output * 0.5;
                    }
                }
                OccupationType::Merchant => {
                    // Merchants generate treasury through trade activity
                    settlement.treasury += (wage * 1.2) as f64;
                }
                OccupationType::Laborer => {
                    // Laborers assist infrastructure maintenance
                    settlement.housing_capacity =
                        settlement.housing_capacity.max(settlement.population + 10);
                }
                OccupationType::Unemployed => {}
            }
        }
    }
}
