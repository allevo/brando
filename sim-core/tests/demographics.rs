//! Phase 14 — births and deaths.
//!
//! Every horizon in here is **computed from the `DataSet`**, never written as a
//! number, for phase 07's reason: a hardcoded value breaks on every rebalancing
//! without signalling anything real.

mod common;

/// Wrapped in a module so the test names carry the phase's word: the
/// verification command filters on the **test name**, not on the file it lives
/// in — the trap phase 12 fell into and phase 13 recorded.
mod demographics {
    use super::common::*;
    use sim_core::{Command, Event, Flow, HouseId, RngKind, ServiceKind, World};

    // --- helpers ------------------------------------------------------------

    fn roads(w: &mut World, cells: &[(u8, u8)]) {
        let cmds: Vec<_> = cells
            .iter()
            .map(|(x, y)| Command::PlaceRoad { at: pos(*x, *y) })
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

    /// A well, a farm and `n` houses on one road: a city that is fully served
    /// and can therefore grow.
    fn a_served_city(n: u8) -> World {
        a_served_city_on(world(), n)
    }

    fn a_served_city_on(mut w: World, n: u8) -> World {
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);
        build(&mut w, FARM, 5, 2);
        for i in 0..n {
            build(&mut w, HOUSE, 8 + i, 5);
        }
        w
    }

    fn run(w: &mut World, ticks: u32) {
        for _ in 0..ticks {
            tick(w, &[]);
        }
    }

    /// Everyone who has ever arrived or left, flow by flow.
    fn flows(w: &World) -> (u64, u64, u64) {
        let t = w.population_totals();
        (t.born, t.died, t.evicted)
    }

    /// Every place in every house: the ceiling the population climbs towards.
    fn places(w: &World) -> u32 {
        w.houses()
            .map(|(_, h)| u32::from(w.data().rules.max_residents(h.level).unwrap_or(0)))
            .sum()
    }

    // --- 1. growth, and the plateau -----------------------------------------

    /// A served city grows — **and stops**.
    ///
    /// The plateau counts as much as the climb. Births are counted against the
    /// *eligible* residents, those in a house with room to spare, so a city
    /// whose houses are all full has nobody eligible and the rate falls to
    /// zero on its own. If growth were counted against the population instead,
    /// the ceiling would be an accident of `max_residents` clamping every birth
    /// and the numbers would go on being drawn for ever.
    #[test]
    fn a_served_city_grows_until_its_houses_are_full() {
        // **One house**, so the farm is not the binding constraint: its
        // capacity is twenty residents and a house at the top rung holds
        // twelve. With more houses the city hits the food capacity first, one
        // of them falls out of the coverage and decays, and the ceiling stops
        // being `places` — that is a real behaviour and
        // `the_coverage_follows_a_city_that_outgrows_it` is where it is
        // checked. Here the question is only whether growth stops on its own.
        let mut w = a_served_city(1);
        let start = w.population();

        let year = w.data().rules.ticks_per_year();
        run(&mut w, year);
        let grown = w.population();
        assert!(
            grown > start,
            "a fully served city has to grow: {start} -> {grown}"
        );

        // Long enough that any remaining eligible house would have filled.
        run(&mut w, year * 2);
        assert_eq!(
            w.population(),
            places(&w),
            "the population has to settle exactly on the places available"
        );
        assert!(
            w.population_totals().born > 0,
            "and it got there by births, not by construction"
        );
    }

    // --- 2. emptying out ----------------------------------------------------

    /// Take the food away and the city empties out, instead of merely stopping.
    ///
    /// It is the raised rate doing the work: a house below `unserved_threshold`
    /// on a service **its own level** requires dies at ten times the base rate.
    /// Without that channel a starving city would simply stop growing, which
    /// reads to the player as a plateau rather than as a disaster.
    #[test]
    fn a_city_that_loses_its_food_empties_out() {
        let mut w = a_served_city(3);
        let year = w.data().rules.ticks_per_year();
        run(&mut w, year);
        let peak = w.population();
        assert!(peak > 0);

        // The farm is at (5,2) and takes (5,2)..(6,3).
        let r = tick(&mut w, &[Command::Demolish { at: pos(5, 2) }]);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);

        run(&mut w, year * 2);
        assert!(
            w.population() < peak,
            "with no food the city has to shrink: {peak} -> {}",
            w.population()
        );
        assert!(
            w.population_totals().died > 0,
            "and it shrank by deaths, which is the channel that makes hunger \
             cost something"
        );
    }

    /// The last resident leaving is the one thing worth an event.
    #[test]
    fn a_house_that_empties_says_so_once() {
        let mut w = world_of(16, 16);
        roads(&mut w, &[(1, 4), (2, 4), (3, 4)]);
        build(&mut w, HOUSE, 2, 5);
        let house: HouseId = w.houses().next().map(|(id, _)| id).expect("the house");

        // No well and no farm: level 1 asks for water, so this house is going
        // without from the first tick and dies at the raised rate.
        let mut abandoned = 0;
        for _ in 0..w.data().rules.ticks_per_year() * 3 {
            let r = tick(&mut w, &[]);
            abandoned += r
                .events
                .iter()
                .filter(|e| matches!(e, Event::HouseAbandoned { house: h } if *h == house))
                .count();
            if w.house(house).expect("alive").residents == 0 {
                break;
            }
        }
        assert_eq!(
            w.house(house).expect("alive").residents,
            0,
            "an unserved house has to empty out"
        );
        assert_eq!(
            abandoned, 1,
            "once, on the tick it emptied — not every tick"
        );
    }

    // --- 6b. the draws are decided by the city ------------------------------

    /// A world with no houses draws nothing at all, whatever the seed.
    ///
    /// 6a's consequence seen through `step`, and the sharper companion of
    /// `commands.rs`'s assertion. Nothing to decide, nothing drawn.
    #[test]
    fn an_empty_city_draws_nothing() {
        for seed in [1u64, 7, 99] {
            let mut w = world_seeded(seed);
            run(&mut w, 50);
            for kind in RngKind::ALL {
                assert_eq!(w.rng().draws(kind), 0, "seed {seed}, {kind:?}");
            }
        }
    }

    /// A city whose rates are zero draws exactly one value per flow per tick,
    /// the same under every seed.
    ///
    /// This is what says the count is decided by the **city** and not by the
    /// values that come out of the generator. With rejection sampling it would
    /// vary with the seed, `draws` would stop being a function of the state,
    /// and `different_seeds_give_different_hashes` would pass without the
    /// demographics doing anything at all.
    #[test]
    fn a_still_city_draws_a_fixed_number_of_values() {
        const TICKS: u32 = 40;
        let mut counts = Vec::new();
        for seed in [1u64, 7, 99, 12345] {
            let mut w = a_served_city_on(world_seeded_without_demographics(seed), 2);
            let before = w.rng().draws(RngKind::Demographics);
            run(&mut w, TICKS);
            counts.push(w.rng().draws(RngKind::Demographics) - before);
        }

        // **The property is that the count does not depend on the seed.** The
        // phase file predicted the exact value `flows × ticks`, and that is an
        // artefact rather than the rule: births are eligible only in a house
        // with room to spare and a satisfaction above the threshold, so on the
        // early ticks — when every house is still full from construction and
        // its accumulators are still climbing — the births flow has nobody
        // eligible and takes no jitter at all. That is deliberate ("nothing is
        // drawn when there is nothing to draw for") and it is what makes the
        // count a readable function of the city. Asserting the predicted
        // constant would have meant asserting the opposite.
        assert!(
            counts.windows(2).all(|p| p[0] == p[1]),
            "the draw count has to be the same under every seed: {counts:?}"
        );
        assert!(
            counts[0] > 0 && counts[0] <= u64::from(TICKS) * 2,
            "and it has to be between one and two draws a tick: {}",
            counts[0]
        );
    }

    // --- 7. the jitter is symmetric -----------------------------------------

    /// The jitter is symmetric: over many seeds it does not move the mean.
    ///
    /// An asymmetric jitter would shift the whole balancing invisibly — every
    /// rate in the table would quietly mean something other than what it says,
    /// and no test of a trajectory would notice, because there is nothing to
    /// compare a trajectory against.
    ///
    /// Measured on **one tick under many seeds** rather than many ticks under
    /// one. A long run cannot serve: deaths change the population, the
    /// population is what the rate is multiplied by, and the quantity being
    /// averaged then moves for a second reason. One tick from an identical city
    /// isolates the jitter, which is the only thing in question.
    #[test]
    fn the_jitter_does_not_move_the_mean() {
        const SEEDS: u64 = 300;

        let mut total = 0i64;
        let mut residents = 0i64;
        for seed in 0..SEEDS {
            let mut w = a_served_city_on(world_seeded(seed), 2);
            // The city is identical for every seed at this point: the tick has
            // not run yet, so nothing has diverged.
            residents = i64::from(w.population());
            let divisor = i64::from(w.data().rules.ticks_per_month) * 1_000 * 1_000;
            // The **delta**, not the accumulator: building the city took five
            // ticks and each of them added its own numerator. And if a death
            // matured on this tick the accumulator wrapped, so the events it
            // paid for have to be added back — otherwise the measurement would
            // be of the modulo rather than of the jitter.
            let before = w.demographics().remainder(Flow::Deaths);
            let died_before = w.population_totals().died;
            tick(&mut w, &[]);
            let matured = i64::try_from(w.population_totals().died - died_before).expect("fits");
            total += w.demographics().remainder(Flow::Deaths) - before + matured * divisor;
        }

        // The **unserved** rate, and that is not a slip. The tick measured is
        // the one right after construction, when every accumulator is still
        // climbing from zero and so every house is below `unserved_threshold`
        // on a service its own level requires. Using the base rate here would
        // have been a test asserting the wrong number and passing only if the
        // jitter were broken in a compensating way.
        let d = a_served_city(2).data().rules.demographics.clone();
        let unjittered =
            residents * i64::from(d.deaths_per_thousand_per_month_when_unserved) * 1_000;
        let mean = total / i64::try_from(SEEDS).expect("fits");

        // Within one percent of the unjittered numerator. The jitter spans
        // +/-20%, so a mean this close cannot be an accident of the sample, and
        // a sign error or a truncated negative half would land far outside.
        let drift = (mean - unjittered).abs() * 100 / unjittered;
        assert!(
            drift <= 1,
            "the jitter moved the mean by {drift}%: mean {mean}, unjittered {unjittered}"
        );
    }

    // --- 8. plausible but never identical -----------------------------------

    /// Different seeds give different trajectories, all of them plausible.
    ///
    /// The operational definition of "a minimum of randomness, but not two
    /// identical games": without the first half the balancing is a lottery,
    /// without the second the RNG is of no use.
    #[test]
    fn many_seeds_give_different_but_plausible_trajectories() {
        let year = 360u32;
        let mut finals = Vec::new();
        for seed in 0..40u64 {
            let mut w = a_served_city_on(world_seeded(seed), 3);
            run(&mut w, year);
            finals.push(w.population());
        }

        let low = *finals.iter().min().expect("40 runs");
        let high = *finals.iter().max().expect("40 runs");
        assert!(
            finals.iter().any(|p| *p != finals[0]),
            "forty seeds cannot all give the same population: {finals:?}"
        );
        assert!(
            low * 2 >= high,
            "the band is too wide to be balancing rather than a lottery: \
             {low}..{high}"
        );
    }

    // --- 10. demolition counts what it destroys -----------------------------

    /// Demolishing an inhabited house counts its residents.
    ///
    /// The same test as `demolishing_a_farm_records_the_stock_it_loses`, and
    /// for the same reason: without the term, conservation stops being an
    /// equality and reports a bug that is not there.
    #[test]
    fn demolishing_a_house_records_the_residents_it_loses() {
        let mut w = a_served_city(1);
        let house = w.houses().next().map(|(id, _)| id).expect("the house");
        let residents = u64::from(w.house(house).expect("alive").residents);
        assert!(residents > 0, "the house has to have somebody in it");
        let before = w.population_totals().lost_to_demolition;

        let r = tick(&mut w, &[Command::Demolish { at: pos(8, 5) }]);
        assert!(r.rejected.is_empty(), "{:?}", r.rejected);
        assert_eq!(w.population_totals().lost_to_demolition - before, residents);
    }

    // --- 12. the invalidation contract (A12) --------------------------------

    /// If anybody moved, the coverage is dirty at the end of the tick — and if
    /// nobody moved, it is not.
    ///
    /// The converse is the half that matters: it is what says the invalidation
    /// is conditional and not an unconditional `mark_all_providers_dirty`
    /// dressed up as one. Without it, a tick that recomputed the coverage
    /// unconditionally would pass just as well, and A12's price would be paid
    /// on every tick of every game instead of only on the ticks that need it.
    #[test]
    fn demographics_invalidate_the_coverage_and_only_then() {
        let mut w = a_served_city(2);
        let mut saw_a_move = false;
        let mut saw_a_still_tick = false;

        for _ in 0..w.data().rules.ticks_per_year() {
            // The **flows**, not the net balance. A birth and a death in the
            // same tick leave the balance where it was and still move two
            // people between houses, and the coverage is decided by each
            // house's residents rather than by the city's total — so comparing
            // the net would call that tick still and then find it dirty. It is
            // the same reason `demographics::run` returns a flag instead of
            // letting the caller compare populations.
            let before = flows(&w);
            tick(&mut w, &[]);
            let moved = flows(&w) != before;
            let dirty = w.dirty().coverage_needs_recompute();

            if moved {
                saw_a_move = true;
                assert!(dirty, "somebody moved and the coverage was left clean");
            } else {
                saw_a_still_tick = true;
                assert!(!dirty, "nobody moved and the coverage was dirtied anyway");
            }
        }
        assert!(
            saw_a_move,
            "the run has to contain a tick where somebody moved"
        );
        assert!(
            saw_a_still_tick,
            "and one where nobody did, or the converse is vacuous"
        );
    }

    // --- 14. the city outgrows its services ---------------------------------

    /// A house that grows past what the provider can serve falls out of the
    /// coverage on the next tick, and *covered ⇒ eats* stays true.
    ///
    /// A12's game loop observed at its smallest: the services chase the
    /// population, so the city can outgrow them, and the player's signal to
    /// build is that somebody has stopped being served.
    #[test]
    fn the_coverage_follows_a_city_that_outgrows_it() {
        // Counted in **residents**, not in houses. Houses would be the wrong
        // unit twice over: the capacity is in residents (A12), and an emptied
        // house weighs nothing and goes on being served for free (A18), so the
        // count of served houses can go *up* while the city outgrows its farm.
        // That is not a bug in the coverage, it is the thing A18 is about, and
        // measuring in houses here would have quietly hidden it.
        fn residents_served(w: &World) -> u16 {
            w.houses()
                .filter(|(_, h)| h.served.get(ServiceKind::Food))
                .map(|(_, h)| h.residents)
                .sum()
        }

        let mut w = a_served_city(3);
        let year = w.data().rules.ticks_per_year();

        assert_eq!(
            u32::from(residents_served(&w)),
            w.population(),
            "while the houses are newly built the farm reaches everybody"
        );

        run(&mut w, year);
        assert!(
            u32::from(residents_served(&w)) < w.population(),
            "as the houses fill, somebody has to fall outside the farm's \
             capacity: {} served out of {}",
            residents_served(&w),
            w.population()
        );
        assert_eq!(
            w.food().covered_but_unfed,
            0,
            "and everyone the farm did keep, it fed: covered still means fed"
        );
    }

    // --- 15. what this phase hands to A18 -----------------------------------

    /// A house emptied by deaths keeps its coverage and goes on consuming no
    /// capacity.
    ///
    /// **Written to change its outcome, not to break.** A18 — an empty house
    /// consumes no provider capacity — was recorded about `hard`, where a house
    /// is *born* empty. Deaths make zero residents reachable on every profile,
    /// `easy` included, so the behaviour is now inside the committed
    /// recordings. This states it as it is today; when phase 15 closes A18, the
    /// assertion below is what has to be flipped, deliberately, rather than
    /// discovered.
    #[test]
    fn an_emptied_house_still_consumes_no_capacity() {
        let mut w = world_of(16, 16);
        roads(&mut w, &[(1, 4), (2, 4), (3, 4), (4, 4), (5, 4)]);
        build(&mut w, SMALL_WELL, 2, 3);
        for i in 0..3 {
            build(&mut w, HOUSE, 2 + i, 5);
        }
        let small_well = w.buildings().next().map(|(id, _)| id).expect("the well");

        // No farm: level 1 asks for water only, so these houses are served and
        // will not die of hunger. Empty one by hand — deaths are what would do
        // it in a longer game, and this keeps the test about the capacity.
        let victim = w.houses().next().map(|(id, _)| id).expect("a house");
        w.house_mut(victim).expect("alive").residents = 0;
        tick(&mut w, &[]);
        tick(&mut w, &[]);

        assert_eq!(w.house(victim).expect("alive").residents, 0);
        assert!(
            w.coverage().houses_served_by(small_well).contains(&victim),
            "an emptied house keeps its coverage, and weighs nothing while it \
             has it — that is exactly what A18 is about"
        );
    }
}
