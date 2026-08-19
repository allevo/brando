//! Phase 13 — house levels.
//!
//! Every horizon in here is **computed from the `DataSet`**, never written as a
//! number: it is the lesson of phase 07's test 3, where a hardcoded value broke
//! on every rebalancing without signalling anything real. The thresholds, the
//! climb rate and the cadence of the review all come out of the tables, so this
//! file keeps saying the same thing after a rebalancing — or goes red for a
//! real reason.

mod common;

/// Wrapped in a module so the test names carry the phase's word: the
/// verification command filters on the **test name**, not on the file it lives
/// in — the trap phase 12 fell into.
mod levels {
    use super::common::*;
    use proptest::prelude::*;
    use sim_core::{
        BuildingKindId, Calendar, Command, Event, HouseId, Level, ServiceKind, Tick, World,
    };
    use std::collections::BTreeMap;

    // --- helpers ------------------------------------------------------------

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

    /// A well and a farm on a road, plus one house served by both.
    ///
    /// The same layout as phase 12's fixture: this phase reads what that one
    /// computes, and a shared shape makes the two files comparable.
    fn a_served_house() -> (World, HouseId) {
        a_served_house_on(world())
    }

    fn a_served_house_on(mut w: World) -> (World, HouseId) {
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);
        build(&mut w, FARM, 5, 2); // takes (5,2)..(6,3), touches row 4
        build(&mut w, HOUSE, 8, 5);

        let house = w.houses().next().map(|(id, _)| id).expect("the house");
        let h = w.house(house).expect("alive");
        assert!(
            h.served.get(ServiceKind::Water) && h.served.get(ServiceKind::Food),
            "the fixture has to start from a house served by both"
        );
        assert_eq!(h.level, level(1), "a house is born at level 1");
        (w, house)
    }

    fn house_level(w: &World, house: HouseId) -> Level {
        w.house(house).expect("alive").level
    }

    fn satisfaction(w: &World, house: HouseId, k: ServiceKind) -> u8 {
        w.house(house).expect("alive").satisfaction[k.index()]
    }

    /// The worst of the accumulators the level cares about — the quantity both
    /// thresholds are compared against.
    fn worst(w: &World, house: HouseId) -> u8 {
        ServiceKind::ALL
            .iter()
            .map(|k| satisfaction(w, house, *k))
            .min()
            .unwrap_or(0)
    }

    /// Steps, with no commands, until the world is about to run `tick_no`.
    fn run_to(w: &mut World, tick_no: Tick) {
        while w.tick() < tick_no {
            tick(w, &[]);
        }
    }

    /// How many ticks of being served it takes to get from `from` up to
    /// `target`.
    fn ticks_up_to(w: &World, from: u8, target: u8) -> u32 {
        let s = &w.data().rules.satisfaction;
        u32::from(target.saturating_sub(from)).div_ceil(u32::from(s.step_up))
    }

    /// How many ticks without the service it takes to get from `from` strictly
    /// **below** `target`.
    fn ticks_down_below(w: &World, from: u8, target: u8) -> u32 {
        let s = &w.data().rules.satisfaction;
        (u32::from(from) + 1)
            .saturating_sub(u32::from(target))
            .div_ceil(u32::from(s.step_down))
    }

    /// The tick of the first review that can see a satisfaction reached after
    /// `steps` more ticks.
    ///
    /// The step for tick T is the `T - now + 1`-th, and 6.2 runs after 6.1
    /// inside it, so the earliest review that can act is the first month
    /// boundary at or after `now + steps - 1`.
    fn review_after(w: &World, steps: u32) -> Tick {
        let month = Calendar::TICKS_PER_MONTH;
        let ready = w.tick().get() + steps.saturating_sub(1);
        Tick::new(ready.div_ceil(month) * month)
    }

    fn threshold_up(w: &World, at: Level) -> u8 {
        w.data()
            .rules
            .house_level(at)
            .expect("the level exists")
            .level_up_threshold
    }

    fn threshold_decay(w: &World, at: Level) -> u8 {
        w.data()
            .rules
            .house_level(at)
            .expect("the level exists")
            .decay_threshold
    }

    // --- 1. levelling up: the phase's goal ---------------------------------

    /// A served house reaches level 2 at the **first review after** its
    /// satisfaction passes level 2's threshold — and not one review before.
    #[test]
    fn a_served_house_reaches_level_two_at_the_first_review_after_the_threshold() {
        let (mut w, house) = a_served_house();
        let target = threshold_up(&w, level(2));
        let review = review_after(&w, ticks_up_to(&w, worst(&w, house), target));

        run_to(&mut w, review);
        assert!(
            worst(&w, house) >= target,
            "the horizon is wrong: {} is still below {target}",
            worst(&w, house)
        );
        assert_eq!(
            house_level(&w, house),
            level(1),
            "no review before tick {review} may have promoted it"
        );

        let r = tick(&mut w, &[]);
        assert_eq!(
            house_level(&w, house),
            level(2),
            "the review at tick {review} promotes"
        );
        assert!(
            r.events.contains(&Event::HouseEvolved {
                house,
                from: level(1),
                to: level(2)
            }),
            "the promotion has to be announced: {:?}",
            r.events
        );
    }

    /// **One jump per review.** A house sitting at the maximum meets level 3's
    /// threshold as well as level 2's, and still climbs one level a month.
    #[test]
    fn a_house_at_the_maximum_still_climbs_one_level_per_review() {
        let (mut w, house) = a_served_house();
        let max = w.data().rules.satisfaction.max;
        let top = w.data().rules.top_house_level();
        assert!(
            top >= Some(level(3)),
            "the fixture needs three levels to say anything"
        );

        // Saturated well before the first review, so the only thing rationing
        // the climb is the cadence.
        let saturated = review_after(&w, ticks_up_to(&w, worst(&w, house), max));
        run_to(&mut w, saturated);
        assert_eq!(worst(&w, house), max);
        assert!(
            max >= threshold_up(&w, level(3)),
            "the fixture has to make level 3 reachable in one climb"
        );

        let month = Calendar::TICKS_PER_MONTH;
        tick(&mut w, &[]);
        assert_eq!(house_level(&w, house), level(2), "one level, not two");
        let eve = Tick::new(w.tick().get() + month - 1);
        run_to(&mut w, eve);
        assert_eq!(
            house_level(&w, house),
            level(2),
            "and nothing between the reviews"
        );
        tick(&mut w, &[]);
        assert_eq!(house_level(&w, house), level(3));
    }

    /// The level only ever changes on a month boundary. It is the property that
    /// makes a recording followable by eye.
    #[test]
    fn the_level_only_changes_at_a_review() {
        let (mut w, house) = a_served_house();
        let month = Calendar::TICKS_PER_MONTH;
        let mut previous = house_level(&w, house);

        for _ in 0..month * 4 {
            let at = w.tick();
            tick(&mut w, &[]);
            let now = house_level(&w, house);
            if now != previous {
                assert!(
                    Calendar::is_month_boundary(at),
                    "the level moved at tick {at}, off a review"
                );
                previous = now;
            }
        }
        assert!(previous > level(1), "the run has to have moved something");
    }

    // --- 2. decay -----------------------------------------------------------

    /// Take the well away and the house comes back down, at the first review
    /// after the water has fallen through level 2's floor.
    #[test]
    fn demolishing_the_well_sends_the_house_back_down_a_level() {
        let (mut w, house) = a_served_house();
        let saturated = review_after(&w, ticks_up_to(&w, worst(&w, house), 100));
        run_to(&mut w, saturated);
        tick(&mut w, &[]);
        assert_eq!(
            house_level(&w, house),
            level(2),
            "it has to be up before it can come down"
        );

        let r = tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);
        assert!(
            !w.house(house)
                .expect("alive")
                .served
                .get(ServiceKind::Water)
        );

        let floor = threshold_decay(&w, level(2));
        let water = satisfaction(&w, house, ServiceKind::Water);
        let review = review_after(&w, ticks_down_below(&w, water, floor));

        run_to(&mut w, review);
        assert!(satisfaction(&w, house, ServiceKind::Water) < floor);
        assert_eq!(
            house_level(&w, house),
            level(2),
            "no earlier review may have demoted it"
        );

        let r = tick(&mut w, &[]);
        assert_eq!(house_level(&w, house), level(1));
        assert!(
            r.events.contains(&Event::HouseDegraded {
                house,
                from: level(2),
                to: level(1)
            }),
            "a separate variant from HouseEvolved, so the renderer need not \
             compare numbers: {:?}",
            r.events
        );

        // And it stops there: level 1 has no level 0 to fall to, and a house
        // that empties out is phase 14's business.
        let two_months = Tick::new(w.tick().get() + Calendar::TICKS_PER_MONTH * 2);
        run_to(&mut w, two_months);
        assert_eq!(house_level(&w, house), level(1));
    }

    // --- 3. no oscillation: the test that defines the phase ------------------

    /// A well, a farm and a house on their own road, in the far corner of the
    /// map — out of reach of the generator below, which only ever touches
    /// `0..14`.
    ///
    /// It exists because a generated layout can perfectly well contain no
    /// served house at all, and a monotonicity test on a city where nothing
    /// ever moves is green for the wrong reason. This corner guarantees that
    /// every case has at least one house that really climbs.
    fn a_served_corner(w: &mut World) -> HouseId {
        let cells: Vec<(u8, u8)> = (20..=27).map(|x| (x, 20)).collect();
        roads(w, &cells);
        build(w, WELL, 21, 19);
        build(w, FARM, 23, 18); // (23,18)..(24,19), touches row 20
        build(w, HOUSE, 26, 21);
        w.houses()
            .find(|(_, h)| h.origin == pos(26, 21))
            .map(|(id, _)| id)
            .expect("the corner house")
    }

    fn any_command() -> impl Strategy<Value = Command> {
        prop_oneof![
            4 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceRoad { at: pos(x, y) }),
            3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
                kind: HOUSE,
                origin: pos(x, y),
            }),
            2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
                kind: WELL,
                origin: pos(x, y),
            }),
            1 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
                kind: FARM,
                origin: pos(x, y),
            }),
        ]
    }

    proptest! {
        /// **The** test of the phase: with the services held constant, every
        /// house's level is monotone over a whole year.
        ///
        /// It is the proof that the gap really works, not merely that
        /// validation imposes it — both are needed and neither replaces the
        /// other. Constant services means: the city is laid out, and after that
        /// no command is issued, so nothing but the levels themselves can move.
        /// If a house went up and then down, the only possible cause would be
        /// the review reading a value that the review before it had changed.
        ///
        /// **On the fixture with the demographics switched off**, which is what
        /// makes that sentence true rather than nearly true. A house emptied by
        /// deaths is served by nobody, so its satisfaction falls and its level
        /// follows — a real fall, with a cause that is neither the gap nor the
        /// review, and one that would make this test red for a reason it is not
        /// asking about. Deaths are the business of the demographics tests.
        #[test]
        fn the_level_shows_no_oscillation_with_constant_services(
            setup in prop::collection::vec(any_command(), 1..14)
        ) {
            let mut w = world_without_demographics();
            let corner = a_served_corner(&mut w);
            tick(&mut w, &setup);

            let mut seen: BTreeMap<HouseId, Level> =
                w.houses().map(|(id, h)| (id, h.level)).collect();
            let year = Calendar::TICKS_PER_YEAR;
            for _ in 0..year {
                tick(&mut w, &[]);
                for (id, h) in w.houses() {
                    let before = seen.entry(id).or_insert(h.level);
                    prop_assert!(
                        h.level >= *before,
                        "{id:?} fell from {} to {} at tick {}",
                        *before, h.level, w.tick()
                    );
                    *before = h.level;
                }
            }

            // And something really did move: monotone is trivially true of a
            // city that never changes.
            prop_assert_eq!(
                Some(house_level(&w, corner)),
                w.data().rules.top_house_level(),
                "the served corner has to have climbed all the way in a year"
            );
        }
    }

    // --- 4. levelling up does not touch the coverage ------------------

    /// A house that goes up a level changes neither the assignments nor the
    /// number of recomputations: it brought nobody in, so no provider's
    /// capacity moved.
    ///
    /// It is the test that pins that rule down where it is easiest to get
    /// wrong — whoever implements the per-level capacity will be tempted to use it in
    /// `coverage.rs` too.
    #[test]
    fn levelling_up_touches_neither_the_coverage_nor_its_recomputes() {
        let (mut w, house) = a_served_house();
        let saturated = review_after(&w, ticks_up_to(&w, worst(&w, house), 100));
        run_to(&mut w, saturated);

        let assignments = w.coverage().assignments().clone();
        let recomputes = w.coverage().recomputes();
        let population = w.population();

        tick(&mut w, &[]);
        assert_eq!(
            house_level(&w, house),
            level(2),
            "the review has to have acted"
        );

        assert_eq!(w.coverage().assignments(), &assignments);
        assert_eq!(
            w.coverage().recomputes(),
            recomputes,
            "levelling up must not invalidate the coverage"
        );
        assert_eq!(
            w.population(),
            population,
            "a level is permission, not people"
        );

        // And the tick after, when a pending invalidation would have surfaced.
        tick(&mut w, &[]);
        assert_eq!(w.coverage().recomputes(), recomputes);
    }

    // --- 5. the levels really do ask for different things --------------------

    /// On the service-levels fixture, level 1 wants water alone and level 2
    /// wants food as well: a house with a well and no farm climbs to the top of
    /// level 1 and stops there, however long it waits.
    #[test]
    fn a_level_that_asks_for_more_is_not_reached_on_the_service_below() {
        let mut w = world_with(dataset_with_service_levels(), 32, 32, EASY);
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);
        build(&mut w, HOUSE, 8, 5);
        let house = w.houses().next().map(|(id, _)| id).expect("the house");

        let a_year = Tick::new(Calendar::TICKS_PER_YEAR);
        run_to(&mut w, a_year);
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Water),
            w.data().rules.satisfaction.max,
            "the water is there and the accumulator is full"
        );
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Food),
            0,
            "and the food one has never moved"
        );
        assert_eq!(
            house_level(&w, house),
            level(1),
            "level 2 wants food, so twelve reviews change nothing"
        );

        // The farm arrives, and with it the level. The satisfaction has to be
        // earned from zero: what the house was receiving anyway was credited,
        // what it was not receiving was not.
        build(&mut w, FARM, 5, 2);
        let target = threshold_up(&w, level(2));
        let review = review_after(&w, ticks_up_to(&w, worst(&w, house), target));
        run_to(&mut w, review);
        assert_eq!(house_level(&w, house), level(1));
        tick(&mut w, &[]);
        assert_eq!(house_level(&w, house), level(2));
    }

    // --- 6. eviction ---------------------------------------------------------

    /// A house that comes down and no longer fits its own residents sends the
    /// difference away, and the difference is **counted**.
    ///
    /// The residents are put there by hand: under a dataset that passes
    /// validation no house can ever exceed the capacity it would fall back to —
    /// `CapacityNotIncreasing` sees to that, and residents only ever arrive at
    /// construction, capped at level 1. Phase 14 is what makes this branch
    /// reachable in play; the term is pinned down here because that phase's
    /// conservation depends on it.
    fn a_house_too_full_for_the_level_below() -> (World, HouseId) {
        let (mut w, house) = a_served_house();
        let saturated = review_after(&w, ticks_up_to(&w, worst(&w, house), 100));
        run_to(&mut w, saturated);
        tick(&mut w, &[]);
        assert_eq!(house_level(&w, house), level(2));

        let capacity = w.data().rules.max_residents(level(2)).expect("level 2");
        w.house_mut(house).expect("alive").residents = capacity;

        // Take the water away: the fall is what the eviction hangs off.
        let r = tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);
        let floor = threshold_decay(&w, level(2));
        let water = satisfaction(&w, house, ServiceKind::Water);
        let fallen = review_after(&w, ticks_down_below(&w, water, floor));
        run_to(&mut w, fallen);
        (w, house)
    }

    #[test]
    fn decay_evicts_whoever_no_longer_fits_and_counts_them() {
        let (mut w, house) = a_house_too_full_for_the_level_below();
        let before = w.house(house).expect("alive").residents;
        let evicted_before = w.population_totals().evicted;

        tick(&mut w, &[]);

        let capacity = w.data().rules.max_residents(level(1)).expect("level 1");
        assert_eq!(house_level(&w, house), level(1));
        assert_eq!(
            w.house(house).expect("alive").residents,
            capacity,
            "the house is full to its new ceiling and no fuller"
        );
        assert_eq!(
            w.population_totals().evicted - evicted_before,
            u64::from(before - capacity),
            "the difference is a flow of population, and phase 14 counts it"
        );
    }

    /// Coming down from a level the table does not contain evicts **nobody**.
    ///
    /// `decays()` sends a house at an unknown level down on purpose, because
    /// descending converges on a level that exists. But the level below can be
    /// off the table too, and reading its capacity as zero emptied the house in
    /// one review — a population wipe wearing the clothes of a convergence.
    ///
    /// Unreachable in play from three directions at once (`rises` is bounded by
    /// `top_house_level`, `residents_within_capacity` would flag a house at an
    /// off-table level, and levels only ever start at 1), which is why it takes
    /// `house_mut` to get here. It is written down because zero is a strange
    /// answer to "what does this level hold?" when the honest answer is "no
    /// idea", and the two differ by the whole population of the house.
    #[test]
    fn coming_down_from_a_level_off_the_table_evicts_nobody() {
        let (mut w, house) = a_served_house();
        let month = Calendar::TICKS_PER_MONTH;
        let top = w
            .data()
            .rules
            .top_house_level()
            .expect("the table is not empty");
        let full = w.data().rules.max_residents(top).expect("the top level");
        let above = top.next().expect("a level above the top");
        let two_above = above.next().expect("two levels above the top");
        assert!(
            w.data().rules.house_level(above).is_none(),
            "the table has to stop below the levels this test invents"
        );
        {
            let h = w.house_mut(house).expect("alive");
            h.level = two_above;
            h.residents = full;
        }
        let evicted_before = w.population_totals().evicted;

        // Two reviews: the first lands on a level that is still off the table,
        // the second on the top one, which exists and holds them all.
        let after_two_reviews = Tick::new((w.tick().get() / month + 2) * month + 1);
        run_to(&mut w, after_two_reviews);

        assert_eq!(
            house_level(&w, house),
            top,
            "it converges on a level that exists"
        );
        assert_eq!(
            w.house(house).expect("alive").residents,
            full,
            "and it still has everybody: the unknown level evicted nobody"
        );
        assert_eq!(w.population_totals().evicted, evicted_before);
    }

    /// Eviction is the only point in this phase where `residents` changes, so
    /// it is the only one that has to invalidate the coverage: capacity is
    /// counted on the residents present.
    #[test]
    fn eviction_invalidates_the_coverage() {
        let (mut w, house) = a_house_too_full_for_the_level_below();
        let recomputes = w.coverage().recomputes();

        tick(&mut w, &[]);
        assert_eq!(
            house_level(&w, house),
            level(1),
            "the eviction has to have happened"
        );
        assert_eq!(
            w.coverage().recomputes(),
            recomputes,
            "the invalidation is raised in step 6, after step 3 has run"
        );

        tick(&mut w, &[]);
        assert_eq!(
            w.coverage().recomputes(),
            recomputes + 1,
            "and the next tick has to act on it"
        );
    }

    // --- founding a house at a level, which is setup and not play ------------

    /// Founding a house at a level brings the residents with it.
    ///
    /// A level and an occupancy that could disagree would be two knobs where the
    /// game has one, so the hook fills the house to that level's capacity rather
    /// than leaving behind whoever happened to be living there.
    #[test]
    fn a_house_is_founded_full_for_the_level_it_is_founded_at() {
        let (mut w, house) = a_served_house();
        let capacity = w.data().rules.max_residents(level(2)).expect("level 2");
        let recomputes = w.coverage().recomputes();

        assert!(w.set_house_level(house, level(2)));
        assert_eq!(house_level(&w, house), level(2));
        assert_eq!(w.house(house).expect("alive").residents, capacity);

        tick(&mut w, &[]);
        assert_eq!(
            w.coverage().recomputes(),
            recomputes + 1,
            "a house that changed size has to be covered again"
        );
    }

    /// A level the tables do not have is refused, and refused without leaving
    /// the house half-founded.
    ///
    /// An answer and not a clamp: a caller asking for a level that does not
    /// exist has a bug, and quietly founding a different city than the one asked
    /// for is how a measurement gets credited to the wrong cause.
    #[test]
    fn founding_at_a_level_the_tables_do_not_have_changes_nothing() {
        let (mut w, house) = a_served_house();
        let before = w.house(house).expect("alive").clone();
        let past_the_end =
            u8::try_from(w.data().rules.house_levels.len() + 1).expect("a short table");

        assert!(!w.set_house_level(house, level(past_the_end)));
        assert_eq!(w.house(house).expect("alive"), &before);
    }
}
