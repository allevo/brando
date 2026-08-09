//! Phase 08 — the project's most valuable test.
//!
//! From here on, every source of non-determinism introduced by accident shows
//! up as a hash that changes, within one commit of being introduced, instead of
//! as an inexplicable balancing bug six months later.

use std::sync::Arc;

use sim_core::{Coins, DataSet, TilePos, World};
use sim_replay::{CHECKPOINT_EVERY, Recording, ReplayError, checkpoints, hash_hex, hash_world};

const SCENARIOS: [&str; 2] = ["minimal", "hunger"];

fn data() -> Arc<DataSet> {
    Arc::new(sim_data::load_default().expect("the production tables must load"))
}

fn recording(name: &str) -> Recording {
    let p = sim_replay::expected_dir().join(format!("{name}.ron"));
    Recording::load(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn committed_hashes(name: &str) -> Vec<(u32, String)> {
    let p = sim_replay::expected_dir().join(format!("{name}.hashes"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    sim_replay::expected::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn until(data: &DataSet) -> u32 {
    data.rules.ticks_per_year()
}

// --- 1. determinism within one process --------------------------------------

#[test]
fn the_same_replay_twice_gives_the_same_hashes() {
    let data = data();
    for name in SCENARIOS {
        let rec = recording(name);
        let a = checkpoints(&rec, Arc::clone(&data), until(&data), CHECKPOINT_EVERY)
            .expect("valid replay");
        let b = checkpoints(&rec, Arc::clone(&data), until(&data), CHECKPOINT_EVERY)
            .expect("valid replay");
        assert_eq!(a, b, "scenario {name}");
        assert!(!a.is_empty(), "scenario {name} produces no checkpoints");
    }
}

// --- 2. determinism across processes (the recorded replay proper) -----------

/// Catches what test 1 cannot: dependence on memory addresses, on
/// `RandomState`, on the iteration order of a hash collection.
///
/// If it fails without the balancing having changed, do not regenerate: a
/// source of non-determinism has been introduced.
#[test]
fn the_hashes_match_the_committed_ones() {
    let data = data();
    for name in SCENARIOS {
        let rec = recording(name);
        let computed = checkpoints(&rec, Arc::clone(&data), until(&data), CHECKPOINT_EVERY)
            .expect("valid replay");
        let expected = committed_hashes(name);

        assert_eq!(
            computed.len(),
            expected.len(),
            "scenario {name}: different number of checkpoints"
        );
        for (c, (tick, hex)) in computed.iter().zip(expected) {
            assert_eq!(c.tick, tick, "scenario {name}: misaligned ticks");
            assert_eq!(
                hash_hex(&c.hash),
                hex,
                "scenario {name}: different hash at tick {tick}"
            );
        }
    }
}

// --- 3. determinism of partial execution ------------------------------------

/// Stopping halfway and picking back up has to give the same state as a single
/// run. It catches the "hidden" state rebuilt wrongly on restart — the road
/// network and the coverage, which are not in the save file.
#[test]
fn stopping_halfway_and_resuming_gives_the_same_state() {
    let data = data();
    for name in SCENARIOS {
        let rec = recording(name);
        let halfway = until(&data) / 2;

        let one_run = sim_replay::replay(&rec, Arc::clone(&data), until(&data)).expect("replay");

        let mut split = sim_replay::replay(&rec, Arc::clone(&data), halfway).expect("replay");
        sim_replay::advance(&mut split, &rec, until(&data));

        assert_eq!(
            hash_hex(&hash_world(&one_run)),
            hash_hex(&hash_world(&split)),
            "scenario {name}: resuming halfway changes the state"
        );
    }
}

// --- 4. dataset mismatch ----------------------------------------------------

/// With one number in the dataset altered, the replay has to fail **at once**
/// and for the right reason, not diverge silently at tick 200.
#[test]
fn a_different_dataset_fails_with_datasetmismatch() {
    let dir = sim_data::production_data_dir();
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).expect("readable table");
    let buildings = read("buildings.ron").replacen(
        "output_per_tick: Some(400)",
        "output_per_tick: Some(401)",
        1,
    );
    let altered = Arc::new(
        sim_data::from_ron_str(&read("rules.ron"), &read("terrain.ron"), &buildings)
            .expect("the altered table is still valid"),
    );

    let rec = recording("minimal");
    match sim_replay::replay(&rec, altered, 10) {
        Err(ReplayError::DatasetMismatch { expected, found }) => {
            assert_eq!(expected, rec.header.dataset_hash);
            assert_ne!(found, expected);
        }
        other => panic!("expected DatasetMismatch, found {other:?}"),
    }
}

#[test]
fn an_unknown_format_version_is_an_error() {
    let data = data();
    let mut rec = recording("minimal");
    rec.header.format_version = sim_replay::FORMAT_VERSION + 1;
    assert!(matches!(
        sim_replay::replay(&rec, data, 1),
        Err(ReplayError::UnsupportedFormat { .. })
    ));
}

// --- 6. the hash covers the whole state -------------------------------------

/// A mitigation of A3's known risk: the hash is written by hand, so a new field
/// can stay outside it without anyone noticing — and from that moment the
/// recordings are **blind** on that field.
///
/// If this test fails, `hash_world` has stopped covering a field of the state.
/// Do not silence it: add the field to `hash_world`.
#[test]
fn the_hash_covers_the_whole_state() {
    let data = data();
    let rec = recording("minimal");
    let base = sim_replay::replay(&rec, Arc::clone(&data), 100).expect("replay");
    let h0 = hash_world(&base);

    /// A perturbation of a single field of the state.
    type Perturbation = (&'static str, fn(&mut World));

    let perturbations: Vec<Perturbation> = vec![
        ("the tick", |w| {
            sim_core::step(w, &[]);
        }),
        ("a tile", |w| {
            w.set_terrain(TilePos::new(0, 0), sim_core::Terrain::Rock);
        }),
        ("a building", |w| {
            let id = w.buildings().next().map(|(id, _)| id).expect("a building");
            w.building_mut(id).expect("alive").level += 1;
        }),
        ("a building's stock", |w| {
            let id = w.buildings().next().map(|(id, _)| id).expect("a building");
            let b = w.building_mut(id).expect("alive");
            b.stock = b.stock.saturating_add(sim_core::Milli::from_millis(1));
        }),
        ("a house", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("a house");
            w.house_mut(id).expect("alive").residents += 1;
        }),
        ("a house's services", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("a house");
            let h = w.house_mut(id).expect("alive");
            let current = h.served.get(sim_core::ServiceKind::Water);
            h.served.set(sim_core::ServiceKind::Water, !current);
        }),
        ("the treasury", |w| {
            w.economy_mut().treasury = w.economy().treasury.saturating_add(Coins::new(1));
        }),
    ];

    for (what, perturb) in perturbations {
        let mut w = base.clone();
        perturb(&mut w);
        assert_ne!(
            hash_hex(&hash_world(&w)),
            hash_hex(&h0),
            "the hash does not cover {what}: from here on the recordings are blind on that field"
        );
    }

    // The position of every RNG stream, one domain at a time.
    for d in sim_core::RngDomain::ALL {
        let mut w = base.clone();
        w.consume_rng(d);
        assert_ne!(
            hash_hex(&hash_world(&w)),
            hash_hex(&h0),
            "the hash does not cover the position of the {d:?} stream"
        );
    }
}

/// The derived structures must **not** enter the hash: if they did, a bug in an
/// incremental rebuild would show up as a hash divergence instead of a failing
/// equivalence test (phase 06).
#[test]
fn the_derived_structures_stay_out_of_the_hash() {
    let data = data();
    let rec = recording("minimal");
    let a = sim_replay::replay(&rec, Arc::clone(&data), 100).expect("replay");

    // The same state, but with different diagnostic counters: the network has
    // been rebuilt more times because the game was split in two.
    let mut b = sim_replay::replay(&rec, Arc::clone(&data), 50).expect("replay");
    sim_replay::advance(&mut b, &rec, 100);

    assert_eq!(hash_hex(&hash_world(&a)), hash_hex(&hash_world(&b)));
}

// --- 7. sensitivity to the seed (expected red in M0) ------------------------

/// A different seed with the same commands ⇒ a different hash, **as soon as**
/// an RNG domain is used.
///
/// In M0 no system draws from the RNG: migration and random events are M1. So
/// the test is expected to fail, and it is written now because now is when you
/// can see why it is needed. To be re-enabled with migration (M1, phase 2),
/// which is the first real use of `RngDomain::Migration`.
#[test]
#[ignore = "in M0 no system uses the RNG: re-enable with migration (M1)"]
fn different_seeds_give_different_hashes() {
    let data = data();
    let rec = recording("minimal");
    let mut other = rec.clone();
    other.header.seed = rec.header.seed + 1;

    let a = sim_replay::replay(&rec, Arc::clone(&data), until(&data)).expect("replay");
    let b = sim_replay::replay(&other, Arc::clone(&data), until(&data)).expect("replay");
    assert_ne!(hash_hex(&hash_world(&a)), hash_hex(&hash_world(&b)));
}

// --- the shape of the recordings --------------------------------------------

#[test]
fn the_recordings_match_the_dataset_and_are_readable() {
    let data = data();
    for name in SCENARIOS {
        let rec = recording(name);
        assert_eq!(rec.header.dataset_hash, data.hash_hex(), "scenario {name}");
        assert_eq!(rec.header.format_version, sim_replay::FORMAT_VERSION);
        assert!(!rec.commands.is_empty(), "scenario {name} has no commands");

        let mut previous = 0;
        for (t, _) in &rec.commands {
            assert!(*t >= previous, "scenario {name}: ticks out of order");
            previous = *t;
        }

        let hashes = committed_hashes(name);
        assert!(!hashes.is_empty(), "scenario {name} has no checkpoints");
        for (_, hex) in &hashes {
            assert_eq!(hex.len(), 64, "malformed hash in {name}");
        }
    }
}
