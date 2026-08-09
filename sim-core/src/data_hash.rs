//! The dataset hash.
//!
//! It feeds the hasher field by field in an order spelled out here explicitly,
//! instead of serialising with serde (A3): with serde the hash would depend on
//! the order the fields are declared in, and moving a field — a refactor with
//! no semantic consequences — would invalidate every recording.
//!
//! Corollary: adding a field to the tables requires adding it here by hand. If
//! that is not done, a balance change on that field will not make the replays
//! fail. It is the deliberate cost of A3.

use std::collections::BTreeMap;

use crate::grid::Terrain;

use crate::data::{BuildingDef, Rules, TerrainDef};

/// Domain prefix: keeps this hash apart from any other blake3 in the project.
/// Changing it regenerates every recording.
const DOMAIN: &[u8] = b"brando/dataset/v1";

pub(crate) fn dataset_hash(
    rules: &Rules,
    terrain: &BTreeMap<Terrain, TerrainDef>,
    buildings: &[BuildingDef],
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMAIN);

    // --- rules ---
    h.update(&rules.ticks_per_month.to_le_bytes());
    h.update(&rules.months_per_year.to_le_bytes());
    h.update(&rules.starting_treasury.get().to_le_bytes());
    h.update(&(rules.residents_per_house_level.len() as u64).to_le_bytes());
    for a in &rules.residents_per_house_level {
        h.update(&a.to_le_bytes());
    }
    h.update(&rules.food_per_resident.to_millis().to_le_bytes());

    // --- terrain, in Terrain order (the BTreeMap guarantees it) ---
    h.update(&(terrain.len() as u64).to_le_bytes());
    for (t, d) in terrain {
        h.update(&[*t as u8]);
        h.update(&[u8::from(d.buildable), u8::from(d.walkable)]);
        h.update(&d.road_cost.get().to_le_bytes());
    }

    // --- buildings, in BuildingKindId order ---
    h.update(&(buildings.len() as u64).to_le_bytes());
    for b in buildings {
        // The length before the content: without it, "ab"+"c" and "a"+"bc"
        // would give the same hash.
        h.update(&(b.id.len() as u64).to_le_bytes());
        h.update(b.id.as_bytes());
        h.update(&[b.size.0, b.size.1, b.levels]);
        h.update(&b.cost.get().to_le_bytes());

        match &b.service {
            None => {
                h.update(&[0u8]);
            }
            Some(s) => {
                h.update(&[1u8]);
                h.update(&[s.kind.index() as u8]);
                hash_u16_slice(&mut h, &s.range_per_level);
                hash_u16_slice(&mut h, &s.capacity_per_level);
            }
        }

        h.update(&(b.required_services.len() as u64).to_le_bytes());
        for s in &b.required_services {
            h.update(&[s.index() as u8]);
        }

        hash_opt_milli(&mut h, b.output_per_tick);
        hash_opt_milli(&mut h, b.max_stock);
    }

    *h.finalize().as_bytes()
}

fn hash_u16_slice(h: &mut blake3::Hasher, v: &[u16]) {
    h.update(&(v.len() as u64).to_le_bytes());
    for x in v {
        h.update(&x.to_le_bytes());
    }
}

fn hash_opt_milli(h: &mut blake3::Hasher, v: Option<crate::units::Milli>) {
    match v {
        None => {
            h.update(&[0u8]);
        }
        Some(m) => {
            h.update(&[1u8]);
            h.update(&m.to_millis().to_le_bytes());
        }
    }
}
