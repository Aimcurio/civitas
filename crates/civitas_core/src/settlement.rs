use bevy_ecs::system::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::{OccupationType, ResourceType, SettlementId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementPolicy {
    pub tax_rate: f32, // Municipal wage tax withheld [0.0, 0.50], default 0.10 (10%)
    pub emergency_relief: bool, // Municipal bread dole distributed if starving and food exists, default true
    pub wage_bonus: f32,        // Multiplier on base wages [-0.50, +1.00], default 0.0
}

impl Default for SettlementPolicy {
    fn default() -> Self {
        Self {
            tax_rate: 0.10,
            emergency_relief: true,
            wage_bonus: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settlement {
    pub id: SettlementId,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub population: u32,
    pub housing_capacity: u32,
    pub occupied_housing: u32,
    pub treasury: f64,
    pub inventories: BTreeMap<ResourceType, f64>,
    pub prices: BTreeMap<ResourceType, f32>,
    pub wages: BTreeMap<OccupationType, f32>,
    pub demand_accumulators: BTreeMap<ResourceType, f32>,
    pub supply_accumulators: BTreeMap<ResourceType, f32>,
    pub job_headcounts: BTreeMap<OccupationType, u32>,
    pub job_openings: BTreeMap<OccupationType, u32>,
    pub total_births: u64,
    pub total_deaths: u64,
    pub net_migration: i64,
    pub policy: SettlementPolicy,
    pub fertility_factor: f32,
    pub timber_factor: f32,
    pub mineral_factor: f32,
}

impl Settlement {
    pub fn new(id: SettlementId, name: String, x: u32, y: u32, initial_capacity: u32) -> Self {
        let mut inventories = BTreeMap::new();
        let mut prices = BTreeMap::new();
        let mut wages = BTreeMap::new();
        let mut demand = BTreeMap::new();
        let mut supply = BTreeMap::new();
        let mut job_headcounts = BTreeMap::new();
        let mut job_openings = BTreeMap::new();

        for res in ResourceType::ALL {
            inventories.insert(res, 500.0);
            prices.insert(
                res,
                match res {
                    ResourceType::Food => 2.0,
                    ResourceType::Timber => 3.0,
                    ResourceType::Stone => 4.0,
                    ResourceType::Tools => 10.0,
                    ResourceType::Luxury => 25.0,
                },
            );
            demand.insert(res, 10.0);
            supply.insert(res, 10.0);
        }

        for occ in OccupationType::ALL {
            wages.insert(
                occ,
                match occ {
                    OccupationType::Unemployed => 0.0,
                    OccupationType::Farmer => 3.0,
                    OccupationType::Forester => 3.5,
                    OccupationType::Miner => 4.0,
                    OccupationType::Artisan => 6.0,
                    OccupationType::Merchant => 8.0,
                    OccupationType::Laborer => 2.5,
                },
            );
            job_headcounts.insert(occ, 0);
            job_openings.insert(occ, 50);
        }

        Self {
            id,
            name,
            x,
            y,
            population: 0,
            housing_capacity: initial_capacity,
            occupied_housing: 0,
            treasury: 10_000.0,
            inventories,
            prices,
            wages,
            demand_accumulators: demand,
            supply_accumulators: supply,
            job_headcounts,
            job_openings,
            total_births: 0,
            total_deaths: 0,
            net_migration: 0,
            policy: SettlementPolicy::default(),
            fertility_factor: 1.0,
            timber_factor: 1.0,
            mineral_factor: 1.0,
        }
    }

    pub fn set_policy(&mut self, mut policy: SettlementPolicy) {
        policy.tax_rate = policy.tax_rate.clamp(0.0, 0.50);
        policy.wage_bonus = policy.wage_bonus.clamp(-0.50, 1.00);
        self.policy = policy;
    }

    pub fn effective_wage(&self, occ: OccupationType) -> f32 {
        let base = self.get_wage(occ);
        let bonus_factor = (1.0 + self.policy.wage_bonus).clamp(0.2, 2.0);
        let fiscal_factor = if self.treasury < 2_000.0 {
            (self.treasury as f32 / 2_000.0).clamp(0.40, 1.0)
        } else {
            1.0
        };
        (base * bonus_factor * fiscal_factor).clamp(0.5, 50.0)
    }

    pub fn commission_housing_expansion(&mut self) -> Result<(), &'static str> {
        let timber = *self.inventories.get(&ResourceType::Timber).unwrap_or(&0.0);
        let stone = *self.inventories.get(&ResourceType::Stone).unwrap_or(&0.0);

        if self.treasury < 50.0 {
            return Err("Insufficient municipal treasury (requires 50.0 coins)");
        }
        if timber < 10.0 {
            return Err("Insufficient timber in warehouse (requires 10.0 units)");
        }
        if stone < 5.0 {
            return Err("Insufficient stone in warehouse (requires 5.0 units)");
        }

        self.treasury -= 50.0;
        if let Some(t) = self.inventories.get_mut(&ResourceType::Timber) {
            *t -= 10.0;
        }
        if let Some(s) = self.inventories.get_mut(&ResourceType::Stone) {
            *s -= 5.0;
        }
        self.housing_capacity += 25;
        Ok(())
    }

    pub fn housing_utilization(&self) -> f32 {
        if self.housing_capacity == 0 {
            1.0
        } else {
            (self.occupied_housing as f32) / (self.housing_capacity as f32)
        }
    }

    pub fn get_price(&self, res: ResourceType) -> f32 {
        *self.prices.get(&res).unwrap_or(&2.0)
    }

    pub fn get_wage(&self, occ: OccupationType) -> f32 {
        *self.wages.get(&occ).unwrap_or(&3.0)
    }

    pub fn average_wage(&self) -> f32 {
        let mut total_wage = 0.0;
        let mut count = 0;
        for (occ, &headcount) in &self.job_headcounts {
            if *occ != OccupationType::Unemployed && headcount > 0 {
                total_wage += self.get_wage(*occ) * (headcount as f32);
                count += headcount;
            }
        }
        if count == 0 {
            3.0
        } else {
            total_wage / (count as f32)
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementDirectory {
    pub settlements: BTreeMap<SettlementId, Settlement>,
}

impl SettlementDirectory {
    pub fn new() -> Self {
        Self {
            settlements: BTreeMap::new(),
        }
    }

    pub fn get(&self, id: SettlementId) -> Option<&Settlement> {
        self.settlements.get(&id)
    }

    pub fn get_mut(&mut self, id: SettlementId) -> Option<&mut Settlement> {
        self.settlements.get_mut(&id)
    }
}

impl Default for SettlementDirectory {
    fn default() -> Self {
        Self::new()
    }
}
