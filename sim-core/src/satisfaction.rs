//! House satisfaction: step 6.1 of the tick.
//!
//! It measures **how long** a service has been arriving, not how much of it
//! arrives. Measuring the quantity would break phase 07's "no partial
//! consumption" — the choice that makes food conservation an exact equality
//! instead of an inequality — and it would be degenerate anyway: with
//! capacity/output consistency a covered house always receives 100%.
//!
//! This phase computes the accumulator and nothing else. Every **consequence**
//! of it — levelling up, decay, migration, births — is phases 13 to 15, just as
//! phase 06 recorded who was served without drawing consequences.

use crate::data::{Rules, SatisfactionRules};
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
    Awful,
    Unhappy,
    Happy,
    Great,
}

impl Mood {
    pub const ALL: [Mood; 4] = [Mood::Awful, Mood::Unhappy, Mood::Happy, Mood::Great];

    pub const COUNT: usize = Self::ALL.len();

    /// The band a satisfaction value falls into.
    ///
    /// The boundaries are data ([`SatisfactionRules::mood_thresholds`]), and a
    /// threshold is the **lower bound, inclusive**, of the band above.
    /// Validation guarantees they are strictly ascending and that the first is
    /// above zero, so a satisfaction of zero is always `Awful` — which is
    /// what lets a newly-built house have a well-defined mood before anyone has
    /// computed one for it (see `tick::emit_events`).
    pub fn of(value: u8, rules: &SatisfactionRules) -> Self {
        let bands_passed = rules
            .mood_thresholds
            .iter()
            .filter(|t| value >= **t)
            .count();
        Self::ALL.get(bands_passed).copied().unwrap_or(Self::Great)
    }
}

/// A house's mood: the **minimum** across the services **its own level**
/// requires.
///
/// A house with water and no food is awful, not half happy. Services the
/// level does not require are not looked at — even though [`update`] keeps
/// their accumulators moving, which is a different question and answered there.
/// A house that levels up into a stricter requirement can therefore lose mood
/// in the very tick it is promoted, which is correct and is what the renderer
/// has to be told.
///
/// The level is looked up here rather than passed in: with the requirements per
/// level, a caller holding the wrong list is a mistake nobody would see, and
/// there were three callers.
///
/// A house that requires nothing cannot exist — [`BuildingDef::is_house`]
/// classifies as a house exactly what declares required services, and
/// validation refuses a level that demands none — and the fallback is
/// `Awful` rather than `Great` so that even in that impossible case it
/// agrees with the default a newborn house is compared against, and no event is
/// emitted out of nothing.
///
/// [`BuildingDef::is_house`]: crate::data::BuildingDef::is_house
pub fn mood_of(house: &House, rules: &Rules) -> Mood {
    rules
        .required_at(house.level)
        .iter()
        .map(|k| Mood::of(house.satisfaction[k.index()], &rules.satisfaction))
        .min()
        .unwrap_or(Mood::Awful)
}

/// Step 6.1 — the accumulators move by one tick.
///
/// Walks the houses in `HouseId` order and, for each service in the house
/// kind's declared list, applies `step_up` or `step_down` depending on
/// `served`.
///
/// **The union, not the current level's requirements** (phase 13). It looks
/// like the wrong list and it is the right one: a level that introduces a
/// service the level below does not require would otherwise be unreachable —
/// nothing would ever move that accumulator off zero, so its threshold could
/// never be met. It is also what [`House::satisfaction`]'s doc comment promised
/// in phase 12: the time accumulated on a service the house was receiving
/// anyway was not a lie. Which services are **read** is a separate question,
/// and the answer there is per level: see [`mood_of`] and `levels::review`.
pub(crate) fn update(world: &mut World) {
    // The fields are named separately so the tables can be read while the
    // houses are written: they are disjoint fields of the same `World`, and the
    // borrow checker only sees that if the destructuring says so. It replaces a
    // refcount bump that was there for the same reason and cost an atomic.
    let World { data, houses, .. } = world;
    let rules = &data.rules.satisfaction;
    // Resolved **once per tick**, not once per house: it is a scan over the
    // building definitions, irrelevant when it happens once and wrong when it
    // happens 40,000 times.
    let Some(def) = data.house_def() else {
        // No house kind in the tables means no house in the world: there is
        // nothing to walk, and it is not this function's job to complain.
        return;
    };

    for (_, h) in houses.iter_mut() {
        for k in def.required_services() {
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
    use crate::data::HouseLevelDef;
    use crate::service::ServiceKind;
    use crate::units::{Coins, Milli};

    fn rules() -> SatisfactionRules {
        SatisfactionRules {
            max: 100,
            step_up: 4,
            step_down: 8,
            mood_thresholds: [25, 50, 75],
        }
    }

    /// A threshold is the lower bound of the band above, inclusive, and zero is
    /// always `Awful`.
    #[test]
    fn the_bands_are_inclusive_at_the_bottom() {
        let r = rules();
        let cases = [
            (0, Mood::Awful),
            (24, Mood::Awful),
            (25, Mood::Unhappy),
            (49, Mood::Unhappy),
            (50, Mood::Happy),
            (74, Mood::Happy),
            (75, Mood::Great),
            (100, Mood::Great),
            (255, Mood::Great),
        ];
        for (value, expected) in cases {
            assert_eq!(Mood::of(value, &r), expected, "satisfaction {value}");
        }
    }

    /// A two-level table: level 1 wants water only, level 2 wants both.
    fn two_levels() -> Rules {
        let def = |max_residents, required: &[ServiceKind]| HouseLevelDef {
            max_residents,
            required_services: required.to_vec(),
            level_up_threshold: 50,
            decay_threshold: 25,
            taxable_per_resident: Milli::ZERO,
        };
        Rules {
            starting_treasury: Coins::ZERO,
            house_levels: vec![
                def(4, &[ServiceKind::Water]),
                def(8, &[ServiceKind::Water, ServiceKind::Food]),
            ],
            food_per_resident: Milli::ZERO,
            satisfaction: rules(),
            demographics: crate::data::DemographicsRules {
                births_per_thousand_per_month: 1,
                deaths_per_thousand_per_month: 0,
                deaths_per_thousand_per_month_when_unserved: 1,
                unserved_threshold: 25,
                birth_threshold: 60,
                jitter_per_thousand: 0,
            },
            migration: crate::data::MigrationRules {
                satisfaction_weight: 700,
                free_places_weight: 300,
                founding_immigration_per_thousand_per_month: 1,
                founding_population_threshold: 20,
                immigration_per_thousand_per_month: 1,
                emigration_per_thousand_per_month_unhappy: 1,
                emigration_threshold: 25,
                jitter_per_thousand: 0,
            },
        }
    }

    fn a_house(level: u8) -> House {
        let mut h = House {
            origin: crate::grid::TilePos::new(0, 0),
            level: crate::ids::Level::new(level).expect("levels count from 1"),
            residents: 4,
            served: crate::service::ServiceFlags::empty(),
            satisfaction: [0; ServiceKind::COUNT],
        };
        h.satisfaction[ServiceKind::Water.index()] = 100;
        h.satisfaction[ServiceKind::Food.index()] = 0;
        h
    }

    /// The worst service decides: water at the maximum and food at zero is
    /// awful, not half happy — but only once the level asks for food.
    #[test]
    fn the_mood_is_the_worst_of_the_required_services() {
        let r = two_levels();
        assert_eq!(mood_of(&a_house(2), &r), Mood::Awful);
        assert_eq!(
            mood_of(&a_house(1), &r),
            Mood::Great,
            "a service the level does not require does not drag the mood down"
        );
    }

    /// A level outside the table demands nothing, and the fallback is the mood
    /// a newborn house is compared against: no event out of nothing.
    #[test]
    fn a_level_off_the_table_is_desperate_not_thriving() {
        assert_eq!(mood_of(&a_house(9), &two_levels()), Mood::Awful);
    }
}
