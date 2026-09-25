use bevy_ecs::prelude::*;

use crate::components::{CitizenMeta, HouseholdRef, PersonalFinances, PhysicalNeeds};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::types::ResourceType;

pub fn market_price_update_system(mut settlements: ResMut<SettlementDirectory>) {
    for settlement in settlements.settlements.values_mut() {
        for res in ResourceType::ALL {
            let demand = *settlement.demand_accumulators.get(&res).unwrap_or(&1.0);
            let supply = *settlement.supply_accumulators.get(&res).unwrap_or(&1.0);
            let inventory = *settlement.inventories.get(&res).unwrap_or(&0.0);

            let old_price = *settlement.prices.get(&res).unwrap_or(&2.0);

            // Price discovery adjustment
            let inventory_pressure = if inventory < 50.0 {
                0.15
            } else if inventory > 1000.0 {
                -0.10
            } else {
                0.0
            };

            let market_imbalance = (demand - supply) / supply.max(1.0);
            let delta = (market_imbalance * 0.10 + inventory_pressure).clamp(-0.25, 0.25);
            let new_price = (old_price * (1.0 + delta)).clamp(0.5, 250.0);

            settlement.prices.insert(res, new_price);

            // Reset accumulators for next interval
            settlement.demand_accumulators.insert(res, 0.0);
            settlement.supply_accumulators.insert(res, 0.0);
        }
    }
}

pub fn household_consumption_system(
    mut households: ResMut<HouseholdDirectory>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &CitizenMeta,
        &HouseholdRef,
        &mut PersonalFinances,
        &mut PhysicalNeeds,
    )>,
) {
    // Phase 1: Transfer personal savings to household pools
    for (meta, hh_ref, mut finances, _) in query.iter_mut() {
        if !meta.alive {
            continue;
        }
        if let Some(hh) = households.get_mut(hh_ref.household_id) {
            if finances.savings > 10.0 {
                let transfer = finances.savings - 10.0;
                finances.savings = 10.0;
                hh.savings += transfer;
            }
        }
    }

    // Phase 2: Households purchase sustenance from settlement warehouse
    for hh in households.households.values_mut() {
        if hh.members.is_empty() {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(hh.settlement_id) {
            let member_count = hh.members.len() as f64;
            let food_price = settlement.get_price(ResourceType::Food) as f64;
            let total_food_cost = food_price * member_count;

            let food_inv = settlement
                .inventories
                .entry(ResourceType::Food)
                .or_insert(0.0);
            let food_demand = settlement
                .demand_accumulators
                .entry(ResourceType::Food)
                .or_insert(0.0);
            *food_demand += member_count as f32;

            if *food_inv >= member_count && hh.savings >= total_food_cost {
                // Successful purchase
                *food_inv -= member_count;
                hh.savings -= total_food_cost;
                settlement.treasury += total_food_cost;
                hh.food_reserve = (hh.food_reserve + member_count as f32).min(50.0);
                hh.migration_pressure = (hh.migration_pressure - 0.2).max(0.0);
            } else {
                // Food deficit or impoverishment
                hh.migration_pressure += 1.0;
            }
        }
    }

    // Phase 3: Citizens consume food and replenish satiety
    for (meta, hh_ref, _, mut needs) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        if let Some(hh) = households.get_mut(hh_ref.household_id) {
            if hh.food_reserve >= 1.0 {
                hh.food_reserve -= 1.0;
                needs.satiety = 100;
            }
        }
    }
}
