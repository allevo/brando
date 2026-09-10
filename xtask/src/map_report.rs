//! What a generated map turned out like, for the person deciding whether to
//! keep it.
//!
//! It reads only the finished `MapDef` and the tables, never anything from
//! inside the generator, and it never changes the map: a tool that quietly
//! redraws until its own statistics look good is a tool whose seed no longer
//! means anything.

use std::fmt;

use sim_core::{DataSet, Grid, MapDef, Terrain, TileIndex};

/// What the map turned out like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub id: String,
    pub width: u16,
    pub height: u16,
    /// How many tiles of each terrain, indexed by `Terrain::index`.
    pub tiles: [u32; Terrain::COUNT],
    pub lowest: u8,
    pub highest: u8,
    /// One entry per distinct footprint in the buildings table.
    pub places: Vec<Places>,
}

/// How many positions on the map one footprint could stand in.
///
/// It is the number that actually decides whether a map is worth keeping: a
/// map that reads as mostly land in a diff is worthless if no farm fits on
/// it, and the terrain shares cannot tell you that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// (width, height) in tiles.
    pub size: (u8, u8),
    /// Every building in the table that has this footprint.
    pub buildings: Vec<String>,
    pub count: u32,
}

/// Reads a map and says what is in it.
pub fn report(map: &MapDef, data: &DataSet) -> Report {
    let grid = Grid::from_map(map);

    let mut tiles = [0u32; Terrain::COUNT];
    for t in &map.terrain {
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
                count: places_for(&grid, map, b.size, data.rules.max_build_slope),
            });
        }
    }

    Report {
        id: map.id.clone(),
        width: map.width,
        height: map.height,
        tiles,
        lowest: map.ground.iter().copied().min().unwrap_or(0),
        highest: map.ground.iter().copied().max().unwrap_or(0),
        places,
    }
}

/// Where one footprint could stand: every tile of it buildable ground, and
/// the slope across it within `max_build_slope`.
///
/// Read through the real `Grid::slope_over`, so the tool and the game cannot
/// come to different conclusions about what is placeable.
fn places_for(grid: &Grid, map: &MapDef, size: (u8, u8), max_slope: u8) -> u32 {
    let (bw, bh) = (u16::from(size.0), u16::from(size.1));
    if bw == 0 || bh == 0 || bw > map.width || bh > map.height {
        return 0;
    }

    let area = usize::from(bw) * usize::from(bh);
    let mut count = 0;
    let mut footprint = Vec::with_capacity(area);
    for oy in 0..=(map.height - bh) {
        for ox in 0..=(map.width - bw) {
            footprint.clear();
            for dy in 0..bh {
                for dx in 0..bw {
                    let i = usize::from(oy + dy) * usize::from(map.width) + usize::from(ox + dx);
                    if map.terrain[i].is_buildable() {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use map_gen::{Config, Source, SourceKind};
    use sim_core::{Tile, TilePos};

    use super::*;

    const W: u16 = 48;
    const H: u16 = 32;

    /// A 48x32 map grown from its four corners and its centre, the same
    /// sample `map-gen`'s own tests generate from.
    fn config(seed: u64) -> Config {
        let point = |x, y, kind, strength| Source {
            point: TilePos::new(x, y),
            kind,
            strength,
        };
        Config {
            seed,
            sources: vec![
                point(0, 0, SourceKind::Mountain, 3),
                point(47, 31, SourceKind::Sea, 5),
                point(0, 31, SourceKind::Land, 1),
                point(47, 0, SourceKind::Mountain, 4),
                point(23, 15, SourceKind::Sea, 2),
            ],
            width: W,
            height: H,
            min_height: 0,
            max_height: Tile::MAX_GROUND_HEIGHT,
            id: "reported".to_string(),
        }
    }

    /// Somewhere to put the largest building, on a size and source layout large
    /// enough to have room for one — an arbitrary source list is not
    /// guaranteed to leave anywhere buildable at all, so this is read off a
    /// specific, generous configuration rather than swept over many.
    #[test]
    fn somewhere_to_put_the_largest_building() {
        let data = sim_data::load_default().expect("the production tables load");
        let biggest = data
            .buildings
            .iter()
            .max_by_key(|b| u32::from(b.size.0) * u32::from(b.size.1))
            .expect("the tables hold at least one building");

        let found = (0..24u64).any(|seed| {
            let map = map_gen::generate(&config(seed)).expect("a map at a usable size");
            report(&map, &data)
                .places
                .iter()
                .find(|p| p.size == biggest.size)
                .expect("the report covers every footprint in the table")
                .count
                > 0
        });
        assert!(
            found,
            "no seed in the sweep left room for a {}x{} '{}'",
            biggest.size.0, biggest.size.1, biggest.id
        );
    }

    /// The report counts what the map holds and nothing else.
    #[test]
    fn the_report_adds_up_to_the_map() {
        let data = sim_data::load_default().expect("the production tables load");
        let map = map_gen::generate(&config(5)).expect("a map");
        let r = report(&map, &data);

        assert_eq!(r.tiles.iter().sum::<u32>(), u32::from(W) * u32::from(H));
        assert!(r.lowest <= r.highest);
        assert_eq!(
            r.places.len(),
            data.buildings
                .iter()
                .map(|b| b.size)
                .collect::<BTreeSet<_>>()
                .len(),
            "one entry per distinct footprint, no more and no fewer"
        );
    }
}
