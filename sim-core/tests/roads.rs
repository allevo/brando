//! Phase 05 — connected components, dirty flag and walked distance.

mod common;

use common::*;
use proptest::prelude::*;
use sim_core::{Command, TilePos, World};

/// Builds the given roads, in a single tick.
fn roads(w: &mut World, cells: &[(u8, u8)]) {
    let cmds: Vec<_> = cells
        .iter()
        .map(|(x, y)| Command::PlaceRoad {
            at: TilePos::new(*x, *y),
        })
        .collect();
    let r = tick(w, &cmds);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
}

fn comp(w: &World, x: u8, y: u8) -> Option<sim_core::ComponentId> {
    w.roads()
        .component(w.grid().index(pos(x, y)).expect("on the map"))
}

// --- 1. components ----------------------------------------------------------

#[test]
fn two_separate_groups_then_joined_then_separate_again() {
    let mut w = world();
    roads(&mut w, &[(1, 1), (2, 1), (4, 1), (5, 1)]);
    assert_eq!(w.roads().component_count(), 2);
    assert_ne!(comp(&w, 1, 1), comp(&w, 4, 1));
    assert_eq!(comp(&w, 1, 1), comp(&w, 2, 1));

    // The tile that joins them.
    roads(&mut w, &[(3, 1)]);
    assert_eq!(w.roads().component_count(), 1);
    assert_eq!(comp(&w, 1, 1), comp(&w, 5, 1));

    // And removing it breaks them apart again.
    let r = tick(&mut w, &[Command::Demolish { at: pos(3, 1) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.roads().component_count(), 2);
    assert_ne!(comp(&w, 1, 1), comp(&w, 4, 1));
}

/// A component's id is the smallest `TileIndex` of its tiles, not a counter:
/// that is what makes the labelling independent of the history.
#[test]
fn the_component_id_is_the_smallest_tile() {
    let mut w = world();
    roads(&mut w, &[(5, 5), (5, 4), (5, 6)]);
    let expected = w.grid().index(pos(5, 4)).expect("on the map");
    for (x, y) in [(5, 4), (5, 5), (5, 6)] {
        assert_eq!(
            comp(&w, x, y).map(sim_core::ComponentId::tile),
            Some(expected)
        );
    }
}

// --- 2. adjacency -----------------------------------------------------------

#[test]
fn a_diagonal_does_not_connect() {
    let mut w = world();
    roads(&mut w, &[(1, 1), (2, 2)]);
    assert_eq!(w.roads().component_count(), 2);
}

/// The phase 01 bug coming back one level up: without checks, the last tile of
/// a row is a "neighbour" of the first tile of the next one.
#[test]
fn no_row_wraparound() {
    let mut w = world_of(8, 8);
    roads(&mut w, &[(7, 0), (0, 1)]);
    assert_eq!(w.roads().component_count(), 2);
    let a = w.grid().index(pos(7, 0)).expect("on the map");
    let b = w.grid().index(pos(0, 1)).expect("on the map");
    assert_eq!(b.get(), a.get() + 1, "they are adjacent as indices...");
    assert!(!w.roads().connected(a, b), "...but not as roads");
}

// --- 4. dirty flag ----------------------------------------------------------

#[test]
fn the_network_does_not_rebuild_without_commands() {
    let mut w = world();
    roads(&mut w, &[(1, 1)]);
    let after_first_road = w.roads().rebuilds();
    assert_eq!(after_first_road, 1);

    for _ in 0..10 {
        tick(&mut w, &[]);
    }
    assert_eq!(
        w.roads().rebuilds(),
        after_first_road,
        "ten empty ticks must rebuild nothing"
    );

    // Building a building does not touch the network either.
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: WELL,
            origin: pos(5, 5),
        }],
    );
    assert_eq!(w.roads().rebuilds(), after_first_road);
}

/// A regression against the case where someone marks dirty inside the loop:
/// N roads in one tick must cost **one** rebuild, not N.
#[test]
fn many_roads_in_one_tick_rebuild_only_once() {
    let mut w = world();
    let before = w.roads().rebuilds();
    roads(&mut w, &[(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)]);
    assert_eq!(w.roads().rebuilds(), before + 1);
}

// --- 5. walked distance -----------------------------------------------------

/// Builds a horizontal corridor with two buildings facing its two ends.
fn corridor(length: u8) -> (World, sim_core::BuildingId, sim_core::BuildingId) {
    let mut w = world();
    let cells: Vec<(u8, u8)> = (0..length).map(|i| (i + 1, 5)).collect();
    roads(&mut w, &cells);
    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(1, 4),
            },
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(length, 4),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    (w, ids[0], ids[1])
}

#[test]
fn distance_along_a_straight_corridor() {
    let (w, a, b) = corridor(10);
    // Entrances at the ends of a 10-tile corridor: 9 steps.
    assert_eq!(w.road_distance(a, b, 100), Some(9));
    assert_eq!(w.road_distance(a, a, 100), Some(0));
}

#[test]
fn beyond_max_is_none() {
    let (w, a, b) = corridor(10);
    assert_eq!(w.road_distance(a, b, 9), Some(9), "the bound is inclusive");
    assert_eq!(w.road_distance(a, b, 8), None);
}

/// The test that embodies D2: two buildings very close as the crow flies but
/// reachable only the long way round. If this passes, the implementation has no
/// Euclidean shortcuts.
#[test]
fn the_distance_is_walked_not_euclidean() {
    let mut w = world();
    // A U: down column 2, right along row 10, up column 6.
    let mut cells: Vec<(u8, u8)> = (2..=10).map(|y| (2, y)).collect();
    cells.extend((3..=6).map(|x| (x, 10)));
    cells.extend((2..=9).map(|y| (6, y)));
    roads(&mut w, &cells);

    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(3, 2),
            },
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(5, 2),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    let (a, b) = (ids[0], ids[1]);

    // As the crow flies they are 2 tiles apart.
    assert_eq!(pos(3, 2).manhattan(pos(5, 2)), 2);
    // On the network, the whole way round the U.
    let d = w.road_distance(a, b, 100).expect("connected");
    assert!(d >= 16, "expected a long walked distance, found {d}");
    assert_eq!(
        w.road_distance(a, b, 8),
        None,
        "the short range is not enough"
    );
}

#[test]
fn buildings_not_connected_or_with_no_road() {
    let mut w = world();
    roads(&mut w, &[(1, 1), (10, 10)]);
    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(1, 2),
            },
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(10, 11),
            },
            // This one touches no road at all.
            Command::PlaceBuilding {
                kind: WELL,
                origin: pos(20, 20),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    assert_eq!(
        w.road_distance(ids[0], ids[1], 200),
        None,
        "separate networks"
    );
    assert_eq!(
        w.road_distance(ids[0], ids[2], 200),
        None,
        "not hooked up at all"
    );
    assert!(w.building_entrances(ids[2]).is_empty());
}

/// It holds in M0 because the graph is undirected. The day one-way streets are
/// introduced this test will fail, and will force a conscious decision instead
/// of an oversight.
#[test]
fn the_distance_is_symmetric() {
    let (w, a, b) = corridor(10);
    assert_eq!(w.road_distance(a, b, 100), w.road_distance(b, a, 100));
}

// --- 3. independence from the order (the phase's goal) ----------------------

proptest! {
    /// Given a set of positions, two permutations of the corresponding
    /// `PlaceRoad` commands produce the **same** labelling. It is the reason a
    /// component id is the smallest tile and not a counter.
    #[test]
    fn the_labelling_does_not_depend_on_the_order(
        mut cells in prop::collection::vec((0u8..12, 0u8..12), 1..40),
        shuffle in prop::collection::vec(0usize..1000, 40),
    ) {
        cells.sort_unstable();
        cells.dedup();

        let mut a = world();
        roads(&mut a, &cells);

        // The same cells, shuffled deterministically.
        let mut others = cells.clone();
        let n = others.len();
        if n > 1 {
            for (i, k) in shuffle.iter().enumerate() {
                others.swap(i % n, k % n);
            }
        }
        let mut b = world();
        roads(&mut b, &others);

        for idx in a.grid().indices() {
            prop_assert_eq!(
                a.roads().component(idx),
                b.roads().component(idx),
                "different labelling at {:?}", idx
            );
        }
    }
}
