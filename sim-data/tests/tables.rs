//! Table tests: the production fixture loads, the broken ones produce a
//! **complete** report, and the hash tells a balance change apart from a
//! reformatting.

use std::path::Path;

use sim_core::{Coins, Milli, ServiceKind, Terrain};
use sim_data::{DataSet, LoadError, ValidationErrorKind};

fn valid_rules() -> String {
    read_data("rules.ron")
}

fn valid_terrain() -> String {
    read_data("terrain.ron")
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
    sim_data::from_ron_str(&valid_rules(), &valid_terrain(), buildings)
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

    let house = d.kind_by_id("casa").expect("the house exists");
    let well = d.kind_by_id("pozzo").expect("the well exists");
    let farm = d.kind_by_id("fattoria").expect("the farm exists");
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
    assert_eq!(s.range(1), Some(12));
    assert_eq!(
        s.capacity(1),
        Some(8 * d.rules.residents_per_house_level[0]),
        "the capacity is in residents: eight houses of four"
    );
    assert_eq!(s.range(2), None, "the well has only one level in M0");
    assert_eq!(s.range(0), None, "levels start at 1");

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
    let f = d
        .def(d.kind_by_id("fattoria").expect("the farm"))
        .expect("def");
    let s = f.service.as_ref().expect("service");

    let residents = i32::from(d.rules.residents_per_house_level[0]);
    let per_house = d
        .rules
        .food_per_resident
        .checked_mul_int(residents)
        .expect("one house's consumption");
    let capacity = i32::from(s.capacity(1).expect("capacity"));
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
        "in M0 every house is the same: a capacity that is not a multiple of \
         {residents} would leave places no house could take"
    );
}

/// The production tables pass the consistency check, and not just the farm: any
/// food provider that gets added is covered.
#[test]
fn no_food_provider_promises_more_than_it_produces() {
    let d = sim_data::load_default().expect("valid tables");
    assert_eq!(d.unsustainable_food_capacity(), vec![]);
}

// --- 2. broken fixtures, one per check ---

fn errors(buildings: &str) -> Vec<(String, ValidationErrorKind)> {
    match with_buildings(buildings) {
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
                ValidationErrorKind::CapacityBeyondOutput {
                    capacity: 24,
                    sustainable: 20
                }
            ),
            // Here the limit is not the output but the granary: 100 milli of
            // stock is enough for five residents, not twenty.
            (
                "buildings[1].service.capacity_per_level".to_string(),
                ValidationErrorKind::CapacityBeyondOutput {
                    capacity: 20,
                    sustainable: 5
                }
            ),
        ]
    );
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

/// The rules and the terrains go into the hash too: if only `buildings` did,
/// changing the food consumption would make no recording fail.
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
        &read_data("buildings.ron"),
    )
    .expect("valid");
    assert_ne!(base.hash, a.hash, "the rules must go into the hash");

    let modified_terrain = valid_terrain().replacen("road_cost: 2", "road_cost: 3", 1);
    let b = sim_data::from_ron_str(
        &valid_rules(),
        &modified_terrain,
        &read_data("buildings.ron"),
    )
    .expect("valid");
    assert_ne!(base.hash, b.hash, "the terrains must go into the hash");
}
