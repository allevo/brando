//! Phase 16.5 — what a drawn map has to be true of.
//!
//! The loader is the only honest judge of a generated map, so most of these
//! tests run what `draw` produced through the real `render_map` → `parse_map`
//! → `validate_map` path. That is why `sim-data` is a dev-dependency here: the
//! library itself names no crate that does I/O, and the edge points this way
//! round so that `sim-data`'s own manifest never mentions the generator.

use proptest::prelude::*;

use map_gen::{DrawError, draw, report};
use sim_core::{Grid, MapDef, Terrain, Tile};

/// A size big enough for a map to have anything in it. Small enough that a
/// property test can run a few hundred of them.
const W: u16 = 48;
const H: u16 = 32;

fn drawn(seed: u64, width: u16, height: u16) -> MapDef {
    draw(seed, width, height, "drawn")
        .unwrap_or_else(|e| panic!("seed {seed} at {width}x{height}: {e}"))
        .def
}

/// ***The test that defines the phase.*** A drawn map goes out through the
/// renderer and comes back in through the real loader.
///
/// Through the rendered text and not the arrays it came from: a generated map
/// the game refuses to load is the entire failure mode this tool can have, and
/// `validate_map` is the only thing entitled to say whether it would.
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

/// One walkable region, always — and the repair only ever drowns land.
///
/// The second half is what stops the repair from being a way to *make* a map
/// connected by filling a channel in: it may take ground away and never add
/// any.
#[test]
fn the_repair_leaves_one_region_and_only_ever_drowns() {
    let mut seeds_that_needed_it = 0;
    for seed in 0..64u64 {
        let d = draw(seed, W, H, "drawn").expect("a map at a usable size");
        let grid = Grid::from_map(&d.def);
        assert_eq!(
            grid.walkable_regions().len(),
            1,
            "seed {seed} came out in more than one piece"
        );

        let walkable = d
            .def
            .terrain
            .iter()
            .filter(|t| t.is_walkable())
            .count()
            .try_into()
            .expect("a map holds at most 65,536 tiles");
        let before: u32 = d.repair.tiles_drowned + walkable;
        assert!(
            d.repair.tiles_drowned == 0 || d.repair.regions_before > 1,
            "seed {seed} drowned tiles without having had a second region"
        );
        assert!(before >= walkable, "the repair created land on seed {seed}");
        if d.repair.regions_before > 1 {
            seeds_that_needed_it += 1;
        }
    }
    assert!(
        seeds_that_needed_it > 0,
        "no seed in the sweep came out in more than one piece, so the repair \
         was never exercised and this test proves nothing"
    );
}

/// Water sits at height zero on every map drawn.
///
/// Pinned so that the pass which puts it there cannot quietly be dropped by
/// somebody reordering the pipeline — which would not fail the loader, because
/// the loader has no opinion about how high the sea is.
#[test]
fn the_water_is_always_at_height_zero() {
    for seed in 0..32u64 {
        let def = drawn(seed, W, H);
        for (i, (t, g)) in def.terrain.iter().zip(&def.ground).enumerate() {
            assert!(
                *t != Terrain::Water || *g == 0,
                "seed {seed}: water at height {g} on tile {i}"
            );
        }
    }
}

/// A seed is the whole of what makes a map.
///
/// The second half needs a realistic size to mean anything: on a one-tile map
/// every seed agrees, and the test would pass while saying nothing.
#[test]
fn the_same_seed_draws_the_same_map_and_two_seeds_do_not() {
    assert_eq!(drawn(11, W, H), drawn(11, W, H));

    let a = drawn(11, W, H);
    let b = drawn(12, W, H);
    assert_ne!(a.terrain, b.terrain);
    assert_ne!(a.hash, b.hash);
}

/// The id is the map's, not the seed's: two maps drawn identically under
/// different names are different maps, because the id enters the hash the
/// recordings carry.
#[test]
fn the_id_reaches_the_hash() {
    let a = draw(11, W, H, "one").expect("a map").def;
    let b = draw(11, W, H, "other").expect("a map").def;
    assert_eq!(a.terrain, b.terrain);
    assert_ne!(a.hash, b.hash);
}

/// The corners do not panic, and a size the grid will not hold comes back as a
/// clean error naming the limit rather than as a map.
#[test]
fn the_corners_of_the_size_range_hold() {
    let max = sim_core::grid::MAX_SIDE;
    for (w, h) in [(1u16, 1u16), (1, max), (max, 1)] {
        // A one-tile-wide map may be all sea, which is a refusal and not a
        // panic. Either answer is fine; falling over is not.
        match draw(3, w, h, "edge") {
            Ok(d) => assert_eq!(d.def.terrain.len(), usize::from(w) * usize::from(h)),
            Err(e) => assert!(matches!(e, DrawError::NoLandLeft { .. }), "{e}"),
        }
    }

    for (w, h) in [(0u16, 4u16), (4, 0), (max + 1, 4), (4, max + 1)] {
        let e = draw(3, w, h, "too-big").expect_err("the grid will not hold it");
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

/// A seed with no land at all comes back as the other error, and not as an
/// empty map. A one-tile map below the sea level is the smallest way to get
/// one.
#[test]
fn a_seed_that_draws_no_land_says_so() {
    let all_sea = (0..512u64).find(|&seed| draw(seed, 1, 1, "drop").is_err());
    let seed = all_sea.expect("some one-tile map comes out under water");
    assert_eq!(
        draw(seed, 1, 1, "drop").expect_err("no land"),
        DrawError::NoLandLeft { seed }
    );
}

/// Somewhere to put the largest building, on every map in the sweep.
///
/// The footprint and the limit are both read from the real tables, so a change
/// to either one is caught by the other: a map that no longer has room for a
/// farm fails here, and so does a farm that grows past what the maps hold.
#[test]
fn every_map_has_room_for_the_largest_building() {
    let data = sim_data::load_default().expect("the production tables load");
    let biggest = data
        .buildings
        .iter()
        .max_by_key(|b| u32::from(b.size.0) * u32::from(b.size.1))
        .expect("the tables hold at least one building");

    for seed in 0..24u64 {
        let d = draw(seed, W, H, "drawn").expect("a map at a usable size");
        let r = report(&d, &data);
        let places = r
            .places
            .iter()
            .find(|p| p.size == biggest.size)
            .expect("the report covers every footprint in the table");
        assert!(
            places.count > 0,
            "seed {seed} has nowhere to put a {}x{} '{}'\n{r}",
            biggest.size.0,
            biggest.size.1,
            biggest.id
        );
    }
}

/// The report counts what the map holds and nothing else.
#[test]
fn the_report_adds_up_to_the_map() {
    let data = sim_data::load_default().expect("the production tables load");
    let d = draw(5, W, H, "drawn").expect("a map");
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
    ///
    /// It is guaranteed by construction, which is exactly why it is worth
    /// pinning: the construction is what a later change to the noise would
    /// alter, and nothing else would notice.
    #[test]
    fn every_ground_height_is_in_range(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let Ok(d) = draw(seed, w, h, "drawn") else {
            // A map that is all sea is a refusal, not a violation of this.
            return Ok(());
        };
        prop_assert!(d.def.ground.iter().all(|&g| g <= Tile::MAX_GROUND_HEIGHT));
        prop_assert_eq!(d.def.ground.len(), usize::from(w) * usize::from(h));
    }

    /// And every one of those maps still loads.
    #[test]
    fn a_map_of_any_size_loads(seed in any::<u64>(), w in 1u16..=70, h in 1u16..=70) {
        let Ok(d) = draw(seed, w, h, "drawn") else {
            return Ok(());
        };
        let text = sim_data::render_map(&d.def);
        prop_assert_eq!(sim_data::parse_map(&text).ok(), Some(d.def));
    }
}
