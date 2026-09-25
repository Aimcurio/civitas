use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CitizenId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HouseholdId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SettlementId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    Female,
    Male,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OccupationType {
    Unemployed,
    Farmer,
    Forester,
    Miner,
    Artisan,
    Merchant,
    Laborer,
}

impl OccupationType {
    pub const ALL: [OccupationType; 7] = [
        OccupationType::Unemployed,
        OccupationType::Farmer,
        OccupationType::Forester,
        OccupationType::Miner,
        OccupationType::Artisan,
        OccupationType::Merchant,
        OccupationType::Laborer,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Food,
    Timber,
    Stone,
    Tools,
    Luxury,
}

impl ResourceType {
    pub const ALL: [ResourceType; 5] = [
        ResourceType::Food,
        ResourceType::Timber,
        ResourceType::Stone,
        ResourceType::Tools,
        ResourceType::Luxury,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Plains,
    Forest,
    Hills,
    Mountains,
    Water,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    Settled,
    InTransit {
        origin: SettlementId,
        destination: SettlementId,
        ticks_remaining: u16,
        total_ticks: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HouseholdRole {
    Head,
    Spouse,
    Child,
    Dependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasonCode {
    NaturalBirth,
    OldAgeSenescence,
    StarvationDeath,
    JobOpportunityFound,
    JobLaidOff,
    HouseholdFormedMarriage,
    HouseholdDissolved,
    MigrationBetterWages,
    MigrationFleeingHunger,
    MigrationCheaperHousing,
    MigrationFamilyReunion,
    InitialSpawn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionTrace {
    pub reason: ReasonCode,
    pub tick: u64,
    pub primary_metric: f32,
    pub secondary_metric: f32,
    pub context_id: u32,
}

impl DecisionTrace {
    pub fn new(
        reason: ReasonCode,
        tick: u64,
        primary: f32,
        secondary: f32,
        context_id: u32,
    ) -> Self {
        Self {
            reason,
            tick,
            primary_metric: primary,
            secondary_metric: secondary,
            context_id,
        }
    }

    pub fn to_human_explanation(&self) -> String {
        match self.reason {
            ReasonCode::InitialSpawn => "Created during world generation".to_string(),
            ReasonCode::NaturalBirth => format!("Born to parents at tick {}", self.tick),
            ReasonCode::OldAgeSenescence => format!(
                "Passed away from natural old age (senescence factor {:.2})",
                self.primary_metric
            ),
            ReasonCode::StarvationDeath => {
                "Died from acute starvation after health collapsed to 0".to_string()
            }
            ReasonCode::JobOpportunityFound => format!(
                "Employed due to wage offering of {:.1} coins (demand factor {:.2})",
                self.primary_metric, self.secondary_metric
            ),
            ReasonCode::JobLaidOff => {
                "Displaced from previous occupation due to production contraction".to_string()
            }
            ReasonCode::HouseholdFormedMarriage => format!(
                "Established independent household with partner #{} at tick {}",
                self.context_id, self.tick
            ),
            ReasonCode::HouseholdDissolved => {
                "Household dissolved due to loss of all residing members".to_string()
            }
            ReasonCode::MigrationBetterWages => format!(
                "Migrated to settlement #{} seeking {:.1}% higher real wages",
                self.context_id,
                (self.primary_metric - 1.0) * 100.0
            ),
            ReasonCode::MigrationFleeingHunger => format!(
                "Fled food deficit (local satiety dropped to {:.0}%) for settlement #{}",
                self.primary_metric, self.context_id
            ),
            ReasonCode::MigrationCheaperHousing => format!(
                "Relocated to settlement #{} to escape housing shortage (utilization {:.0}%)",
                self.context_id,
                self.primary_metric * 100.0
            ),
            ReasonCode::MigrationFamilyReunion => format!(
                "Relocated to rejoin family members in settlement #{}",
                self.context_id
            ),
        }
    }
}

#[derive(bevy_ecs::system::Resource, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimClock {
    pub tick: u64,
}

impl SimClock {
    pub fn new() -> Self {
        Self { tick: 0 }
    }
}

impl Default for SimClock {
    fn default() -> Self {
        Self::new()
    }
}
