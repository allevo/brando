//! Immigration and emigration: the two flows, the two regimes the inbound one
//! runs under, and the count of everybody a full city could not house.
//!
//! Every horizon in here is **computed from the `DataSet`**, never written as a
//! number, for phase 07's reason: a hardcoded value breaks on every
//! rebalancing without signalling anything real.
//!
//! Three of the plan's eleven tests are not repeated here because an existing
//! test already proves them and migration changes nothing about the proof:
//! test 5 (conservation) is `population_is_conserved_after_any_game` in
//! `invariants.rs`, whose equation already carried `immigrated` and
//! `emigrated` since phase 14; test 6 (an empty city) is
//! `migration::tests::an_empty_city_is_maximally_attractive`, inline next to
//! the function it pins down; test 10 (the coverage is not sticky) is
//! `at_equal_distance_the_smaller_tile_wins` and
//! `at_equal_distance_the_order_is_the_houses_and_not_the_road_tiles` in
//! `coverage.rs` — `compute_from_scratch` reads no history to begin with, and
//! migration gives it none either.
//!
//! `turned_away` leans on that same conservation test, and on it needing **no
//! edit at all**: a counter that had crept into the population's arithmetic
//! would have had to be added to the equation, and the proof that it did not is
//! that the proptest went on passing untouched.

mod common;

/// Wrapped in a module so the test names carry the phase's word: the
/// verification command filters on the **test name**, not on the file it
/// lives in.
mod migration {
    use super::common::*;
    use sim_core::data::DataSet;
    use sim_core::{BuildingKindId, Calendar, Command, Event, RngKind, ServiceKind, World};
    use std::sync::Arc;

    // --- helpers --------------------------------------------------------------

    /// The fixture with migration switched off — both rates at zero, the same
    /// shape `MigrationRules::is_off` reads and `dataset_without_demographics`
    /// already builds for the demographics' own two rates.
    fn dataset_with_migration_off() -> Arc<DataSet> {
        let data = dataset();
        let mut rules = data.rules.clone();
        rules.migration.founding_immigration_per_thousand_per_month = 0;
        rules.migration.immigration_per_thousand_per_month = 0;
        rules.migration.emigration_per_thousand_per_month_unhappy = 0;
        Arc::new(DataSet::new(
            rules,
            data.road_cost_per_terrain,
            data.buildings.clone(),
            data.difficulties.clone(),
        ))
    }

    /// The fixture with immigration turned up well past the fixture's own
    /// number: a house with nobody in reach of a provider still weighs on
    /// `attractiveness`'s satisfaction share, which caps the rate at whatever
    /// `free_places_weight` alone is worth. A handful of houses is not enough
    /// free places for the fixture's ordinary rate to mature an event inside
    /// a test's lifetime at that cap — this is what lets a small, deliberately
    /// uncovered scene demonstrate the property in a few thousand ticks
    /// instead of needing a city-sized one.
    ///
    /// Both rates, because the scene it serves is a two-house city: it lives its
    /// whole life under `FOUNDING_POPULATION_THRESHOLD`, so the growth rate
    /// alone would never be read.
    fn dataset_with_fast_immigration() -> Arc<DataSet> {
        let data = dataset();
        let mut rules = data.rules.clone();
        rules.migration.founding_immigration_per_thousand_per_month = 5_000;
        rules.migration.immigration_per_thousand_per_month = 5_000;
        Arc::new(DataSet::new(
            rules,
            data.road_cost_per_terrain,
            data.buildings.clone(),
            data.difficulties.clone(),
        ))
    }

    fn roads(w: &mut World, cells: &[(u8, u8)]) {
        let cmds: Vec<_> = cells
            .iter()
            .map(|(x, y)| Command::PlaceRoad { at: pos(*x, *y) })
            .collect();
        let r = tick(w, &cmds);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    }

    fn build(w: &mut World, kind: BuildingKindId, x: u8, y: u8) {
        let r = tick(
            w,
            &[Command::PlaceBuilding {
                kind,
                origin: pos(x, y),
            }],
        );
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    }

    fn run(w: &mut World, ticks: u32) {
        for _ in 0..ticks {
            tick(w, &[]);
        }
    }

    // --- 1. the goal ------------------------------------------------------------

    /// Two cities, identical except for their coverage, on the same seed: the
    /// served one grows faster.
    ///
    /// `hard`, so that every resident either city ever holds arrived by
    /// migration (`starting_residents_per_house` is zero) — the plateau this
    /// test is about is not diluted by construction-time residents.
    #[test]
    fn a_served_city_attracts_faster_than_an_unserved_one() {
        fn city(served: bool) -> World {
            let mut w = world_seeded_with(dataset(), 32, 32, HARD, SEED);
            roads(&mut w, &(1..=6).map(|x| (x, 4)).collect::<Vec<_>>());
            // The provider pair stands in both cities, so the two play out the
            // same number of commands and the same number of ticks up to this
            // point — only whether they are hooked up to the network differs.
            // Off in a far corner with no road touching it, a provider serves
            // nobody (`RULES.md`'s coverage section): "a provider without an
            // entrance serves nobody".
            if served {
                build(&mut w, WELL, 1, 3);
                build(&mut w, FARM, 3, 2);
            } else {
                build(&mut w, WELL, 20, 20);
                build(&mut w, FARM, 22, 22);
            }
            for i in 0..3 {
                build(&mut w, HOUSE, 2 + i, 5);
            }
            w
        }

        let mut served_city = city(true);
        let mut unserved_city = city(false);

        let years = Calendar::TICKS_PER_YEAR * 3;
        run(&mut served_city, years);
        run(&mut unserved_city, years);

        assert!(
            served_city.population() > unserved_city.population(),
            "served {} residents, unserved {}: coverage has to make the \
             difference",
            served_city.population(),
            unserved_city.population()
        );
        // Not the unserved city's final population: an uncovered house that
        // does get an immigrant is immediately eligible for both the raised
        // death rate and emigration, so a lone arrival can be gone again
        // within the same handful of ticks and the snapshot at exactly three
        // years can land on either side of zero by chance. `immigrated`
        // is the counter that cannot be erased by what happened afterwards —
        // it is the proof that the flow reached this city at all.
        assert!(
            unserved_city.population_totals().immigrated > 0,
            "immigration is not gated on coverage, so the unserved city has \
             to attract somebody too, even if it cannot keep them"
        );
    }

    // --- 2. the hard gate --------------------------------------------------------

    /// Zero free places ⇒ zero immigrants, at any attractiveness, for as long
    /// as it lasts — and on founding one house with room, the flow resumes.
    #[test]
    fn zero_free_places_is_zero_immigration_and_a_new_house_restarts_it() {
        let mut w = world_at(16, 16, EASY);
        roads(&mut w, &[(1, 4), (2, 4), (3, 4)]);
        build(&mut w, HOUSE, 2, 5);
        // `easy` fills the house whole: no free place anywhere in the city.
        assert_eq!(w.house_count(), 1);

        run(&mut w, 100);
        assert_eq!(
            w.population_totals().immigrated,
            0,
            "there was nowhere for anybody to go"
        );

        build(&mut w, HOUSE, 6, 5);
        run(&mut w, 200);
        assert!(
            w.population_totals().immigrated > 0,
            "a house with a free place has to restart the flow"
        );
    }

    // --- 3. the destination is not gated on coverage (revised) ------------------

    /// An uncovered, already-inhabited house with room receives immigrants
    /// all the same, and an uncovered, **empty** house is refilled too — the
    /// two cases slot 14.5 hands this phase.
    ///
    /// The already-inhabited house is what proves the point: an empty one is
    /// never served either way (slot 14.5), so it cannot tell "not gated on
    /// coverage" apart from "gated on coverage, but empty houses are exempt".
    /// A house with residents and no provider in reach is the case where the
    /// two readings disagree.
    #[test]
    fn immigration_reaches_houses_the_coverage_never_touches() {
        let mut w = world_with(dataset_with_fast_immigration(), 16, 16, EASY);
        roads(&mut w, &[(1, 4), (2, 4), (3, 4)]);
        build(&mut w, HOUSE, 2, 5);
        let inhabited = w.houses().next().map(|(id, _)| id).expect("the house");

        // No well, no farm: this house is uncovered on every service its
        // level asks for, for the whole test. Founded at level 2 with room to
        // spare and residents already in it, directly through the test-only
        // hooks — `House`'s fields are public, and this is the shape the
        // phase 15 plan itself asks the test to build.
        {
            let h = w.house_mut(inhabited).expect("alive");
            h.level = level(2);
            h.residents = 2;
        }

        // A second house, built empty by hand the same way: the other case
        // slot 14.5 is about — an empty, uncovered house has to be
        // refillable, and refilling it is the one thing worth an event.
        build(&mut w, HOUSE, 6, 5);
        let empty = w
            .houses()
            .find(|(id, _)| *id != inhabited)
            .map(|(id, _)| id)
            .expect("the second house");
        w.house_mut(empty).expect("alive").residents = 0;

        assert!(
            !w.coverage().is_served(inhabited, ServiceKind::Water),
            "the setup has to be uncovered, or the test proves nothing"
        );
        assert!(!w.coverage().is_served(empty, ServiceKind::Water));

        // The high-water mark, not the final reading: an uncovered house is
        // immediately eligible for both the raised death rate and emigration
        // once it has anybody in it, so an immigrant who did arrive can be
        // gone again a handful of ticks later. The property under test is
        // that immigration *reaches* the house, which the peak proves even
        // when the final snapshot does not.
        let mut peak = 2u16;
        let mut repopulated = false;
        for _ in 0..Calendar::TICKS_PER_YEAR * 5 {
            let r = tick(&mut w, &[]);
            repopulated |= r
                .events
                .iter()
                .any(|e| matches!(e, Event::HouseRepopulated { house } if *house == empty));
            peak = peak.max(w.house(inhabited).expect("alive").residents);
            if peak > 2 && repopulated {
                break;
            }
        }

        assert!(
            peak > 2,
            "an inhabited, uncovered house with room never received an \
             immigrant"
        );
        assert!(
            repopulated,
            "an empty, uncovered house was never refilled — \
             `Event::HouseRepopulated` never fired"
        );
        assert!(
            !w.coverage().is_served(inhabited, ServiceKind::Water),
            "and it stayed uncovered throughout: nothing here ever built a \
             well"
        );
    }

    // --- 4. emigration and death are told apart ----------------------------------

    /// With the well demolished, a city empties out through **both**
    /// emigration and death, and the totals — not the population — are what
    /// distinguishes "leaving" from "starving to death".
    #[test]
    fn emigration_and_death_are_distinct_outflows() {
        let mut w = world_at(24, 24, EASY);
        roads(&mut w, &(1..=10).map(|x| (x, 4)).collect::<Vec<_>>());
        build(&mut w, WELL, 1, 3);
        build(&mut w, FARM, 3, 2);
        for i in 0..4 {
            build(&mut w, HOUSE, 6 + i, 5);
        }

        run(&mut w, Calendar::TICKS_PER_YEAR);
        let peak = w.population();
        assert!(peak > 0);

        let r = tick(&mut w, &[Command::Demolish { at: pos(1, 3) }]);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);

        run(&mut w, Calendar::TICKS_PER_YEAR * 2);

        let t = w.population_totals();
        assert!(
            t.emigrated > 0,
            "residents below the emigration threshold have to leave"
        );
        assert!(
            t.died > 0,
            "residents going without still die at the raised rate"
        );
    }

    // --- 7. the domains stay separate --------------------------------------------

    /// With migration switched off, a growing, dying, levelling-up city goes
    /// on doing all three — `RngKind::Demographics` runs and events mature —
    /// while `RngKind::Migration` matures **no** events of its own.
    ///
    /// **Not** "`RngKind::Migration` draws nothing at all": a rate of zero
    /// still leaves somebody eligible whenever a house has a free place or a
    /// resident below the emigration threshold, and jitter is drawn once for
    /// every eligible flow on every tick *regardless of the table's numbers*
    /// — deliberately, so that `draws` stays a function of the city and not
    /// of a rate somebody could retune. `MigrationRules::is_off` is the
    /// contract `coverage_equivalence` actually leans on: nobody **moves**,
    /// not that nothing is **drawn**. What this test is really about is that
    /// the two domains do not knock each other out of phase — the same proof
    /// phase 14's `sequences_are_reproducible_with_the_expected_values` gives
    /// at the level of raw values, given here at the level of the game.
    #[test]
    fn migration_off_leaves_demographics_running_and_untouched() {
        let mut w = world_with(dataset_with_migration_off(), 24, 24, EASY);
        roads(&mut w, &(1..=8).map(|x| (x, 4)).collect::<Vec<_>>());
        build(&mut w, WELL, 1, 3);
        build(&mut w, FARM, 3, 2);
        build(&mut w, HOUSE, 6, 5);

        run(&mut w, Calendar::TICKS_PER_YEAR);

        let t = w.population_totals();
        assert_eq!(
            (t.immigrated, t.emigrated),
            (0, 0),
            "migration is off at the table: no event of either flow should \
             ever have matured"
        );
        assert!(
            w.rng().draws(RngKind::Demographics) > 0,
            "the demographics have to go on running while migration is off"
        );
        assert!(
            t.born > 0 || t.died > 0,
            "and it has to be a real, live city and not a still one"
        );
    }

    // --- 8. a hundred seeds -------------------------------------------------------

    /// On `hard`, where migration is the only way anybody arrives, different
    /// seeds give different but plausible five-year populations.
    ///
    /// Phase 14's `many_seeds_give_different_but_plausible_trajectories`
    /// extended to the profile where migration, and not births, is the
    /// dominant flow.
    #[test]
    fn many_seeds_give_different_but_plausible_populations_on_hard() {
        fn a_served_city(seed: u64) -> World {
            let mut w = world_seeded_with(dataset(), 32, 32, HARD, seed);
            roads(&mut w, &(1..=10).map(|x| (x, 4)).collect::<Vec<_>>());
            build(&mut w, WELL, 1, 3);
            build(&mut w, FARM, 3, 2);
            for i in 0..3 {
                build(&mut w, HOUSE, 6 + i, 5);
            }
            w
        }

        let years = Calendar::TICKS_PER_YEAR * 5;
        let mut finals = Vec::new();
        for seed in 0..40u64 {
            let mut w = a_served_city(seed);
            run(&mut w, years);
            finals.push(w.population());
        }

        let low = *finals.iter().min().expect("40 runs");
        let high = *finals.iter().max().expect("40 runs");
        assert!(
            finals.iter().any(|p| *p != finals[0]),
            "forty seeds cannot all give the same population: {finals:?}"
        );
        assert!(low > 0, "on `hard` migration has to fill the city at all");
        assert!(
            high <= low * 3 + 1,
            "the band is too wide to be balancing rather than a lottery: \
             {low}..{high}"
        );
    }

    // --- 9. & 11. the loop damps, and the overshoot is reachable ----------------

    /// A city that outgrows its own services, left to itself for five years:
    /// the overshoot is real — the farm's capacity is exceeded — and the
    /// population still converges to a band instead of pulsing wider and
    /// wider.
    ///
    /// *The test that defines the phase.* If it fails, the phase file is
    /// explicit about the remedy: balancing — a lower emigration threshold or
    /// a slower accumulator — not a new mechanism.
    #[test]
    fn the_city_outgrows_its_services_and_the_loop_damps() {
        let mut w = world_seeded_with(dataset(), 32, 32, HARD, SEED);
        roads(&mut w, &(1..=10).map(|x| (x, 4)).collect::<Vec<_>>());
        build(&mut w, WELL, 1, 3);
        build(&mut w, FARM, 3, 2);
        for i in 0..3 {
            build(&mut w, HOUSE, 6 + i, 5);
        }

        fn residents_served(w: &World) -> u16 {
            w.houses()
                .filter(|(_, h)| h.served.get(ServiceKind::Food))
                .map(|(_, h)| h.residents)
                .sum()
        }

        let years = Calendar::TICKS_PER_YEAR * 5;
        let mut samples: Vec<u32> = Vec::new();
        let mut overshot = false;
        for t in 0..years {
            tick(&mut w, &[]);
            if u32::from(residents_served(&w)) < w.population() {
                overshot = true;
            }
            if t % Calendar::TICKS_PER_MONTH == 0 {
                samples.push(w.population());
            }
        }
        assert!(
            overshot,
            "the city never outgrew the farm's capacity in five years"
        );

        // The amplitude in the second half of the run against the first: not
        // a Fourier analysis, but the property the phase file asks for is
        // that the swings do not keep growing, and a widening range across
        // the two halves is exactly what unbounded oscillation looks like in
        // monthly samples.
        let half = samples.len() / 2;
        let range = |s: &[u32]| {
            let lo = *s.iter().min().expect("samples");
            let hi = *s.iter().max().expect("samples");
            hi - lo
        };
        let first_half = range(&samples[..half]);
        let second_half = range(&samples[half..]);
        assert!(
            second_half <= first_half * 2 + 2,
            "the oscillation grew instead of damping: {first_half} in the \
             first half of the run, {second_half} in the second"
        );
    }

    // --- the two regimes, and what a full city does with the people it draws ---

    /// How long the two-city tests run for, once their cities are pinned down.
    ///
    /// Short on both ends and for two different reasons. It has to stay well
    /// inside `FREE_PLACES_EACH`, or a city that fills up is being measured on
    /// its capacity instead of on its rate, which is the thing under test. And
    /// setup plus run has to stay inside one month, or the review at the month
    /// boundary decays the levels the setup pinned — the houses are uncovered,
    /// so every one of them is below its decay threshold from the first tick.
    const A_SHORT_RUN: u32 = Calendar::TICKS_PER_MONTH / 5;

    /// The free places each of the two cities is built with. Their populations
    /// differ four-fold; this does not, which is the whole construction.
    const FREE_PLACES_EACH: u16 = 24;

    /// The fixture with immigration turned up, and nothing else able to move
    /// anybody.
    ///
    /// The demographics go to zero, and emigration is stopped by a
    /// `emigration_threshold` of zero — a threshold no house can ever sit below
    /// — rather than by a rate of zero, which is the "lone zero rate" a real
    /// table is refused for. The jitter goes to zero so the rate is exact: these
    /// tests compare one city's arrivals against another's, and a wobble would
    /// be a difference that is not the one being asked about.
    fn dataset_where_only_immigration_moves(threshold: u32, per_thousand: u16) -> Arc<DataSet> {
        let data = dataset();
        let mut rules = data.rules.clone();
        rules.demographics.births_per_thousand_per_month = 0;
        rules.demographics.deaths_per_thousand_per_month = 0;
        rules
            .demographics
            .deaths_per_thousand_per_month_when_unserved = 0;
        rules.migration.founding_population_threshold = threshold;
        rules.migration.founding_immigration_per_thousand_per_month = per_thousand;
        rules.migration.immigration_per_thousand_per_month = per_thousand;
        rules.migration.emigration_threshold = 0;
        rules.migration.jitter_per_thousand = 0;
        Arc::new(DataSet::new(
            rules,
            data.road_cost_per_terrain,
            data.buildings.clone(),
            data.difficulties.clone(),
        ))
    }

    /// `count` houses in a row, each pinned to `at` with `residents` in it, and
    /// no provider anywhere on the map.
    ///
    /// **Uncovered on purpose, and that is what makes two of these comparable.**
    /// With no house served, `attractiveness` reads its free-places share at the
    /// ceiling — there is no served house to divide by — and its satisfaction
    /// share at zero, and it does so whatever the city's size. Two of these
    /// cities therefore have exactly the same attractiveness however differently
    /// they are built, so the only thing left that can differ between them is
    /// what the rate is counted against.
    fn an_uncovered_city(data: &Arc<DataSet>, count: u8, at: u8, residents: u16) -> World {
        let mut w = world_with(Arc::clone(data), 24, 24, EASY);
        roads(&mut w, &(1..=count + 1).map(|x| (x, 4)).collect::<Vec<_>>());
        for i in 0..count {
            build(&mut w, HOUSE, 1 + i, 5);
        }
        let ids: Vec<_> = w.houses().map(|(id, _)| id).collect();
        for id in ids {
            let h = w.house_mut(id).expect("just built");
            h.level = level(at);
            h.residents = residents;
        }
        w
    }

    /// The small city and the big one, in that order: three houses of four
    /// against six houses of eight, all of them at the top level, so that the
    /// second holds four times the population of the first with exactly the same
    /// number of free places.
    fn a_small_city_and_a_big_one(data: &Arc<DataSet>) -> (World, World) {
        let top = LEVEL_RESIDENTS.len() as u8;
        let small = an_uncovered_city(data, 3, top, RESIDENTS_PER_HOUSE);
        let big = an_uncovered_city(data, 6, top, RESIDENTS_PER_HOUSE * 2);
        (small, big)
    }

    fn free_places(w: &World) -> u16 {
        w.houses()
            .map(|(_, h)| {
                w.data()
                    .rules
                    .max_residents(h.level)
                    .unwrap_or(0)
                    .saturating_sub(h.residents)
            })
            .sum()
    }

    /// Below the threshold the rate is counted against the free places, so two
    /// cities with the same free places draw the same immigrants however
    /// differently populated they are.
    ///
    /// This is phase 15's formula, kept, and it is what a city founded with
    /// nobody in it depends on: a rate counted against a population of nobody is
    /// zero for ever, whatever the attractiveness.
    #[test]
    fn below_the_threshold_the_rate_does_not_read_the_population() {
        // Above the bigger city's population, so neither of them crosses over,
        // even after a run's worth of arrivals.
        let data = dataset_where_only_immigration_moves(1_000, 5_000);
        let (mut small, mut big) = a_small_city_and_a_big_one(&data);
        assert_eq!(free_places(&small), FREE_PLACES_EACH);
        assert_eq!(free_places(&big), FREE_PLACES_EACH);
        assert!(big.population() > small.population());

        run(&mut small, A_SHORT_RUN);
        run(&mut big, A_SHORT_RUN);

        let (a, b) = (
            small.population_totals().immigrated,
            big.population_totals().immigrated,
        );
        assert!(
            a > 0,
            "neither city drew anybody, so the comparison below says nothing"
        );
        assert!(
            b < u64::from(FREE_PLACES_EACH),
            "the big city filled up, so it was measured on its capacity and \
             not on its rate: {b}"
        );
        assert_eq!(
            a, b,
            "the founding rate reads the free places, which the two cities \
             hold equally: {a} against {b}"
        );
    }

    /// At or above the threshold the rate is counted against the population, so
    /// the bigger of two equally attractive cities with equally many free places
    /// draws people faster. **The property this phase is for.**
    #[test]
    fn past_the_threshold_a_bigger_city_draws_faster() {
        // Below the smaller city's population, so both are in the growth regime
        // from the first tick of the run.
        let data = dataset_where_only_immigration_moves(1, 5_000);
        let (mut small, mut big) = a_small_city_and_a_big_one(&data);

        run(&mut small, A_SHORT_RUN);
        run(&mut big, A_SHORT_RUN);

        let (a, b) = (
            small.population_totals().immigrated,
            big.population_totals().immigrated,
        );
        assert!(
            b < u64::from(FREE_PLACES_EACH),
            "the big city filled up, so the comparison is about its capacity \
             and not its rate: {b}"
        );
        assert!(
            b > a,
            "the same free places and the same attractiveness, four times the \
             population, and no more arrivals: {a} against {b}"
        );
    }

    /// A city past the threshold with nowhere to put anybody lets nobody in and
    /// counts every one of them, which is the whole point of the counter: the
    /// population alone reads the same whether a city is full or unwanted.
    ///
    /// Below the threshold the same city counts nobody, and that is right rather
    /// than an omission — down there the rate *is* a multiple of the free
    /// places, so a city with none draws nobody to turn away in the first place.
    #[test]
    fn a_full_city_turns_people_away_and_a_founding_one_has_nobody_to_turn() {
        fn a_city_with_no_room(threshold: u32) -> World {
            let data = dataset_where_only_immigration_moves(threshold, 5_000);
            // Untouched after building: on `easy` a house is born holding the
            // whole of its first level, so the city is full from the start.
            let w = an_uncovered_city(&data, 3, 1, RESIDENTS_PER_HOUSE);
            assert_eq!(free_places(&w), 0);
            w
        }

        let mut full = a_city_with_no_room(1);
        let mut founding = a_city_with_no_room(1_000);
        assert!(founding.population() < 1_000);

        run(&mut full, Calendar::TICKS_PER_MONTH / 2);
        run(&mut founding, Calendar::TICKS_PER_MONTH / 2);

        let t = full.population_totals();
        assert_eq!(t.immigrated, 0, "there was nowhere for anybody to go");
        assert!(
            t.turned_away > 0,
            "a full city past the threshold still draws people, and every one \
             of them has to be counted"
        );
        assert_eq!(
            i64::from(full.population()),
            t.balance(),
            "turning somebody away must not disturb the conservation equality: \
             they never became a resident of anywhere"
        );

        assert_eq!(
            (
                founding.population_totals().immigrated,
                founding.population_totals().turned_away
            ),
            (0, 0),
            "below the threshold the rate is a multiple of the free places, so \
             a city with none draws nobody at all"
        );
    }

    /// `turned_away` is exactly the arrivals the city could not seat, and it is
    /// checked against a city that could seat all of them rather than against a
    /// number written into the test.
    ///
    /// The two cities are built by the same commands and pinned to the same
    /// population, so the rate — counted against that population, scaled by an
    /// attractiveness both read identically — matures the same events in each on
    /// the tick that is measured. Only the room differs. What one city lets in,
    /// the other has to turn away.
    #[test]
    fn turned_away_counts_exactly_what_the_city_could_not_seat() {
        let data = dataset_where_only_immigration_moves(1, 60_000);
        // Identical up to here — same commands, same ticks, so both carry the
        // same unmatured fraction into the tick below.
        let mut roomy =
            an_uncovered_city(&data, 3, LEVEL_RESIDENTS.len() as u8, RESIDENTS_PER_HOUSE);
        let mut cramped = an_uncovered_city(&data, 3, 1, RESIDENTS_PER_HOUSE);
        // One house of the cramped city is opened up by exactly one level.
        let one = cramped.houses().next().map(|(id, _)| id).expect("a house");
        cramped.house_mut(one).expect("alive").level = level(2);

        let room = LEVEL_RESIDENTS[1] - RESIDENTS_PER_HOUSE;
        assert_eq!(free_places(&cramped), room);
        assert_eq!(roomy.population(), cramped.population());

        let r = tick(&mut roomy, &[]);
        let c = tick(&mut cramped, &[]);

        assert_eq!(
            r.summary.turned_away, 0,
            "the roomy city had a place for everyone it drew"
        );
        assert!(
            r.summary.immigrated > room,
            "the rate has to mature more arrivals than the cramped city can \
             seat, or there is no shortfall to count: {} against {room}",
            r.summary.immigrated
        );
        assert_eq!(
            c.summary.immigrated, room,
            "the cramped city seats exactly its free places and not one more"
        );
        assert_eq!(
            c.summary.turned_away,
            r.summary.immigrated - room,
            "and turns away exactly the rest of what the same rate drew"
        );
    }
}
