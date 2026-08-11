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

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_core::data::{
    BuildingDef, DataSet, DifficultyDef, Rules, SatisfactionRules, ServiceDef, TerrainDef,
};
use sim_core::{
    BuildingKindId, Coins, Command, DifficultyId, Grid, Milli, ServiceKind, Terrain, TilePos, World,
};

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
pub const STARTING_TREASURY: i32 = 1000;
pub const RESIDENTS_PER_HOUSE: u16 = 4;

// The providers' capacity, in **residents served** and not in houses: once
// houses have levels (M1) the population per house varies. Divided by
// `RESIDENTS_PER_HOUSE` they give the houses the tests expect to see served.
pub const WELL_CAPACITY: u16 = 32;
/// As in the real tables, it is what the output sustains: 400 / 20 = 20
/// residents, five houses. The fixture has to respect the same consistency as
/// the production dataset, otherwise `sim-core`'s tests would run on a
/// balancing that `sim-data`'s validation would reject — that is what
/// `invariants.rs::the_fixture_keeps_capacity_and_output_consistent` pins down.
pub const FARM_CAPACITY: u16 = 20;
pub const SMALL_WELL_CAPACITY: u16 = 4;

// The satisfaction curve of the fixture (phase 12). Deliberately divisible:
// `MAX / STEP_UP` and `MAX / STEP_DOWN` are whole numbers of ticks, so a test
// that computes its own horizon from the `DataSet` gets an exact answer instead
// of one off by a rounding.
pub const SATISFACTION_MAX: u8 = 100;
pub const SATISFACTION_STEP_UP: u8 = 4;
pub const SATISFACTION_STEP_DOWN: u8 = 10;

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

/// Like [`dataset`], with the house declaring the services it wants.
///
/// It exists for the one thing today's tables cannot express: a house that does
/// **not** require a service the coverage reaches it with anyway. Coverage
/// never reads `required_services` (the gap A9 names), so a house that requires
/// only water is still assigned a farm — and its food satisfaction has to stay
/// still all the same. Phase 13, which gives each level its own requirements,
/// makes the case real.
pub fn dataset_where_a_house_requires(required: &[ServiceKind]) -> Arc<DataSet> {
    let rules = Rules {
        ticks_per_month: 30,
        months_per_year: 12,
        starting_treasury: Coins::new(STARTING_TREASURY),
        residents_per_house_level: vec![RESIDENTS_PER_HOUSE],
        food_per_resident: Milli::from_millis(20),
        satisfaction: SatisfactionRules {
            max: SATISFACTION_MAX,
            step_up: SATISFACTION_STEP_UP,
            step_down: SATISFACTION_STEP_DOWN,
            mood_thresholds: [25, 50, 75],
        },
    };

    let mut terrain = BTreeMap::new();
    terrain.insert(
        Terrain::Plain,
        TerrainDef {
            buildable: true,
            walkable: true,
            road_cost: Coins::new(PLAIN_ROAD_COST),
        },
    );
    terrain.insert(
        Terrain::Water,
        TerrainDef {
            buildable: false,
            walkable: false,
            road_cost: Coins::new(0),
        },
    );
    terrain.insert(
        Terrain::Rock,
        TerrainDef {
            buildable: false,
            walkable: true,
            road_cost: Coins::new(6),
        },
    );

    let buildings = vec![
        BuildingDef {
            id: "house".into(),
            size: (1, 1),
            cost: Coins::new(HOUSE_COST),
            levels: 1,
            service: None,
            required_services: required.to_vec(),
            output_per_tick: None,
            max_stock: None,
        },
        BuildingDef {
            id: "well".into(),
            size: (1, 1),
            cost: Coins::new(WELL_COST),
            levels: 1,
            service: Some(ServiceDef {
                kind: ServiceKind::Water,
                range_per_level: vec![12],
                // Residents, not houses: eight houses of four.
                capacity_per_level: vec![WELL_CAPACITY],
            }),
            required_services: vec![],
            output_per_tick: None,
            max_stock: None,
        },
        BuildingDef {
            id: "farm".into(),
            size: (2, 2),
            cost: Coins::new(FARM_COST),
            levels: 1,
            service: Some(ServiceDef {
                kind: ServiceKind::Food,
                range_per_level: vec![10],
                capacity_per_level: vec![FARM_CAPACITY],
            }),
            required_services: vec![],
            output_per_tick: Some(Milli::from_millis(400)),
            max_stock: Some(Milli::from_millis(20_000)),
        },
        BuildingDef {
            id: "small_well".into(),
            size: (1, 1),
            cost: Coins::new(WELL_COST),
            levels: 1,
            service: Some(ServiceDef {
                kind: ServiceKind::Water,
                range_per_level: vec![12],
                capacity_per_level: vec![SMALL_WELL_CAPACITY],
            }),
            required_services: vec![],
            output_per_tick: None,
            max_stock: None,
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

    Arc::new(DataSet::new(rules, terrain, buildings, difficulties))
}

/// Resolves a profile in the fixture's dataset.
pub fn difficulty(data: &DataSet, id: &str) -> DifficultyId {
    data.difficulty_by_id(id)
        .unwrap_or_else(|| panic!("the fixture must contain the '{id}' profile"))
}

/// A test world: a 32x32 grid of plain (A4), fixed seed.
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
    let grid = Grid::new(w, h, Terrain::Plain).expect("valid dimensions");
    let difficulty = difficulty(&data, profile);
    World::new(grid, data, 42, difficulty)
}

/// Applies the commands in a single tick and returns the report.
pub fn tick(world: &mut World, cmds: &[Command]) -> sim_core::StepReport {
    sim_core::step(world, cmds)
}

pub fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}
