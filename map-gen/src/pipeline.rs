//! The passes that turn a seed into a map, in the order they run.
//!
//! The order carries meaning in two places, and both are stated at the pass
//! that depends on it: the water is flattened *after* the ground is
//! classified, and the walkable ground is repaired *last*.

use sim_core::{Grid, GridError, MapDef, Terrain, Tile};

use crate::noise;

/// The one field drawn so far. It is named rather than assumed, so a second
/// field added later cannot shift this one.
const RELIEF: &str = "relief";

// The numbers below shape a map, and they live here rather than in `sim-data`.
// What binds a number to the tables is *the simulation reading it*, and the
// simulation never reads any of these: it reads the file this tool wrote, and
// by then they have become terrain and heights like any a person could have
// typed. A number the tables do own, `max_build_slope`, arrives through the
// dataset and is never restated here.

/// How far apart the corners of the first layer of noise stand, in tiles. It
/// is roughly the size of the biggest landforms; every layer after it halves
/// the spacing.
const CORNER_SPACING: u16 = 12;

/// How many tiles from the frame the ground is pulled down to nothing.
const BORDER_MARGIN: u16 = 5;

/// Below this height there is water.
const SEA_LEVEL: u8 = 10;

/// Above this height there is rock: high ground nobody settles on.
const ROCK_HEIGHT: u8 = 23;

/// A tile whose step to its steepest neighbour reaches this is rock however
/// low it stands, because it is a cliff rather than a hillside.
const ROCK_STEP: u8 = 3;

/// A map, and what the repair had to do to it.
///
/// The two repair numbers travel with the map rather than being read back off
/// it, because they cannot be read back off it: after the repair there is
/// always exactly one walkable region, so a `MapDef` alone can no longer say
/// how many there were.
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

/// The two ways a seed can fail to become a map, and there are no others: a
/// size the grid will not hold, and a seed whose ground is all water.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DrawError {
    #[error("cannot draw {width}x{height}: each side must be 1..={max}")]
    UnusableSize { width: u16, height: u16, max: u16 },

    #[error("seed {seed} drew no land at all: nothing to build a city on")]
    NoLandLeft { seed: u64 },
}

/// Draws a map from a seed.
///
/// Pure: the same arguments give the same map, and nothing is read from or
/// written to a disk.
pub fn draw(seed: u64, width: u16, height: u16, id: &str) -> Result<Drawn, DrawError> {
    // The size limit is asked of the grid rather than restated here, so there
    // is only ever one answer to how big a map may be.
    if let Err(GridError::InvalidSize { width, height, max }) =
        Grid::new(width, height, Terrain::Plain)
    {
        return Err(DrawError::UnusableSize { width, height, max });
    }

    let mut relief = noise::field(seed, RELIEF, width, height, CORNER_SPACING);
    fall_away_at_the_border(&mut relief, width, height);
    let mut ground = to_steps(&relief);
    let mut terrain = classify(&ground, width, height);
    flatten_the_water(&terrain, &mut ground);
    let repair = keep_one_walkable_region(&mut terrain, &mut ground, width, height);

    if repair.regions_before == 0 {
        return Err(DrawError::NoLandLeft { seed });
    }
    Ok(Drawn {
        def: MapDef::new(id.to_string(), width, height, terrain, ground),
        repair,
    })
}

/// Pulls the ground down towards nothing over the last [`BORDER_MARGIN`]
/// tiles, so a map ends in sea rather than in land sliced off at the frame.
///
/// Down the same curve the noise eases between its corners, and not down a
/// straight line. A straight fall meets the interior at an angle, and that
/// angle is a step of its own — enough of one that [`classify`] read it as a
/// cliff and ringed every map with rock, which looks like a bathtub and not
/// like a coast. The curve is flat where it meets the land, so what steepness
/// there is falls in the middle of the margin, where the ground has already
/// gone under the sea and is water either way.
fn fall_away_at_the_border(field: &mut [i64], width: u16, height: u16) {
    let (w, h) = (width as usize, height as usize);
    let margin = i64::from(BORDER_MARGIN);
    for y in 0..h {
        for x in 0..w {
            let to_edge = x.min(y).min(w - 1 - x).min(h - 1 - y) as i64;
            let part = to_edge.min(margin) * noise::UNIT / margin;
            field[y * w + x] = field[y * w + x] * noise::ease(part) / noise::UNIT;
        }
    }
}

/// The field cut into ground heights, `0..=Tile::MAX_GROUND_HEIGHT`, against
/// the range the noise has **by construction** and never against the range
/// this particular seed happened to reach.
///
/// Stretching each map to fill the field would make a gentle seed come out as
/// jagged as a violent one, and would make the sea level mean a different
/// height on every map — which is the same failure as a threshold that cannot
/// be compared between two runs.
fn to_steps(field: &[i64]) -> Vec<u8> {
    let steps = i64::from(Tile::MAX_GROUND_HEIGHT) + 1;
    field
        .iter()
        .map(|&v| {
            // No clamp, deliberately: the noise's own range is what keeps this
            // in the field, and a clamp here would quietly repair a broken
            // construction instead of failing the test that pins it.
            debug_assert!((0..noise::UNIT).contains(&v), "the noise left its range");
            (v * steps / noise::UNIT) as u8
        })
        .collect()
}

/// Water below the sea level; rock above the rock height, or where the step to
/// the steepest neighbour reaches the rock step; plain otherwise.
///
/// **The step to the steepest neighbour is not a slope.** A slope is the range
/// of ground heights across the tiles one building sits on, which is a
/// different quantity measured over a different set of tiles, and a second
/// meaning for a word that already has one is how a vocabulary stops being
/// worth learning.
fn classify(ground: &[u8], width: u16, height: u16) -> Vec<Terrain> {
    let (w, h) = (width as usize, height as usize);
    (0..ground.len())
        .map(|i| {
            if ground[i] < SEA_LEVEL {
                Terrain::Water
            } else if ground[i] > ROCK_HEIGHT || steepest_step(ground, w, h, i) >= ROCK_STEP {
                Terrain::Rock
            } else {
                Terrain::Plain
            }
        })
        .collect()
}

/// The largest difference in height between this tile and any of the four it
/// touches **that is also land**. Zero on a tile whose neighbours are all
/// level with it, and zero on an island of one tile.
///
/// The sea is left out, and that is not a detail: the ground goes on falling
/// under the water, so the step from a shore tile to the sea beside it is
/// large on almost every map — and counting it turned every coastline into
/// cliffs, ringing each map with rock. A beach is not a cliff. Nobody climbs
/// the drop into the sea, so it is not a step.
fn steepest_step(ground: &[u8], w: usize, h: usize, i: usize) -> u8 {
    let here = ground[i];
    neighbours(i, w, h)
        .filter(|&n| ground[n] >= SEA_LEVEL)
        .map(|n| here.abs_diff(ground[n]))
        .max()
        .unwrap_or(0)
}

/// The water drops to height zero.
///
/// **After** the ground has been classified, and that order is the whole
/// point: dropping the sea first would leave a large step between every shore
/// tile and the water beside it, and [`classify`] would read that step as a
/// cliff and turn the entire coastline to rock.
fn flatten_the_water(terrain: &[Terrain], ground: &mut [u8]) {
    for (t, g) in terrain.iter().zip(ground) {
        if *t == Terrain::Water {
            *g = 0;
        }
    }
}

/// Drowns every walkable tile outside the largest connected region.
///
/// It is the gate this tool stands or falls on, not a polish pass at the end.
/// The loader refuses a map whose walkable tiles come in more than one piece,
/// because until bridges exist a river across the map silently strands half of
/// it — so a generator that can emit such a map is a generator that emits files
/// the game will not load.
///
/// Largest by tile count, ties broken by the lowest tile index in the region.
/// The regions are found in index order, so keeping the first of equal size
/// *is* that tie-break; an unstated one is a source of non-determinism, and
/// writing it down is what means nobody has to wonder later.
///
/// The fill lives here and not on `Grid`. `Grid::walkable_regions` answers with
/// the regions' sizes and not with which tile is in which, and widening a
/// signature in the core for a tool's convenience is exactly how this phase
/// would lose the right to say it changed nothing inside the simulation.
fn keep_one_walkable_region(
    terrain: &mut [Terrain],
    ground: &mut [u8],
    width: u16,
    height: u16,
) -> Repair {
    let (w, h) = (width as usize, height as usize);
    const NONE: u32 = u32::MAX;
    let mut region = vec![NONE; terrain.len()];
    let mut sizes: Vec<usize> = Vec::new();

    for root in 0..terrain.len() {
        if region[root] != NONE || !terrain[root].is_walkable() {
            continue;
        }
        let id = sizes.len() as u32;
        let mut size = 0usize;
        let mut queue = vec![root];
        region[root] = id;
        while let Some(t) = queue.pop() {
            size += 1;
            for n in neighbours(t, w, h) {
                if region[n] == NONE && terrain[n].is_walkable() {
                    region[n] = id;
                    queue.push(n);
                }
            }
        }
        sizes.push(size);
    }

    // Strictly greater, so the earliest region of an equal size keeps it — and
    // the earliest is the one holding the lowest tile index, because that is
    // the order they were found in.
    let mut best: Option<(usize, u32)> = None;
    for (id, &size) in sizes.iter().enumerate() {
        if best.is_none_or(|(largest, _)| size > largest) {
            best = Some((size, id as u32));
        }
    }

    let mut tiles_drowned = 0;
    if let Some((_, keep)) = best {
        for i in 0..terrain.len() {
            if region[i] != NONE && region[i] != keep {
                terrain[i] = Terrain::Water;
                ground[i] = 0;
                tiles_drowned += 1;
            }
        }
    }
    Repair {
        regions_before: sizes.len(),
        tiles_drowned,
    }
}

/// The four tiles a tile touches, the same rule `Grid::neighbors4` follows.
fn neighbours(i: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % w, i / w);
    [
        (x > 0).then(|| i - 1),
        (y > 0).then(|| i - w),
        (x + 1 < w).then_some(i + 1),
        (y + 1 < h).then_some(i + w),
    ]
    .into_iter()
    .flatten()
}
