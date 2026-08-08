#![forbid(unsafe_code)]

//! Core della simulazione: stato, tick, comandi.
//!
//! Crate Rust puro (D1): nessun ECS, nessuna dipendenza da Bevy, nessun I/O,
//! nessun accesso all'orologio di sistema. Il core e' concettualmente una
//! funzione pura `step(&mut World, &[Command])` (D4).

pub mod grid;
pub mod ids;
pub mod units;

pub use grid::{Grid, GridError, OccupantSlot, Terrain, Tile, TileFlags};
pub use ids::{BuildingId, HouseId, TileIdx, TilePos};
pub use units::{Coins, Milli};
