use bevy_ecs::system::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::{CitizenId, HouseholdId, SettlementId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Household {
    pub id: HouseholdId,
    pub settlement_id: SettlementId,
    pub head: CitizenId,
    pub members: Vec<CitizenId>,
    pub savings: f64,
    pub food_reserve: f32,
    pub housing_quality: u8,
    pub migration_pressure: f32,
}

impl Household {
    pub fn new(id: HouseholdId, settlement_id: SettlementId, head: CitizenId) -> Self {
        Self {
            id,
            settlement_id,
            head,
            members: vec![head],
            savings: 50.0,
            food_reserve: 10.0,
            housing_quality: 100,
            migration_pressure: 0.0,
        }
    }

    pub fn add_member(&mut self, citizen_id: CitizenId) {
        if !self.members.contains(&citizen_id) {
            self.members.push(citizen_id);
        }
    }

    pub fn remove_member(&mut self, citizen_id: CitizenId) -> bool {
        if let Some(pos) = self.members.iter().position(|&id| id == citizen_id) {
            self.members.swap_remove(pos);
            if self.head == citizen_id && !self.members.is_empty() {
                self.head = self.members[0];
            }
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseholdDirectory {
    pub next_id: u32,
    pub households: BTreeMap<HouseholdId, Household>,
}

impl HouseholdDirectory {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            households: BTreeMap::new(),
        }
    }

    pub fn allocate_id(&mut self) -> HouseholdId {
        let id = HouseholdId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn insert(&mut self, household: Household) {
        self.households.insert(household.id, household);
    }

    pub fn get(&self, id: HouseholdId) -> Option<&Household> {
        self.households.get(&id)
    }

    pub fn get_mut(&mut self, id: HouseholdId) -> Option<&mut Household> {
        self.households.get_mut(&id)
    }

    pub fn remove(&mut self, id: HouseholdId) -> Option<Household> {
        self.households.remove(&id)
    }
}

impl Default for HouseholdDirectory {
    fn default() -> Self {
        Self::new()
    }
}
