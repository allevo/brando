#![forbid(unsafe_code)]

//! Core della simulazione: stato, tick, comandi.
//!
//! Crate Rust puro (D1): nessun ECS, nessuna dipendenza da Bevy, nessun I/O,
//! nessun accesso all'orologio di sistema. Il core e' concettualmente una
//! funzione pura `step(&mut World, &[Command])` (D4).

pub mod command;
pub mod data;
mod data_hash;
pub mod event;
pub mod grid;
pub mod ids;
pub mod rng;
pub mod service;
pub mod tick;
pub mod units;
pub mod world;

pub use command::{Command, CommandError};
pub use data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef};
pub use event::Event;
pub use grid::{Grid, GridError, Terrain, Tile, TileFlags, TileOccupant};
pub use ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
pub use rng::{RngDomain, RngSet};
pub use service::{ServiceFlags, ServiceKind};
pub use tick::{StepReport, step};
pub use units::{Coins, Milli};
pub use world::{Building, DirtyFlags, Economy, House, Occupante, Walker, World};
