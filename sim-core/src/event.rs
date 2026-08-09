//! Delta events sent to the renderer.
//!
//! Bevy gets one full snapshot on the first frame and then only these events
//! (CLAUDE.md, core/renderer boundary): rebuilding 40,000 entities every tick
//! is unacceptable. An event is emitted on a **change of state**, never every
//! tick: the wrong choice here costs 40,000 events per tick.

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
    /// Emitted **only** on changes of state, never every tick: it is a delta,
    /// not polling. Polling would cost one event per house per tick.
    ServiceCoverageChanged {
        house: HouseId,
        service: ServiceKind,
        served: bool,
    },
}
