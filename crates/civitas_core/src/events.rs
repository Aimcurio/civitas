use bevy_ecs::system::Resource;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::types::{CitizenId, DecisionTrace, SettlementId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimEventType {
    Birth,
    Death,
    JobChange,
    Marriage,
    MigrationStart,
    MigrationArrival,
    PriceShock,
    SettlementExpansion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimEvent {
    pub tick: u64,
    pub event_type: SimEventType,
    pub entity: Option<CitizenId>,
    pub settlement: Option<SettlementId>,
    pub reason: DecisionTrace,
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct EventRing {
    pub capacity: usize,
    pub events: VecDeque<SimEvent>,
    pub total_emitted: u64,
}

impl EventRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::with_capacity(capacity),
            total_emitted: 0,
        }
    }

    pub fn push(&mut self, event: SimEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.total_emitted += 1;
    }

    pub fn recent(&self, count: usize) -> Vec<&SimEvent> {
        self.events.iter().rev().take(count).collect()
    }
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new(10_000)
    }
}
