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

use crate::data::{
    BuildingDef, DemographicsRules, DifficultyDef, HouseLevelDef, Rules, SatisfactionRules,
    TerrainDef,
};

/// Hash prefix: keeps this hash apart from any other blake3 in the project.
/// Changing it regenerates every recording.
const PREFIX: &[u8] = b"brando/dataset/v1";

pub(crate) fn dataset_hash(
    rules: &Rules,
    terrain: &BTreeMap<Terrain, TerrainDef>,
    buildings: &[BuildingDef],
    difficulties: &[DifficultyDef],
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(PREFIX);

    // --- rules ---
    // Destructured, not accessed field by field: the exhaustive pattern stops
    // compiling the moment a field is added to the table, which is the cheapest
    // possible reminder that A3's cost has to be paid here by hand.
    let Rules {
        ticks_per_month,
        months_per_year,
        starting_treasury,
        house_levels,
        food_per_resident,
        satisfaction,
        demographics,
    } = rules;
    h.update(&ticks_per_month.to_le_bytes());
    h.update(&months_per_year.to_le_bytes());
    h.update(&starting_treasury.get().to_le_bytes());
    h.update(&(house_levels.len() as u64).to_le_bytes());
    for l in house_levels {
        let HouseLevelDef {
            max_residents,
            required_services,
            level_up_threshold,
            decay_threshold,
            taxable_per_resident,
        } = l;
        h.update(&max_residents.to_le_bytes());
        h.update(&(required_services.len() as u64).to_le_bytes());
        for s in required_services {
            h.update(&[s.index() as u8]);
        }
        h.update(&[*level_up_threshold, *decay_threshold]);
        h.update(&taxable_per_resident.to_millis().to_le_bytes());
    }
    h.update(&food_per_resident.to_millis().to_le_bytes());

    let SatisfactionRules {
        max,
        step_up,
        step_down,
        mood_thresholds,
    } = satisfaction;
    h.update(&[*max, *step_up, *step_down]);
    h.update(mood_thresholds);

    let DemographicsRules {
        births_per_thousand_per_month,
        deaths_per_thousand_per_month,
        deaths_per_thousand_per_month_when_unserved,
        unserved_threshold,
        birth_threshold,
        jitter_per_thousand,
    } = demographics;
    h.update(&births_per_thousand_per_month.to_le_bytes());
    h.update(&deaths_per_thousand_per_month.to_le_bytes());
    h.update(&deaths_per_thousand_per_month_when_unserved.to_le_bytes());
    h.update(&[*unserved_threshold, *birth_threshold]);
    h.update(&jitter_per_thousand.to_le_bytes());

    // --- terrain, in Terrain order (the BTreeMap guarantees it) ---
    h.update(&(terrain.len() as u64).to_le_bytes());
    for (t, d) in terrain {
        let TerrainDef {
            buildable,
            walkable,
            road_cost,
        } = d;
        h.update(&[*t as u8]);
        h.update(&[u8::from(*buildable), u8::from(*walkable)]);
        h.update(&road_cost.get().to_le_bytes());
    }

    // --- buildings, in BuildingKindId order ---
    h.update(&(buildings.len() as u64).to_le_bytes());
    for b in buildings {
        let BuildingDef {
            id,
            size,
            cost,
            levels,
            service,
            required_services,
            output_per_tick,
            max_stock,
        } = b;
        // The length before the content: without it, "ab"+"c" and "a"+"bc"
        // would give the same hash.
        h.update(&(id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(&[size.0, size.1, *levels]);
        h.update(&cost.get().to_le_bytes());

        match service {
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

        h.update(&(required_services.len() as u64).to_le_bytes());
        for s in required_services {
            h.update(&[s.index() as u8]);
        }

        hash_opt_milli(&mut h, *output_per_tick);
        hash_opt_milli(&mut h, *max_stock);
    }

    // --- difficulty profiles, in DifficultyId order ---
    h.update(&(difficulties.len() as u64).to_le_bytes());
    for d in difficulties {
        let DifficultyDef {
            id,
            starting_residents_per_house,
        } = d;
        h.update(&(id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(&starting_residents_per_house.to_le_bytes());
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
