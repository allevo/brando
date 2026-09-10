//! Step 3: height becomes terrain.
//!
//! Two thresholds, and no others: below [`SEA_LEVEL`] is water, and among
//! the land, a tile whose step to its steepest land neighbour reaches
//! [`ROCK_STEP`] is rock, otherwise plain. This crate adds no third,
//! absolute "always rock above height N" rule the way `map-gen` has — the
//! specification this crate implements names exactly two thresholds, and a
//! third would be scope nobody asked for.

use sim_core::{Grid, Terrain, TileIndex};

use crate::pipeline::SEA_LEVEL;

/// A tile whose step to its steepest land neighbour reaches this is rock,
/// however flat the rest of the map around it: "slopes lower than 2 are
/// good ... with higher slopes the tiles are considered rock" is this
/// crate's own specification, and this is its exact boundary — 2 or more is
/// rock, not merely "more than 2".
const ROCK_STEP: u8 = 2;

pub(crate) fn run(height: &[u8], grid: &Grid) -> Vec<Terrain> {
    (0..height.len())
        .map(|i| {
            let idx = TileIndex::new(i as u16);
            if height[i] < SEA_LEVEL {
                Terrain::Water
            } else if steepest_step(height, grid, idx) >= ROCK_STEP {
                Terrain::Rock
            } else {
                Terrain::Plain
            }
        })
        .collect()
}

/// The largest height difference to a neighbour that is also land (height
/// at least [`SEA_LEVEL`]) — the quantity `map-gen` calls "the step to the
/// steepest neighbour" (`GLOSSARY.md`'s `slope` entry reserves that word
/// for a different quantity over a different set of tiles and never this
/// one). Sub-sea neighbours are excluded for the same reason `map-gen`
/// excludes them: the ground keeps falling under the water, so the drop
/// from a shore tile to the sea beside it is large on almost every map, and
/// counting it rings every coastline with rock. A beach is not a cliff.
fn steepest_step(height: &[u8], grid: &Grid, idx: TileIndex) -> u8 {
    let here = height[usize::from(idx.get())];
    grid.neighbors4(idx)
        .filter(|&n| height[usize::from(n.get())] >= SEA_LEVEL)
        .map(|n| here.abs_diff(height[usize::from(n.get())]))
        .max()
        .unwrap_or(0)
}

/// The water drops to height zero, **after** classification — the same
/// order `map-gen` uses and for the same reason: dropping it first would
/// put a large step between every shore tile and the water beside it, and
/// the pass above would read that step as a cliff.
pub(crate) fn flatten_the_water(terrain: &[Terrain], height: &mut [u8]) {
    for (t, h) in terrain.iter().zip(height) {
        if *t == Terrain::Water {
            *h = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use sim_core::{Grid, Terrain};

    use super::*;

    fn grid(w: u16, h: u16) -> Grid {
        Grid::new(w, h, Terrain::Plain).expect("valid size")
    }

    #[test]
    fn heights_below_eight_are_water_eight_and_above_are_land() {
        let g = grid(2, 1);
        let terrain = run(&[7, 8], &g);
        assert_eq!(terrain, vec![Terrain::Water, Terrain::Plain]);
    }

    #[test]
    fn a_steep_pair_of_land_neighbours_is_rock_a_gentle_one_is_plain() {
        let g = grid(2, 1);
        let steep = run(&[8, 30], &g);
        assert_eq!(steep, vec![Terrain::Rock, Terrain::Rock]);

        let gentle = run(&[8, 9], &g);
        assert_eq!(gentle, vec![Terrain::Plain, Terrain::Plain]);
    }

    #[test]
    fn steepest_step_ignores_underwater_neighbours() {
        let g = grid(2, 1);
        // A shore tile beside deep water: a huge drop, but into the sea.
        let idx = TileIndex::new(0);
        assert_eq!(steepest_step(&[8, 0], &g, idx), 0);
    }

    #[test]
    fn flatten_the_water_only_ever_touches_water_tiles() {
        let terrain = vec![Terrain::Water, Terrain::Plain, Terrain::Rock];
        let mut height = vec![5, 12, 20];
        flatten_the_water(&terrain, &mut height);
        assert_eq!(height, vec![0, 12, 20]);
    }
}
