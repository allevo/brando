#![forbid(unsafe_code)]

//! Draws a map from a seed, and says what it drew.
//!
//! It is a tool, not part of the game. A generator inside the core would become
//! a frozen part of the determinism contract: every improvement to it would move
//! every recorded hash, and the rule at the head of every `.hashes` file — *if
//! this changes without the balancing having changed, a source of
//! non-determinism has been introduced* — would stop meaning anything at all. So
//! the core only ever loads files, and this crate only ever produces the map a
//! person then reads and commits.
//!
//! **Nothing here touches a disk.** [`draw`] takes numbers and gives back a
//! `MapDef`; writing one out belongs to the crate that owns the format. The
//! dependency list says so and is the whole of what this crate promises, and
//! `cargo xtask doc-check` refuses a workspace in which any crate the simulation
//! loads names this one.
//!
//! **The numbers that shape a map live here and not in the tables.** What binds
//! a number to `sim-data` is the simulation reading it, and the simulation never
//! reads any of these: it reads the file this tool wrote, by which point they
//! have become terrain and heights indistinguishable from ones a person typed.
//! The one number here the tables do own, `max_build_slope`, arrives through the
//! dataset and is never restated.

mod noise;
mod pipeline;

use std::fmt;

use sim_core::{DataSet, Grid, MapDef, Terrain, TileIndex};

pub use pipeline::{DrawError, Drawn, Repair, draw};

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
/// It is the number that actually decides whether a map is worth keeping. A map
/// that is 60% land and beautiful in a diff is worthless if no farm fits on it,
/// and the terrain shares cannot tell you that — they are the statistic that is
/// easy to compute rather than the one that answers the question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// (width, height) in tiles.
    pub size: (u8, u8),
    /// Every building in the table that has this footprint.
    pub buildings: Vec<String>,
    pub count: u32,
}

/// Reads a drawn map and says what is in it.
///
/// It never changes what was generated. A tool that quietly redraws until its
/// own statistics look good is a tool whose seed no longer means anything, and
/// the seed meaning something is the only reason to have one.
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

/// Where one footprint could stand: every tile of it buildable ground, and the
/// slope across it within `max_build_slope`.
///
/// The two conditions the game applies that are missing here — a road on the
/// tile, a building already on it — are the two that cannot be true of a map
/// nothing has been built on yet. What is left is read from the real tables
/// through the real `Grid::slope_over`, so the tool and the game cannot come to
/// different conclusions about what is placeable.
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
                        // `u16::MAX` and every index fits.
                        footprint.push(TileIndex::new(i as u16));
                    }
                }
            }
            // Every tile of it, and not merely some: a footprint with a hole
            // in it is not a place a building can stand.
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

/// `n` as a percentage of `total`, and nothing at all of a map with no tiles.
fn share(n: u32, total: u32) -> u32 {
    (n * 100).checked_div(total).unwrap_or(0)
}
