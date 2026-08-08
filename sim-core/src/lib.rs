#![forbid(unsafe_code)]

//! Core della simulazione: stato, tick, comandi.
//!
//! Crate Rust puro (D1): nessun ECS, nessuna dipendenza da Bevy, nessun I/O,
//! nessun accesso all'orologio di sistema. Il core e' concettualmente una
//! funzione pura `step(&mut World, &[Command])` (D4).

pub mod data;
mod data_hash;
pub mod grid;
pub mod ids;
pub mod rng;
pub mod service;
pub mod units;

pub use data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef};
pub use grid::{Grid, GridError, OccupantSlot, Terrain, Tile, TileFlags};
pub use ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
pub use rng::{RngDomain, RngSet};
pub use service::{ServiceFlags, ServiceKind};
pub use units::{Coins, Milli};
