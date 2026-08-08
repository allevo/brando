//! Comandi primitivi e i loro errori.
//!
//! I comandi sono l'unico canale in scrittura verso il core: il renderer non
//! chiama mai metodi che mutano lo stato (CLAUDE.md, confine core/renderer), e
//! un salvataggio e' `seed + Vec<Command>` (D4).
//!
//! I messaggi di errore non sono cosmetici: sono il feedback che tornera'
//! all'LLM (M3), che produrra' comandi invalidi per costruzione. Rifiutarli e'
//! il comportamento normale, non un guasto.

use crate::grid::Terrain;
use crate::ids::{BuildingKindId, TilePos};
use crate::units::Coins;
use crate::world::Occupante;

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
    #[error("posizione fuori dalla mappa: {0:?}")]
    OutOfBounds(TilePos),

    #[error("tile {at:?} gia' occupato da {occupant}")]
    TileOccupied { at: TilePos, occupant: Occupato },

    #[error("terreno non adatto in {at:?}: {terrain:?}")]
    UnsuitableTerrain { at: TilePos, terrain: Terrain },

    #[error("fondi insufficienti: servono {needed}, disponibili {available}")]
    InsufficientFunds { needed: Coins, available: Coins },

    #[error("tipo di edificio sconosciuto: {0:?}")]
    UnknownBuildingKind(BuildingKindId),

    #[error("niente da demolire in {0:?}")]
    NothingToDemolish(TilePos),
}

/// Cosa occupa un tile, in forma leggibile per il messaggio d'errore.
///
/// Non porta l'id: all'LLM che riceve il messaggio un `BuildingId` non dice
/// nulla, mentre "una strada" o "un edificio" gli dice cosa fare dopo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupato {
    Strada,
    Edificio,
    Casa,
}

impl std::fmt::Display for Occupato {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Strada => "una strada",
            Self::Edificio => "un edificio",
            Self::Casa => "una casa",
        };
        f.write_str(s)
    }
}

impl From<Occupante> for Occupato {
    fn from(o: Occupante) -> Self {
        match o {
            Occupante::Edificio(_) => Self::Edificio,
            Occupante::Casa(_) => Self::Casa,
        }
    }
}
