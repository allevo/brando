#![forbid(unsafe_code)]

//! The simulation core: state, tick, commands.
//!
//! A pure Rust crate (D1): no ECS, no dependency on Bevy, no I/O, no access to
//! the system clock. The core is conceptually a pure function
//! `step(&mut World, &[Command])` (D4).

pub mod command;
pub mod coverage;
pub mod data;
mod data_hash;
pub mod event;
pub mod grid;
pub mod ids;
pub mod network;
pub mod production;
pub mod rng;
pub mod service;
pub mod tick;
pub mod units;
pub mod world;

pub use command::{Command, CommandError};
pub use coverage::Coverage;
pub use data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef, UnsustainableCapacity};
pub use event::Event;
pub use grid::{Grid, GridError, Terrain, Tile, TileFlags, TileOccupant};
pub use ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
pub use network::{ComponentId, RoadNetwork, Visited};
pub use production::FoodTotals;
pub use rng::{RngDomain, RngSet};
pub use service::{ServiceFlags, ServiceKind};
pub use tick::{StepReport, step};
pub use units::{Coins, Milli};
pub use world::{Building, DirtyFlags, Economy, House, Occupant, Walker, World};
