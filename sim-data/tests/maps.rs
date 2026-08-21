//! Phase 16 — the map loader's validation.
//!
//! Four faults, each refused at load with a message naming where: a row that
//! does not match the declared size, a character neither block claims, a
//! ground height beyond the field's maximum, and walkable ground that does
//! not form one connected region.

use std::path::Path;

use proptest::prelude::*;

use sim_core::{Grid, Tile};
use sim_data::{RawMap, ValidationErrorKind, validate_map};

fn broken_fixture(name: &str) -> RawMap {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/broken")
        .join(name);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn errors_of(raw: &RawMap) -> Vec<(String, ValidationErrorKind)> {
    match validate_map(raw) {
        Err(rep) => rep.errors.into_iter().map(|e| (e.path, e.kind)).collect(),
        Ok(_) => panic!("the broken map passed validation"),
    }
}

/// A valid, uniform, flat 4x3 map — a fixture-free baseline these tests can
/// deform in one dimension at a time.
fn valid_map() -> RawMap {
    RawMap {
        id: "valid".to_string(),
        width: 4,
        height: 3,
        terrain: vec!["....".to_string(); 3],
        ground: vec!["0000".to_string(); 3],
    }
}

/// The production `river-valley` map (test 12 of the phase) loads and is one
/// connected region, the same guarantee any other map gets.
#[test]
fn the_river_valley_map_loads() {
    let def = sim_data::load_map_by_id("river-valley").expect("river-valley loads");
    assert_eq!(def.width, 12);
    assert_eq!(def.height, 8);
    let grid = Grid::from_map(&def);
    assert_eq!(grid.walkable_regions().len(), 1);
}

#[test]
fn a_valid_map_loads() {
    let def = validate_map(&valid_map()).expect("a uniform flat map is valid");
    assert_eq!(def.id, "valid");
    assert_eq!(def.width, 4);
    assert_eq!(def.height, 3);
    assert_eq!(def.terrain.len(), 12);
    assert_eq!(def.ground.len(), 12);
    assert!(def.ground.iter().all(|&h| h == 0));
}

#[test]
fn a_row_count_that_does_not_match_height_is_refused() {
    let e = errors_of(&broken_fixture("map_bad_row_count.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "terrain");
    assert_eq!(
        e[0].1,
        ValidationErrorKind::WrongMapRowCount {
            block: "terrain",
            expected: 3,
            found: 2
        }
    );
}

#[test]
fn an_unclaimed_character_is_refused_naming_its_row_and_column() {
    let e = errors_of(&broken_fixture("map_bad_character.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "terrain[1][2]");
    assert_eq!(
        e[0].1,
        ValidationErrorKind::UnclaimedMapCharacter {
            block: "terrain",
            row: 1,
            col: 2,
            found: 'x',
            known: ". (plain), ~ (water), # (rock)",
        }
    );
}

#[test]
fn a_ground_height_beyond_the_maximum_is_refused() {
    let e = errors_of(&broken_fixture("map_height_out_of_range.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "ground[0][0]");
    assert_eq!(
        e[0].1,
        ValidationErrorKind::GroundHeightBeyondMax {
            row: 0,
            col: 0,
            found: 32,
            max: Tile::MAX_GROUND_HEIGHT,
        }
    );
}

/// The fourth fault, and the one worth pinning by name: a river drawn corner
/// to corner severs the walkable ground, and the message names both regions'
/// sizes rather than just saying "disconnected".
#[test]
fn a_severed_map_is_refused_naming_both_regions() {
    let e = errors_of(&broken_fixture("map_severed.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "terrain");
    assert_eq!(
        e[0].1,
        ValidationErrorKind::SeveredMap { sizes: vec![2, 2] }
    );
}

/// Small dimensions, plus a ground height for every tile they hold.
fn dims_and_heights() -> impl Strategy<Value = (u16, u16, Vec<u8>)> {
    (1u16..=8, 1u16..=8).prop_flat_map(|(width, height)| {
        let n = width as usize * height as usize;
        (
            Just(width),
            Just(height),
            prop::collection::vec(0u8..=31, n),
        )
    })
}

proptest! {
    /// Every map that loads: every ground height is in range, every tile
    /// round-trips through the packing, and the walkable tiles are one
    /// region. Terrain stays uniform `Plain` — trivially connected — because
    /// this property is about the packing and the range, not about drawing
    /// rivers that do or do not sever the map (tests above already cover
    /// that by name).
    #[test]
    fn a_loaded_map_is_in_range_round_trips_and_is_connected((width, height, heights) in dims_and_heights()) {
        let ground: Vec<String> = heights
            .chunks(width as usize)
            .map(|row| row.iter().map(|h| char::from_digit(u32::from(*h), 36).unwrap()).collect())
            .collect();
        let terrain = vec![".".repeat(width as usize); height as usize];

        let raw = RawMap {
            id: "generated".to_string(),
            width,
            height,
            terrain,
            ground,
        };
        let def = validate_map(&raw).expect("in-range heights on a uniform terrain always validate");

        prop_assert!(def.ground.iter().all(|&h| h <= Tile::MAX_GROUND_HEIGHT));

        let grid = Grid::from_map(&def);
        for (i, idx) in grid.indices().enumerate() {
            prop_assert_eq!(grid.get(idx).map(Tile::ground_height), Some(def.ground[i]));
        }
        prop_assert_eq!(grid.walkable_regions().len(), 1);
    }
}
