//! Eventi delta verso il renderer.
//!
//! Bevy riceve uno snapshot completo al primo frame e poi solo questi eventi
//! (CLAUDE.md, confine core/renderer): ricostruire 40.000 entita' ogni tick e'
//! inaccettabile. Un evento si emette sul **cambio di stato**, mai a ogni
//! tick: la scelta sbagliata qui costa 40.000 eventi per tick.

use crate::ids::{BuildingId, BuildingKindId, HouseId, TilePos};
use crate::service::ServiceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    BuildingPlaced {
        id: BuildingId,
        kind: BuildingKindId,
        origin: TilePos,
    },
    BuildingRemoved {
        id: BuildingId,
    },
    HousePlaced {
        id: HouseId,
        origin: TilePos,
    },
    HouseRemoved {
        id: HouseId,
    },
    RoadPlaced {
        at: TilePos,
    },
    RoadRemoved {
        at: TilePos,
    },
    /// Emesso **solo** sui cambi di stato, mai a ogni tick: e' un delta, non
    /// un polling. Il polling costerebbe un evento per casa per tick.
    ServiceCoverageChanged {
        house: HouseId,
        service: ServiceKind,
        served: bool,
    },
}
