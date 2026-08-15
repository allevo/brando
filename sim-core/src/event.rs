//! Delta events sent to the renderer.
//!
//! Bevy gets one full snapshot on the first frame and then only these events
//! (CLAUDE.md, core/renderer boundary): rebuilding 40,000 entities every tick
//! is unacceptable. An event is emitted on a **change of state**, never every
//! tick: the wrong choice here costs 40,000 events per tick.

use crate::ids::{BuildingId, BuildingKindId, HouseId, Level, TilePos};
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
    /// which is always [`Mood::Awful`], and that is the mood the renderer
    /// has to assume from [`Event::HousePlaced`].
    HouseMoodChanged {
        house: HouseId,
        mood: Mood,
    },
    /// A house has gone up a level at the monthly review (phase 13).
    ///
    /// It is emitted where the transition happens, in step 6.2, and not derived
    /// in step 10 like the mood: the site knows `from` and `to` exactly, and a
    /// delta computed afterwards could only guess at them.
    ///
    /// A house that levels up **brings nobody in** (A12): the renderer is being
    /// told the building has changed, not that the city has grown.
    HouseEvolved {
        house: HouseId,
        from: Level,
        to: Level,
    },
    /// A house has come down a level at the monthly review (phase 13).
    ///
    /// A separate variant rather than [`Event::HouseEvolved`] with `to < from`:
    /// the renderer hangs two different animations off them, and discriminating
    /// on a numeric comparison is the kind of thing you get wrong once but for
    /// ever.
    ///
    /// It may have sent residents away — decay evicts whoever no longer fits —
    /// which phase 14 will report separately.
    HouseDegraded {
        house: HouseId,
        from: Level,
        to: Level,
    },
    /// The last resident of a house has died: the house still stands, and
    /// nobody lives in it.
    ///
    /// The only thing about the demographics worth an event. Births and deaths
    /// themselves travel in [`StepReport::summary`] as an aggregate: one event
    /// per house per tick is precisely the polling the boundary with the
    /// renderer exists to forbid.
    ///
    /// There is deliberately no `HouseRepopulated` yet. Nothing in phase 14 can
    /// put a resident back into an empty house — a birth needs `residents > 0` —
    /// so the variant would be unreachable from the day it was added. Events are
    /// outside the hash, so adding it with immigration in phase 15 costs
    /// nothing; `taxable_per_resident` was declared early for the opposite
    /// reason, being a **table** field, where arriving late means a second
    /// regeneration.
    ///
    /// [`StepReport::summary`]: crate::tick::StepReport::summary
    HouseAbandoned {
        house: HouseId,
    },
}
