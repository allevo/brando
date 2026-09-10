//! Test scenarios for the headless runner.
//!
//! They are not the game's scenarios: those have objectives and victory
//! conditions and arrive with `sim-scenario` in M1 (D7). These are situations
//! built by hand to watch the numbers move, and to give the recorded replays of
//! phase 08 something to contain.

use std::sync::Arc;

use sim_core::{Command, DataSet, DifficultyId, Grid, Terrain, Tick, TilePos, World};
use sim_replay::MapSpec;

pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub seed: u64,
    pub map: MapSpec,
    /// The profile the scenario is played on. It is state, so it travels in the
    /// recording's header and goes into the hash.
    pub difficulty: DifficultyId,
    /// The commands to apply, each with the tick it belongs to.
    pub commands: Vec<(Tick, Command)>,
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
        "river-valley" => Some(river_valley(data)),
        "open-plain" => Some(open_plain(data)),
        "hills-and-sea" => Some(hills_and_sea(data)),
        _ => None,
    }
}

/// Every scenario `--scenario`/`by_name` know about.
pub const NAMES: [&str; 5] = [
    "minimal",
    "hunger",
    "river-valley",
    "open-plain",
    "hills-and-sea",
];
/// The scenarios that get a committed recording. A separate list from
/// [`NAMES`], deliberately: a scenario built on a named map is a real playtest
/// (phase 16's test 12), by hand and by eye, not one more file `regen-expected`
/// keeps hashed forever the day it is added.
pub const RECORDED: [&str; 2] = ["minimal", "hunger"];

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
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(x, 8) }));
    }
    commands.push((
        Tick::new(1),
        Command::PlaceBuilding {
            kind: well,
            origin: pos(2, 7),
        },
    ));
    commands.push((
        Tick::new(1),
        Command::PlaceBuilding {
            kind: farm,
            origin: pos(4, 6),
        },
    ));
    for i in 0..4u8 {
        commands.push((
            Tick::new(2),
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
        map: MapSpec::Uniform {
            width: 32,
            height: 32,
            terrain: Terrain::Plain,
        },
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
            Tick::new(2),
            Command::PlaceBuilding {
                kind: house,
                origin: pos(8 + i, 9),
            },
        ));
    }
    s
}

/// Phase 16's test 12: a real map, not a flat lattice. A river runs
/// north-south, crossed by a ford at the south end; the west bank is the
/// valley floor (flat, cheap to build on), the east bank rises gently into
/// rock. The city sits mostly on the flat west bank, with one farm and one
/// house reaching onto the gentler part of the east bank's slope — enough to
/// pay a real, small `flatten_cost_per_step` surcharge without needing the
/// map redrawn or the rule loosened.
fn river_valley(data: &DataSet) -> Scenario {
    let house = kind(data, "house");
    let well = kind(data, "well");
    let farm = kind(data, "farm");

    let mut commands = Vec::new();
    // The road network: a spine down the west bank, a ford crossing the
    // river at the south end, and a short spur reaching the east-bank farm.
    for y in 0..=7u8 {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(2, y) }));
    }
    for x in [0u8, 1, 3, 4, 5, 6, 7, 8, 9] {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(x, 6) }));
    }
    for y in [4u8, 5] {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(8, y) }));
    }

    commands.push((
        Tick::new(1),
        Command::PlaceBuilding {
            kind: well,
            origin: pos(1, 1),
        },
    ));
    // On the flat valley floor: no slope, no surcharge.
    commands.push((
        Tick::new(1),
        Command::PlaceBuilding {
            kind: farm,
            origin: pos(0, 3),
        },
    ));
    // On the east bank's gentle rise: a real slope of 1, so this one costs
    // `flatten_cost_per_step` more than the west-bank farm above it.
    commands.push((
        Tick::new(1),
        Command::PlaceBuilding {
            kind: farm,
            origin: pos(6, 3),
        },
    ));
    for (x, y) in [(1, 0), (3, 0), (3, 1), (3, 2)] {
        commands.push((
            Tick::new(2),
            Command::PlaceBuilding {
                kind: house,
                origin: pos(x, y),
            },
        ));
    }
    // A hut on the slope itself: one tile, one height, nothing to flatten.
    commands.push((
        Tick::new(2),
        Command::PlaceBuilding {
            kind: house,
            origin: pos(7, 5),
        },
    ));

    Scenario {
        name: "river-valley",
        description: "a river valley: two farms, one on the flat and one on the slope",
        seed: 42,
        map: MapSpec::File {
            id: "river-valley".to_string(),
            hash: sim_data::load_map_by_id("river-valley")
                .unwrap_or_else(|e| panic!("the 'river-valley' map failed to load: {e}"))
                .hash_hex(),
        },
        difficulty: difficulty(data, RECORDED_DIFFICULTY),
        commands,
    }
}

/// Phase 16.5's closing test: a city on a map no person drew.
///
/// The point is not the layout, which is ordinary — it is that the ground under
/// it came out of `cargo xtask gen-map` and was committed unedited. A map that
/// cannot be built on is a seed to throw away, not a rule to loosen, so this
/// scenario is deliberately laid out the way a player would lay one out and
/// makes no allowance for the ground being awkward.
fn open_plain(data: &DataSet) -> Scenario {
    let house = kind(data, "house");
    let well = kind(data, "well");
    let farm = kind(data, "farm");

    let mut commands = Vec::new();
    // A spine across the open middle of the island, with three arms hanging
    // off it. Everything sits within one provider's range of an arm, because
    // a range is walked along the roads and not measured across the ground.
    for x in 8..=30u8 {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(x, 14) }));
    }
    for arm in [12u8, 19, 26] {
        for y in 10..=18u8 {
            // The spine already runs along row 14: laying a road twice is a
            // rejected command, and this scenario has none.
            if y != 14 {
                commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(arm, y) }));
            }
        }
    }

    for origin in [pos(11, 13), pos(18, 13), pos(25, 13)] {
        commands.push((Tick::new(1), Command::PlaceBuilding { kind: well, origin }));
    }
    // The last of these sits on a step of ground: a slope of two, which is
    // inside `max_build_slope` and costs a real `flatten_cost_per_step`
    // surcharge. Ground a seed drew is not flat, and a scenario that only ever
    // put its buildings on the flat parts would be proving nothing.
    for origin in [
        pos(13, 15),
        pos(20, 15),
        pos(27, 15),
        pos(13, 11),
        pos(27, 11),
        pos(20, 12),
    ] {
        commands.push((Tick::new(1), Command::PlaceBuilding { kind: farm, origin }));
    }

    for (x, y) in [
        (11, 10),
        (11, 11),
        (11, 12),
        (11, 15),
        (11, 16),
        (18, 10),
        (18, 11),
        (18, 12),
        (18, 15),
        (18, 16),
        (25, 10),
        (25, 11),
        (25, 12),
        (25, 15),
        (25, 16),
    ] {
        commands.push((
            Tick::new(2),
            Command::PlaceBuilding {
                kind: house,
                origin: pos(x, y),
            },
        ));
    }

    Scenario {
        name: "open-plain",
        description: "a city on ground a seed drew: six farms, three wells, fifteen houses",
        seed: 42,
        map: MapSpec::File {
            id: "open-plain".to_string(),
            hash: sim_data::load_map_by_id("open-plain")
                .unwrap_or_else(|e| panic!("the 'open-plain' map failed to load: {e}"))
                .hash_hex(),
        },
        difficulty: difficulty(data, RECORDED_DIFFICULTY),
        commands,
    }
}

/// Phase 16.9.5: a city on a map the source-based generator drew, committed
/// unedited — the first time a map it drew has been played at all.
///
/// Laid out the way a player would, with no allowance for where the ground
/// came from. The middle of the map is level, because the generator never
/// moves a tile the land source claims, so the one farm that pays for a slope
/// has to reach out to the edge of the western hills, on a spur of its own.
fn hills_and_sea(data: &DataSet) -> Scenario {
    let house = kind(data, "house");
    let well = kind(data, "well");
    let farm = kind(data, "farm");

    let mut commands = Vec::new();
    for x in 8..=30u8 {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(x, 14) }));
    }
    for arm in [12u8, 19, 26] {
        for y in 10..=18u8 {
            if y != 14 {
                commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(arm, y) }));
            }
        }
    }
    // The spur up to the hillside farm's southern edge.
    for y in [13u8, 12] {
        commands.push((Tick::ZERO, Command::PlaceRoad { at: pos(9, y) }));
    }

    for origin in [pos(11, 13), pos(18, 13), pos(25, 13)] {
        commands.push((Tick::new(1), Command::PlaceBuilding { kind: well, origin }));
    }
    // The last of these stands on a slope of one, and costs one
    // `flatten_cost_per_step` more than the rest.
    for origin in [
        pos(13, 15),
        pos(20, 15),
        pos(27, 15),
        pos(13, 11),
        pos(27, 11),
        pos(8, 10),
    ] {
        commands.push((Tick::new(1), Command::PlaceBuilding { kind: farm, origin }));
    }

    for (x, y) in [
        (11, 10),
        (11, 11),
        (11, 12),
        (11, 15),
        (11, 16),
        (18, 10),
        (18, 11),
        (18, 12),
        (18, 15),
        (18, 16),
        (25, 10),
        (25, 11),
        (25, 12),
        (25, 15),
        (25, 16),
    ] {
        commands.push((
            Tick::new(2),
            Command::PlaceBuilding {
                kind: house,
                origin: pos(x, y),
            },
        ));
    }

    Scenario {
        name: "hills-and-sea",
        description: "a city on a map drawn from sources: six farms, three wells, fifteen houses",
        seed: 42,
        map: MapSpec::File {
            id: "hills-and-sea".to_string(),
            hash: sim_data::load_map_by_id("hills-and-sea")
                .unwrap_or_else(|e| panic!("the 'hills-and-sea' map failed to load: {e}"))
                .hash_hex(),
        },
        difficulty: difficulty(data, RECORDED_DIFFICULTY),
        commands,
    }
}

const fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}

impl Scenario {
    pub fn world(&self, data: Arc<DataSet>) -> World {
        let grid = match &self.map {
            MapSpec::Uniform {
                width,
                height,
                terrain,
            } => Grid::new(*width, *height, *terrain)
                .unwrap_or_else(|e| panic!("the scenario's grid is invalid: {e}")),
            MapSpec::File { id, .. } => {
                let def = sim_data::load_map_by_id(id)
                    .unwrap_or_else(|e| panic!("the scenario's map {id:?} failed to load: {e}"));
                Grid::from_map(&def)
            }
        };
        World::new(grid, data, self.seed, self.difficulty)
    }

    /// The commands to apply at a given tick, in the order they were added:
    /// the order is part of the determinism contract (D4).
    pub fn commands_at_tick(&self, tick: Tick) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| *c)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use sim_core::Calendar;

    use super::*;

    /// A map the generator drew is accepted by the replay's own entry point,
    /// then played for thirty months with every command accepted and people
    /// still living on it at the end.
    #[test]
    fn a_map_the_generator_drew_loads_and_plays() {
        let data = Arc::new(sim_data::load_default().expect("the tables load"));
        let sc = by_name("hills-and-sea", &data).expect("the scenario exists");

        let rec = crate::expected::record(&sc, &data);
        sim_replay::initial_world(&rec, Arc::clone(&data)).expect("the replay accepts the map");

        let mut w = sc.world(Arc::clone(&data));
        for t in 0..30 * Calendar::TICKS_PER_MONTH {
            let r = sim_core::step(&mut w, &sc.commands_at_tick(Tick::new(t)));
            assert!(r.rejected.is_empty(), "tick {t}: {:?}", r.rejected);
        }
        assert!(w.population() > 0, "nobody lives on the map");
    }
}
