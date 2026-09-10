//! `draw`: validates a [`crate::DrawInput`] and runs the three passes plus
//! the repair, in order.

use sim_core::{Grid, GridError, MapDef, Terrain, Tile};

use crate::{DrawInput, assign, classify, relief, repair};

/// Every tile starts here (step 1's own words: "initially, all tiles have
/// height 8"), and it is also step 3's water threshold — one constant, not
/// two literal `8`s that could otherwise drift apart from each other.
pub(crate) const SEA_LEVEL: u8 = 8;

/// A map, and what the repair had to do to it.
///
/// The two repair numbers travel with the map rather than being read back
/// off it, because they cannot be: after the repair there is always exactly
/// one walkable region, so a `MapDef` alone can no longer say how many
/// there were before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    pub def: MapDef,
    pub repair: Repair,
}

/// What keeping one walkable region cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repair {
    /// How many walkable regions the classified ground came out in.
    pub regions_before: usize,
    /// How many walkable tiles were drowned to leave one of them.
    pub tiles_drowned: u32,
}

/// Every way a [`DrawInput`] can fail, and no others.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DrawError {
    #[error("cannot draw {width}x{height}: each side must be 1..={max}")]
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

/// Draws a map from a [`DrawInput`].
///
/// Pure: the same input gives the same map, and nothing is read from or
/// written to a disk.
pub fn draw(input: &DrawInput) -> Result<Drawn, DrawError> {
    let width = input.width;
    let height = input.height;
    let min_height = input.min_height;
    let max_height = input.max_height;
    let seed = input.seed;
    let sources = &input.sources;

    // The size limit is asked of the grid rather than restated here, so
    // there is only ever one answer to how big a map may be.
    let grid = match Grid::new(width, height, Terrain::Plain) {
        Ok(g) => g,
        Err(GridError::InvalidSize { width, height, max }) => {
            return Err(DrawError::UnusableSize { width, height, max });
        }
    };

    let cap = Tile::MAX_GROUND_HEIGHT;
    if min_height > max_height || max_height > cap {
        return Err(DrawError::InvalidHeightRange {
            min_height,
            max_height,
            cap,
        });
    }
    if !(min_height <= SEA_LEVEL && SEA_LEVEL <= max_height) {
        return Err(DrawError::HeightRangeExcludesSeaLevel {
            min_height,
            max_height,
            sea_level: SEA_LEVEL,
        });
    }

    if sources.is_empty() {
        return Err(DrawError::NoSources);
    }
    for (index, s) in sources.iter().enumerate() {
        if !grid.in_bounds(s.point) {
            return Err(DrawError::SourceOutOfBounds {
                index,
                x: s.point.x,
                y: s.point.y,
                width,
                height,
            });
        }
    }
    for i in 0..sources.len() {
        for j in (i + 1)..sources.len() {
            if sources[i].point == sources[j].point {
                return Err(DrawError::DuplicateSourceOrigin {
                    first: i,
                    second: j,
                    x: sources[i].point.x,
                    y: sources[i].point.y,
                });
            }
        }
    }

    // Every source's strength is at most this by construction, so
    // `max_strength - s.strength` below never underflows.
    let max_strength = sources
        .iter()
        .map(|s| s.strength)
        .max()
        .expect("checked non-empty above");

    let assigned = assign::run(seed, sources, &grid, max_strength);
    let mut ground = relief::run(
        seed,
        sources,
        &assigned,
        &grid,
        max_strength,
        min_height,
        max_height,
    );
    let mut terrain = classify::run(&ground, &grid);
    classify::flatten_the_water(&terrain, &mut ground);
    let rep = repair::keep_one_walkable_region(&mut terrain, &mut ground, &grid);

    // Provable invariant, not a defensive guess: every source contributes at
    // least one tile that step 2 never rolls on. A mountain/sea source whose
    // `A` is a single tile never acts at all (`C` starts equal to `A`), and
    // one whose `A` is larger stops acting the instant a growth act brings
    // `C` up to `A` — so the tiles that very act adds are grown into `C` but
    // never subsequently rolled, and a `Land` source's tiles are never
    // touched in the first place. Every one of those tiles is still at
    // height 8, which step 3 always classifies as land. So `regions_before`
    // can never be 0 for a non-empty source list, unlike `map-gen`'s own
    // noise, which really can submerge an entire seed.
    debug_assert!(
        rep.regions_before > 0,
        "no walkable tile survived, which step 2's own growth rule rules out"
    );
    Ok(Drawn {
        def: MapDef::new(input.id.clone(), width, height, terrain, ground),
        repair: rep,
    })
}
