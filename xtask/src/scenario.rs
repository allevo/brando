//! Test scenarios for the headless runner.
//!
//! They are not the game's scenarios: those have objectives and victory
//! conditions and arrive with `sim-scenario` in M1 (D7). These are situations
//! built by hand to watch the numbers move, and to give the recorded replays of
//! phase 08 something to contain.

use std::sync::Arc;

use sim_core::{Command, DataSet, Grid, Terrain, TilePos, World};

pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub seed: u64,
    pub side: u16,
    /// The commands to apply, each with the tick it belongs to.
    pub commands: Vec<(u32, Command)>,
}

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

/// A road, a well, a farm and four houses: all of them served, food to spare.
/// This is the city that works.
fn minimal(data: &DataSet) -> Scenario {
    let house = kind(data, "casa");
    let well = kind(data, "pozzo");
    let farm = kind(data, "fattoria");

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
    let house = kind(data, "casa");
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
        World::new(grid, data, self.seed)
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
