//! The raw shape of the tables, exactly as they sit in the RON files.
//!
//! The fields are bare integers and strings: the newtypes (`Milli`, `Coins`,
//! `ServiceKind`) only appear after validation. That way a RON file with a
//! number out of range produces a readable validation error instead of an
//! obscure deserialisation one, and the files stay readable by eye.

use serde::Deserialize;
use sim_core::{Mood, Terrain};

#[derive(Debug, Clone, Deserialize)]
pub struct RawRules {
    pub ticks_per_month: u32,
    pub months_per_year: u32,
    pub starting_treasury: i32,
    pub house_levels: Vec<RawHouseLevelDef>,
    pub food_per_resident: i32,
    pub satisfaction: RawSatisfaction,
}

/// One rung of the house ladder (phase 13).
#[derive(Debug, Clone, Deserialize)]
pub struct RawHouseLevelDef {
    pub max_residents: u16,
    pub required_services: Vec<String>,
    pub level_up_threshold: u8,
    pub decay_threshold: u8,
    pub taxable_per_resident: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSatisfaction {
    pub max: u8,
    pub step_up: u8,
    pub step_down: u8,
    /// A fixed-size array on purpose: the number of bands is decided by
    /// [`Mood`], so a table with two thresholds or four is a deserialisation
    /// error and needs no check of its own. In RON it is written as a tuple,
    /// `(25, 50, 75)`, which is what serde asks of an array of fixed length.
    pub mood_thresholds: [u8; Mood::COUNT - 1],
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTerrainTable {
    pub terrains: Vec<RawTerrainDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTerrainDef {
    pub terrain: Terrain,
    pub buildable: bool,
    pub walkable: bool,
    pub road_cost: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingTable {
    pub buildings: Vec<RawBuildingDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingDef {
    pub id: String,
    pub size: (u8, u8),
    pub cost: i32,
    pub levels: u8,
    #[serde(default)]
    pub service: Option<RawServiceDef>,
    #[serde(default)]
    pub required_services: Vec<String>,
    #[serde(default)]
    pub output_per_tick: Option<i32>,
    #[serde(default)]
    pub max_stock: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawServiceDef {
    pub kind: String,
    pub range_per_level: Vec<u16>,
    pub capacity_per_level: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawDifficultyTable {
    pub profiles: Vec<RawDifficultyDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawDifficultyDef {
    pub id: String,
    pub starting_residents_per_house: u16,
}

/// The four tables just deserialised, before any check.
#[derive(Debug, Clone)]
pub struct RawDataSet {
    pub rules: RawRules,
    pub terrain: RawTerrainTable,
    pub buildings: RawBuildingTable,
    pub difficulty: RawDifficultyTable,
}
