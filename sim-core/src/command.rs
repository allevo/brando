//! The primitive commands and their errors.
//!
//! Commands are the only write channel into the core: the renderer never calls
//! a method that mutates state, because it is only a rendering client that reads
//! snapshots and emits commands (D1). A save file is `seed + Vec<Command>` (D4).
//!
//! The error messages are not cosmetic: they are the feedback that will go
//! back to the LLM (M3), which will produce invalid commands by construction.
//! Rejecting them is normal behaviour, not a failure.

use crate::grid::{Terrain, TilePos};
use crate::ids::BuildingKindId;
use crate::units::Coins;
use crate::world::Occupant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    PlaceRoad {
        at: TilePos,
    },
    PlaceBuilding {
        kind: BuildingKindId,
        origin: TilePos,
    },
    Demolish {
        at: TilePos,
    },
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("position outside the map: {0:?}")]
    OutsideMap(TilePos),

    #[error("tile {at:?} is already taken by {occupant}")]
    TileOccupied { at: TilePos, occupant: OccupantKind },

    #[error("wrong terrain at {at:?}: {terrain:?}")]
    WrongTerrain { at: TilePos, terrain: Terrain },

    #[error("not enough money: {needed} needed, {available} available")]
    NotEnoughMoney { needed: Coins, available: Coins },

    #[error("unknown kind of building: {0:?}")]
    UnknownBuildingKind(BuildingKindId),

    #[error("nothing to demolish at {0:?}")]
    NothingToDemolish(TilePos),
}

/// What is taking up a tile, in a form readable in an error message.
///
/// It does not carry the id: to the LLM reading the message a `BuildingId`
/// says nothing, whereas "a road" or "a building" tells it what to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccupantKind {
    Road,
    Building,
    House,
}

impl std::fmt::Display for OccupantKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Road => "a road",
            Self::Building => "a building",
            Self::House => "a house",
        };
        f.write_str(s)
    }
}

impl From<Occupant> for OccupantKind {
    fn from(o: Occupant) -> Self {
        match o {
            Occupant::Building(_) => Self::Building,
            Occupant::House(_) => Self::House,
        }
    }
}
