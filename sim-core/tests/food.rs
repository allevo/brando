//! Phase 07 — production, consumption and food conservation.

mod common;

use common::*;
use proptest::prelude::*;
use sim_core::{Command, Event, Milli, ServiceKind, TilePos, World};

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

/// Conservation, as an exact equality.
fn conservation(w: &World) -> Result<(), String> {
    let t = w.food();
    let stock = w.total_stock();
    if t.expected_stock() != stock {
        return Err(format!(
            "produced {} != consumed {} + lost_to_full_stock {} + lost_to_demolition {} \
             + stock {stock}",
            t.produced, t.consumed, t.lost_to_full_stock, t.lost_to_demolition
        ));
    }
    Ok(())
}

/// Stocks always between zero and the maximum declared in the table.
fn stocks_in_range(w: &World) -> Result<(), String> {
    for (id, b) in w.buildings() {
        let Some(def) = w.data().def(b.kind) else {
            continue;
        };
        let Some(max) = def.max_stock else {
            continue;
        };
        if b.stock.is_negative() {
            return Err(format!("{id:?} has a negative stock: {}", b.stock));
        }
        if b.stock > max {
            return Err(format!("{id:?} exceeds its maximum stock: {}", b.stock));
        }
    }
    Ok(())
}

/// How much food one house eats in one tick, read from the `DataSet`.
fn food_per_house(w: &World) -> Milli {
    let residents = i32::from(w.data().rules.residents_per_house_level[0]);
    w.data()
        .rules
        .food_per_resident
        .checked_mul_int(residents)
        .expect("one house's consumption")
}

/// A farm at (2,2) facing a horizontal road, plus `n` houses.
fn farm_scenario(n: u8) -> World {
    let mut w = world();
    let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
    roads(&mut w, &cells);
    build(&mut w, FARM, 2, 2); // takes (2,2)..(3,3), touches row 4
    for i in 0..n {
        build(&mut w, HOUSE, 5 + i, 5);
    }
    w
}

// --- 5. saturation ----------------------------------------------------------

#[test]
fn with_no_houses_the_stock_grows_to_the_maximum_and_stops() {
    let mut w = farm_scenario(0);
    let (id, _) = w.buildings().next().expect("the farm");
    let def = w
        .data()
        .def(w.building(id).expect("alive").kind)
        .expect("def");
    let max = def.max_stock.expect("maximum stock");
    let per_tick = def.output_per_tick.expect("output");

    // The ticks still needed to fill the granary, computed from the data and
    // the current stock: the farm already produces on the tick it is born, so
    // starting from zero would be off by one.
    let missing = max.to_millis() - w.building(id).expect("alive").stock.to_millis();
    let needed = missing / per_tick.to_millis();
    for _ in 0..needed {
        tick(&mut w, &[]);
    }
    assert_eq!(w.building(id).expect("alive").stock, max);
    assert_eq!(
        w.food().lost_to_full_stock,
        0,
        "nothing is lost until it is full"
    );

    for _ in 0..5 {
        tick(&mut w, &[]);
    }
    assert_eq!(
        w.building(id).expect("alive").stock,
        max,
        "the stock stops at the maximum"
    );
    assert_eq!(
        w.food().lost_to_full_stock,
        i64::from(per_tick.to_millis()) * 5,
        "the rest is lost, and the totals record it"
    );
    conservation(&w).expect("conservation");
}

// --- 3, 4. hunger and recovery ----------------------------------------------

/// More houses than the farm covers: the ones in excess stay **outside the
/// coverage**, and those inside it eat their fill, forever.
///
/// This is how hunger exists in M0. The farm declares the capacity its output
/// sustains, so it cannot take on houses it will then fail to feed: hunger is
/// lack of coverage, not a deficit inside the coverage.
#[test]
fn houses_beyond_the_capacity_stay_uncovered_and_the_others_eat() {
    let mut w = farm_scenario(6);
    let (id, _) = w.buildings().next().expect("the farm");
    let def = w
        .data()
        .def(w.building(id).expect("alive").kind)
        .expect("def");
    let per_tick = def.output_per_tick.expect("output");
    let per_house = food_per_house(&w);

    let demand = per_house
        .checked_mul_int(w.house_count() as i32)
        .expect("demand");
    assert!(
        demand > per_tick,
        "the scenario has to ask for more than the farm produces: \
         {demand} <= {per_tick}"
    );

    // Run long enough to use up any stock built up along the way.
    for _ in 0..200 {
        tick(&mut w, &[]);
        conservation(&w).expect("conservation");
        stocks_in_range(&w).expect("stocks in range");
    }

    let covered = usize::from(FARM_CAPACITY / RESIDENTS_PER_HOUSE);
    let fed = w
        .houses()
        .filter(|(_, h)| h.served.get(ServiceKind::Food))
        .count();
    assert_eq!(fed, covered, "the covered houses, and only those, eat");
    assert_eq!(
        w.house_count() - fed,
        6 - covered,
        "the ones in excess go without food"
    );
}

/// The recovery M0 knows how to do: a second farm covers the houses the first
/// one left outside its capacity, and they start eating again.
#[test]
fn a_second_farm_covers_the_houses_left_out() {
    // Eight houses; the farm covers as many as fit in its capacity.
    let covered = FARM_CAPACITY / RESIDENTS_PER_HOUSE;
    let mut w = farm_scenario(8);
    for _ in 0..200 {
        tick(&mut w, &[]);
    }
    let uncovered: Vec<_> = w
        .houses()
        .filter(|(id, _)| !w.coverage().is_served(*id, ServiceKind::Food))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        uncovered.len(),
        8 - usize::from(covered),
        "eight houses, the farm covers {covered} of them"
    );

    build(&mut w, FARM, 12, 2);
    for _ in 0..50 {
        tick(&mut w, &[]);
    }

    for id in uncovered {
        assert!(
            w.coverage().is_served(id, ServiceKind::Food),
            "{id:?} should have been picked up by the second farm"
        );
        assert!(
            w.house(id).expect("alive").served.get(ServiceKind::Food),
            "{id:?} should also have managed to eat"
        );
    }
    conservation(&w).expect("conservation");
}

/// **Hunger is no longer an absorbing state.** This test used to have the
/// opposite outcome: in M0 a house assigned to a farm running a deficit never
/// ate again, and not even a second farm could take it over — the first
/// provider wins a contest (phase 06), and the place stayed occupied by someone
/// who could not eat.
///
/// Counting the capacity in residents instead of houses was not enough to
/// dissolve it: that just restates the same constraint in another unit. What
/// was needed was for the declared capacity to be consistent with what the
/// output sustains, and from that it follows that a **covered house always
/// eats**: whoever goes without food is only whoever the coverage does not
/// reach, and that is recoverable by building.
///
/// The general form of the implication is in
/// `invariants.rs::covered_means_fed`; here we look at the concrete case it
/// emerged from.
#[test]
fn hunger_is_cured_by_building_a_second_farm() {
    let mut w = farm_scenario(6);
    for _ in 0..200 {
        tick(&mut w, &[]);
    }
    let hungry: Vec<_> = w
        .houses()
        .filter(|(_, h)| !h.served.get(ServiceKind::Food))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        hungry.len(),
        6 - usize::from(FARM_CAPACITY / RESIDENTS_PER_HOUSE),
        "six houses, the farm covers and feeds five of them"
    );
    for id in &hungry {
        assert!(
            !w.coverage().is_served(*id, ServiceKind::Food),
            "{id:?} does not eat because it is uncovered, not because of a deficit"
        );
    }

    build(&mut w, FARM, 12, 2);
    for _ in 0..50 {
        tick(&mut w, &[]);
    }

    assert_eq!(
        w.houses()
            .filter(|(_, h)| !h.served.get(ServiceKind::Food))
            .count(),
        0,
        "the second farm takes over the houses the first did not cover"
    );
    conservation(&w).expect("conservation");
}

// --- 6. deterministic order -------------------------------------------------

/// With more houses than the provider covers, it is always the same ones that
/// eat: the ones the assignment order puts first. Repeated, because a
/// non-deterministic order would pass every so often.
#[test]
fn with_scarce_capacity_the_same_houses_always_eat() {
    let mut expected: Option<Vec<bool>> = None;
    for _ in 0..100 {
        let mut w = farm_scenario(6);
        for _ in 0..200 {
            tick(&mut w, &[]);
        }
        let who: Vec<bool> = w
            .houses()
            .map(|(_, h)| h.served.get(ServiceKind::Food))
            .collect();
        match &expected {
            None => expected = Some(who),
            Some(a) => assert_eq!(*a, who, "the set of who eats has to be stable"),
        }
    }
}

// --- 7. events --------------------------------------------------------------

/// `ServiceCoverageChanged` is a delta: emitted on the transition, not on every
/// tick of hunger.
#[test]
fn the_coverage_event_is_a_delta_not_polling() {
    let mut w = world();
    let cells: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
    roads(&mut w, &cells);
    build(&mut w, WELL, 2, 3);

    // The house is born and gets served: one event.
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: HOUSE,
            origin: pos(5, 5),
        }],
    );
    let house = w.houses().next().map(|(id, _)| id).expect("the house");
    let water_on = r
        .events
        .iter()
        .filter(|e| {
            matches!(e, Event::ServiceCoverageChanged { house: h, service, served }
                if *h == house && *service == ServiceKind::Water && *served)
        })
        .count();
    assert_eq!(water_on, 1);

    // Ten empty ticks: no events, the state does not change.
    for _ in 0..10 {
        let r = tick(&mut w, &[]);
        assert!(
            !r.events
                .iter()
                .any(|e| matches!(e, Event::ServiceCoverageChanged { .. })),
            "no change of state, no event"
        );
    }

    // The well demolished: one event of the opposite sign, exactly once.
    let r = tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
    let water_off = r
        .events
        .iter()
        .filter(|e| {
            matches!(e, Event::ServiceCoverageChanged { house: h, service, served }
                if *h == house && *service == ServiceKind::Water && !*served)
        })
        .count();
    assert_eq!(water_off, 1);

    for _ in 0..10 {
        let r = tick(&mut w, &[]);
        assert!(
            !r.events
                .iter()
                .any(|e| matches!(e, Event::ServiceCoverageChanged { .. })),
            "uncovered and that is that: it is not re-emitted every tick"
        );
    }
}

// --- 1, 2. the property tests that close the phase --------------------------

fn any_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        4 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: HOUSE,
            origin: TilePos::new(x, y),
        }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: FARM,
            origin: TilePos::new(x, y),
        }),
        1 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: WELL,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

proptest! {
    /// Conservation: `produced == consumed + lost + stock`, after any sequence
    /// of commands and any number of ticks.
    ///
    /// An exact equality, not an inequality: that is what makes the test able
    /// to find a bug instead of merely reassuring.
    #[test]
    fn food_is_conserved(p in prop::collection::vec(
        prop::collection::vec(any_command(), 0..4), 1..12)
    ) {
        let mut w = world();
        for cmds in &p {
            tick(&mut w, cmds);
            // A few empty ticks, to let the production run.
            for _ in 0..3 {
                tick(&mut w, &[]);
            }
            if let Err(e) = conservation(&w) {
                return Err(TestCaseError::fail(e));
            }
            if let Err(e) = stocks_in_range(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}

/// Demolishing a farm throws away its stock, and the totals record it: without
/// the `lost_to_demolition` term the conservation equality would break, and the
/// phase's most important test would report a bug that is not there.
#[test]
fn demolishing_a_farm_records_the_stock_it_loses() {
    let mut w = farm_scenario(0);
    for _ in 0..10 {
        tick(&mut w, &[]);
    }
    let stock = w.total_stock();
    assert!(stock > 0);
    conservation(&w).expect("conservation while the farm is alive");

    tick(&mut w, &[Command::Demolish { at: pos(2, 2) }]);

    assert_eq!(w.total_stock(), 0);
    assert_eq!(w.food().lost_to_demolition, stock);
    conservation(&w).expect("conservation after the demolition too");
}
