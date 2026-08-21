//! Phase 04 — the table-driven behaviour of `step` and of the commands.
//!
//! The global invariants (no panic, no overlap, a consistent treasury) live in
//! `invariants.rs`.

mod common;

use common::*;
use sim_core::{Coins, Command, CommandError, Event, Occupant, Terrain, TileOccupant};

#[test]
fn an_empty_tick_only_advances_the_tick() {
    // The second world is never played: `World` is not `Clone`, so the
    // game as it stood before the tick is a world that has not taken it.
    let (mut w, before) = twins();

    let r = tick(&mut w, &[]);

    assert_eq!(w.tick(), before.tick().next());
    assert!(r.rejected.is_empty());
    assert!(r.events.is_empty());
    assert_eq!(w.grid(), before.grid());
    assert_eq!(w.economy(), before.economy());
    assert_eq!(w.building_count(), 0);
    assert_eq!(w.house_count(), 0);
    // The outcome is the same as it was in M0 and the reason is not. This
    // world has no houses, so no flow has an eligible resident and the
    // demographics draw nothing: *a tick with nothing to decide draws nothing*,
    // which is a live rule rather than a note about a milestone that is over.
    // Its sharper companion is `a_still_city_draws_a_fixed_number_of_values`.
    assert_eq!(
        w.rng(),
        before.rng(),
        "a tick with nothing to decide has to draw nothing"
    );
}

#[test]
fn a_farm_takes_four_tiles_and_costs_money() {
    let mut w = world();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(3, 3),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.building_count(), 1);
    assert_eq!(
        w.economy().treasury,
        Coins::new(STARTING_TREASURY - FARM_COST)
    );

    let origin = w.grid().index(pos(3, 3)).expect("on the map");
    let mut occupied = 0;
    for (x, y) in [(3, 3), (4, 3), (3, 4), (4, 4)] {
        let idx = w.grid().index(pos(x, y)).expect("on the map");
        let tile = w.grid().get(idx).expect("tile");
        assert_eq!(
            tile.occupant(),
            Some(TileOccupant {
                origin,
                is_house: false
            }),
            "tile ({x},{y}) must point at the farm's origin"
        );
        assert!(matches!(w.occupant(idx), Some(Occupant::Building(_))));
        occupied += 1;
    }
    assert_eq!(occupied, 4);

    // The tiles just outside the area stay free.
    let outside = w.grid().index(pos(5, 3)).expect("on the map");
    assert_eq!(w.occupant(outside), None);
}

/// Phase 16 — a farm spanning a slope pays `cost + flatten_cost_per_step` per
/// step of relief, and the treasury moves by exactly that much.
#[test]
fn a_sloped_farm_costs_more() {
    let mut w = world();
    for (x, y, h) in [(3, 3, 0), (4, 3, 0), (3, 4, 3), (4, 4, 3)] {
        w.set_ground_height(pos(x, y), h);
    }
    let before = w.economy().treasury;

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(3, 3),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(
        before.get() - w.economy().treasury.get(),
        FARM_COST + 3 * FLATTEN_COST_PER_STEP,
        "3 steps of slope, within max_build_slope"
    );
}

/// Phase 16 — a 1x1 building has one height and nothing to flatten: it never
/// pays a slope surcharge and can never be refused for one, however uneven
/// its surroundings are. Pinned so nobody "fixes" this later by measuring a
/// 1x1 against its neighbours instead of just its own tile.
#[test]
fn a_one_tile_building_never_pays_or_is_refused_for_slope() {
    let mut w = world();
    for (x, y, h) in [(4, 2, 0), (6, 2, 30), (5, 1, 30), (5, 3, 30)] {
        w.set_ground_height(pos(x, y), h);
    }
    let before = w.economy().treasury;

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: HOUSE,
            origin: pos(5, 2),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(before.get() - w.economy().treasury.get(), HOUSE_COST);
}

/// Phase 16 — a footprint steeper than `max_build_slope` is refused cleanly:
/// `TooSteep` carries the slope found, the treasury is untouched, and no tile
/// is mutated. The no-partial-mutation discipline, on the new failure path.
#[test]
fn too_steep_is_refused_without_mutating_anything() {
    let (mut w, mut still) = twins();
    for g in [&mut w, &mut still] {
        for (x, y, h) in [(3, 3, 0), (4, 3, 0), (3, 4, 4), (4, 4, 4)] {
            g.set_ground_height(pos(x, y), h);
        }
    }

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(3, 3),
        }],
    );
    tick(&mut still, &[]);

    assert_eq!(r.rejected.len(), 1);
    assert_eq!(
        r.rejected[0].1,
        CommandError::TooSteep {
            at: pos(3, 3),
            slope: 4
        }
    );
    assert_eq!(w.building_count(), 0);
    assert_eq!(
        w.economy(),
        still.economy(),
        "a rejected command costs nothing"
    );
    assert_eq!(
        w.grid(),
        still.grid(),
        "no partial mutation on a refused placement"
    );
    same_game(&w, &still).unwrap_or_else(|e| panic!("and nothing else moved either: {e}"));
}

#[test]
fn an_overlap_is_rejected_without_mutating_anything() {
    let (mut w, mut still) = twins();
    let first_farm = [Command::PlaceBuilding {
        kind: FARM,
        origin: pos(3, 3),
    }];
    // The same game in both, up to here.
    tick(&mut w, &first_farm);
    tick(&mut still, &first_farm);

    // The second farm touches a single tile of the first.
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(4, 4),
        }],
    );
    // The twin takes the tick with nothing in it, which is what the rejected
    // tick has to be indistinguishable from.
    tick(&mut still, &[]);

    assert_eq!(r.rejected.len(), 1);
    assert!(matches!(r.rejected[0].1, CommandError::TileOccupied { .. }));
    assert_eq!(w.building_count(), still.building_count());
    assert_eq!(
        w.economy(),
        still.economy(),
        "a rejected command costs nothing"
    );
    assert_eq!(
        w.grid(),
        still.grid(),
        "no partial occupation: validation comes before the mutations"
    );
    // The three named above are phase 04's statement of the rule and are worth
    // failing by name; this is the rest of the state, which only a second world
    // can be compared against.
    same_game(&w, &still).unwrap_or_else(|e| panic!("and nothing else moved either: {e}"));
}

#[test]
fn a_building_cannot_stick_out_past_the_edge() {
    let (mut w, mut still) = twins_of(8, 8);

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(7, 7),
        }],
    );
    tick(&mut still, &[]);

    assert_eq!(r.rejected.len(), 1);
    assert!(matches!(r.rejected[0].1, CommandError::OutsideMap(_)));
    assert_eq!(w.grid(), still.grid());
    assert_eq!(w.economy(), still.economy());
    same_game(&w, &still).unwrap_or_else(|e| panic!("and nothing else moved either: {e}"));
}

#[test]
fn an_empty_treasury_reports_the_right_numbers() {
    let mut w = world();
    // Drain the treasury by building houses for as long as there is money.
    let houses = STARTING_TREASURY / HOUSE_COST;
    let cmds: Vec<_> = (0..houses)
        .map(|i| Command::PlaceBuilding {
            kind: HOUSE,
            origin: pos((i % 32) as u8, (i / 32) as u8),
        })
        .collect();
    let r = tick(&mut w, &cmds);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.economy().treasury, Coins::ZERO);

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: WELL,
            origin: pos(20, 20),
        }],
    );
    assert_eq!(
        r.rejected[0].1,
        CommandError::NotEnoughMoney {
            needed: Coins::new(WELL_COST),
            available: Coins::ZERO,
        }
    );
}

#[test]
fn demolishing_frees_every_tile_and_removes_the_id() {
    let mut w = world();
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FARM,
            origin: pos(3, 3),
        }],
    );
    let id = w
        .buildings()
        .next()
        .map(|(id, _)| id)
        .expect("one building");

    // Demolishing from a tile that is not the origin must work all the same.
    let r = tick(&mut w, &[Command::Demolish { at: pos(4, 4) }]);

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.building_count(), 0);
    assert_eq!(w.building(id), None, "the id is no longer live");
    assert!(r.events.contains(&Event::BuildingRemoved { id }));
    for (x, y) in [(3, 3), (4, 3), (3, 4), (4, 4)] {
        let idx = w.grid().index(pos(x, y)).expect("on the map");
        assert_eq!(w.occupant(idx), None, "({x},{y}) must be free again");
    }
}

#[test]
fn demolishing_nothing_is_an_error() {
    let mut w = world();
    let r = tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert_eq!(r.rejected[0].1, CommandError::NothingToDemolish(pos(5, 5)));
}

#[test]
fn a_kind_of_building_that_does_not_exist() {
    let mut w = world();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: UNKNOWN_KIND,
            origin: pos(1, 1),
        }],
    );
    assert_eq!(
        r.rejected[0].1,
        CommandError::UnknownBuildingKind(UNKNOWN_KIND)
    );
    assert_eq!(w.economy().treasury, Coins::new(STARTING_TREASURY));
}

#[test]
fn a_road_costs_money_sets_the_flag_and_dirties_the_network() {
    let mut w = world();
    let r = tick(&mut w, &[Command::PlaceRoad { at: pos(2, 2) }]);

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(
        w.economy().treasury,
        Coins::new(STARTING_TREASURY - PLAIN_ROAD_COST)
    );
    let idx = w.grid().index(pos(2, 2)).expect("on the map");
    assert!(w.grid().get(idx).expect("tile").has_road());
    // Step 2 has already consumed the flag within the same tick: what stays
    // observable is that the rebuild happened.
    assert!(!w.dirty().roads);
    assert_eq!(w.roads().rebuilds(), 1);

    // Two roads on the same tile: the second is rejected.
    let r = tick(&mut w, &[Command::PlaceRoad { at: pos(2, 2) }]);
    assert!(matches!(r.rejected[0].1, CommandError::TileOccupied { .. }));
}

#[test]
fn nothing_gets_built_on_the_wrong_terrain() {
    let mut w = world();
    // A lake in the middle of the map.
    assert!(w.set_terrain(pos(6, 6), Terrain::Water));

    let r = tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: HOUSE,
                origin: pos(6, 6),
            },
            Command::PlaceRoad { at: pos(6, 6) },
        ],
    );

    assert_eq!(r.rejected.len(), 2);
    for (_, e) in &r.rejected {
        assert!(
            matches!(
                e,
                CommandError::WrongTerrain {
                    terrain: Terrain::Water,
                    ..
                }
            ),
            "{e:?}"
        );
    }
}

#[test]
fn a_road_crosses_rock_but_no_building_stands_on_it() {
    // The asymmetric terrain, and the only one: water refuses both questions,
    // so a test on water alone cannot tell the two apart. You cross a mountain,
    // you do not settle on it.
    let mut w = world();
    assert!(w.set_terrain(pos(6, 6), Terrain::Rock));
    let before = w.economy().treasury;

    let r = tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: HOUSE,
                origin: pos(6, 6),
            },
            Command::PlaceRoad { at: pos(6, 6) },
        ],
    );

    assert_eq!(r.rejected.len(), 1, "only the building is refused");
    assert!(
        matches!(
            r.rejected[0].1,
            CommandError::WrongTerrain {
                terrain: Terrain::Rock,
                ..
            }
        ),
        "{:?}",
        r.rejected[0].1
    );
    assert!(w.grid().at(pos(6, 6)).expect("on the map").has_road());

    // Charged at rock's price and not at plain's. Every recording is uniformly
    // plain, so this is the only thing in the tree that would catch a road
    // that always charged for plain ground.
    assert_eq!(
        before.checked_sub(w.economy().treasury),
        Some(Coins::new(ROCK_ROAD_COST)),
        "a road on rock costs what rock costs"
    );
}

#[test]
fn a_house_is_a_house_not_a_building() {
    let mut w = world();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: HOUSE,
            origin: pos(1, 1),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.house_count(), 1);
    assert_eq!(
        w.building_count(),
        0,
        "houses are not counted among the buildings (D5)"
    );
    assert_eq!(w.population(), u32::from(RESIDENTS_PER_HOUSE));
    let (_, h) = w.houses().next().expect("one house");
    assert_eq!(h.level, level(1));
    assert!(matches!(
        r.events[0],
        Event::HousePlaced { origin, .. } if origin == pos(1, 1)
    ));
}

/// Phase 11 — the difficulty's one knob, seen from the core.
///
/// It is the whole mechanic: at `easy` a house is born full, at `hard` empty.
/// A house at zero residents is a legitimate state and not a degenerate one —
/// migration (phase 15) is what fills it, and until then it stays empty, which
/// is the point of the profile.
#[test]
fn how_full_a_new_house_is_born_is_the_difficulty() {
    let cases = [(EASY, RESIDENTS_PER_HOUSE), (HARD, 0)];

    for (profile, expected) in cases {
        let mut w = world_at(32, 32, profile);
        let r = tick(
            &mut w,
            &[Command::PlaceBuilding {
                kind: HOUSE,
                origin: pos(1, 1),
            }],
        );

        assert!(r.rejected.is_empty(), "{profile}: {:?}", r.rejected);
        assert_eq!(w.house_count(), 1, "{profile}");
        assert_eq!(w.population(), u32::from(expected), "{profile}");
    }
}

#[test]
fn one_invalid_command_does_not_stop_the_others() {
    let mut w = world();
    let r = tick(
        &mut w,
        &[
            Command::PlaceRoad { at: pos(0, 0) },
            Command::Demolish { at: pos(31, 31) }, // nothing to demolish
            Command::PlaceRoad { at: pos(1, 0) },
        ],
    );

    assert_eq!(r.rejected.len(), 1);
    assert_eq!(r.rejected[0].0, 1, "the index of the discarded command");
    assert_eq!(r.accepted(3), 2);
    assert!(w.grid().at(pos(0, 0)).expect("tile").has_road());
    assert!(w.grid().at(pos(1, 0)).expect("tile").has_road());
}

#[test]
fn building_marks_the_provider_as_dirty() {
    let mut w = world();
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: WELL,
            origin: pos(5, 5),
        }],
    );
    let id = w.buildings().next().map(|(id, _)| id).expect("the well");
    // As with roads, step 3 consumes the list within the same tick: the
    // observable effect is that coverage was recomputed.
    assert!(w.dirty().coverage.is_empty());
    assert!(!w.dirty().coverage_invalidated);
    assert_eq!(w.coverage().recomputes(), 1);

    tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert!(
        !w.dirty().coverage.contains(&id),
        "a demolished provider does not stay in the dirty list"
    );
    assert_eq!(
        w.coverage().recomputes(),
        2,
        "demolishing even the last provider forces a recomputation"
    );
}
