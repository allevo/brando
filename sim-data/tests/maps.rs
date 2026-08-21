//! Phase 16 — the map loader's validation.
//!
//! Four faults, each refused at load with a message naming where: a row that
//! does not match the declared size, a character neither block claims, a
//! ground height beyond the field's maximum, and walkable ground that does
//! not form one connected region.
//!
//! Phase 16.5 adds the other direction, and it is tested here rather than
//! beside the generator because both halves of the round trip live in this
//! crate: the alphabets have one owner, and this is where it is.

use std::path::{Path, PathBuf};

use proptest::prelude::*;

use sim_core::{Grid, MapDef, Terrain, Tile};
use sim_data::{MapSaveError, RawMap, ValidationErrorKind, validate_map};

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

// --- phase 16.5: the format writes itself ---------------------------------

/// The committed hand-drawn map survives being rendered and read back.
///
/// Maps are compared and not bytes: the comments and the spacing belong to
/// the renderer, and the file on disk was laid out by a person. `MapDef`
/// carries its own blake3, so this equality is also the hash `MapSpec::File`
/// travels with in a recording.
#[test]
fn rendering_the_committed_map_and_reading_it_back_gives_the_same_map() {
    let def = sim_data::load_map_by_id("river-valley").expect("river-valley loads");
    let text = sim_data::render_map(&def);
    let back = sim_data::parse_map(&text).expect("what the renderer writes, the parser reads");
    assert_eq!(back, def);
}

/// Every terrain and every ground height a map can hold, through the text and
/// back.
///
/// It is the guard on the two alphabets having one owner: a fourth terrain
/// reaching `decode_terrain` and not `terrain_char` fails here, rather than in
/// a map somebody writes a year later. The first column is plain on every row
/// so the walkable ground stays one region across the row of water — the same
/// job `river-valley`'s ford does.
#[test]
fn every_terrain_and_every_height_survives_the_round_trip() {
    let width = u16::from(Tile::MAX_GROUND_HEIGHT) + 2;
    let height = Terrain::COUNT as u16;

    let mut terrain = Vec::new();
    let mut ground = Vec::new();
    for row in 0..height {
        for col in 0..width {
            terrain.push(if col == 0 {
                Terrain::Plain
            } else {
                Terrain::ALL[row as usize]
            });
            ground.push(if col == 0 { 0 } else { (col - 1) as u8 });
        }
    }
    let def = MapDef::new(
        "every-character".to_string(),
        width,
        height,
        terrain,
        ground,
    );

    let back =
        sim_data::parse_map(&sim_data::render_map(&def)).expect("every character reads back");
    assert_eq!(back, def);
}

/// A path in the system's temporary directory, named after this process so
/// two runs at once cannot see each other's files — this test asserts that a
/// file is *absent*, which a leftover would quietly break.
fn scratch_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("brando-{}-{name}.ron", std::process::id()))
}

/// The writer writes nothing the loader would refuse, and leaves no file
/// behind when it refuses. Two faults, because they are caught at two
/// different stages: a severed map at the connectivity stage, a height beyond
/// the field at the character stage.
#[test]
fn the_writer_refuses_a_map_the_loader_would_not_accept() {
    // Plain, water, plain in a row: two walkable regions of one tile each.
    let severed = MapDef::new(
        "severed".to_string(),
        3,
        1,
        vec![Terrain::Plain, Terrain::Water, Terrain::Plain],
        vec![0, 0, 0],
    );
    let path = scratch_path("severed");
    let err = sim_data::save_map(&severed, "", &path).expect_err("a severed map is not written");
    assert!(matches!(err, MapSaveError::Refused { .. }), "{err}");
    assert!(
        !path.exists(),
        "nothing reaches the disk before the loader agrees"
    );

    // A height the base-32 alphabet has no digit for. It reaches the text as
    // '?', which neither block claims, so the loader names the row and column.
    let too_high = MapDef::new(
        "too-high".to_string(),
        2,
        1,
        vec![Terrain::Plain, Terrain::Plain],
        vec![0, Tile::MAX_GROUND_HEIGHT + 1],
    );
    let path = scratch_path("too-high");
    let err =
        sim_data::save_map(&too_high, "", &path).expect_err("an unwritable height is refused");
    assert!(matches!(err, MapSaveError::Refused { .. }), "{err}");
    assert!(
        !path.exists(),
        "nothing reaches the disk before the loader agrees"
    );
}

/// What the writer does write comes back through the loader, note and all.
#[test]
fn a_written_map_loads_back_with_its_note() {
    let def = MapDef::new(
        "written".to_string(),
        2,
        2,
        vec![Terrain::Plain; 4],
        vec![0, 1, 2, 3],
    );
    let path = scratch_path("written");
    sim_data::save_map(&def, "drawn by a test\n\nand read back by it", &path)
        .expect("a valid map is written");

    let text = std::fs::read_to_string(&path).expect("the file is there");
    assert!(text.starts_with("// drawn by a test\n//\n// and read back by it\n"));
    assert_eq!(sim_data::parse_map(&text).expect("it loads"), def);

    std::fs::remove_file(&path).expect("the test cleans up after itself");
}
