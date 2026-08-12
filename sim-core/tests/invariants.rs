//! The core's invariants (CLAUDE.md, Testing point 1).
//!
//! Each of these has found or will find a real bug; none of them is to be
//! silenced to make CI pass. This is where you read *what the project
//! guarantees*, regardless of how the code is organised — what a contributor,
//! or an LLM working on the repo, reads to learn what must not be broken.
//!
//! | Invariant | Where |
//! |---|---|
//! | No overlap, in both directions tile <-> building | here, `no_overlap_ever` |
//! | Every occupied tile resolves to a live id in the slotmap | here, `no_overlap_ever` |
//! | Population never negative and consistent with the houses that exist | here, `population_stays_consistent` |
//! | Treasury consistent: starting amount minus the sum of accepted costs | here, `the_treasury_adds_up` |
//! | Food conserved: produced = consumed + lost + stock | here, `food_is_conserved` |
//! | A house covered by food always eats | here, `covered_means_fed` |
//! | Satisfaction inside `0..=max`, on every service | here, `satisfaction_stays_within_bounds` |
//! | A house never holds more residents than its level allows | here, `residents_stay_within_the_house_capacity` |
//! | A house's level is monotone with constant services | `levels.rs`, `the_level_shows_no_oscillation` |
//! | A rejected command mutates nothing | here, `rejected_commands_mutate_nothing` |
//! | No panic on arbitrary commands, malformed ones included | here, `no_panic_on_ten_thousand_commands` |
//! | Incremental coverage identical to from-scratch | `coverage.rs`, `coverage_equivalence` |
//! | Network labelling independent of the build order | `roads.rs`, `the_labelling_does_not_depend_on_the_order` |
//! | Determinism: same seed and same commands, same hash | `sim-replay/tests/expected.rs` |
//!
//! The last three live next to the system they check, not here: they need
//! purpose-built scenarios, and moving them would make this file less readable
//! without making them any truer.
//!
//! Shared generator: sequences of arbitrary `Command`s — invalid ones included,
//! in significant proportion — on a 32x32 grid, then N ticks. A generator that
//! only produces valid commands checks a tenth of what it looks like it checks.
//!
//! **Known debt.** There is no real `cargo-fuzz` target
//! (`fuzz/fuzz_targets/commands.rs`): `proptest` with many cases plus the
//! deterministic fuzz below already cover point 3 of `CLAUDE.md`, and a
//! half-finished target would be worse than none. To be done when a new
//! mechanic widens the command space.

mod common;

use common::*;
use proptest::prelude::*;
use sim_core::{Coins, Command, CommandError, Occupant, TilePos, World};

/// The side of the test grid.
const SIDE: u8 = 32;
/// The generated coordinates deliberately run past the edge.
const GEN_SIDE: u8 = SIDE + 2;
/// The generated `kind`s go beyond the three in the table: the LLM will produce
/// ones that do not exist (CLAUDE.md, AI interface).
const GEN_KINDS: u16 = 5;

// --- the invariants, as reusable functions ----------------------------------

/// Every tile has at most one occupant, and for every building the tiles it
/// covers point back at it. Both directions: that is what catches demolition
/// bugs.
fn no_overlap(w: &World) -> Result<(), String> {
    // tile -> id: every declared occupant resolves to a live id.
    let mut occupied = 0usize;
    for idx in w.grid().indices() {
        let tile = w.grid().get(idx).ok_or("index outside the grid")?;
        if tile.occupant().is_none() {
            continue;
        }
        occupied += 1;
        match w.occupant(idx) {
            Some(Occupant::Building(id)) => {
                if w.building(id).is_none() {
                    return Err(format!("{idx:?} points at a dead building"));
                }
            }
            Some(Occupant::House(id)) => {
                if w.house(id).is_none() {
                    return Err(format!("{idx:?} points at a dead house"));
                }
            }
            None => return Err(format!("{idx:?} is occupied but resolves to no id")),
        }
        if tile.flags.has_road() {
            return Err(format!("{idx:?} has both a road and an occupant"));
        }
    }

    // id -> tile: every building owns exactly the tiles it covers.
    let mut expected = 0usize;
    for (id, b) in w.buildings() {
        let def = w
            .data()
            .def(b.kind)
            .ok_or_else(|| format!("building {id:?} has an unknown kind"))?;
        expected += usize::from(def.tile_count());
        for (dx, dy) in tiles_of(def.size) {
            let p = TilePos::new(b.origin.x + dx, b.origin.y + dy);
            let idx = w
                .grid()
                .idx(p)
                .ok_or_else(|| format!("building {id:?} sticks off the map at {p:?}"))?;
            if w.occupant(idx) != Some(Occupant::Building(id)) {
                return Err(format!("{p:?} does not belong to {id:?}"));
            }
        }
    }
    for (id, h) in w.houses() {
        expected += 1; // houses are 1x1 in M0
        let idx = w
            .grid()
            .idx(h.origin)
            .ok_or_else(|| format!("house {id:?} off the map"))?;
        if w.occupant(idx) != Some(Occupant::House(id)) {
            return Err(format!("{:?} does not belong to {id:?}", h.origin));
        }
    }

    if occupied != expected {
        return Err(format!(
            "{occupied} occupied tiles, {expected} expected: there is an orphan tile or an overlap"
        ));
    }
    Ok(())
}

/// Population never negative and consistent with the houses that exist (D5).
///
/// Since phase 13 it also pins A12 down from the other side: a house that goes
/// up a level gains **permission** to hold more residents, never residents.
/// If levelling up brought people in, the equality would break here first.
fn population_is_consistent(w: &World) -> Result<(), String> {
    let expected = w.house_count() as u32 * u32::from(RESIDENTS_PER_HOUSE);
    if w.population() != expected {
        return Err(format!(
            "population {} but {} houses of {RESIDENTS_PER_HOUSE}",
            w.population(),
            w.house_count()
        ));
    }
    Ok(())
}

/// A house covered by the food service has eaten, on every tick since the
/// world began.
///
/// It is not a property of the coverage code on its own: it follows from the
/// **balancing**. If a food provider could take on more residents than it
/// feeds, the ones in excess would stay covered and hungry forever, because the
/// first provider wins a contest. It holds because the declared capacity does
/// not exceed what the output sustains — which is exactly what
/// `Inconsistency::CapacityBeyondOutput` guards, and what
/// `the_fixture_has_no_inconsistencies` pins down for these tests' fixture.
///
/// **How it is asked changed in phase 12.** Until then `House::served`'s food
/// bit was written by step 4 and meant "it ate", so comparing it against the
/// coverage — which means "a farm reaches it" — was a real question with two
/// independent sources. Phase 12 made both bits mean "covered" and both written
/// by step 3 (A9), and that comparison became `x == x`. What replaces it is
/// step 4's own counter, and it is the better question: it holds over the whole
/// history, not just at the moment somebody looks.
fn covered_houses_are_fed(w: &World) -> Result<(), String> {
    let unfed = w.food().covered_but_unfed;
    if unfed != 0 {
        return Err(format!(
            "{unfed} house-ticks with a food provider assigned and nothing eaten: \
             hunger has gone back to being an absorbing state"
        ));
    }
    Ok(())
}

/// No house holds more residents than its level allows (phase 13).
///
/// It is the invariant the whole ladder rests on: `residents <=
/// max_residents(level)` is assumed by the coverage, by the food arithmetic and
/// by phase 14's demographics. Two things could break it — a house born beyond
/// its level-1 capacity, and decay that shrinks a house without sending anyone
/// away — and the first is refused by validation while the second is what
/// `levels::review` evicts for.
///
/// A level outside the table is a failure too, not a skip: it is the only other
/// way `max_residents` returns `None`, and it would make the check vacuous
/// exactly where it matters.
fn residents_within_capacity(w: &World) -> Result<(), String> {
    for (id, h) in w.houses() {
        let Some(max) = w.data().rules.max_residents(h.level) else {
            return Err(format!("{id:?} is at level {}, off the table", h.level));
        };
        if h.residents > max {
            return Err(format!(
                "{id:?} holds {} residents at level {}, which takes {max}",
                h.residents, h.level
            ));
        }
    }
    Ok(())
}

/// Satisfaction inside `0..=max`, for every house and every service.
///
/// Never negative is guaranteed by the type, and that is half the reason it is
/// a `u8`; the ceiling is game semantics and has to be checked.
fn satisfaction_in_range(w: &World) -> Result<(), String> {
    let max = w.data().rules.satisfaction.max;
    for (id, h) in w.houses() {
        for k in sim_core::ServiceKind::ALL {
            let v = h.satisfaction[k.index()];
            if v > max {
                return Err(format!("{id:?} has {k:?} satisfaction {v}, beyond {max}"));
            }
        }
    }
    Ok(())
}

fn tiles_of((w, h): (u8, u8)) -> impl Iterator<Item = (u8, u8)> {
    (0..h).flat_map(move |dy| (0..w).map(move |dx| (dx, dy)))
}

/// The cost of an accepted command, for the treasury's balance.
fn accepted_cost(w: &World, cmd: &Command) -> Coins {
    match cmd {
        Command::Demolish { .. } => Coins::ZERO,
        Command::PlaceRoad { at } => w
            .grid()
            .at(*at)
            .and_then(|t| w.data().terrain(t.terrain))
            .map_or(Coins::ZERO, |d| d.road_cost),
        Command::PlaceBuilding { kind, .. } => w.data().def(*kind).map_or(Coins::ZERO, |d| d.cost),
    }
}

// --- generators -------------------------------------------------------------

fn any_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        3 => (0..GEN_SIDE, 0..GEN_SIDE)
            .prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        4 => (0..GEN_KINDS, 0..GEN_SIDE, 0..GEN_SIDE)
            .prop_map(|(k, x, y)| Command::PlaceBuilding {
                kind: sim_core::BuildingKindId::new(k),
                origin: TilePos::new(x, y),
            }),
        2 => (0..GEN_SIDE, 0..GEN_SIDE)
            .prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

/// A sequence of ticks, each with its own commands.
fn a_game() -> impl Strategy<Value = Vec<Vec<Command>>> {
    prop::collection::vec(prop::collection::vec(any_command(), 0..6), 1..12)
}

// --- the tests --------------------------------------------------------------

proptest! {
    /// No overlap, in both directions tile <-> building.
    #[test]
    fn no_overlap_ever(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = no_overlap(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Every occupied tile resolves to a live id in the slotmap, and the
    /// population stays consistent with the houses.
    #[test]
    fn population_stays_consistent(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = population_is_consistent(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// The treasury adds up: the starting amount minus the sum of the accepted
    /// costs. In M0 there is no income, so it holds as an exact equality.
    #[test]
    fn the_treasury_adds_up(p in a_game()) {
        let mut w = world();
        let mut spent = Coins::ZERO;

        for cmds in &p {
            // The cost has to be read **before** the tick: afterwards, the tile
            // the road was built on has changed.
            let costs: Vec<Coins> = cmds.iter().map(|c| accepted_cost(&w, c)).collect();
            let r = tick(&mut w, cmds);

            for (i, cost) in costs.iter().enumerate() {
                let was_rejected = r.rejected.iter().any(|(j, _)| *j == i);
                if !was_rejected {
                    spent = spent.checked_add(*cost).expect("the costs do not overflow");
                }
            }

            let expected = Coins::new(STARTING_TREASURY)
                .checked_sub(spent)
                .expect("the treasury does not overflow");
            prop_assert_eq!(w.economy().treasury, expected);
            prop_assert!(!w.economy().treasury.is_negative(), "negative treasury");
        }
    }

    /// Food conserved: `produced == consumed + lost + stock`, as an exact
    /// equality. The generator also builds farms and demolishes them, so it
    /// covers both loss terms.
    #[test]
    fn food_is_conserved(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            tick(&mut w, &[]);
            let t = w.food();
            prop_assert_eq!(
                t.expected_stock(),
                w.total_stock(),
                "produced {} != consumed {} + lost_to_full_stock {} + lost_to_demolition {} + stock",
                t.produced, t.consumed, t.lost_to_full_stock, t.lost_to_demolition
            );
        }
    }

    /// A house covered by food eats: hunger is lack of coverage, never a state
    /// you can be stuck in while being served.
    #[test]
    fn covered_means_fed(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = covered_houses_are_fed(&w) {
                return Err(TestCaseError::fail(e));
            }
            // In the steady state too, not only on the tick it was built: a
            // brand new farm has the first tick's stock, which would mask a
            // structural deficit.
            for _ in 0..3 {
                tick(&mut w, &[]);
                if let Err(e) = covered_houses_are_fed(&w) {
                    return Err(TestCaseError::fail(e));
                }
            }
        }
    }

    /// No house ever holds more residents than its level allows, over a game
    /// long enough for the monthly reviews to have acted.
    #[test]
    fn residents_stay_within_the_house_capacity(p in a_game()) {
        let mut w = world();
        let month = w.data().rules.ticks_per_month;
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = residents_within_capacity(&w) {
                return Err(TestCaseError::fail(e));
            }
            // Past two reviews: a run in which the levels never move would make
            // this test say nothing.
            for _ in 0..=month * 2 {
                tick(&mut w, &[]);
                if let Err(e) = residents_within_capacity(&w) {
                    return Err(TestCaseError::fail(e));
                }
            }
        }
    }

    /// Satisfaction saturates at both ends: after any sequence of commands and
    /// any number of ticks it stays inside `0..=max`.
    #[test]
    fn satisfaction_stays_within_bounds(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = satisfaction_in_range(&w) {
                return Err(TestCaseError::fail(e));
            }
            // Long enough to reach the ceiling from zero, so the clamp is
            // really exercised and not merely never approached.
            let climb = w.data().rules.satisfaction.max / w.data().rules.satisfaction.step_up;
            for _ in 0..=climb {
                tick(&mut w, &[]);
                if let Err(e) = satisfaction_in_range(&w) {
                    return Err(TestCaseError::fail(e));
                }
            }
        }
    }

    /// A rejected command mutates nothing: the state after a tick made only of
    /// rejected commands is the one from before, tick aside.
    #[test]
    fn rejected_commands_mutate_nothing(p in a_game()) {
        let mut w = world();
        for cmds in &p {
            let before = w.clone();
            let r = tick(&mut w, cmds);
            if r.rejected.len() == cmds.len() {
                prop_assert_eq!(w.grid(), before.grid());
                prop_assert_eq!(w.economy(), before.economy());
                prop_assert_eq!(w.building_count(), before.building_count());
                prop_assert_eq!(w.house_count(), before.house_count());
            }
        }
    }
}

// --- fuzzing the commands (CLAUDE.md, Testing point 3) ----------------------

/// A PRNG local to the test: deterministic and with no extra dependencies.
/// Deliberately not `RngSet` — this test must not depend on the game's
/// generator, which is itself under test.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// 10,000 random commands over 1,000 ticks: the core never panics and
/// `rejected` grows consistently.
///
/// It is not a property test because one long run costs as much as thousands of
/// short cases, and repeating it 256 times would add no coverage: the seed is
/// fixed, so it is as reproducible as a committed recording.
#[test]
fn no_panic_on_ten_thousand_commands() {
    const TICKS: usize = 1_000;
    const COMMANDS: usize = 10_000;

    let mut rng = SplitMix64(0x5EED);
    let mut w = world();
    let mut issued = 0usize;
    let mut rejected = 0usize;
    let mut accepted = 0usize;

    for t in 0..TICKS {
        let how_many = if issued < COMMANDS {
            (rng.range(21) as usize).min(COMMANDS - issued)
        } else {
            0
        };
        let cmds: Vec<Command> = (0..how_many).map(|_| random_command(&mut rng)).collect();
        issued += cmds.len();

        let r = tick(&mut w, &cmds);

        assert!(
            r.rejected.len() <= cmds.len(),
            "more rejections than commands at tick {t}"
        );
        for (i, _) in &r.rejected {
            assert!(*i < cmds.len(), "rejection index out of range at tick {t}");
        }
        rejected += r.rejected.len();
        accepted += cmds.len() - r.rejected.len();

        assert!(
            !w.economy().treasury.is_negative(),
            "negative treasury at tick {t}"
        );
        no_overlap(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
        population_is_consistent(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
        satisfaction_in_range(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
        residents_within_capacity(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
    }
    covered_houses_are_fed(&w).expect("no house stayed covered and hungry over 1,000 ticks");

    assert_eq!(w.tick(), TICKS as u32);
    assert_eq!(issued, COMMANDS, "the test has to issue every command");
    assert_eq!(accepted + rejected, issued);
    assert!(rejected > 0, "the generator produces no bad commands");
    assert!(accepted > 0, "the generator produces no valid commands");
}

fn random_command(rng: &mut SplitMix64) -> Command {
    let x = rng.range(u64::from(GEN_SIDE)) as u8;
    let y = rng.range(u64::from(GEN_SIDE)) as u8;
    match rng.range(9) {
        0..=2 => Command::PlaceRoad {
            at: TilePos::new(x, y),
        },
        3..=6 => Command::PlaceBuilding {
            kind: sim_core::BuildingKindId::new(rng.range(u64::from(GEN_KINDS)) as u16),
            origin: TilePos::new(x, y),
        },
        _ => Command::Demolish {
            at: TilePos::new(x, y),
        },
    }
}

/// The fixture has to be balanced like the real tables.
///
/// `sim-core`'s tests run on a hand-built dataset, which never goes through
/// `sim-data`'s validation. Without this check the fixture could drift onto
/// numbers production would reject, and `covered_means_fed` would be checking a
/// property the real game does not have.
///
/// Since phase 13 it is one assertion for **every** cross-table check rather
/// than one test per check: whoever adds a rule to
/// `DataSet::inconsistencies()` gets the fixture protected by it for free,
/// which is the whole reason those checks live in `sim-core`.
#[test]
fn the_fixture_has_no_inconsistencies() {
    for (what, d) in [
        ("dataset", dataset()),
        ("the service ladder", dataset_with_a_service_ladder()),
        (
            "a house that wants water only",
            dataset_where_a_house_requires(&[sim_core::ServiceKind::Water]),
        ),
    ] {
        assert_eq!(d.inconsistencies(), vec![], "{what}");
    }
}

/// A command off the map is always rejected, never a panic and never a silent
/// success.
#[test]
fn off_the_map_is_always_rejected() {
    let mut w = world_of(4, 4);
    let outside = [
        Command::PlaceRoad { at: pos(4, 0) },
        Command::PlaceRoad { at: pos(0, 4) },
        Command::PlaceRoad { at: pos(255, 255) },
        Command::PlaceBuilding {
            kind: HOUSE,
            origin: pos(9, 9),
        },
        Command::Demolish { at: pos(200, 1) },
    ];
    let r = tick(&mut w, &outside);
    assert_eq!(r.rejected.len(), outside.len());
    for (_, e) in &r.rejected {
        assert!(matches!(e, CommandError::OutOfBounds(_)), "{e:?}");
    }
}
