use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};

use crate::types::{
    CitizenId, DecisionTrace, Gender, HouseholdId, HouseholdRole, MigrationStatus, OccupationType,
    SettlementId,
};

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitizenMeta {
    pub id: CitizenId,
    pub gender: Gender,
    pub alive: bool,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demographics {
    pub age_years: u16,
    pub age_ticks: u16,
    pub health: u8,
    pub fertility_timer: u8,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseholdRef {
    pub household_id: HouseholdId,
    pub role: HouseholdRole,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementRef {
    pub settlement_id: SettlementId,
    pub district_id: u8,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OccupationProfile {
    pub occupation: OccupationType,
    pub skill_level: u8,
    pub experience: u16,
    pub productivity: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalFinances {
    pub savings: f64,
    pub last_income: f32,
    pub last_expense: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalNeeds {
    pub satiety: u8,
    pub shelter: u8,
    pub comfort: u8,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilityProfile {
    pub status: MigrationStatus,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kinship {
    pub spouse: Option<CitizenId>,
    pub parent_a: Option<CitizenId>,
    pub parent_b: Option<CitizenId>,
    pub children_count: u16,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalAudit {
    pub trace: DecisionTrace,
}
