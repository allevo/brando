#![allow(dead_code)]
// Every integration test compiles this module on its own, so what only
// `commands.rs` needs shows up as dead in `invariants.rs` and vice versa.

//! Fixtures shared by `sim-core`'s integration tests.
//!
//! The dataset is built by hand instead of loading the production tables:
//! `sim-core` cannot depend on `sim-data` (that would be a cycle), and above
//! all a test about the **mechanics** must not break every time the balancing
//! changes. The tests that really are about the production numbers live in
//! `sim-data`.

use std::sync::Arc;

use sim_core::data::{
    BuildingDef, BuildingRole, DataSet, DemographicsRules, DifficultyDef, HouseLevelDef,
    Production, Rules, SatisfactionRules, ServiceDef,
};
use sim_core::{
    BuildingKindId, Coins, Command, DifficultyId, Grid, Level, Milli, ServiceKind, Terrain,
    TilePos, World,
};

/// A level from its number, so a test can go on saying "level 2".
pub fn level(number: u8) -> Level {
    Level::new(number).expect("levels count from 1")
}

pub const HOUSE: BuildingKindId = BuildingKindId::new(0);
pub const WELL: BuildingKindId = BuildingKindId::new(1);
pub const FARM: BuildingKindId = BuildingKindId::new(2);
/// A well big enough for one house only: it makes the capacity bite in the
/// tests, which would not be observable with thirty-two residents.
pub const SMALL_WELL: BuildingKindId = BuildingKindId::new(3);
/// A kind that does not exist in the table: this is what the LLM will produce.
pub const UNKNOWN_KIND: BuildingKindId = BuildingKindId::new(99);

pub const HOUSE_COST: i32 = 10;
pub const WELL_COST: i32 = 12;
pub const FARM_COST: i32 = 40;
pub const PLAIN_ROAD_COST: i32 = 2;
/// What a road costs on rock: dearer than plain, which is the only thing in the
/// tree that tells a cost read by terrain apart from one that always charges
/// plain.
pub const ROCK_ROAD_COST: i32 = 6;
pub const STARTING_TREASURY: i32 = 1000;
/// What a house holds at level 1, which is also what `easy` builds it with.
pub const RESIDENTS_PER_HOUSE: u16 = 4;

// The house levels of the fixture (phase 13), one entry per level. Its own
// numbers, like everything else here: a test about the mechanics must not break
// when the production balancing moves. What a test may **not** do is copy them
// — the horizons are computed from the `DataSet`, which is what makes them stay
// true if these change.
pub const LEVEL_RESIDENTS: [u16; HOUSE_LEVELS] = [RESIDENTS_PER_HOUSE, 8, 12];
/// The satisfaction it takes to rise to each level. Level 1's is unread: a
/// house is born there.
pub const LEVEL_UP: [u8; HOUSE_LEVELS] = [0, 50, 90];
/// The satisfaction below which a house at each level comes down. Level 1's is
/// unread: there is no level 0. Strictly under [`LEVEL_UP`] on the same level,
/// which is the gap.
pub const LEVEL_DECAY: [u8; HOUSE_LEVELS] = [0, 25, 60];
pub const HOUSE_LEVELS: usize = 3;

// The providers' capacity, in **residents served** and not in houses: once
// houses have levels (M1) the population per house varies. Divided by
// `RESIDENTS_PER_HOUSE` they give the houses the tests expect to see served.
pub const WELL_CAPACITY: u16 = 32;
/// As in the real tables, it is what the output sustains: 400 / 20 = 20
/// residents, five houses. The fixture has to respect the same consistency as
/// the production dataset, otherwise `sim-core`'s tests would run on a
/// balancing that `sim-data`'s validation would reject — that is what
/// `invariants.rs::the_fixture_has_no_inconsistencies` pins down.
pub const FARM_CAPACITY: u16 = 20;
pub const SMALL_WELL_CAPACITY: u16 = 4;

// The satisfaction curve of the fixture (phase 12). Deliberately divisible:
// `MAX / STEP_UP` and `MAX / STEP_DOWN` are whole numbers of ticks, so a test
// that computes its own horizon from the `DataSet` gets an exact answer instead
// of one off by a rounding.
pub const SATISFACTION_MAX: u8 = 100;
pub const SATISFACTION_STEP_UP: u8 = 4;
pub const SATISFACTION_STEP_DOWN: u8 = 10;
/// The cadence of the level review (phase 13). Divisible by nothing in
/// particular: what the tests compute from it is *the first review after* a
/// horizon, never a fixed tick.
pub const TICKS_PER_MONTH: u32 = 30;

// --- the demographics (phase 14) -------------------------------------------
//
// The fixture's own numbers, keeping production's relations: births above
// deaths, going without much worse than being served, both thresholds
// reachable. They are deliberately **large** compared with production's — a
// test that had to run five game years to see one birth would be a test nobody
// runs.
pub const BIRTHS_PER_THOUSAND: u16 = 300;
pub const DEATHS_PER_THOUSAND: u16 = 30;
pub const DEATHS_PER_THOUSAND_UNSERVED: u16 = 600;
pub const UNSERVED_THRESHOLD: u8 = 25;
pub const BIRTH_THRESHOLD: u8 = 60;
pub const JITTER_PER_THOUSAND: u16 = 200;

/// The profile the tests play on unless they say otherwise: a house is born
/// full, which is M0's behaviour and keeps every test written before phase 11
/// saying what it said.
pub const EASY: &str = "easy";
/// A house born empty. It only fills up by migration (phase 15), so in these
/// tests it stays at zero: that is exactly what makes the knob observable.
pub const HARD: &str = "hard";

pub fn dataset() -> Arc<DataSet> {
    dataset_where_a_house_requires(&[ServiceKind::Water, ServiceKind::Food])
}

/// Like [`dataset`], with **every** level declaring the same services.
///
/// It exists for the one thing M0's tables could not express: a house that does
/// **not** require a service the coverage reaches it with anyway. Coverage
/// never reads `required_services`, which is the hole, so a house that requires
/// only water is still assigned a farm — and its food satisfaction has to stay
/// still all the same.
pub fn dataset_where_a_house_requires(required: &[ServiceKind]) -> Arc<DataSet> {
    dataset_with_levels_requiring(&[required; HOUSE_LEVELS])
}

/// Like [`dataset`], with a level that asks for less than the level above it:
/// level 1 wants water alone, the two above want water and food.
///
/// This is the shape the production tables have since phase 13, and the one
/// that makes the per-level requirements do real work — with three identical
/// levels the mechanism is there but nothing distinguishes it from reading the
/// building's union.
pub fn dataset_with_service_levels() -> Arc<DataSet> {
    const BOTH: &[ServiceKind] = &[ServiceKind::Water, ServiceKind::Food];
    dataset_with_levels_requiring(&[&[ServiceKind::Water], BOTH, BOTH])
}

/// The fixture, with the requirements of each level spelled out.
///
/// The building's `required_services` is derived as the **union** of the levels,
/// never written by hand: it is what `is_house()` reads, and a fixture where
/// the two disagreed would be one `Inconsistency::InconsistentRequirements`
/// away from failing for a reason that has nothing to do with the test using
/// it.
pub fn dataset_with_levels_requiring(levels: &[&[ServiceKind]]) -> Arc<DataSet> {
    dataset_built(levels, FarmOutput::AsProduction)
}

/// Like [`dataset`], with a farm that keeps its declared capacity and grows
/// **nothing**: no `output_per_tick` and no `max_stock`.
///
/// The only constructor here that deliberately builds a dataset
/// `inconsistencies()` refuses, and it is the counterpart of
/// `the_fixture_has_no_inconsistencies`: that test says the fixture is clean,
/// this one gives the check something to catch. Without it, a hole in
/// `check_food_capacity` leaves both green — which is how the hole survived.
/// The fixture with the demographics **switched off**: no births, no deaths.
///
/// What `coverage_equivalence` runs on. See [`Rates`] for why that matters and
/// what breaks if somebody puts the real rates back.
pub fn dataset_without_demographics() -> Arc<DataSet> {
    const BOTH: &[ServiceKind] = &[ServiceKind::Water, ServiceKind::Food];
    dataset_built_with(&[BOTH; HOUSE_LEVELS], FarmOutput::AsProduction, Rates::Off)
}

/// A world on the fixture with the demographics switched off.
pub fn world_without_demographics() -> World {
    world_with(dataset_without_demographics(), 32, 32, EASY)
}

pub fn dataset_with_a_farm_that_grows_nothing() -> Arc<DataSet> {
    const BOTH: &[ServiceKind] = &[ServiceKind::Water, ServiceKind::Food];
    dataset_built(&[BOTH; HOUSE_LEVELS], FarmOutput::None)
}

/// Whether the fixture's farm produces what its capacity claims.
enum FarmOutput {
    AsProduction,
    None,
}

/// Whether the fixture's city can grow and shrink.
///
/// `Off` is what `coverage_equivalence` runs on, and the reason is worth
/// stating where somebody tempted to "fix" it will read it: that test compares
/// the stored coverage against a from-scratch one **at the end of the tick**,
/// and with the demographics running the two always diverge — not because of a
/// bug, but because step 6 moved somebody after step 3 assigned. Put the real
/// rates back and the project's most valuable oracle stops checking anything
/// without ever going red.
#[derive(Clone, Copy)]
pub enum Rates {
    AsProduction,
    Off,
}

/// The fixture's demographic rates. Like every other number here they are the
/// fixture's own, but they keep production's relations: births above deaths,
/// going without much worse than being served, both thresholds reachable.
fn demographics_rules(rates: Rates) -> DemographicsRules {
    match rates {
        Rates::AsProduction => DemographicsRules {
            births_per_thousand_per_month: BIRTHS_PER_THOUSAND,
            deaths_per_thousand_per_month: DEATHS_PER_THOUSAND,
            deaths_per_thousand_per_month_when_unserved: DEATHS_PER_THOUSAND_UNSERVED,
            unserved_threshold: UNSERVED_THRESHOLD,
            birth_threshold: BIRTH_THRESHOLD,
            jitter_per_thousand: JITTER_PER_THOUSAND,
        },
        Rates::Off => DemographicsRules {
            births_per_thousand_per_month: 0,
            deaths_per_thousand_per_month: 0,
            deaths_per_thousand_per_month_when_unserved: 0,
            unserved_threshold: UNSERVED_THRESHOLD,
            birth_threshold: BIRTH_THRESHOLD,
            jitter_per_thousand: JITTER_PER_THOUSAND,
        },
    }
}

fn dataset_built(levels: &[&[ServiceKind]], farm_output: FarmOutput) -> Arc<DataSet> {
    dataset_built_with(levels, farm_output, Rates::AsProduction)
}

fn dataset_built_with(
    levels: &[&[ServiceKind]],
    farm_output: FarmOutput,
    rates: Rates,
) -> Arc<DataSet> {
    let house_levels: Vec<HouseLevelDef> = levels
        .iter()
        .enumerate()
        .map(|(i, required)| HouseLevelDef {
            max_residents: LEVEL_RESIDENTS[i.min(HOUSE_LEVELS - 1)],
            required_services: required.to_vec(),
            level_up_threshold: LEVEL_UP[i.min(HOUSE_LEVELS - 1)],
            decay_threshold: LEVEL_DECAY[i.min(HOUSE_LEVELS - 1)],
            taxable_per_resident: Milli::ZERO,
        })
        .collect();

    let mut union: Vec<ServiceKind> = levels.iter().flat_map(|l| l.iter().copied()).collect();
    union.sort_unstable();
    union.dedup();

    let rules = Rules {
        ticks_per_month: TICKS_PER_MONTH,
        months_per_year: 12,
        starting_treasury: Coins::new(STARTING_TREASURY),
        house_levels,
        food_per_resident: Milli::from_millis(20),
        demographics: demographics_rules(rates),
        satisfaction: SatisfactionRules {
            max: SATISFACTION_MAX,
            step_up: SATISFACTION_STEP_UP,
            step_down: SATISFACTION_STEP_DOWN,
            mood_thresholds: [25, 50, 75],
        },
    };

    // Assigned by index rather than positionally: the declaration order of
    // `Terrain` is frozen because the hashes store a terrain by its position,
    // and naming the variant keeps this fixture readable without depending on
    // that order. Water keeps a cost of zero, which is also what the loader
    // demands of ground no road can be laid on.
    let mut road_cost_per_terrain = [Coins::ZERO; Terrain::COUNT];
    road_cost_per_terrain[Terrain::Plain.index()] = Coins::new(PLAIN_ROAD_COST);
    road_cost_per_terrain[Terrain::Rock.index()] = Coins::new(ROCK_ROAD_COST);

    let buildings = vec![
        BuildingDef {
            id: "house".into(),
            size: (1, 1),
            cost: Coins::new(HOUSE_COST),
            levels: u8::try_from(levels.len()).expect("the fixture has few levels"),
            role: BuildingRole::House {
                required_services: union,
            },
            production: None,
        },
        BuildingDef {
            id: "well".into(),
            size: (1, 1),
            cost: Coins::new(WELL_COST),
            levels: 1,
            role: BuildingRole::Provider {
                service: ServiceDef {
                    kind: ServiceKind::Water,
                    range_per_level: vec![12],
                    // Residents, not houses: eight houses of four.
                    capacity_per_level: vec![WELL_CAPACITY],
                },
            },
            production: None,
        },
        BuildingDef {
            id: "farm".into(),
            size: (2, 2),
            cost: Coins::new(FARM_COST),
            levels: 1,
            role: BuildingRole::Provider {
                service: ServiceDef {
                    kind: ServiceKind::Food,
                    range_per_level: vec![10],
                    capacity_per_level: vec![FARM_CAPACITY],
                },
            },
            // The farm is the one building that is a provider **and** a
            // producer, which is why production is its own field and not a role
            // of its own: no single variant could hold both.
            production: match farm_output {
                FarmOutput::AsProduction => Some(Production {
                    output_per_tick: Milli::from_millis(400),
                    max_stock: Milli::from_millis(20_000),
                }),
                FarmOutput::None => None,
            },
        },
        BuildingDef {
            id: "small_well".into(),
            size: (1, 1),
            cost: Coins::new(WELL_COST),
            levels: 1,
            role: BuildingRole::Provider {
                service: ServiceDef {
                    kind: ServiceKind::Water,
                    range_per_level: vec![12],
                    capacity_per_level: vec![SMALL_WELL_CAPACITY],
                },
            },
            production: None,
        },
    ];

    // The same three profiles as production, and with the same meaning: `easy`
    // fills a new house up to `RESIDENTS_PER_HOUSE`, `hard` leaves it empty.
    // The values are the fixture's own, like every other number here.
    let difficulties = vec![
        DifficultyDef {
            id: EASY.into(),
            starting_residents_per_house: RESIDENTS_PER_HOUSE,
        },
        DifficultyDef {
            id: "normal".into(),
            starting_residents_per_house: RESIDENTS_PER_HOUSE / 2,
        },
        DifficultyDef {
            id: HARD.into(),
            starting_residents_per_house: 0,
        },
    ];

    Arc::new(DataSet::new(
        rules,
        road_cost_per_terrain,
        buildings,
        difficulties,
    ))
}

/// Resolves a profile in the fixture's dataset.
pub fn difficulty(data: &DataSet, id: &str) -> DifficultyId {
    data.difficulty_by_id(id)
        .unwrap_or_else(|| panic!("the fixture must contain the '{id}' profile"))
}

/// A test world: a 32x32 grid of plain, fixed seed.
pub fn world() -> World {
    world_of(32, 32)
}

pub fn world_of(w: u16, h: u16) -> World {
    world_at(w, h, EASY)
}

/// Like [`world_of`], on a chosen difficulty profile.
pub fn world_at(w: u16, h: u16, profile: &str) -> World {
    world_with(dataset(), w, h, profile)
}

/// Like [`world_at`], on a dataset of your own.
pub fn world_with(data: Arc<DataSet>, w: u16, h: u16, profile: &str) -> World {
    world_seeded_with(data, w, h, profile, SEED)
}

/// The seed every fixture world uses unless a test asks for another.
pub const SEED: u64 = 42;

/// Like [`world`], on a seed of your choosing.
///
/// It exists for phase 14: until something drew from the RNG, one seed was as
/// good as another and nothing needed to vary it.
pub fn world_seeded(seed: u64) -> World {
    world_seeded_with(dataset(), 32, 32, EASY, seed)
}

/// Like [`world_without_demographics`], on a seed of your choosing.
pub fn world_seeded_without_demographics(seed: u64) -> World {
    world_seeded_with(dataset_without_demographics(), 32, 32, EASY, seed)
}

pub fn world_seeded_with(data: Arc<DataSet>, w: u16, h: u16, profile: &str, seed: u64) -> World {
    let grid = Grid::new(w, h, Terrain::Plain).expect("valid dimensions");
    let difficulty = difficulty(&data, profile);
    World::new(grid, data, seed, difficulty)
}

/// Two worlds at the start of the same game: the same tables, the same grid,
/// the same seed.
///
/// It is what a test uses in place of copying a world, which `World` does not
/// allow. A world is `seed + Vec<Command>` (D4), so the way to a second
/// world holding a given state is to play the same game again — and the pair
/// says something the copy could not: *a tick in which every command was
/// rejected leaves the world where an empty tick would have left it*.
///
/// The two share one `Arc<DataSet>`, so the tables are the same tables and not
/// merely equal ones. That is also the reason the field is an `Arc` at all
///: the second world costs a refcount bump.
pub fn twins() -> (World, World) {
    twins_of(32, 32)
}

/// Like [`twins`], on a grid of your choosing.
pub fn twins_of(w: u16, h: u16) -> (World, World) {
    let data = dataset();
    (
        world_seeded_with(Arc::clone(&data), w, h, EASY, SEED),
        world_seeded_with(data, w, h, EASY, SEED),
    )
}

/// `Ok` when the two worlds are the same game, and which field parted them
/// otherwise.
///
/// A thin reading of [`World::first_difference`], which is the exhaustive
/// comparison: it covers the derived and diagnostic structures as well as the
/// state, and it checks the tick before anything else, so two worlds that were
/// not played the same number of times are reported as such instead of
/// agreeing about an empty grid.
pub fn same_game(a: &World, b: &World) -> Result<(), String> {
    match a.first_difference(b) {
        None => Ok(()),
        Some(field) => Err(format!(
            "the two worlds parted on `{field}`, at tick {} against {}",
            a.tick(),
            b.tick()
        )),
    }
}

/// Applies the commands in a single tick and returns the report.
pub fn tick(world: &mut World, cmds: &[Command]) -> sim_core::StepReport {
    sim_core::step(world, cmds)
}

pub fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}
