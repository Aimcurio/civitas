use bevy_ecs::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::components::{
    CitizenMeta, Demographics, OccupationProfile, PersonalFinances, SettlementRef,
};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::types::SimClock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    pub seed: u64,
    pub initial_population: usize,
    pub total_ticks: u64,
    pub checkpoints: BTreeMap<u64, u64>,
}

pub fn compute_authoritative_state_hash(world: &mut World) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a 64-bit basis
    let prime: u64 = 0x100000001b3;

    let hash_bytes = |hash: &mut u64, bytes: &[u8]| {
        for &b in bytes {
            *hash ^= b as u64;
            *hash = hash.wrapping_mul(prime);
        }
    };

    if let Some(clock) = world.get_resource::<SimClock>() {
        hash_bytes(&mut h, &clock.tick.to_le_bytes());
    }

    if let Some(world_map) = world.get_resource::<crate::world::WorldMap>() {
        for (sid, pos) in &world_map.settlement_positions {
            hash_bytes(&mut h, &sid.0.to_le_bytes());
            hash_bytes(&mut h, &pos.0.to_le_bytes());
            hash_bytes(&mut h, &pos.1.to_le_bytes());
        }
    }

    if let Some(settlements) = world.get_resource::<SettlementDirectory>() {
        for (sid, s) in &settlements.settlements {
            hash_bytes(&mut h, &sid.0.to_le_bytes());
            hash_bytes(&mut h, &s.population.to_le_bytes());
            hash_bytes(&mut h, &(s.treasury as i64).to_le_bytes());
            for (res, &inv) in &s.inventories {
                hash_bytes(&mut h, &(*res as u8).to_le_bytes());
                hash_bytes(&mut h, &(inv as i64).to_le_bytes());
            }
            for (res, &price) in &s.prices {
                hash_bytes(&mut h, &(*res as u8).to_le_bytes());
                hash_bytes(&mut h, &((price * 1000.0) as i64).to_le_bytes());
            }
        }
    }

    if let Some(households) = world.get_resource::<HouseholdDirectory>() {
        hash_bytes(&mut h, &(households.households.len() as u64).to_le_bytes());
        for (hid, hh) in &households.households {
            hash_bytes(&mut h, &hid.0.to_le_bytes());
            hash_bytes(&mut h, &(hh.savings as i64).to_le_bytes());
            hash_bytes(&mut h, &(hh.members.len() as u64).to_le_bytes());
        }
    }

    let mut query = world.query::<(
        &CitizenMeta,
        &Demographics,
        &SettlementRef,
        &OccupationProfile,
        &PersonalFinances,
    )>();

    let mut citizen_hashes: Vec<(u64, u64)> = Vec::new();
    for (m, d, s, o, f) in query.iter(world) {
        let mut ch: u64 = m.id.0;
        ch = ch.wrapping_mul(prime) ^ (if m.alive { 1 } else { 0 });
        ch = ch.wrapping_mul(prime) ^ (d.age_years as u64);
        ch = ch.wrapping_mul(prime) ^ (d.health as u64);
        ch = ch.wrapping_mul(prime) ^ (s.settlement_id.0 as u64);
        ch = ch.wrapping_mul(prime) ^ (o.occupation as u64);
        ch = ch.wrapping_mul(prime) ^ ((f.savings * 100.0) as u64);
        citizen_hashes.push((m.id.0, ch));
    }

    // Sort by citizen ID to guarantee deterministic reduction
    citizen_hashes.sort_by_key(|k| k.0);
    for (_, ch) in citizen_hashes {
        hash_bytes(&mut h, &ch.to_le_bytes());
    }

    h
}
