use bevy_ecs::prelude::*;

use crate::components::{CausalAudit, CitizenMeta, Demographics, PhysicalNeeds, SettlementRef};
use crate::events::{EventRing, SimEvent, SimEventType};
use crate::settlement::SettlementDirectory;
use crate::types::{DecisionTrace, ReasonCode, SimClock};

pub fn physiology_system(
    clock: Res<SimClock>,
    mut events: ResMut<EventRing>,
    mut settlements: ResMut<SettlementDirectory>,
    mut query: Query<(
        &mut CitizenMeta,
        &mut PhysicalNeeds,
        &mut Demographics,
        &mut CausalAudit,
        &SettlementRef,
    )>,
) {
    let tick = clock.tick;

    for (mut meta, mut needs, mut demo, mut audit, settlement_ref) in query.iter_mut() {
        if !meta.alive {
            continue;
        }

        // Daily hunger degradation
        if needs.satiety > 0 {
            needs.satiety = needs.satiety.saturating_sub(1);
        }

        // Health impact of starvation or nourishment
        if needs.satiety == 0 {
            demo.health = demo.health.saturating_sub(5);
        } else if needs.satiety > 70 && demo.health < 100 {
            demo.health = (demo.health + 1).min(100);
        }

        // Check for mortality
        if demo.health == 0 {
            meta.alive = false;
            let trace = DecisionTrace::new(ReasonCode::StarvationDeath, tick, 0.0, 0.0, 0);
            audit.trace = trace.clone();

            if let Some(settlement) = settlements.get_mut(settlement_ref.settlement_id) {
                settlement.total_deaths += 1;
                if settlement.population > 0 {
                    settlement.population -= 1;
                }
            }

            events.push(SimEvent {
                tick,
                event_type: SimEventType::Death,
                entity: Some(meta.id),
                settlement: Some(settlement_ref.settlement_id),
                reason: trace,
            });
        }
    }
}
