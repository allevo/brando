//! Delta events sent to the renderer.
//!
//! Bevy gets one full snapshot on the first frame and then only these events
//! (CLAUDE.md, core/renderer boundary): rebuilding 40,000 entities every tick
//! is unacceptable. An event is emitted on a **change of state**, never every
//! tick: the wrong choice here costs 40,000 events per tick.

use crate::ids::{BuildingId, BuildingKindId, HouseId, TilePos};
use crate::satisfaction::Mood;
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
    /// A house has crossed a satisfaction band (phase 12).
    ///
    /// On the **band**, not on the value: the renderer needs a mood, not a
    /// number, and one event per house per tick is the thing this boundary
    /// exists to avoid. A house going from 40 to 44 inside the same band emits
    /// nothing.
    ///
    /// A newly-built house does not emit one: it is born at zero satisfaction,
    /// which is always [`Mood::Desperate`], and that is the mood the renderer
    /// has to assume from [`Event::HousePlaced`].
    HouseMoodChanged {
        house: HouseId,
        mood: Mood,
    },
}
