//! House satisfaction: step 6.1 of the tick.
//!
//! It measures **how long** a service has been arriving, not how much of it
//! arrives (A10). Measuring the quantity would break phase 07's "no partial
//! consumption" — the choice that makes food conservation an exact equality
//! instead of an inequality — and it would be degenerate anyway: with
//! capacity/output consistency (A5) a covered house always receives 100%.
//!
//! This phase computes the accumulator and nothing else. Every **consequence**
//! of it — levelling up, decay, migration, births — is phases 13 to 15, just as
//! phase 06 recorded who was served without drawing consequences.

use std::sync::Arc;

use crate::data::SatisfactionRules;
use crate::service::ServiceKind;
use crate::world::{House, World};

/// A house's satisfaction band. It is what the renderer draws, and it is
/// deliberately coarse: the event is emitted on a change of **band**, not of
/// value. One event per house per tick would be 40,000 events per tick, which
/// is exactly what the boundary with the renderer forbids.
///
/// The order of the variants is the order of the bands, and it is what makes
/// [`mood_of`]'s `min` mean "the worst service wins".
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Mood {
    Desperate,
    Unhappy,
    Happy,
    Thriving,
}

impl Mood {
    pub const ALL: [Mood; 4] = [Mood::Desperate, Mood::Unhappy, Mood::Happy, Mood::Thriving];

    pub const COUNT: usize = Self::ALL.len();

    /// The band a satisfaction value falls into.
    ///
    /// The boundaries are data ([`SatisfactionRules::mood_thresholds`]), and a
    /// threshold is the **lower bound, inclusive**, of the band above.
    /// Validation guarantees they are strictly ascending and that the first is
    /// above zero, so a satisfaction of zero is always `Desperate` — which is
    /// what lets a newly-built house have a well-defined mood before anyone has
    /// computed one for it (see `tick::emit_events`).
    pub fn of(value: u8, rules: &SatisfactionRules) -> Self {
        let bands_passed = rules
            .mood_thresholds
            .iter()
            .filter(|t| value >= **t)
            .count();
        Self::ALL
            .get(bands_passed)
            .copied()
            .unwrap_or(Self::Thriving)
    }
}

/// A house's mood: the **minimum** across the services its level requires.
///
/// A house with water and no food is desperate, not half happy. Services the
/// level does not require are not looked at, the same way [`update`] does not
/// touch them.
///
/// A house that requires nothing cannot exist — [`BuildingDef::is_house`]
/// classifies as a house exactly what declares required services — and the
/// fallback is `Desperate` rather than `Thriving` so that even in that
/// impossible case it agrees with the default a newborn house is compared
/// against, and no event is emitted out of nothing.
///
/// [`BuildingDef::is_house`]: crate::data::BuildingDef::is_house
pub fn mood_of(house: &House, required: &[ServiceKind], rules: &SatisfactionRules) -> Mood {
    required
        .iter()
        .map(|k| Mood::of(house.satisfaction[k.index()], rules))
        .min()
        .unwrap_or(Mood::Desperate)
}

/// Step 6.1 — the accumulators move by one tick.
///
/// Walks the houses in `HouseId` order and, for each service the current level
/// requires, applies `step_up` or `step_down` depending on `served`.
pub(crate) fn update(world: &mut World) {
    // A refcount bump, not a copy of the dataset: it lets the tables be read
    // while the houses are mutated.
    let data = Arc::clone(&world.data);
    let rules = &data.rules.satisfaction;
    // Resolved **once per tick**, not once per house: it is a scan over the
    // building definitions, irrelevant when it happens once and wrong when it
    // happens 40,000 times.
    let Some(def) = data.house_def() else {
        // No house kind in the tables means no house in the world: there is
        // nothing to walk, and it is not this function's job to complain.
        return;
    };

    for (_, h) in world.houses.iter_mut() {
        for k in &def.required_services {
            let current = h.satisfaction[k.index()];
            // **The saturation is game semantics**, not a shortcut against
            // overflow — the same distinction as the full granary in phase 07.
            // Service time does not accumulate forever: past the maximum, one
            // more month of water buys nothing.
            h.satisfaction[k.index()] = if h.served.get(*k) {
                current.saturating_add(rules.step_up).min(rules.max)
            } else {
                current.saturating_sub(rules.step_down)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> SatisfactionRules {
        SatisfactionRules {
            max: 100,
            step_up: 4,
            step_down: 8,
            mood_thresholds: [25, 50, 75],
        }
    }

    /// A threshold is the lower bound of the band above, inclusive, and zero is
    /// always `Desperate`.
    #[test]
    fn the_bands_are_inclusive_at_the_bottom() {
        let r = rules();
        let cases = [
            (0, Mood::Desperate),
            (24, Mood::Desperate),
            (25, Mood::Unhappy),
            (49, Mood::Unhappy),
            (50, Mood::Happy),
            (74, Mood::Happy),
            (75, Mood::Thriving),
            (100, Mood::Thriving),
            (255, Mood::Thriving),
        ];
        for (value, expected) in cases {
            assert_eq!(Mood::of(value, &r), expected, "satisfaction {value}");
        }
    }

    /// The worst service decides: water at the maximum and food at zero is
    /// desperate, not half happy.
    #[test]
    fn the_mood_is_the_worst_of_the_required_services() {
        let r = rules();
        let mut h = House {
            origin: crate::ids::TilePos::new(0, 0),
            level: 1,
            residents: 4,
            served: crate::service::ServiceFlags::empty(),
            satisfaction: [0; ServiceKind::COUNT],
        };
        h.satisfaction[ServiceKind::Water.index()] = 100;
        h.satisfaction[ServiceKind::Food.index()] = 0;

        assert_eq!(mood_of(&h, &ServiceKind::ALL, &r), Mood::Desperate);
        assert_eq!(
            mood_of(&h, &[ServiceKind::Water], &r),
            Mood::Thriving,
            "a service the level does not require does not drag the mood down"
        );
    }
}
