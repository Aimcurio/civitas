use bevy_ecs::world::World;
use std::collections::{BTreeMap, BTreeSet};

use crate::components::{CitizenMeta, HouseholdRef, MobilityProfile, SettlementRef};
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::types::{CitizenId, MigrationStatus, SettlementId};

pub fn verify_invariants(world: &mut World) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let hh_dir = match world.get_resource::<HouseholdDirectory>() {
        Some(h) => h.clone(),
        None => {
            errors.push("Missing HouseholdDirectory resource".to_string());
            return Err(errors);
        }
    };

    let settlement_dir = match world.get_resource::<SettlementDirectory>() {
        Some(s) => s.clone(),
        None => {
            errors.push("Missing SettlementDirectory resource".to_string());
            return Err(errors);
        }
    };

    // 1. Verify settlement resources & inventories are non-negative and non-NaN
    for (sid, s) in &settlement_dir.settlements {
        if s.treasury.is_nan() || s.treasury < 0.0 {
            errors.push(format!(
                "Settlement {:?} has invalid treasury: {}",
                sid, s.treasury
            ));
        }
        for (res, &inv) in &s.inventories {
            if inv.is_nan() || inv < 0.0 {
                errors.push(format!(
                    "Settlement {:?} has invalid {:?} inventory: {}",
                    sid, res, inv
                ));
            }
        }
        for (res, &price) in &s.prices {
            if price.is_nan() || price <= 0.0 {
                errors.push(format!(
                    "Settlement {:?} has invalid {:?} price: {}",
                    sid, res, price
                ));
            }
        }
    }

    // 2. Query all citizens
    let mut query = world.query::<(
        &CitizenMeta,
        &HouseholdRef,
        &SettlementRef,
        &MobilityProfile,
    )>();

    let mut living_citizen_ids = BTreeSet::new();
    let mut settlement_pop_counts: BTreeMap<SettlementId, u32> = BTreeMap::new();
    let mut citizen_to_hh: BTreeMap<CitizenId, crate::types::HouseholdId> = BTreeMap::new();

    for (meta, hh_ref, settlement_ref, mobility) in query.iter(world) {
        if meta.alive {
            living_citizen_ids.insert(meta.id);
            citizen_to_hh.insert(meta.id, hh_ref.household_id);

            // Check settlement reference validity
            if !settlement_dir
                .settlements
                .contains_key(&settlement_ref.settlement_id)
            {
                errors.push(format!(
                    "Citizen {:?} references invalid settlement {:?}",
                    meta.id, settlement_ref.settlement_id
                ));
            }

            // Check mobility transit state consistency
            match mobility.status {
                MigrationStatus::Settled => {
                    *settlement_pop_counts
                        .entry(settlement_ref.settlement_id)
                        .or_insert(0) += 1;
                }
                MigrationStatus::InTransit {
                    origin,
                    destination,
                    ..
                } => {
                    if !settlement_dir.settlements.contains_key(&origin) {
                        errors.push(format!(
                            "Citizen {:?} has invalid transit origin {:?}",
                            meta.id, origin
                        ));
                    }
                    if !settlement_dir.settlements.contains_key(&destination) {
                        errors.push(format!(
                            "Citizen {:?} has invalid transit destination {:?}",
                            meta.id, destination
                        ));
                    }
                }
            }
        }
    }

    // 3. Verify bidirectional household consistency
    for (&cid, &hid) in &citizen_to_hh {
        if let Some(hh) = hh_dir.get(hid) {
            if !hh.members.contains(&cid) {
                errors.push(format!(
                    "Citizen {:?} points to Household {:?}, but household does not list citizen as member",
                    cid, hid
                ));
            }
        } else {
            errors.push(format!(
                "Citizen {:?} references non-existent Household {:?}",
                cid, hid
            ));
        }
    }

    for (hid, hh) in &hh_dir.households {
        for &member_id in &hh.members {
            if living_citizen_ids.contains(&member_id) {
                if let Some(&actual_hid) = citizen_to_hh.get(&member_id) {
                    if actual_hid != *hid {
                        errors.push(format!(
                            "Household {:?} lists Member {:?}, but citizen points to Household {:?}",
                            hid, member_id, actual_hid
                        ));
                    }
                }
            }
        }
    }

    // 4. Verify settlement population tally consistency
    for (sid, s) in &settlement_dir.settlements {
        let actual_count = *settlement_pop_counts.get(sid).unwrap_or(&0);
        if s.population != actual_count {
            errors.push(format!(
                "Settlement {:?} recorded population {} differs from actual living count {}",
                sid, s.population, actual_count
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
