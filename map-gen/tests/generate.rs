//! Phase 16.6 — what a generated map has to be true of.
//!
//! The loader is the only honest judge of a generated map, so most of these
//! tests run what `generate` produced through the real `render_map` →
//! `parse_map` → `validate_map` path. `sim-data` is a dev-dependency here,
//! never an ordinary one, so this crate itself names no crate that does I/O.

use proptest::prelude::*;

use map_gen::{Config, ConfigError, Source, SourceKind, generate};
use sim_core::{MapDef, Tile, TilePos};

const W: u16 = 48;
const H: u16 = 32;

/// A handful of points scattered over a `width x height` grid: the four
/// corners and the centre, deduplicated for a grid too small to tell them
/// apart. Cycled through mountain/sea/land so every kind is exercised on
/// any grid with room for more than one point.
fn sample_sources(width: u16, height: u16) -> Vec<Source> {
    // saturating, not `- 1` directly: a zero-sized grid is one of the shapes
    // `generate` itself refuses, and this helper still has to build *something*
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

fn config(seed: u64, width: u16, height: u16) -> Config {
    Config {
        seed,
        sources: sample_sources(width, height),
        width,
        height,
        min_height: 0,
        max_height: Tile::MAX_GROUND_HEIGHT,
        id: "generated".to_string(),
    }
}

fn generated(seed: u64, width: u16, height: u16) -> MapDef {
    generate(&config(seed, width, height))
        .unwrap_or_else(|e| panic!("seed {seed} at {width}x{height}: {e}"))
}

/// ***The test that defines the phase.*** A generated map goes out through
/// the renderer and comes back in through the real loader.
#[test]
fn every_generated_map_survives_the_real_loader() {
    for seed in 0..24u64 {
        for (w, h) in [(W, H), (16u16, 16u16), (64, 24), (7, 61)] {
            let def = generated(seed, w, h);
            let text = sim_data::render_map(&def);
            let back = sim_data::parse_map(&text)
                .unwrap_or_else(|e| panic!("seed {seed} at {w}x{h} does not load: {e}"));
            assert_eq!(back, def, "seed {seed} at {w}x{h}");
        }
    }
}

/// A seed is the whole of what makes a map.
#[test]
fn the_same_seed_generates_the_same_map_and_two_seeds_do_not() {
    assert_eq!(generated(11, W, H), generated(11, W, H));

    let a = generated(11, W, H);
    let b = generated(12, W, H);
    assert_ne!(a.terrain, b.terrain);
    assert_ne!(a.hash, b.hash);
}

/// Every ground height stays within the caller's declared range, and here
/// specifically within the game's own real ceiling — the range this test
/// sweeps at its widest.
#[test]
fn every_ground_height_stays_within_the_callers_declared_range() {
    for seed in 0..16u64 {
        let def = generated(seed, W, H);
        assert!(def.ground.iter().all(|&g| g <= Tile::MAX_GROUND_HEIGHT));
    }
}

/// One walkable region, always: the repair's own unit tests pin that it
/// only ever drowns land.
#[test]
fn every_generated_map_is_one_walkable_region() {
    for seed in 0..32u64 {
        let grid = sim_core::Grid::from_map(&generated(seed, W, H));
        assert_eq!(
            grid.walkable_regions().len(),
            1,
            "seed {seed} came out in more than one piece"
        );
    }
}

/// A size the grid refuses comes back as a clean error, not a panic.
#[test]
fn a_size_the_grid_refuses_comes_back_as_a_clean_error_not_a_panic() {
    let max = sim_core::grid::MAX_SIDE;
    for (w, h) in [(1u16, 1u16), (1, max), (max, 1)] {
        let map = generated(3, w, h);
        assert_eq!(map.terrain.len(), usize::from(w) * usize::from(h));
    }

    for (w, h) in [(0u16, 4u16), (4, 0), (max + 1, 4), (4, max + 1)] {
        let e = generate(&config(3, w, h)).expect_err("the grid will not hold it");
        assert_eq!(
            e,
            ConfigError::UnusableSize {
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
    let mut base = config(1, 10, 10);

    base.sources = Vec::new();
    assert_eq!(generate(&base).unwrap_err(), ConfigError::NoSources);

    base.sources = vec![Source {
        point: TilePos::new(10, 0),
        kind: SourceKind::Land,
        strength: 1,
    }];
    assert_eq!(
        generate(&base).unwrap_err(),
        ConfigError::SourceOutOfBounds {
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
        generate(&base).unwrap_err(),
        ConfigError::DuplicateSourceOrigin {
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
        generate(&base).unwrap_err(),
        ConfigError::InvalidHeightRange {
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
        generate(&base).unwrap_err(),
        ConfigError::InvalidHeightRange {
            min_height: 0,
            max_height: over_cap,
            cap: Tile::MAX_GROUND_HEIGHT
        }
    );

    base.min_height = 9;
    base.max_height = Tile::MAX_GROUND_HEIGHT;
    assert_eq!(
        generate(&base).unwrap_err(),
        ConfigError::HeightRangeExcludesSeaLevel {
            min_height: 9,
            max_height: Tile::MAX_GROUND_HEIGHT,
            sea_level: 8
        }
    );
}

proptest! {
    /// Every ground height in range, over seeds and sizes.
    #[test]
    fn every_ground_height_is_in_range(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let map = generate(&config(seed, w, h)).expect("sample_sources always leaves room for its own points");
        prop_assert!(map.ground.iter().all(|&g| g <= Tile::MAX_GROUND_HEIGHT));
        prop_assert_eq!(map.ground.len(), usize::from(w) * usize::from(h));
    }

    /// And every one of those maps still loads.
    #[test]
    fn a_map_of_any_size_loads(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let map = generate(&config(seed, w, h)).expect("sample_sources always leaves room for its own points");
        let text = sim_data::render_map(&map);
        prop_assert_eq!(sim_data::parse_map(&text).ok(), Some(map));
    }
}
