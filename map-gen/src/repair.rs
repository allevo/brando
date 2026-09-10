//! Keeps the walkable ground in one connected piece.
//!
//! Nothing in the three passes before this one guarantees it: a ring of sea
//! sources, or an unlucky run of mountain claims, can sever the walkable
//! ground, and the loader refuses a severed map (`RULES.md`: "every tile a
//! building or a road could stand on is reachable from every other"). Run
//! against a real [`Grid`] rather than a hand-rolled adjacency function:
//! this crate already has a `Grid` in hand at every step for its geometry,
//! so a second adjacency implementation kept in sync with
//! [`Grid::neighbors4`] by convention alone would buy nothing.

use sim_core::{Grid, Terrain};

use crate::pipeline::Repair;

/// Drowns every walkable tile outside the largest connected region.
///
/// Largest by tile count, ties broken by the lowest tile index in the
/// region: the regions are found in index order, so keeping the first of
/// equal size *is* that tie-break, and writing it down is what means nobody
/// has to wonder later.
pub(crate) fn keep_one_walkable_region(
    terrain: &mut [Terrain],
    ground: &mut [u8],
    grid: &Grid,
) -> Repair {
    const NONE: u32 = u32::MAX;
    let mut region = vec![NONE; terrain.len()];
    let mut sizes: Vec<usize> = Vec::new();

    for root in grid.indices() {
        let r = usize::from(root.get());
        if region[r] != NONE || !terrain[r].is_walkable() {
            continue;
        }
        let id = sizes.len() as u32;
        let mut size = 0usize;
        let mut queue = vec![root];
        region[r] = id;
        while let Some(t) = queue.pop() {
            size += 1;
            for nb in grid.neighbors4(t) {
                let ni = usize::from(nb.get());
                if region[ni] == NONE && terrain[ni].is_walkable() {
                    region[ni] = id;
                    queue.push(nb);
                }
            }
        }
        sizes.push(size);
    }

    // Strictly greater, so the earliest region of an equal size keeps it.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_leaves_exactly_one_walkable_region() {
        let g = Grid::new(5, 1, Terrain::Plain).expect("valid size");
        let mut terrain = vec![
            Terrain::Plain,
            Terrain::Plain,
            Terrain::Plain,
            Terrain::Water,
            Terrain::Plain,
        ];
        let mut ground = vec![10, 10, 10, 0, 10];
        let repair = keep_one_walkable_region(&mut terrain, &mut ground, &g);
        assert_eq!(repair.regions_before, 2);
        assert_eq!(repair.tiles_drowned, 1);

        let repaired = Grid::from_map(&sim_core::MapDef::new(
            "t".to_string(),
            5,
            1,
            terrain.clone(),
            ground.clone(),
        ));
        assert_eq!(repaired.walkable_regions().len(), 1);
        assert_eq!(
            terrain[4],
            Terrain::Water,
            "the smaller region gets drowned"
        );
        assert_eq!(
            terrain[0],
            Terrain::Plain,
            "the larger region survives untouched"
        );
    }

    #[test]
    fn repair_only_ever_drowns_and_never_creates_land() {
        let g = Grid::new(4, 1, Terrain::Plain).expect("valid size");
        let mut terrain = vec![
            Terrain::Plain,
            Terrain::Water,
            Terrain::Rock,
            Terrain::Plain,
        ];
        let before = terrain.clone();
        let mut ground = vec![10, 0, 12, 10];
        keep_one_walkable_region(&mut terrain, &mut ground, &g);
        for (b, a) in before.iter().zip(&terrain) {
            if *b == Terrain::Water {
                assert_eq!(*a, Terrain::Water, "water never becomes land");
            }
        }
    }
}
