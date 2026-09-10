#![forbid(unsafe_code)]

//! Draws a map from a seeded list of sources that compete for tiles and
//! then shape their own ground — `map-gen`'s sibling, built from a
//! different algorithm: growth outward from named points rather than
//! layered noise.
//!
//! The same promise `map-gen` makes, unchanged: **nothing here touches a
//! disk.** [`draw`] takes a [`DrawInput`] and gives back a `MapDef` in
//! memory; writing one out belongs to the crate that owns the format
//! (`sim_data::save_map`). `cargo xtask doc-check` refuses a workspace in
//! which any crate the simulation loads names either generator — a
//! generator the simulation could reach would become a frozen part of the
//! determinism contract, and every improvement to it would move every
//! recorded hash.

mod assign;
mod classify;
mod dice;
mod pipeline;
mod relief;
mod repair;

use std::fmt;

use sim_core::{DataSet, Grid, MapDef, Terrain, TileIndex, TilePos};

pub use pipeline::{DrawError, Drawn, Repair, draw};

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

/// Everything one draw needs, gathered so [`draw`] does not grow a seventh
/// positional argument the day an eighth is wanted.
#[derive(Debug, Clone)]
pub struct DrawInput {
    pub seed: u64,
    pub sources: Vec<Source>,
    pub width: u16,
    pub height: u16,
    pub min_height: u8,
    pub max_height: u8,
    pub id: String,
}

/// What the map turned out like, for the person deciding whether to keep it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub id: String,
    pub width: u16,
    pub height: u16,
    /// How many tiles of each terrain, indexed by `Terrain::index`.
    pub tiles: [u32; Terrain::COUNT],
    pub lowest: u8,
    pub highest: u8,
    pub repair: Repair,
    /// One entry per distinct footprint in the buildings table.
    pub places: Vec<Places>,
}

/// How many positions on the map one footprint could stand in.
///
/// It is the number that actually decides whether a map is worth keeping —
/// the same argument `map-gen`'s own report makes for its identical field:
/// a map that reads as mostly land in a diff is worthless if no farm fits
/// on it, and the terrain shares cannot tell you that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// (width, height) in tiles.
    pub size: (u8, u8),
    /// Every building in the table that has this footprint.
    pub buildings: Vec<String>,
    pub count: u32,
}

/// Reads a drawn map and says what is in it. It never changes what was
/// generated — a tool that quietly redraws until its own statistics look
/// good is a tool whose seed no longer means anything.
pub fn report(drawn: &Drawn, data: &DataSet) -> Report {
    let def = &drawn.def;
    let grid = Grid::from_map(def);

    let mut tiles = [0u32; Terrain::COUNT];
    for t in &def.terrain {
        tiles[t.index()] += 1;
    }

    let mut places: Vec<Places> = Vec::new();
    for b in &data.buildings {
        if let Some(p) = places.iter_mut().find(|p| p.size == b.size) {
            p.buildings.push(b.id.clone());
        } else {
            places.push(Places {
                size: b.size,
                buildings: vec![b.id.clone()],
                count: places_for(&grid, def, b.size, data.rules.max_build_slope),
            });
        }
    }

    Report {
        id: def.id.clone(),
        width: def.width,
        height: def.height,
        tiles,
        lowest: def.ground.iter().copied().min().unwrap_or(0),
        highest: def.ground.iter().copied().max().unwrap_or(0),
        repair: drawn.repair,
        places,
    }
}

/// Where one footprint could stand: every tile of it buildable ground, and
/// the slope across it within `max_build_slope`.
///
/// Copied from `map-gen`'s own `places_for` rather than shared between the
/// two crates: it depends on nothing but `Grid`/`Terrain`, and sharing it
/// would make one generator depend on the other for no reason either has
/// to. Read through the real `Grid::slope_over`, so the tool and the game
/// cannot come to different conclusions about what is placeable.
fn places_for(grid: &Grid, def: &MapDef, size: (u8, u8), max_slope: u8) -> u32 {
    let (bw, bh) = (u16::from(size.0), u16::from(size.1));
    if bw == 0 || bh == 0 || bw > def.width || bh > def.height {
        return 0;
    }

    let area = usize::from(bw) * usize::from(bh);
    let mut count = 0;
    let mut footprint = Vec::with_capacity(area);
    for oy in 0..=(def.height - bh) {
        for ox in 0..=(def.width - bw) {
            footprint.clear();
            for dy in 0..bh {
                for dx in 0..bw {
                    let i = usize::from(oy + dy) * usize::from(def.width) + usize::from(ox + dx);
                    if def.terrain[i].is_buildable() {
                        // The map is at most 256x256, so the last index is
                        // u16::MAX and every index fits.
                        footprint.push(TileIndex::new(i as u16));
                    }
                }
            }
            let whole = footprint.len() == area;
            if whole && grid.slope_over(&footprint) <= max_slope {
                count += 1;
            }
        }
    }
    count
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = u32::from(self.width) * u32::from(self.height);
        writeln!(f, "map '{}' — {}x{}", self.id, self.width, self.height)?;
        for t in Terrain::ALL {
            let n = self.tiles[t.index()];
            writeln!(
                f,
                "  {:<8} {n:>6}  {:>3}%",
                format!("{t:?}"),
                share(n, total)
            )?;
        }
        writeln!(f, "  height   {:>6}..{}", self.lowest, self.highest)?;
        writeln!(
            f,
            "  repair   {:>6} walkable regions, {} tiles drowned to leave one",
            self.repair.regions_before, self.repair.tiles_drowned
        )?;
        writeln!(f, "  places a building of each footprint could stand in:")?;
        for p in &self.places {
            writeln!(
                f,
                "    {}x{} {:>6}   {}",
                p.size.0,
                p.size.1,
                p.count,
                p.buildings.join(", ")
            )?;
        }
        Ok(())
    }
}

/// `n` as a percentage of `total`, and nothing at all of a map with no
/// tiles.
fn share(n: u32, total: u32) -> u32 {
    (n * 100).checked_div(total).unwrap_or(0)
}
