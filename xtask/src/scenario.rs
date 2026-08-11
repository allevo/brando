//! Test scenarios for the headless runner.
//!
//! They are not the game's scenarios: those have objectives and victory
//! conditions and arrive with `sim-scenario` in M1 (D7). These are situations
//! built by hand to watch the numbers move, and to give the recorded replays of
//! phase 08 something to contain.

use std::sync::Arc;

use sim_core::{Command, DataSet, DifficultyId, Grid, Terrain, TilePos, World};

pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub seed: u64,
    pub side: u16,
    /// The profile the scenario is played on. It is state, so it travels in the
    /// recording's header and goes into the hash (A13).
    pub difficulty: DifficultyId,
    /// The commands to apply, each with the tick it belongs to.
    pub commands: Vec<(u32, Command)>,
}

/// The profile the committed recordings are recorded on.
///
/// `easy` on purpose: it puts the same residents in a new house as M0 did, so
/// phase 11's regeneration is attributable to the header and the difficulty
/// byte alone, and nothing in the simulation moves with it. A recording on a
/// different profile arrives when a profile has knobs that really change the
/// simulation (phase 14): the full scenario × profile matrix would triple the
/// files and add no coverage, because the determinism is the same.
pub const RECORDED_DIFFICULTY: &str = "easy";

pub fn by_name(name: &str, data: &DataSet) -> Option<Scenario> {
    match name {
        "minimal" => Some(minimal(data)),
        "hunger" => Some(hunger(data)),
        _ => None,
    }
}

pub const NAMES: [&str; 2] = ["minimal", "hunger"];

fn kind(data: &DataSet, id: &str) -> sim_core::BuildingKindId {
    data.kind_by_id(id)
        .unwrap_or_else(|| panic!("the dataset must contain '{id}'"))
}

fn difficulty(data: &DataSet, id: &str) -> DifficultyId {
    data.difficulty_by_id(id)
        .unwrap_or_else(|| panic!("the dataset must contain the '{id}' profile"))
}

/// A road, a well, a farm and four houses: all of them served, food to spare.
/// This is the city that works.
fn minimal(data: &DataSet) -> Scenario {
    let house = kind(data, "house");
    let well = kind(data, "well");
    let farm = kind(data, "farm");

    let mut commands = Vec::new();
    for x in 1..=16u8 {
        commands.push((0, Command::PlaceRoad { at: pos(x, 8) }));
    }
    commands.push((
        1,
        Command::PlaceBuilding {
            kind: well,
            origin: pos(2, 7),
        },
    ));
    commands.push((
        1,
        Command::PlaceBuilding {
            kind: farm,
            origin: pos(4, 6),
        },
    ));
    for i in 0..4u8 {
        commands.push((
            2,
            Command::PlaceBuilding {
                kind: house,
                origin: pos(8 + i, 9),
            },
        ));
    }

    Scenario {
        name: "minimal",
        description: "four houses served by one well and one farm",
        seed: 42,
        side: 32,
        difficulty: difficulty(data, RECORDED_DIFFICULTY),
        commands,
    }
}

/// Like `minimal`, but with more houses than the providers can cover.
///
/// The hunger you see here is **lack of coverage**: the farm declares the
/// capacity its output sustains, so the houses it manages to take on all eat,
/// and the ones in excess stay outside. It is cured by building — and that is
/// what tells this scenario apart from how it behaved before capacity and
/// output were made consistent, when a covered house could stay hungry forever.
fn hunger(data: &DataSet) -> Scenario {
    let house = kind(data, "house");
    let mut s = minimal(data);
    s.name = "hunger";
    s.description = "more houses than the providers can cover";
    for i in 4..8u8 {
        s.commands.push((
            2,
            Command::PlaceBuilding {
                kind: house,
                origin: pos(8 + i, 9),
            },
        ));
    }
    s
}

const fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}

impl Scenario {
    pub fn world(&self, data: Arc<DataSet>) -> World {
        let grid = Grid::new(self.side, self.side, Terrain::Plain)
            .unwrap_or_else(|e| panic!("the scenario's grid is invalid: {e}"));
        World::new(grid, data, self.seed, self.difficulty)
    }

    /// The commands to apply at a given tick, in the order they were added:
    /// the order is part of the determinism contract (D4).
    pub fn commands_at_tick(&self, tick: u32) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| *c)
            .collect()
    }
}
