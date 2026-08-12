//! Phase 12 — house satisfaction.
//!
//! Every horizon in here is **computed from the `DataSet`**, never written as a
//! number: it is the lesson of phase 07's test 3, where a hardcoded value broke
//! on every rebalancing without signalling anything real.

mod common;

/// Wrapped in a module so the test names carry the phase's word: the
/// verification command is `cargo test -p sim-core satisfaction`, and cargo
/// filters on the **test name**, not on the file it lives in.
mod satisfaction {
    use super::common::*;
    use sim_core::{BuildingKindId, Command, Event, HouseId, Mood, ServiceKind, World};

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
    fn a_served_house() -> (World, HouseId) {
        let mut w = world();
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);
        build(&mut w, FARM, 5, 2); // takes (5,2)..(6,3), touches row 4
        build(&mut w, HOUSE, 8, 5);

        let house = w.houses().next().map(|(id, _)| id).expect("the house");
        assert!(
            w.house(house)
                .expect("alive")
                .served
                .get(ServiceKind::Water)
                && w.house(house).expect("alive").served.get(ServiceKind::Food),
            "the fixture has to start from a house served by both"
        );
        (w, house)
    }

    fn satisfaction(w: &World, house: HouseId, k: ServiceKind) -> u8 {
        w.house(house).expect("alive").satisfaction[k.index()]
    }

    /// How many more ticks it takes to get from the current value to the target,
    /// read from the `DataSet`.
    ///
    /// The current value and not zero: the house is already covered on the tick it
    /// is built, so step 6.1 has credited it once before the test looks. It is the
    /// same correction as `food.rs`'s "the farm already produces on the tick it is
    /// born".
    fn ticks_to_climb(w: &World, from: u8) -> u32 {
        let r = &w.data().rules.satisfaction;
        u32::from(r.max - from).div_ceil(u32::from(r.step_up))
    }

    // --- 1. going up: the phase's goal ------------------------------------------

    #[test]
    fn a_served_house_reaches_the_maximum_in_max_over_step_up_ticks() {
        let (mut w, house) = a_served_house();
        let max = w.data().rules.satisfaction.max;

        let needed = ticks_to_climb(&w, satisfaction(&w, house, ServiceKind::Water));
        assert!(needed > 1, "the climb has to take more than one tick");

        // One tick short of the horizon it must **not** be there yet: without this
        // half, a satisfaction that jumped straight to the maximum would pass.
        for _ in 0..needed - 1 {
            tick(&mut w, &[]);
        }
        assert!(
            satisfaction(&w, house, ServiceKind::Water) < max,
            "it arrives one tick early: the curve is not the one in the table"
        );

        tick(&mut w, &[]);
        for k in ServiceKind::ALL {
            assert_eq!(
                satisfaction(&w, house, k),
                max,
                "{k:?} has not reached the maximum in the ticks the DataSet says"
            );
        }

        // And it stops there: the saturation is game semantics, not a defence
        // against overflow.
        for _ in 0..50 {
            tick(&mut w, &[]);
        }
        for k in ServiceKind::ALL {
            assert_eq!(satisfaction(&w, house, k), max, "{k:?} past the maximum");
        }
    }

    // --- 2. coming down ---------------------------------------------------------

    /// With the well demolished the water satisfaction goes back to zero in
    /// `max / step_down` ticks, **and the food one stays at the maximum**.
    ///
    /// The second half is the one that counts: it pins down that the accumulators
    /// are independent.
    #[test]
    fn losing_the_water_empties_only_the_water_accumulator() {
        let (mut w, house) = a_served_house();
        let max = w.data().rules.satisfaction.max;
        let step_down = w.data().rules.satisfaction.step_down;

        let climb = ticks_to_climb(&w, satisfaction(&w, house, ServiceKind::Water));
        for _ in 0..climb {
            tick(&mut w, &[]);
        }
        assert_eq!(satisfaction(&w, house, ServiceKind::Water), max);

        tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
        assert!(
            !w.house(house)
                .expect("alive")
                .served
                .get(ServiceKind::Water),
            "the well is gone: the house is uncovered"
        );
        // The demolition tick has already applied one step down.
        let after_demolition = satisfaction(&w, house, ServiceKind::Water);
        assert_eq!(after_demolition, max - step_down);

        let falling = u32::from(after_demolition).div_ceil(u32::from(step_down));
        for _ in 0..falling - 1 {
            tick(&mut w, &[]);
        }
        assert!(
            satisfaction(&w, house, ServiceKind::Water) > 0,
            "it empties one tick early"
        );

        tick(&mut w, &[]);
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Water),
            0,
            "the water has not emptied in the ticks the DataSet says"
        );
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Food),
            max,
            "the food accumulator must not have moved: they are independent"
        );

        // And it stops at zero.
        for _ in 0..10 {
            tick(&mut w, &[]);
        }
        assert_eq!(satisfaction(&w, house, ServiceKind::Water), 0);
    }

    // --- 4. a new house starts at zero ------------------------------------------

    /// It is born uncovered — step 3 covers it in the same tick, but the
    /// accumulator starts from zero anyway — so its first levelling up costs the
    /// full time.
    #[test]
    fn a_new_house_starts_from_zero_even_though_it_is_covered_at_once() {
        let mut w = world();
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);

        // An old house, already at the maximum.
        build(&mut w, HOUSE, 8, 5);
        let old = w.houses().next().map(|(id, _)| id).expect("the house");
        let climb = ticks_to_climb(&w, satisfaction(&w, old, ServiceKind::Water));
        for _ in 0..climb {
            tick(&mut w, &[]);
        }
        let max = w.data().rules.satisfaction.max;
        let step_up = w.data().rules.satisfaction.step_up;
        assert_eq!(satisfaction(&w, old, ServiceKind::Water), max);

        // A new one next door, covered by the same well on the tick it is built.
        build(&mut w, HOUSE, 9, 5);
        let new = w
            .houses()
            .map(|(id, _)| id)
            .find(|id| *id != old)
            .expect("the new house");

        assert!(
            w.house(new).expect("alive").served.get(ServiceKind::Water),
            "the well covers it at once"
        );
        assert_eq!(
            satisfaction(&w, new, ServiceKind::Water),
            step_up,
            "one tick of credit, not the maximum its neighbour has"
        );
        assert_eq!(
            satisfaction(&w, old, ServiceKind::Water),
            max,
            "and the neighbour has not lost anything"
        );
    }

    // --- 5. services the level does not require ---------------------------------

    /// A service outside `required_services` is not touched, even when the coverage
    /// reaches the house with it anyway.
    ///
    /// Coverage does not read `required_services` (A9): the farm assigns itself to
    /// this house all the same, `served` says food is there, and the accumulator
    /// still has to stay still.
    #[test]
    fn a_service_the_house_does_not_require_is_left_alone() {
        let data = dataset_where_a_house_requires(&[ServiceKind::Water]);
        let mut w = world_with(data, 32, 32, EASY);
        let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
        roads(&mut w, &cells);
        build(&mut w, WELL, 2, 3);
        build(&mut w, FARM, 5, 2);
        build(&mut w, HOUSE, 8, 5);

        let house = w.houses().next().map(|(id, _)| id).expect("the house");
        assert!(
            w.house(house).expect("alive").served.get(ServiceKind::Food),
            "the farm covers it anyway: that is the whole point of the case"
        );

        for _ in 0..40 {
            tick(&mut w, &[]);
        }
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Water),
            w.data().rules.satisfaction.max,
            "the required service accumulates"
        );
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Food),
            0,
            "the one the level does not require does not"
        );

        // And the mood follows the required services only: this house is thriving
        // on water alone.
        let m = mood(&w, house);
        assert_eq!(m, Mood::Thriving);
    }

    // --- 6. the event fires on the band, not on the value -----------------------

    /// Since phase 13 the mood reads the services the house's **own level**
    /// requires, so there is no list to pass in: `mood_of` looks it up.
    fn mood(w: &World, house: HouseId) -> Mood {
        sim_core::satisfaction::mood_of(w.house(house).expect("alive"), &w.data().rules)
    }

    #[test]
    fn the_mood_event_fires_on_the_band_not_on_the_value() {
        let (mut w, house) = a_served_house();
        let rules = w.data().rules.satisfaction.clone();

        // A newborn house emits nothing: it is Desperate, which is what the
        // renderer assumes from HousePlaced.
        assert_eq!(mood(&w, house), Mood::Desperate);

        let mut moods = Vec::new();
        let mut ticks_without_event = 0usize;
        let climb = ticks_to_climb(&w, satisfaction(&w, house, ServiceKind::Water));
        for _ in 0..climb {
            let value_before = satisfaction(&w, house, ServiceKind::Water);
            let r = tick(&mut w, &[]);
            let emitted: Vec<Mood> = r
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::HouseMoodChanged { house: h, mood } if *h == house => Some(*mood),
                    _ => None,
                })
                .collect();

            let band_before = Mood::of(value_before, &rules);
            let band_now = Mood::of(satisfaction(&w, house, ServiceKind::Water), &rules);
            if band_before == band_now {
                assert!(
                    emitted.is_empty(),
                    "no band crossed and yet an event was emitted: {emitted:?}"
                );
                ticks_without_event += 1;
            } else {
                assert_eq!(
                    emitted,
                    vec![band_now],
                    "a band crossed has to emit exactly once"
                );
                moods.push(band_now);
            }
        }

        assert_eq!(
            moods,
            vec![Mood::Unhappy, Mood::Happy, Mood::Thriving],
            "the bands are crossed in order, one event each"
        );
        assert!(
            ticks_without_event > moods.len(),
            "most ticks move the value without crossing a band: that is the point"
        );

        // At the maximum the value stops moving, so nothing else is emitted.
        for _ in 0..20 {
            let r = tick(&mut w, &[]);
            assert!(
                !r.events
                    .iter()
                    .any(|e| matches!(e, Event::HouseMoodChanged { .. })),
                "nothing changes and yet it is re-emitted every tick"
            );
        }
    }

    /// The mood is the **worst** of the required services, so losing the water
    /// drags a house that is still eating all the way down.
    #[test]
    fn the_mood_follows_the_worst_service() {
        let (mut w, house) = a_served_house();
        let climb = ticks_to_climb(&w, satisfaction(&w, house, ServiceKind::Water));
        for _ in 0..climb {
            tick(&mut w, &[]);
        }
        assert_eq!(mood(&w, house), Mood::Thriving);

        let r = tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
        let step_down = w.data().rules.satisfaction.step_down;
        let max = w.data().rules.satisfaction.max;
        // The one step down of the demolition tick is not yet enough to leave the
        // top band, so nothing has been emitted yet.
        assert_eq!(
            Mood::of(max - step_down, &w.data().rules.satisfaction),
            Mood::Thriving
        );
        assert!(
            !r.events
                .iter()
                .any(|e| matches!(e, Event::HouseMoodChanged { .. }))
        );

        let mut seen = Vec::new();
        for _ in 0..20 {
            let r = tick(&mut w, &[]);
            for e in &r.events {
                if let Event::HouseMoodChanged { house: h, mood } = e {
                    assert_eq!(*h, house);
                    seen.push(*mood);
                }
            }
        }
        assert_eq!(
            seen,
            vec![Mood::Happy, Mood::Unhappy, Mood::Desperate],
            "the bands come back down in order"
        );
        assert_eq!(
            satisfaction(&w, house, ServiceKind::Food),
            max,
            "and it is still eating the whole time"
        );
    }
}
