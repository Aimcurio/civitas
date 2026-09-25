use bevy_ecs::system::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::{OccupationType, ResourceType, SettlementId};

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
        }
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
