#![forbid(unsafe_code)]

//! Generates a map from a seeded list of sources that compete for tiles and
//! then shape their own ground: growth outward from named points, not
//! layered noise.
//!
//! One entry point: [`generate`] takes a [`Config`] and gives back the
//! [`MapDef`] the game plays — the same type `sim_data::load_map_by_id`
//! returns, so a generated map and a loaded one are the same kind of thing.
//! **Nothing here touches a disk**: writing a map out belongs to the crate
//! that owns the format (`sim_data::save_map`). `cargo xtask doc-check`
//! refuses a workspace in which any crate the simulation loads names this
//! one — a generator the simulation could reach would become a frozen part
//! of the determinism contract, and every improvement to it would move every
//! recorded hash.

mod assign;
mod check;
mod classify;
mod dice;
mod relief;
mod repair;

use sim_core::{MapDef, TilePos};

/// Every tile starts here (step 1's own words: "initially, all tiles have
/// height 8"), and it is also step 3's water threshold — one constant, not
/// two literal `8`s that could otherwise drift apart from each other.
pub(crate) const SEA_LEVEL: u8 = 8;

/// What a source turns the tiles it claims into, and — for two of the three
/// kinds — how it goes on shaping their height afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Mountain,
    Land,
    Sea,
}

/// One point a map grows from: where it starts, what it makes, and how
/// often it gets a turn relative to the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub point: TilePos,
    pub kind: SourceKind,
    pub strength: u16,
}

/// Everything one map is generated from.
#[derive(Debug, Clone)]
pub struct Config {
    pub seed: u64,
    pub sources: Vec<Source>,
    pub width: u16,
    pub height: u16,
    pub min_height: u8,
    pub max_height: u8,
    pub id: String,
}

/// Every way a [`Config`] can be unusable, and no others.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot generate a {width}x{height} map: each side must be 1..={max}")]
    UnusableSize { width: u16, height: u16, max: u16 },

    #[error(
        "height range {min_height}..={max_height} is not usable: each bound must be 0..={cap}, and min <= max"
    )]
    InvalidHeightRange {
        min_height: u8,
        max_height: u8,
        cap: u8,
    },

    #[error(
        "height range {min_height}..={max_height} excludes sea level ({sea_level}): every tile starts there"
    )]
    HeightRangeExcludesSeaLevel {
        min_height: u8,
        max_height: u8,
        sea_level: u8,
    },

    #[error("no sources: nothing would ever get a type")]
    NoSources,

    #[error("source {index} sits at ({x}, {y}), outside the {width}x{height} map")]
    SourceOutOfBounds {
        index: usize,
        x: u8,
        y: u8,
        width: u16,
        height: u16,
    },

    #[error(
        "sources {first} and {second} both start at ({x}, {y}); every source needs its own tile"
    )]
    DuplicateSourceOrigin {
        first: usize,
        second: usize,
        x: u8,
        y: u8,
    },
}

/// Generates the map `config` describes — pure, so the same config always
/// gives the same map, and nothing is read from or written to a disk.
pub fn generate(config: &Config) -> Result<MapDef, ConfigError> {
    let grid = check::run(config)?;

    // Every source's strength is at most this by construction, so
    // `max_strength - s.strength` in the passes never underflows.
    let max_strength = config
        .sources
        .iter()
        .map(|s| s.strength)
        .max()
        .expect("check::run refuses an empty source list");

    let assigned = assign::run(config.seed, &config.sources, &grid, max_strength);
    let mut ground = relief::run(
        config.seed,
        &config.sources,
        &assigned,
        &grid,
        max_strength,
        config.min_height,
        config.max_height,
    );
    let mut terrain = classify::run(&ground, &grid);
    classify::flatten_the_water(&terrain, &mut ground);

    // Provable, not a guess: every source leaves a tile step 2 never rolls
    // on — a `Land` source never touches height, and a mountain or sea
    // source never acts again after the act that grows `C` up to `A` (or
    // never acts at all, when `A` is one tile), so the tiles that act adds
    // stay at sea level, which is land.
    debug_assert!(
        terrain.iter().any(|t| t.is_walkable()),
        "no walkable tile survived, which step 2's own growth rule rules out"
    );
    repair::keep_one_walkable_region(&mut terrain, &mut ground, &grid);

    Ok(MapDef::new(
        config.id.clone(),
        config.width,
        config.height,
        terrain,
        ground,
    ))
}
