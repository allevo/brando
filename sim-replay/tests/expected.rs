//! Phase 08 — the project's most valuable test.
//!
//! From here on, every source of non-determinism introduced by accident shows
//! up as a hash that changes, within one commit of being introduced, instead of
//! as an inexplicable balancing bug six months later.

use std::sync::Arc;

use sim_core::{Coins, DataSet, Level, TilePos, World};
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
        sim_data::from_ron_str(
            &read("rules.ron"),
            &read("terrain.ron"),
            &buildings,
            &read("difficulty.ron"),
        )
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

/// Version 1 is the M0 header, which has no difficulty: reading it as if it
/// were a version 2 would start a game on a profile nobody chose.
#[test]
fn the_previous_format_version_is_refused() {
    let data = data();
    let mut rec = recording("minimal");
    rec.header.format_version = 1;
    assert!(matches!(
        sim_replay::replay(&rec, data, 1),
        Err(ReplayError::UnsupportedFormat { found: 1, .. })
    ));
}

// --- 5. difficulty (phase 11) -----------------------------------------------

/// The phase's goal, in three parts.
///
/// The difficulty byte enters the state hash at tick 0, so two games on
/// different profiles differ **from the start** — that is the first part, and it
/// is what makes a recording attributable to the profile it was played on.
///
/// The other two are the ones that say the knob acts *where it should and
/// nowhere else*, and they can only be asked net of the byte itself: with both
/// worlds forced onto the same profile, the states have to be identical before
/// the first house is built and different after. Without the normalisation the
/// question cannot even be put, because the byte alone would answer it.
#[test]
fn the_difficulty_acts_on_new_houses_and_nowhere_else() {
    let data = data();
    let easy = recording("minimal");
    assert_eq!(easy.header.difficulty, "easy", "the committed profile");
    let mut hard = easy.clone();
    hard.header.difficulty = "hard".into();

    let easy_id = data.difficulty_by_id("easy").expect("the easy profile");

    // The scenario builds its houses at tick 2, so `until = 2` is the last
    // state in which no house exists yet.
    const BEFORE_THE_HOUSES: u32 = 2;
    const AFTER_THE_HOUSES: u32 = 3;

    // 1. The byte travels in the hash: different from tick 0, before anything
    //    at all has happened.
    let a = sim_replay::replay(&easy, Arc::clone(&data), 0).expect("replay");
    let b = sim_replay::replay(&hard, Arc::clone(&data), 0).expect("replay");
    assert_ne!(
        hash_hex(&hash_world(&a)),
        hash_hex(&hash_world(&b)),
        "the difficulty must enter the state hash from tick 0"
    );

    // 2. Net of the byte, nothing else has changed before the first house.
    let a = sim_replay::replay(&easy, Arc::clone(&data), BEFORE_THE_HOUSES).expect("replay");
    let mut b = sim_replay::replay(&hard, Arc::clone(&data), BEFORE_THE_HOUSES).expect("replay");
    assert_eq!(b.house_count(), 0, "no house has been built yet");
    b.set_difficulty(easy_id);
    assert_eq!(
        hash_hex(&hash_world(&a)),
        hash_hex(&hash_world(&b)),
        "the difficulty acts somewhere other than on a newly-built house"
    );

    // 3. And from the first house on, it does change the state.
    let a = sim_replay::replay(&easy, Arc::clone(&data), AFTER_THE_HOUSES).expect("replay");
    let mut b = sim_replay::replay(&hard, Arc::clone(&data), AFTER_THE_HOUSES).expect("replay");
    assert!(b.house_count() > 0, "the houses are there");
    assert_eq!(b.population(), 0, "at hard a house is born empty");
    assert!(a.population() > 0, "at easy it is not");
    b.set_difficulty(easy_id);
    assert_ne!(
        hash_hex(&hash_world(&a)),
        hash_hex(&hash_world(&b)),
        "the difficulty does not change the state where it should"
    );
}

/// `easy` is M0's behaviour: a house born at its full level-1 capacity.
///
/// It pins down that `starting_residents_per_house` is the profile's **only**
/// effect — if it grew a second one by accident, the population after a game
/// year would stop being exactly the houses times their capacity.
#[test]
fn easy_fills_a_house_the_way_m0_did() {
    let data = data();
    let max = data
        .rules
        .max_residents(Level::FIRST)
        .expect("houses have a level 1");

    for name in SCENARIOS {
        let rec = recording(name);
        let w = sim_replay::replay(&rec, Arc::clone(&data), until(&data)).expect("replay");

        assert!(w.house_count() > 0, "scenario {name} builds no houses");
        assert_eq!(
            w.population(),
            w.house_count() as u32 * u32::from(max),
            "scenario {name}: at easy every house is full"
        );
        for (_, h) in w.houses() {
            assert_eq!(h.residents, max, "scenario {name}");
        }
    }
}

/// The header survives a round trip as the **textual** id, and a profile that
/// does not exist is a structured error — not a panic, and above all not a
/// silent fallback onto some other game.
#[test]
fn the_header_carries_the_profile_by_name() {
    let data = data();
    let rec = recording("minimal");

    let text = rec.to_ron().expect("serialisable");
    assert!(
        text.contains(r#"difficulty: "easy""#),
        "the recording has to stay readable: {text:.400}"
    );
    let back: Recording = ron::from_str(&text).expect("readable");
    assert_eq!(back.header, rec.header);

    let mut unknown = rec.clone();
    unknown.header.difficulty = "impossible".into();
    match sim_replay::replay(&unknown, data, 1) {
        Err(ReplayError::UnknownDifficulty { found, known }) => {
            assert_eq!(found, "impossible");
            assert!(known.contains("easy"), "the known ones are listed: {known}");
        }
        other => panic!("expected UnknownDifficulty, found {other:?}"),
    }
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

    // The compile-time half of the same guard: this call does nothing at
    // runtime, but `World::field_canary` stops compiling the moment a field is
    // added to the state — which is the reminder that a perturbation for it
    // belongs in the list below. Without it, this test stays green on a field
    // the hash has gone blind on.
    base.field_canary();

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
            let b = w.building_mut(id).expect("alive");
            b.level = b.level.next().expect("a rung above");
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
        // Hashed since M0, but it only started meaning anything in phase 13:
        // a perturbation of its own is what says the recordings would notice a
        // change to the levelling rules and not merely to the tables.
        ("a house's level", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("a house");
            let h = w.house_mut(id).expect("alive");
            h.level = h.level.next().expect("a rung above");
        }),
        ("a house's services", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("a house");
            let h = w.house_mut(id).expect("alive");
            let current = h.served.get(sim_core::ServiceKind::Water);
            h.served.set(sim_core::ServiceKind::Water, !current);
        }),
        ("a house's satisfaction", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("a house");
            let h = w.house_mut(id).expect("alive");
            let k = sim_core::ServiceKind::Food.index();
            h.satisfaction[k] = h.satisfaction[k].wrapping_sub(1);
        }),
        // The walkers are empty until M3, so this is the one perturbation that
        // cannot arise from replaying anything: it has to be put there by hand.
        // That is exactly why it was missing — and why `field_canary` could not
        // help, since `walkers: _` was already written into it.
        ("a walker", |w| w.push_walker(TilePos::new(3, 4))),
        ("the treasury", |w| {
            w.economy_mut().treasury = w.economy().treasury.saturating_add(Coins::new(1));
        }),
        ("the difficulty", |w| {
            let other = w
                .data()
                .difficulty_by_id("hard")
                .expect("the hard profile exists");
            w.set_difficulty(other);
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
        assert!(
            data.difficulty_by_id(&rec.header.difficulty).is_some(),
            "scenario {name}: the header names a profile that does not exist"
        );
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
