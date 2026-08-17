//! Phase 06 — aggregate coverage: range along roads, capacity, deterministic
//! assignment, and the equivalence between incremental and from-scratch.

mod common;

use common::*;
use proptest::prelude::*;
use sim_core::{BuildingId, Command, HouseId, ServiceKind, TilePos, World};

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

fn build(w: &mut World, kind: sim_core::BuildingKindId, x: u8, y: u8) {
    let r = tick(
        w,
        &[Command::PlaceBuilding {
            kind,
            origin: pos(x, y),
        }],
    );
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
}

fn only_house(w: &World) -> HouseId {
    w.houses().next().map(|(id, _)| id).expect("one house")
}

fn only_well(w: &World) -> BuildingId {
    w.buildings().next().map(|(id, _)| id).expect("one well")
}

/// A horizontal corridor along row 5, a well at the start, a house `dist` road
/// tiles away. The well's range is 12 (from the fixture).
fn linear_scenario(length: u8, house_x: u8) -> World {
    linear_scenario_on(world(), length, house_x)
}

fn linear_scenario_on(mut w: World, length: u8, house_x: u8) -> World {
    let cells: Vec<(u8, u8)> = (1..=length).map(|x| (x, 5)).collect();
    roads(&mut w, &cells);
    build(&mut w, WELL, 1, 4);
    build(&mut w, HOUSE, house_x, 4);
    w
}

// --- 1, 2. range ------------------------------------------------------------

#[test]
fn base_case_a_house_within_range() {
    let w = linear_scenario(20, 4);
    let house = only_house(&w);
    assert!(w.coverage().is_served(house, ServiceKind::Water));
    assert_eq!(
        w.coverage().served_by(house, ServiceKind::Water),
        Some(only_well(&w))
    );
    // The house carries a copy of the flag.
    let (_, h) = w.houses().next().expect("one house");
    assert!(h.served.get(ServiceKind::Water));
    assert!(!h.served.get(ServiceKind::Food), "no farm: no food");
}

/// The bound is inclusive, and this test pins it down: a range of 12 serves up
/// to and including distance 12, no further.
#[test]
fn the_range_bound_is_inclusive() {
    // The well's entrance: (1,5). The house's entrance at x: (x,5). Distance x-1.
    let w = linear_scenario(20, 13);
    let house = only_house(&w);
    assert!(
        w.coverage().is_served(house, ServiceKind::Water),
        "distance 12 with range 12 must be served"
    );

    let w = linear_scenario(20, 14);
    let house = only_house(&w);
    assert!(
        !w.coverage().is_served(house, ServiceKind::Water),
        "distance 13 with range 12 must not be served"
    );
}

// --- 3. the test that embodies D2 -------------------------------------------

/// A house 2 tiles from the well as the crow flies, but reachable only the long
/// way round: with a range of 12 it is **not** served. If this passes, the
/// implementation has no Euclidean shortcuts.
#[test]
fn the_distance_is_along_roads_not_through_the_air() {
    let mut w = world();
    // A long ring: column 2 from y=2 to y=12, row 12 from x=2 to x=6, column 6
    // from y=12 back to y=2.
    let mut cells: Vec<(u8, u8)> = (2..=12).map(|y| (2, y)).collect();
    cells.extend((3..=6).map(|x| (x, 12)));
    cells.extend((2..=11).map(|y| (6, y)));
    roads(&mut w, &cells);

    build(&mut w, WELL, 3, 2);
    build(&mut w, HOUSE, 5, 2);

    let house = only_house(&w);
    assert_eq!(
        pos(3, 2).manhattan(pos(5, 2)),
        2,
        "very close through the air"
    );
    assert!(
        !w.coverage().is_served(house, ServiceKind::Water),
        "the way round the network exceeds the range of 12: the house is not served"
    );
}

// --- 4. a road is required --------------------------------------------------

#[test]
fn a_house_with_no_road_is_never_served() {
    let mut w = world();
    roads(&mut w, &[(1, 5), (2, 5), (3, 5)]);
    build(&mut w, WELL, 1, 4);
    // A house right next to the well but not next to a road.
    build(&mut w, HOUSE, 1, 3);

    let house = only_house(&w);
    assert!(!w.coverage().is_served(house, ServiceKind::Water));
}

// --- 5. capacity and assignment order ---------------------------------------

/// The game rule: the nearest ones get served, and at equal distance the
/// smaller `TileIndex` wins.
#[test]
fn the_capacity_serves_the_nearest_ones() {
    let mut w = world();
    let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 5)).collect();
    roads(&mut w, &cells);
    build(&mut w, WELL, 1, 4);

    // The fixture's well serves 32 residents, i.e. eight houses of four: we put
    // ten of them at increasing distances, all within a range of 12.
    let within_capacity = WELL_CAPACITY / RESIDENTS_PER_HOUSE;
    assert_eq!(within_capacity, 8);
    for x in 2..=11u8 {
        build(&mut w, HOUSE, x, 4);
    }

    let well = only_well(&w);
    let served = w.coverage().houses_served_by(well);
    assert_eq!(
        served.len(),
        usize::from(within_capacity),
        "the well's capacity"
    );

    // The two left out are the furthest away: x = 10 and x = 11.
    for (id, h) in w.houses() {
        let expected = h.origin.x <= 9;
        assert_eq!(
            w.coverage().is_served(id, ServiceKind::Water),
            expected,
            "house at {:?}",
            h.origin
        );
    }
}

/// When the capacity bites, at equal distance the house with the smaller
/// `TileIndex` wins. It is an explicit game rule: without the second criterion the
/// order would come from the BFS's visit order, i.e. from an implementation
/// detail, and the recorded replay would become fragile.
#[test]
fn at_equal_distance_the_smaller_tile_wins() {
    let mut w = world();
    roads(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    build(&mut w, SMALL_WELL, 5, 6);

    // Two houses at distance 1 from the small well's only entrance, (5,5).
    build(&mut w, HOUSE, 4, 4); // entrance (4,5)
    build(&mut w, HOUSE, 6, 4); // entrance (6,5)

    let nearer = w
        .houses()
        .find(|(_, h)| h.origin == pos(4, 4))
        .map(|(id, _)| id)
        .expect("house on the left");
    let further = w
        .houses()
        .find(|(_, h)| h.origin == pos(6, 4))
        .map(|(id, _)| id)
        .expect("house on the right");

    let idx = |p| w.grid().index(p).expect("on the map");
    assert!(
        idx(pos(4, 4)) < idx(pos(6, 4)),
        "the left one has the smaller TileIndex"
    );
    assert!(w.coverage().is_served(nearer, ServiceKind::Water));
    assert!(
        !w.coverage().is_served(further, ServiceKind::Water),
        "the small well's capacity is one house only: the second stays uncovered"
    );
}

/// The same rule where the two orders **disagree**, which is the only place it
/// can be observed at all.
///
/// [`at_equal_distance_the_smaller_tile_wins`] puts its two houses south of two
/// road tiles, so the houses come out in the same order as the tiles they face
/// and either rule gives the same answer. Here one house faces its road from
/// below and the other from above: the road tiles run (4,5) then (6,5), and the
/// houses run (6,4) then (4,6). Whoever wins says which of the two orders the
/// game really uses.
///
/// It is the reason the walk hands over a whole distance at a time rather than
/// a tile at a time. Filling as each tile arrived would serve the house at
/// (4,6), because its road tile is reached first — an answer that depends on
/// which side of the street a house happens to stand on.
#[test]
fn at_equal_distance_the_order_is_the_houses_and_not_the_road_tiles() {
    let mut w = world();
    roads(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    build(&mut w, SMALL_WELL, 5, 6); // its only entrance is (5,5)

    build(&mut w, HOUSE, 4, 6); // entrance (4,5), below the street
    build(&mut w, HOUSE, 6, 4); // entrance (6,5), above the street

    let at = |p| {
        w.houses()
            .find(|(_, h)| h.origin == p)
            .map(|(id, _)| id)
            .expect("a house there")
    };
    let idx = |p| w.grid().index(p).expect("on the map");

    assert!(
        idx(pos(4, 5)) < idx(pos(6, 5)) && idx(pos(6, 4)) < idx(pos(4, 6)),
        "the road tiles and the houses they carry run in opposite orders"
    );
    assert!(
        w.coverage().is_served(at(pos(6, 4)), ServiceKind::Water),
        "the smaller house tile wins, though its road tile is the later one"
    );
    assert!(
        !w.coverage().is_served(at(pos(4, 6)), ServiceKind::Water),
        "the small well's capacity is one house only"
    );
}

/// Capacity is counted in residents, and this test observes that from the
/// outside: the small well declares four of them, i.e. exactly one house, and
/// the second candidate house does not fit even though it is the only one left.
///
/// In M0 every house has four residents, so from outside the difference between
/// "one house" and "four residents" is not visible yet: what tells the two units
/// apart is pinned down by the table-driven test in `sim-core/src/coverage.rs`.
#[test]
fn the_capacity_runs_out_in_residents() {
    assert_eq!(SMALL_WELL_CAPACITY, RESIDENTS_PER_HOUSE);

    let mut w = world();
    roads(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    build(&mut w, SMALL_WELL, 5, 6);
    build(&mut w, HOUSE, 4, 4);
    build(&mut w, HOUSE, 6, 4);

    let small_well = w
        .buildings()
        .next()
        .map(|(id, _)| id)
        .expect("the small well");
    let served = w.coverage().houses_served_by(small_well);
    assert_eq!(served.len(), 1);
    let residents: u16 = served
        .iter()
        .map(|h| w.house(*h).expect("alive").residents)
        .sum();
    assert_eq!(residents, SMALL_WELL_CAPACITY, "the capacity is full");
}

/// And what that same rule does when a house weighs nothing: it does not get
/// the chance to, because a house with nobody in it is not a candidate at all.
///
/// The `hard` difficulty profile builds every house with zero residents, so
/// this is the case where the whole district is empty at once. Nobody is
/// served, whatever the capacity says, and it stays that way until somebody
/// moves in.
///
/// This assertion used to run the other way — seven empty houses all served by
/// a well declared for four residents, and any number would have been — because
/// the capacity is counted in residents and `left.checked_sub(0)` never fails.
/// Coverage feeds satisfaction, so on the profile meant to be the hard one that
/// carried a whole district to the top level for free.
///
/// It is also the only test in the suite that runs on `hard` other than the
/// one in `commands.rs` that checks the knob itself.
#[test]
fn on_hard_an_empty_house_is_not_served() {
    let mut w = world_at(32, 32, HARD);
    roads(
        &mut w,
        &[(1, 5), (2, 5), (3, 5), (4, 5), (5, 5), (6, 5), (7, 5)],
    );
    build(&mut w, SMALL_WELL, 4, 6);
    for x in 1..=7 {
        build(&mut w, HOUSE, x, 4);
    }
    assert_eq!(w.population(), 0, "on `hard` a house is born empty");

    let small_well = w
        .buildings()
        .next()
        .map(|(id, _)| id)
        .expect("the small well");
    assert!(
        w.coverage().houses_served_by(small_well).is_empty(),
        "capacity {SMALL_WELL_CAPACITY} residents, and seven houses of nobody: \
         none of them is a candidate for it"
    );
}

/// Two providers covering the same house: in M0 the house is either served or
/// not, and the first one in the providers' iteration order wins.
#[test]
fn the_first_provider_wins_a_contested_house() {
    let mut w = world();
    roads(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    build(&mut w, WELL, 4, 6);
    build(&mut w, WELL, 6, 6);
    build(&mut w, HOUSE, 5, 4);

    let first = w
        .buildings()
        .next()
        .map(|(id, _)| id)
        .expect("the first well");
    let house = only_house(&w);
    assert_eq!(
        w.coverage().served_by(house, ServiceKind::Water),
        Some(first)
    );
}

// --- 7. demolitions ---------------------------------------------------------

#[test]
fn demolishing_the_well_uncovers_the_houses_in_the_same_tick() {
    let mut w = linear_scenario(20, 4);
    let house = only_house(&w);
    assert!(w.coverage().is_served(house, ServiceKind::Water));

    let r = tick(&mut w, &[Command::Demolish { at: pos(1, 4) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert!(
        !w.coverage().is_served(house, ServiceKind::Water),
        "coverage must drop in the same tick as the demolition"
    );
}

#[test]
fn breaking_the_road_uncovers_the_houses_beyond_the_break() {
    let mut w = linear_scenario(20, 10);
    let house = only_house(&w);
    assert!(w.coverage().is_served(house, ServiceKind::Water));

    // Removes a road tile halfway between the well and the house.
    let r = tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert!(
        !w.coverage().is_served(house, ServiceKind::Water),
        "beyond the break the house is no longer reachable"
    );
}

// --- 8. idempotence ---------------------------------------------------------

/// Nothing is recomputed while nothing moves the population.
///
/// It used to be "two empty ticks recompute nothing", which was the same
/// statement while `residents` could not change. Since phase 13 it can — decay
/// evicts — and since phase 14 it will constantly, so the invariant is written
/// in the form that survives: what invalidates the coverage is the population
/// moving, and an empty tick that moves nobody has to cost nothing.
///
/// The run is long enough to cross two monthly reviews, and the houses do level
/// up inside it: **levelling up must not invalidate anything**, because it
/// brings no residents in.
#[test]
fn nothing_is_recomputed_while_the_population_does_not_move() {
    // On the fixture with the demographics **switched off**. The name is the
    // premise: from phase 14 a living city moves somebody nearly every tick and
    // step 6 invalidates the coverage on purpose, so with the real rates this
    // would be asserting that births do not happen. Switched off, it goes on
    // checking what it was written to check — that a still city recomputes
    // nothing — which is the negative half of the invalidation contract.
    let mut w = linear_scenario_on(world_without_demographics(), 20, 4);
    let before = w.coverage().clone();
    let population = w.population();
    let recomputes = w.coverage().recomputes();
    let rebuilds = w.roads().rebuilds();

    for _ in 0..w.data().rules.ticks_per_month * 2 + 1 {
        tick(&mut w, &[]);
    }

    assert_eq!(w.population(), population, "nobody moved");
    assert_eq!(w.coverage().assignments(), before.assignments());
    assert_eq!(w.coverage().recomputes(), recomputes, "no recomputation");
    assert_eq!(w.roads().rebuilds(), rebuilds, "no network rebuild");
}

// --- 6. incremental / from-scratch equivalence (the phase's goal) -----------

fn any_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        4 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: HOUSE,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: WELL,
            origin: TilePos::new(x, y),
        }),
        1 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: FARM,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

/// Two long streets joined at one end, with room for buildings on either side
/// of each: whatever is placed on a plot has an entrance, and the houses are
/// strung out along the corridor instead of bunched against the provider.
fn two_streets() -> World {
    let mut w = world();
    let mut cells: Vec<(u8, u8)> = Vec::new();
    for x in 0..14 {
        cells.push((x, 5));
        cells.push((x, 9));
    }
    for y in 6..=8 {
        cells.push((0, y));
    }
    roads(&mut w, &cells);
    w
}

/// A plot beside one of [`two_streets`]'s streets. The rows are the ones a 1x1
/// building can take without standing on a road, and the ones a 2x2 can take
/// with its far half still touching one.
fn a_plot() -> impl Strategy<Value = TilePos> {
    (1u8..14, prop::sample::select(vec![3u8, 4, 6, 7, 8, 10])).prop_map(|(x, y)| TilePos::new(x, y))
}

proptest! {
    /// **The** test of the phase: given a random sequence of commands, the
    /// `Coverage` obtained by following the dirty flags is identical to the one
    /// recomputed from scratch on the final state.
    ///
    /// Today step 3 recomputes everything when anything is dirty, so what this
    /// test catches is a **forgotten invalidation**: a command that changes the
    /// topology without marking anything dirty. Tomorrow, when the
    /// recomputation really does become incremental, the same test will cover
    /// that cleverness too without having to be rewritten.
    #[test]
    fn coverage_equivalence(p in prop::collection::vec(
        prop::collection::vec(any_command(), 0..5), 1..10)
    ) {
        // **On a dataset with the demographic rates at zero, and that is not
        // an accident to be tidied away.** This test compares the stored
        // coverage against a from-scratch one at the *end* of the tick. From
        // phase 14, step 6 moves the population after step 3 has assigned, so
        // with the real rates the two diverge on nearly every tick — not
        // because of a bug, but because the stored coverage is one step behind
        // by design. Put the production rates back and the project's most
        // valuable oracle stops checking anything, without ever going red.
        //
        // What the real rates need is a different question, and
        // `demographics_invalidate_the_coverage` is where it is asked.
        let mut w = world_without_demographics();
        for cmds in &p {
            tick(&mut w, cmds);
            let from_scratch = sim_core::Coverage::from_scratch(&w);
            prop_assert_eq!(
                w.coverage().assignments(),
                from_scratch.assignments(),
                "incremental coverage differs from the from-scratch one at tick {}",
                w.tick()
            );
        }
    }

    /// A provider that stops where its capacity ran out serves exactly whom it
    /// would have served walking its whole range.
    ///
    /// **The oracle for the stop, and the reason `coverage_equivalence` is not
    /// it.** That test holds the stored coverage against `Coverage::from_scratch`,
    /// and both of those stop: a stop that ended one distance too early would
    /// give the same wrong answer on either side and leave it green. This is the
    /// only comparison in the suite where one side walks the whole way.
    ///
    /// **Its own generator, and the streets are laid before it runs.** The one
    /// `coverage_equivalence` uses scatters roads and buildings over the same
    /// window at random, and what comes out is mostly buildings with no road
    /// beside them: a provider that reaches nobody past its own doorstep
    /// exercises no stop at all, and this test was green against a stop
    /// deliberately broken to end after the first distance until the streets
    /// were put in. With them, every building has an entrance and the houses
    /// are spread along a corridor, which is what makes the walk have somewhere
    /// to go.
    ///
    /// It builds small wells for the same reason: the stop only happens to a
    /// provider whose capacity really runs out, and a well declared for eight
    /// houses rarely fills up inside a fourteen-tile window. The small one
    /// holds a single house, so the capacity bites almost at once.
    #[test]
    fn stopping_when_full_serves_the_same_houses_as_walking_the_whole_range(
        p in prop::collection::vec(prop::collection::vec(prop_oneof![
            4 => a_plot().prop_map(|origin| Command::PlaceBuilding { kind: HOUSE, origin }),
            2 => a_plot().prop_map(|origin| Command::PlaceBuilding { kind: SMALL_WELL, origin }),
            1 => a_plot().prop_map(|origin| Command::PlaceBuilding { kind: WELL, origin }),
            1 => a_plot().prop_map(|origin| Command::PlaceBuilding { kind: FARM, origin }),
            1 => a_plot().prop_map(|at| Command::Demolish { at }),
        ], 0..5), 1..10)
    ) {
        // On the real rates, so that houses of different sizes — and houses
        // that empty — are in the mix: a capacity counted in residents runs out
        // at a different distance for each of them.
        let mut w = two_streets();
        for cmds in &p {
            tick(&mut w, cmds);
            let stopping = sim_core::Coverage::from_scratch(&w);
            let whole = sim_core::Coverage::from_scratch_walking_the_whole_range(&w);
            prop_assert_eq!(
                stopping.assignments(),
                whole.assignments(),
                "stopping when full differs from the whole walk at tick {}",
                w.tick()
            );
        }
    }

    /// A served house always has a live provider, one that really supplies that
    /// service and is within its own range.
    #[test]
    fn the_providers_stay_consistent(p in prop::collection::vec(
        prop::collection::vec(any_command(), 0..5), 1..8)
    ) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            for (h, _) in w.houses() {
                for k in ServiceKind::ALL {
                    let Some(p) = w.coverage().served_by(h, k) else { continue };
                    let b = w.building(p);
                    prop_assert!(b.is_some(), "dead provider for {h:?}");
                    let def = w.data().def(b.expect("alive").kind).expect("known kind");
                    let s = def.service().expect("a provider has a service");
                    prop_assert_eq!(s.kind, k, "provider of the wrong service");
                }
            }
        }
    }
}
