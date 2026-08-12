//! Table tests: the production fixture loads, the broken ones produce a
//! **complete** report, and the hash tells a balance change apart from a
//! reformatting.

use std::path::Path;

use sim_core::data::Inconsistency;
use sim_core::{Coins, Level, Milli, ServiceKind, Terrain};

/// A level from its number, so a test can go on saying "level 2".
fn level(number: u8) -> Level {
    Level::new(number).expect("levels count from 1")
}
use sim_data::{DataSet, LoadError, ValidationErrorKind};

fn valid_rules() -> String {
    read_data("rules.ron")
}

fn valid_terrain() -> String {
    read_data("terrain.ron")
}

fn valid_buildings() -> String {
    read_data("buildings.ron")
}

fn valid_difficulty() -> String {
    read_data("difficulty.ron")
}

fn read_data(name: &str) -> String {
    let p = sim_data::production_data_dir().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn broken_fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/broken")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Loads the valid tables, replacing only `buildings.ron`.
fn with_buildings(buildings: &str) -> Result<DataSet, LoadError> {
    sim_data::from_ron_str(
        &valid_rules(),
        &valid_terrain(),
        buildings,
        &valid_difficulty(),
    )
}

/// Loads the valid tables, replacing only `rules.ron`.
fn with_rules(rules: &str) -> Result<DataSet, LoadError> {
    sim_data::from_ron_str(
        rules,
        &valid_terrain(),
        &valid_buildings(),
        &valid_difficulty(),
    )
}

/// Loads the valid tables, replacing only `difficulty.ron`.
fn with_difficulty(difficulty: &str) -> Result<DataSet, LoadError> {
    sim_data::from_ron_str(
        &valid_rules(),
        &valid_terrain(),
        &valid_buildings(),
        difficulty,
    )
}

// --- 1. the production fixture ---

#[test]
fn the_production_tables_load() {
    let d = sim_data::load_default().expect("the production tables must load");

    assert!(d.rules.ticks_per_month >= 1);
    assert_eq!(d.rules.ticks_per_year(), d.rules.ticks_per_month * 12);
    assert_eq!(d.rules.starting_treasury, Coins::new(1000));

    for t in Terrain::ALL {
        assert!(d.terrain(t).is_some(), "terrain {t:?} missing");
    }

    let house = d.kind_by_id("house").expect("the house exists");
    let well = d.kind_by_id("well").expect("the well exists");
    let farm = d.kind_by_id("farm").expect("the farm exists");
    assert_eq!(d.kind_by_id("pyramid"), None);

    let house = d.def(house).expect("the house's def");
    assert!(house.is_house());
    assert!(!house.is_producer());
    assert_eq!(
        house.required_services,
        [ServiceKind::Water, ServiceKind::Food]
    );

    let well = d.def(well).expect("the well's def");
    let s = well.service.as_ref().expect("the well provides a service");
    assert_eq!(s.kind, ServiceKind::Water);
    assert_eq!(s.range(Level::FIRST), Some(12));
    assert_eq!(
        s.capacity(Level::FIRST),
        d.rules.max_residents(Level::FIRST).map(|r| 8 * r),
        "the capacity is in residents: eight houses at level 1"
    );
    assert_eq!(s.range(level(2)), None, "the well has only one level in M0");

    let farm = d.def(farm).expect("the farm's def");
    assert!(farm.is_producer());
    assert_eq!(farm.tile_count(), 4);
    assert_eq!(farm.output_per_tick, Some(Milli::from_millis(400)));
    assert!(farm.max_stock > Some(Milli::ZERO));
}

/// The farm's capacity is **exactly** what its output sustains: no more, or the
/// houses in excess would stay covered and hungry forever; and no less, or it
/// would declare places it never uses.
///
/// Hunger is still reachable — the "hunger" scenario of phase 08 still exists —
/// but it is hunger through lack of coverage: it is cured by building.
#[test]
fn the_farms_capacity_is_what_its_output_sustains() {
    let d = sim_data::load_default().expect("valid tables");
    let f = d.def(d.kind_by_id("farm").expect("the farm")).expect("def");
    let s = f.service.as_ref().expect("service");

    let residents = i32::from(d.rules.max_residents(Level::FIRST).expect("level 1"));
    let per_house = d
        .rules
        .food_per_resident
        .checked_mul_int(residents)
        .expect("one house's consumption");
    let capacity = i32::from(s.capacity(Level::FIRST).expect("capacity"));
    let max_demand = d
        .rules
        .food_per_resident
        .checked_mul_int(capacity)
        .expect("maximum demand");
    let output = f.output_per_tick.expect("output");

    assert!(
        output > per_house,
        "a farm has to sustain at least one house"
    );
    assert_eq!(
        max_demand, output,
        "at full capacity the farm must consume exactly what it produces"
    );
    assert_eq!(
        capacity % residents,
        0,
        "a capacity that is not a multiple of {residents} would leave places \
         no house at level 1 could take"
    );
}

/// The production tables pass the consistency check, and not just the farm: any
/// food provider that gets added is covered.
#[test]
fn no_food_provider_promises_more_than_it_produces() {
    let d = sim_data::load_default().expect("valid tables");
    assert_eq!(d.inconsistencies(), vec![]);
}

/// The fourth table (A13). `easy` is deliberately today's behaviour — a house
/// born at its full level-1 capacity — which is what makes phase 11's `.hashes`
/// diff attributable to the difficulty byte alone.
#[test]
fn the_difficulty_profiles_load() {
    let d = sim_data::load_default().expect("valid tables");

    let easy = d.difficulty_by_id("easy").expect("the easy profile exists");
    let hard = d.difficulty_by_id("hard").expect("the hard profile exists");
    assert_eq!(d.difficulty_by_id("impossible"), None);
    assert_ne!(easy, hard);

    let max = d
        .rules
        .max_residents(Level::FIRST)
        .expect("houses have a level 1");
    assert_eq!(
        d.difficulty(easy)
            .expect("def")
            .starting_residents_per_house,
        max,
        "at easy a house is born full: that is M0's behaviour"
    );
    assert_eq!(
        d.difficulty(hard)
            .expect("def")
            .starting_residents_per_house,
        0,
        "at hard the house only fills up by migration"
    );
    assert_eq!(Level::new(0), None, "levels start at 1");
    let past_the_top = d
        .rules
        .top_house_level()
        .and_then(Level::next)
        .expect("the ladder is not empty");
    assert_eq!(
        d.rules.max_residents(past_the_top),
        None,
        "and stop at the top of the ladder"
    );
}

/// No profile builds a house beyond its own capacity, and not just `easy`: any
/// profile that gets added is covered.
#[test]
fn no_profile_builds_a_house_beyond_its_capacity() {
    let d = sim_data::load_default().expect("valid tables");
    assert!(
        !d.difficulties.is_empty(),
        "the check above is only worth something with profiles to check"
    );
    assert_eq!(d.inconsistencies(), vec![]);
}

/// The satisfaction curve (phase 12): the shape the balancing means, expressed
/// as relations rather than as the numbers themselves.
///
/// Rewriting `max: 100, step_up: 4` here would be a copy of the table, green by
/// construction and unable to notice anything. What is worth pinning down is
/// what the numbers were chosen *for*.
#[test]
fn the_satisfaction_curve_has_the_shape_the_balancing_means() {
    let d = sim_data::load_default().expect("valid tables");
    let s = &d.rules.satisfaction;

    let climb = u32::from(s.max).div_ceil(u32::from(s.step_up));
    assert!(
        climb > 1 && climb < d.rules.ticks_per_month,
        "a house has to earn its satisfaction over days, not in one tick and \
         not in more than a month: {climb} ticks"
    );
    assert!(
        s.step_down > s.step_up,
        "it is lost faster than it is gained: losing the water is an event, \
         getting it back is an investment"
    );

    let mut previous = 0;
    for (i, &t) in s.mood_thresholds.iter().enumerate() {
        assert!(
            t > previous,
            "band {i} does not come after the one before it"
        );
        assert!(t <= s.max, "band {i} is beyond the maximum");
        previous = t;
    }
    assert_eq!(
        sim_core::Mood::of(0, s),
        sim_core::Mood::Desperate,
        "a house at zero has to be Desperate: it is the mood the renderer \
         assumes for a newly-built one"
    );
    assert_eq!(sim_core::Mood::of(s.max, s), sim_core::Mood::Thriving);
}

/// The house ladder (phase 13): the shape the balancing means, expressed as
/// relations rather than as the numbers themselves.
///
/// The relations that make a dataset *usable* are checked by
/// `DataSet::inconsistencies` and asserted above. What is worth pinning down
/// here is what the numbers were chosen **for**, which no validation can know:
/// that the rungs are earned inside a game and not in an afternoon, and that
/// they really do ask for different things.
#[test]
fn the_house_ladder_has_the_shape_the_balancing_means() {
    let d = sim_data::load_default().expect("valid tables");
    let s = &d.rules.satisfaction;
    let top = d.rules.top_house_level();
    assert!(
        top > Some(Level::FIRST),
        "a ladder with one rung is not a ladder"
    );

    let mut previous: Option<&sim_core::HouseLevelDef> = None;
    for (level, l) in d.rules.house_ladder() {
        if level > Level::FIRST {
            let climb = u32::from(l.level_up_threshold).div_ceil(u32::from(s.step_up));
            assert!(
                climb < d.rules.ticks_per_month,
                "level {level} is earned in {climb} ticks, more than the month \
                 that separates two reviews: it would take two reviews to gain \
                 one rung, and the reason would be invisible"
            );
            let band = u32::from(l.level_up_threshold) - u32::from(l.decay_threshold);
            let fall = band.div_ceil(u32::from(s.step_down));
            assert!(
                fall > 1,
                "level {level}'s band is {band}, which a single tick without \
                 the service crosses: the hysteresis exists on paper only"
            );
        }

        if let Some(previous) = previous {
            assert!(
                previous.required_services.len() <= l.required_services.len(),
                "level {level} asks for less than the level below it: a house \
                 would be promoted into an easier life"
            );
        }
        previous = Some(l);
    }

    let first = d.rules.house_level(Level::FIRST).expect("level 1");
    let last = d
        .rules
        .house_level(top.expect("the ladder is not empty"))
        .expect("the top level");
    assert!(
        last.required_services.len() > first.required_services.len(),
        "with the same demands at every rung the per-level requirements do \
         nothing the building's own list would not do"
    );
    assert_eq!(
        (first.level_up_threshold, first.decay_threshold),
        (0, 0),
        "level 1's thresholds are read by nobody, and the table must not claim \
         a rule that does not exist"
    );
}

// --- 2. broken fixtures, one per check ---

fn errors(buildings: &str) -> Vec<(String, ValidationErrorKind)> {
    errors_of(with_buildings(buildings))
}

fn errors_of(loaded: Result<DataSet, LoadError>) -> Vec<(String, ValidationErrorKind)> {
    match loaded {
        Err(LoadError::Validation(r)) => r.errors.into_iter().map(|e| (e.path, e.kind)).collect(),
        Err(other) => panic!("expected a validation error, found: {other}"),
        Ok(_) => panic!("the broken table passed validation"),
    }
}

#[test]
fn duplicate_id() {
    let e = errors(&broken_fixture("duplicate_id.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "buildings[1].id");
    assert!(matches!(e[0].1, ValidationErrorKind::DuplicateId { .. }));
}

#[test]
fn levels_without_matching_ranges() {
    let e = errors(&broken_fixture("levels_without_ranges.ron"));
    assert_eq!(
        e.len(),
        2,
        "a value is missing from both range and capacity"
    );
    assert_eq!(e[0].0, "buildings[0].service.range_per_level");
    assert_eq!(e[1].0, "buildings[0].service.capacity_per_level");
}

#[test]
fn negative_cost() {
    let e = errors(&broken_fixture("negative_cost.ron"));
    assert_eq!(
        e,
        [(
            "buildings[0].cost".to_string(),
            ValidationErrorKind::Negative { found: -10 }
        )]
    );
}

#[test]
fn unknown_service() {
    let e = errors(&broken_fixture("unknown_service.ron"));
    assert_eq!(e.len(), 2, "the service kind and the required service");
    assert_eq!(e[0].0, "buildings[0].service.kind");
    assert_eq!(e[1].0, "buildings[0].required_services[0]");
}

/// The check that crosses `rules` and `buildings`: a food provider's capacity
/// cannot exceed what its output sustains.
#[test]
fn capacity_beyond_the_output() {
    let e = errors(&broken_fixture("unsustainable_capacity.ron"));
    assert_eq!(
        e,
        [
            (
                "buildings[0].service.capacity_per_level".to_string(),
                ValidationErrorKind::Inconsistent(Inconsistency::CapacityBeyondOutput {
                    building: 0,
                    level: Level::FIRST,
                    capacity: 24,
                    sustainable: 20
                })
            ),
            // Here the limit is not the output but the granary: 100 milli of
            // stock is enough for five residents, not twenty.
            (
                "buildings[1].service.capacity_per_level".to_string(),
                ValidationErrorKind::Inconsistent(Inconsistency::CapacityBeyondOutput {
                    building: 1,
                    level: Level::FIRST,
                    capacity: 20,
                    sustainable: 5
                })
            ),
        ]
    );
}

/// The other check that crosses two tables: a profile cannot build a house
/// beyond what a level-1 house holds.
#[test]
fn a_profile_beyond_the_house_capacity() {
    let e = errors_of(with_difficulty(&broken_fixture(
        "difficulty_beyond_capacity.ron",
    )));
    assert_eq!(
        e,
        [(
            "profiles[0].starting_residents_per_house".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::StartingResidentsBeyondCapacity {
                difficulty: 0,
                starting: 6,
                max_residents: 4
            })
        )],
        "only the first profile is broken: the second is at the limit and legitimate"
    );
}

/// The satisfaction curve, in the only table where a `rules.ron` fixture is
/// broken on purpose: both problems have to show up, not just the first.
#[test]
fn a_satisfaction_curve_that_cannot_be_drawn() {
    let e = errors_of(with_rules(&broken_fixture("satisfaction_out_of_range.ron")));
    assert_eq!(e.len(), 2, "both problems, not just the first: {e:?}");

    assert_eq!(e[0].0, "rules.satisfaction.step_down");
    assert_eq!(
        e[0].1,
        ValidationErrorKind::BeyondMax {
            found: 120,
            max: 100
        }
    );

    assert_eq!(e[1].0, "rules.satisfaction.mood_thresholds[1]");
    assert_eq!(
        e[1].1,
        ValidationErrorKind::MoodBandsOutOfOrder {
            previous: 25,
            found: 25
        }
    );
}

/// A band at zero is not merely odd: it would leave `Mood::Desperate` empty,
/// and a newly-built house would be born into a band nobody expects.
#[test]
fn a_mood_band_at_zero_is_refused() {
    let rules = valid_rules().replacen("mood_thresholds: (25,", "mood_thresholds: (0,", 1);
    assert_ne!(rules, valid_rules(), "the replacement has to have bitten");
    let e = errors_of(with_rules(&rules));
    assert_eq!(
        e,
        vec![(
            "rules.satisfaction.mood_thresholds[0]".to_string(),
            ValidationErrorKind::MoodBandsOutOfOrder {
                previous: 0,
                found: 0
            }
        )]
    );
}

// --- 2b. one broken table per new cross-table check (phase 13) ---
//
// Written as a replacement inside the valid table rather than as a file of its
// own: each of these is a single number or a single word away from a dataset
// that works, and the diff *is* the documentation. The assertion on the whole
// vector is what makes them worth something — it says not only that the check
// fires, but that nothing else does.

/// The one that guards `is_house()`. Empty the building's list and the house
/// stops being classified as one: from that moment nothing levels up, nothing
/// is taxed and nothing says why.
#[test]
fn a_buildings_table_without_a_house() {
    let e = errors_of(with_buildings(&replaced(
        &valid_buildings(),
        r#"required_services: ["water", "food"],"#,
        "required_services: [],",
    )));
    assert_eq!(
        e,
        [(
            "buildings".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::NoHouse)
        )]
    );
}

/// The house and the ladder have to agree on how many rungs there are.
#[test]
fn a_house_that_declares_the_wrong_number_of_levels() {
    let e = errors_of(with_buildings(&replaced(
        &valid_buildings(),
        "levels: 3,",
        "levels: 2,",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::LevelCountMismatch {
                declared: 2,
                in_table: 3,
            })
        )]
    );
}

/// And on **what** they ask for: the building's list is the union of the rungs'.
#[test]
fn a_house_whose_list_is_not_the_union_of_its_rungs() {
    let e = errors_of(with_buildings(&replaced(
        &valid_buildings(),
        r#"required_services: ["water", "food"],"#,
        r#"required_services: ["water"],"#,
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::InconsistentRequirements {
                declared: vec![ServiceKind::Water],
                in_levels: vec![ServiceKind::Water, ServiceKind::Food],
            })
        )]
    );
}

/// A rung that holds no more than the one below it: levelling up would shrink
/// the house.
#[test]
fn a_ladder_that_does_not_go_up() {
    let e = errors_of(with_rules(&replaced(
        &valid_rules(),
        "max_residents: 8,",
        "max_residents: 4,",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels[1].max_residents".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::CapacityNotIncreasing {
                level: level(2),
                max_residents: 4,
                previous: 4,
            })
        )]
    );
}

/// No band between the two thresholds, and the city flips at every review.
#[test]
fn a_rung_with_no_hysteresis_band() {
    let e = errors_of(with_rules(&replaced(
        &valid_rules(),
        "decay_threshold: 25,",
        "decay_threshold: 50,",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels[1].decay_threshold".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::NoHysteresis {
                level: level(2),
                decay: 50,
                level_up: 50,
            })
        )]
    );
}

/// A threshold past the ceiling of the accumulator: a rung nobody can ever
/// reach, and nothing in the game would say so.
#[test]
fn a_rung_nobody_can_reach() {
    let e = errors_of(with_rules(&replaced(
        &valid_rules(),
        "level_up_threshold: 90,",
        "level_up_threshold: 120,",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels[2].level_up_threshold".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::UnreachableThreshold {
                level: level(3),
                threshold: 120,
                max: 100,
            })
        )]
    );
}

/// A service the rungs ask for and no building supplies. One error per rung
/// that asks for it: each is a level that can never be held.
#[test]
fn a_service_no_building_provides() {
    let e = errors_of(with_buildings(&replaced(
        &valid_buildings(),
        r#"kind: "water","#,
        r#"kind: "food","#,
    )));
    let expected: Vec<_> = (1..=3)
        .map(level)
        .map(|at| {
            (
                format!("rules.house_levels[{}].required_services", at.as_usize()),
                ValidationErrorKind::Inconsistent(Inconsistency::ServiceWithoutProvider {
                    level: at,
                    service: ServiceKind::Water,
                }),
            )
        })
        .collect();
    assert_eq!(e, expected);
}

/// A provider too small for a full house of that rung. Not a blocker with A12
/// — the house is servable as long as it stays half empty — but it is a city
/// that plugs up without saying why.
#[test]
fn a_rung_no_provider_can_serve_in_full() {
    let e = errors_of(with_buildings(&replaced(
        &valid_buildings(),
        "capacity_per_level: [32],",
        "capacity_per_level: [8],",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels[2].max_residents".to_string(),
            ValidationErrorKind::Inconsistent(Inconsistency::CapacityBeyondEveryProvider {
                level: level(3),
                service: ServiceKind::Water,
                max_residents: 12,
                best: 8,
            })
        )],
        "level 2 holds 8, which the shrunken well still covers: only the top \
         rung is out of reach"
    );
}

/// A rung that asks for nothing would be climbed for free, `all()` over an
/// empty list being true. It is the one relation that stays in `sim-data`:
/// it is a property of one field of one table.
#[test]
fn a_rung_that_asks_for_nothing() {
    let e = errors_of(with_rules(&replaced(
        &valid_rules(),
        r#"required_services: ["water"],"#,
        "required_services: [],",
    )));
    assert_eq!(
        e,
        [(
            "rules.house_levels[0].required_services".to_string(),
            ValidationErrorKind::Empty
        )]
    );
}

/// A replacement that has to have bitten: a `replacen` that matched nothing
/// would leave the table valid and the test green for the wrong reason.
fn replaced(table: &str, from: &str, to: &str) -> String {
    let out = table.replacen(from, to, 1);
    assert_ne!(out, table, "the replacement {from:?} matched nothing");
    out
}

#[test]
fn a_missing_field_and_invalid_ron_fail_at_parse_time() {
    for name in ["missing_field.ron", "invalid_ron.ron"] {
        match with_buildings(&broken_fixture(name)) {
            Err(LoadError::Ron { file, .. }) => assert_eq!(file, "buildings.ron"),
            other => panic!("{name}: expected a parse error, found {other:?}"),
        }
    }
}

// --- 3. the report is complete ---

/// Tells real validation apart from a `?` on the first check.
#[test]
fn the_report_lists_every_error() {
    let e = errors(&broken_fixture("three_errors.ron"));
    let paths: Vec<_> = e.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        [
            "buildings[0].id",
            "buildings[0].size.0",
            "buildings[1].max_stock",
        ],
        "three distinct errors must all three show up"
    );
}

// --- 4. stable messages ---

#[test]
fn the_report_messages_are_stable() {
    let r = match with_buildings(&broken_fixture("three_errors.ron")) {
        Err(LoadError::Validation(r)) => r,
        other => panic!("expected a report, found {other:?}"),
    };
    insta::assert_snapshot!("report_three_errors", r.to_string());

    let r = match with_buildings(&broken_fixture("unknown_service.ron")) {
        Err(LoadError::Validation(r)) => r,
        other => panic!("expected a report, found {other:?}"),
    };
    insta::assert_snapshot!("report_unknown_service", r.to_string());
}

// --- 5. hash ---

#[test]
fn the_hash_is_stable_across_two_loads() {
    let a = sim_data::load_default().expect("valid tables");
    let b = sim_data::load_default().expect("valid tables");
    assert_eq!(a.hash, b.hash);
    assert_eq!(a.hash_hex().len(), 64);
}

/// Reformatting the RON or adding a comment must not invalidate the recordings:
/// the hash is over the validated content, not over the file's bytes.
#[test]
fn the_hash_ignores_reformatting() {
    let original = read_data("buildings.ron");
    let reformatted = format!(
        "// an added comment\n\n{}\n\n// and another at the end\n",
        original.replace('\n', "\n   ")
    );

    let a = with_buildings(&original).expect("the original is valid");
    let b = with_buildings(&reformatted).expect("the reformatted one is valid");
    assert_eq!(a.hash, b.hash, "a reformatting is not a balance change");
}

/// But a changed number does: that is what makes the replay fail immediately
/// and for the right reason (A2).
#[test]
fn the_hash_notices_a_balance_change() {
    let original = read_data("buildings.ron");
    let modified = original.replacen("cost: 10", "cost: 11", 1);
    assert_ne!(original, modified, "the replacement has to have bitten");

    let a = with_buildings(&original).expect("the original is valid");
    let b = with_buildings(&modified).expect("the modified one is valid");
    assert_ne!(a.hash, b.hash);
}

/// The rules, the terrains and the profiles go into the hash too: if only
/// `buildings` did, changing the food consumption would make no recording fail.
#[test]
fn the_hash_covers_every_table() {
    let base = sim_data::load_default().expect("valid tables");

    // 19 and not 21: raising the consumption would make the farm's capacity
    // unsustainable and the table would not pass validation. What matters here
    // is that the hash moves, not that the load fails.
    let modified_rules =
        valid_rules().replacen("food_per_resident: 20", "food_per_resident: 19", 1);
    let a = sim_data::from_ron_str(
        &modified_rules,
        &valid_terrain(),
        &valid_buildings(),
        &valid_difficulty(),
    )
    .expect("valid");
    assert_ne!(base.hash, a.hash, "the rules must go into the hash");

    // The satisfaction curve is a block inside the rules, and a nested table is
    // exactly the kind that gets forgotten in a hash written by hand (A3):
    // without this, rebalancing the curve would leave every recording green
    // while the game has changed.
    // The satisfaction curve and the house ladder are blocks **inside** the
    // rules, and a nested table is exactly the kind that gets forgotten in a
    // hash written by hand (A3): without these, rebalancing them would leave
    // every recording green while the game has changed. Every field of a rung
    // is perturbed, not just one: the hash is fed field by field, so one of
    // them can be left out on its own.
    for (from, to) in [
        ("step_up: 4", "step_up: 5"),
        (
            "mood_thresholds: (25, 50, 75)",
            "mood_thresholds: (20, 50, 75)",
        ),
        ("max_residents: 8", "max_residents: 9"),
        ("level_up_threshold: 50", "level_up_threshold: 55"),
        ("decay_threshold: 25", "decay_threshold: 30"),
        ("taxable_per_resident: 0", "taxable_per_resident: 1"),
        (
            r#"required_services: ["water"],"#,
            r#"required_services: ["food"],"#,
        ),
    ] {
        let modified = valid_rules().replacen(from, to, 1);
        assert_ne!(
            modified,
            valid_rules(),
            "the replacement has to have bitten"
        );
        let d = with_rules(&modified).expect("valid");
        assert_ne!(
            base.hash, d.hash,
            "the satisfaction curve must go into the hash ({from})"
        );
    }

    let modified_terrain = valid_terrain().replacen("road_cost: 2", "road_cost: 3", 1);
    let b = sim_data::from_ron_str(
        &valid_rules(),
        &modified_terrain,
        &valid_buildings(),
        &valid_difficulty(),
    )
    .expect("valid");
    assert_ne!(base.hash, b.hash, "the terrains must go into the hash");

    // The knob of a profile nobody is playing on still has to move the hash:
    // otherwise rebalancing `normal` would leave every recording green while
    // the game has changed.
    let modified_difficulty = valid_difficulty().replacen(
        r#"(id: "normal", starting_residents_per_house: 2)"#,
        r#"(id: "normal", starting_residents_per_house: 3)"#,
        1,
    );
    assert_ne!(
        modified_difficulty,
        valid_difficulty(),
        "the replacement has to have bitten"
    );
    let c = sim_data::from_ron_str(
        &valid_rules(),
        &valid_terrain(),
        &valid_buildings(),
        &modified_difficulty,
    )
    .expect("valid");
    assert_ne!(base.hash, c.hash, "the profiles must go into the hash");
}
