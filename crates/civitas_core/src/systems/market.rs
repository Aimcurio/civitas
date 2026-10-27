use bevy_ecs::prelude::*;

use crate::components::{CitizenMeta, HouseholdRef, PersonalFinances, PhysicalNeeds};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::types::{DecisionTrace, ReasonCode, ResourceType, SimClock};

pub fn market_price_update_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
) {
    let tick = clock.tick;

    for settlement in settlements.settlements.values_mut() {
        let sid = settlement.id;
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

            // Emit price shock if sudden large price shift
            if delta.abs() >= 0.20 {
                let trace = DecisionTrace::new(
                    ReasonCode::MarketPriceShock,
                    tick,
                    delta,
                    new_price,
                    res as u32,
                );
                events.push(SimEvent {
                    tick,
                    event_type: SimEventType::PriceShock,
                    entity: None,
                    settlement: Some(sid),
                    reason: trace,
                });
            }

            // Reset accumulators for next interval
            settlement.demand_accumulators.insert(res, 0.0);
            settlement.supply_accumulators.insert(res, 0.0);
        }
    }
}

pub fn civic_maintenance_system(
    mut households: ResMut<HouseholdDirectory>,
    mut settlements: ResMut<SettlementDirectory>,
) {
    for hh in households.households.values_mut() {
        if hh.members.is_empty() {
            continue;
        }
        if let Some(settlement) = settlements.get_mut(hh.settlement_id) {
            let member_count = hh.members.len() as f64;
            // Monthly civic contribution proportional to household size and wealth
            let fee = (member_count * 15.0).min(hh.savings * 0.30);
            if fee > 0.0 {
                hh.savings -= fee;
                settlement.treasury += fee;
            }

            // Wealthy households purchase tools and luxury goods from municipal markets
            if hh.savings > 30.0 {
                let lux_price = settlement.get_price(ResourceType::Luxury) as f64;
                let lux_inv = settlement
                    .inventories
                    .entry(ResourceType::Luxury)
                    .or_insert(0.0);
                if *lux_inv >= 1.0 && lux_price > 0.0 {
                    let cost = 1.0 * lux_price;
                    if hh.savings >= cost {
                        *lux_inv -= 1.0;
                        hh.savings -= cost;
                        settlement.treasury += cost;
                    }
                }

                let tools_price = settlement.get_price(ResourceType::Tools) as f64;
                let tools_inv = settlement
                    .inventories
                    .entry(ResourceType::Tools)
                    .or_insert(0.0);
                if *tools_inv >= 1.0 && tools_price > 0.0 {
                    let cost = 1.0 * tools_price;
                    if hh.savings >= cost {
                        *tools_inv -= 1.0;
                        hh.savings -= cost;
                        settlement.treasury += cost;
                    }
                }
            }
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
            if finances.savings > 5.0 {
                let transfer = finances.savings - 5.0;
                finances.savings = 5.0;
                hh.savings += transfer;
            }
        }
    }

    // Phase 2: Households purchase sustenance from settlement warehouse for living members
    for hh in households.households.values_mut() {
        if hh.members.is_empty() {
            continue;
        }

        if let Some(settlement) = settlements.get_mut(hh.settlement_id) {
            let member_count = hh.members.len() as f64;
            let food_price = settlement.get_price(ResourceType::Food) as f64;
            let target_food = member_count * 2.0; // Restock target
            let needed_food = (target_food - hh.food_reserve as f64).max(0.0);

            if needed_food <= 0.0 {
                continue;
            }

            let food_inv = settlement
                .inventories
                .entry(ResourceType::Food)
                .or_insert(0.0);
            let food_demand = settlement
                .demand_accumulators
                .entry(ResourceType::Food)
                .or_insert(0.0);
            *food_demand += needed_food as f32;

            let affordable_food = if food_price > 0.0 {
                (hh.savings / food_price).min(needed_food).min(*food_inv)
            } else {
                needed_food.min(*food_inv)
            };

            if affordable_food > 0.0 {
                let total_food_cost = affordable_food * food_price;
                *food_inv -= affordable_food;
                hh.savings -= total_food_cost;
                settlement.treasury += total_food_cost;
                hh.food_reserve = (hh.food_reserve + affordable_food as f32).min(50.0);
                hh.migration_pressure = (hh.migration_pressure - 0.2).max(0.0);
            } else if settlement.policy.emergency_relief && *food_inv >= 1.0 {
                // Emergency Bread Dole: Settlement warehouse distributes sustenance rations to prevent mass starvation in abundance
                let dole = (needed_food.min(member_count * 0.5)).min(*food_inv);
                if dole > 0.0 {
                    *food_inv -= dole;
                    hh.food_reserve = (hh.food_reserve + dole as f32).min(50.0);
                    hh.migration_pressure = (hh.migration_pressure - 0.1).max(0.0);
                } else {
                    hh.migration_pressure += 0.5;
                }
            } else {
                // Food deficit or impoverishment
                hh.migration_pressure += 0.5;
            }
        }
    }

    // Phase 3: Immediate meal consumption for living members
    for (meta, hh_ref, _, mut needs) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        if let Some(hh) = households.get_mut(hh_ref.household_id) {
            if hh.food_reserve >= 0.5 {
                hh.food_reserve -= 0.5;
                needs.satiety = 100;
            }
        }
    }
}
