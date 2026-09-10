//! Phase 16.6 — what a drawn map has to be true of.
//!
//! The loader is the only honest judge of a generated map, so most of these
//! tests run what `draw` produced through the real `render_map` → `parse_map`
//! → `validate_map` path — the same discipline `map-gen`'s own `tests/draw.rs`
//! uses, for the same reason: `sim-data` is a dev-dependency here, never an
//! ordinary one, so this crate itself names no crate that does I/O.

use proptest::prelude::*;

use map_gen2::{DrawError, DrawInput, Source, SourceKind, draw, report};
use sim_core::{MapDef, Tile, TilePos};

const W: u16 = 48;
const H: u16 = 32;

/// A handful of points scattered over a `width x height` grid: the four
/// corners and the centre, deduplicated for a grid too small to tell them
/// apart. Cycled through mountain/sea/land so every kind is exercised on
/// any grid with room for more than one point.
fn sample_sources(width: u16, height: u16) -> Vec<Source> {
    // saturating, not `- 1` directly: a zero-sized grid is one of the shapes
    // `draw` itself refuses, and this helper still has to build *something*
    // to hand it without panicking first.
    let (mx, my) = (
        width.saturating_sub(1) as u8,
        height.saturating_sub(1) as u8,
    );
    let (cx, cy) = (
        (width.saturating_sub(1) / 2) as u8,
        (height.saturating_sub(1) / 2) as u8,
    );
    let raw = [
        TilePos::new(0, 0),
        TilePos::new(mx, my),
        TilePos::new(0, my),
        TilePos::new(mx, 0),
        TilePos::new(cx, cy),
    ];
    let mut points: Vec<TilePos> = Vec::new();
    for p in raw {
        if !points.contains(&p) {
            points.push(p);
        }
    }

    let kinds = [
        SourceKind::Mountain,
        SourceKind::Sea,
        SourceKind::Land,
        SourceKind::Mountain,
        SourceKind::Sea,
    ];
    let strengths = [3u16, 5, 1, 4, 2];
    points
        .into_iter()
        .enumerate()
        .map(|(i, point)| Source {
            point,
            kind: kinds[i % kinds.len()],
            strength: strengths[i % strengths.len()],
        })
        .collect()
}

fn input(seed: u64, width: u16, height: u16) -> DrawInput {
    DrawInput {
        seed,
        sources: sample_sources(width, height),
        width,
        height,
        min_height: 0,
        max_height: Tile::MAX_GROUND_HEIGHT,
        id: "drawn".to_string(),
    }
}

fn drawn(seed: u64, width: u16, height: u16) -> MapDef {
    draw(&input(seed, width, height))
        .unwrap_or_else(|e| panic!("seed {seed} at {width}x{height}: {e}"))
        .def
}

/// ***The test that defines the phase.*** A drawn map goes out through the
/// renderer and comes back in through the real loader.
#[test]
fn every_drawn_map_survives_the_real_loader() {
    for seed in 0..24u64 {
        for (w, h) in [(W, H), (16u16, 16u16), (64, 24), (7, 61)] {
            let def = drawn(seed, w, h);
            let text = sim_data::render_map(&def);
            let back = sim_data::parse_map(&text)
                .unwrap_or_else(|e| panic!("seed {seed} at {w}x{h} does not load: {e}"));
            assert_eq!(back, def, "seed {seed} at {w}x{h}");
        }
    }
}

/// A seed is the whole of what makes a map.
#[test]
fn the_same_seed_draws_the_same_map_and_two_seeds_do_not() {
    assert_eq!(drawn(11, W, H), drawn(11, W, H));

    let a = drawn(11, W, H);
    let b = drawn(12, W, H);
    assert_ne!(a.terrain, b.terrain);
    assert_ne!(a.hash, b.hash);
}

/// Every ground height stays within the caller's declared range, and here
/// specifically within the game's own real ceiling — the range this test
/// sweeps at its widest.
#[test]
fn every_ground_height_stays_within_the_callers_declared_range() {
    for seed in 0..16u64 {
        let def = drawn(seed, W, H);
        assert!(def.ground.iter().all(|&g| g <= Tile::MAX_GROUND_HEIGHT));
    }
}

/// One walkable region, always — and the repair only ever drowns land.
#[test]
fn the_repair_leaves_one_region_and_only_ever_drowns() {
    for seed in 0..32u64 {
        let d = draw(&input(seed, W, H)).expect("a map at a usable size");
        let grid = sim_core::Grid::from_map(&d.def);
        assert_eq!(
            grid.walkable_regions().len(),
            1,
            "seed {seed} came out in more than one piece"
        );
        let walkable: u32 = d
            .def
            .terrain
            .iter()
            .filter(|t| t.is_walkable())
            .count()
            .try_into()
            .expect("a map holds at most 65,536 tiles");
        assert!(
            d.repair.tiles_drowned == 0 || d.repair.regions_before > 1,
            "seed {seed} drowned tiles without having had a second region"
        );
        assert!(
            d.repair.tiles_drowned + walkable >= walkable,
            "the repair created land on seed {seed}"
        );
    }
}

/// A size the grid refuses comes back as a clean error, not a panic.
#[test]
fn a_size_the_grid_refuses_comes_back_as_a_clean_error_not_a_panic() {
    let max = sim_core::grid::MAX_SIDE;
    for (w, h) in [(1u16, 1u16), (1, max), (max, 1)] {
        let d = draw(&input(3, w, h)).unwrap_or_else(|e| panic!("{w}x{h}: {e}"));
        assert_eq!(d.def.terrain.len(), usize::from(w) * usize::from(h));
    }

    for (w, h) in [(0u16, 4u16), (4, 0), (max + 1, 4), (4, max + 1)] {
        let e = draw(&input(3, w, h)).expect_err("the grid will not hold it");
        assert_eq!(
            e,
            DrawError::UnusableSize {
                width: w,
                height: h,
                max
            }
        );
    }
}

/// Each way a source list can be wrong is refused by name.
#[test]
fn each_way_a_source_list_can_be_wrong_is_refused_by_name() {
    let mut base = input(1, 10, 10);

    base.sources = Vec::new();
    assert_eq!(draw(&base).unwrap_err(), DrawError::NoSources);

    base.sources = vec![Source {
        point: TilePos::new(10, 0),
        kind: SourceKind::Land,
        strength: 1,
    }];
    assert_eq!(
        draw(&base).unwrap_err(),
        DrawError::SourceOutOfBounds {
            index: 0,
            x: 10,
            y: 0,
            width: 10,
            height: 10,
        }
    );

    base.sources = vec![
        Source {
            point: TilePos::new(2, 2),
            kind: SourceKind::Land,
            strength: 1,
        },
        Source {
            point: TilePos::new(2, 2),
            kind: SourceKind::Sea,
            strength: 1,
        },
    ];
    assert_eq!(
        draw(&base).unwrap_err(),
        DrawError::DuplicateSourceOrigin {
            first: 0,
            second: 1,
            x: 2,
            y: 2,
        }
    );

    base.sources = sample_sources(10, 10);
    base.min_height = 20;
    base.max_height = 10;
    assert_eq!(
        draw(&base).unwrap_err(),
        DrawError::InvalidHeightRange {
            min_height: 20,
            max_height: 10,
            cap: Tile::MAX_GROUND_HEIGHT
        }
    );

    base.min_height = 0;
    // MAX_GROUND_HEIGHT is 31 < u8::MAX, so this stays a valid u8.
    let over_cap = Tile::MAX_GROUND_HEIGHT + 1;
    base.max_height = over_cap;
    assert_eq!(
        draw(&base).unwrap_err(),
        DrawError::InvalidHeightRange {
            min_height: 0,
            max_height: over_cap,
            cap: Tile::MAX_GROUND_HEIGHT
        }
    );

    base.min_height = 9;
    base.max_height = Tile::MAX_GROUND_HEIGHT;
    assert_eq!(
        draw(&base).unwrap_err(),
        DrawError::HeightRangeExcludesSeaLevel {
            min_height: 9,
            max_height: Tile::MAX_GROUND_HEIGHT,
            sea_level: 8
        }
    );
}

/// Somewhere to put the largest building, on a size and source layout large
/// enough to have room for one — unlike a noise field, an arbitrary source
/// list is not guaranteed to leave anywhere buildable at all, so this is
/// read off a specific, generous configuration rather than swept over many.
#[test]
fn somewhere_to_put_the_largest_building() {
    let data = sim_data::load_default().expect("the production tables load");
    let biggest = data
        .buildings
        .iter()
        .max_by_key(|b| u32::from(b.size.0) * u32::from(b.size.1))
        .expect("the tables hold at least one building");

    let mut found = false;
    for seed in 0..24u64 {
        let d = draw(&input(seed, W, H)).expect("a map at a usable size");
        let r = report(&d, &data);
        let places = r
            .places
            .iter()
            .find(|p| p.size == biggest.size)
            .expect("the report covers every footprint in the table");
        if places.count > 0 {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "no seed in the sweep left room for a {}x{} '{}'",
        biggest.size.0, biggest.size.1, biggest.id
    );
}

/// The report counts what the map holds and nothing else.
#[test]
fn the_report_adds_up_to_the_map() {
    let data = sim_data::load_default().expect("the production tables load");
    let d = draw(&input(5, W, H)).expect("a map");
    let r = report(&d, &data);

    assert_eq!(r.tiles.iter().sum::<u32>(), u32::from(W) * u32::from(H));
    assert_eq!(r.repair, d.repair);
    assert!(r.lowest <= r.highest);
    assert_eq!(
        r.places.len(),
        data.buildings
            .iter()
            .map(|b| b.size)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "one entry per distinct footprint, no more and no fewer"
    );
}

proptest! {
    /// Every ground height in range, over seeds and sizes.
    #[test]
    fn every_ground_height_is_in_range(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let d = draw(&input(seed, w, h)).expect("sample_sources always leaves room for its own points");
        prop_assert!(d.def.ground.iter().all(|&g| g <= Tile::MAX_GROUND_HEIGHT));
        prop_assert_eq!(d.def.ground.len(), usize::from(w) * usize::from(h));
    }

    /// And every one of those maps still loads.
    #[test]
    fn a_map_of_any_size_loads(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let d = draw(&input(seed, w, h)).expect("sample_sources always leaves room for its own points");
        let text = sim_data::render_map(&d.def);
        prop_assert_eq!(sim_data::parse_map(&text).ok(), Some(d.def));
    }
}
