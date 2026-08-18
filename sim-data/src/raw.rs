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
    pub starting_treasury: i32,
    pub house_levels: Vec<RawHouseLevelDef>,
    pub food_per_resident: i32,
    pub satisfaction: RawSatisfaction,
    pub demographics: RawDemographics,
}

/// The demographic rates (phase 14), per month and per thousand residents.
#[derive(Debug, Clone, Deserialize)]
pub struct RawDemographics {
    pub births_per_thousand_per_month: u16,
    pub deaths_per_thousand_per_month: u16,
    pub deaths_per_thousand_per_month_when_unserved: u16,
    pub unserved_threshold: u8,
    pub birth_threshold: u8,
    pub jitter_per_thousand: u16,
}

/// One entry of the house levels table (phase 13).
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
    pub road_cost: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingTable {
    pub buildings: Vec<RawBuildingDef>,
}

/// The **flat** shape of a building row, which is not the shape `BuildingDef`
/// has after validation.
///
/// `BuildingDef` is a sum type — a house xor a provider — and this is not, on
/// purpose. The file mirrors the file: a role written as a RON enum would make
/// an unknown one a serde "unknown variant" error instead of a validation error
/// naming the row it is in, which is the rule this module opens with. Turning
/// the flat row into the sum is exactly what validation does, and the
/// contradictions it can find on the way — a house that declares a service, a
/// provider that declares none — are reported against the field to blame.
#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingDef {
    pub id: String,
    pub size: (u8, u8),
    pub cost: i32,
    pub levels: u8,
    /// What part the building plays: `"house"` or `"provider"`. **Required** —
    /// a default would let a row that forgets to say what it is become
    /// something silently, which is the very inference this field replaced.
    pub role: String,
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
