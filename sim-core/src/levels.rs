//! House levels: step 6.2 of the tick, the monthly review.
//!
//! Phase 12's satisfaction exists in order to be read by someone, and this is
//! the first system that reads it. A house that has been served long enough
//! rises a level; one that has lost a service it depends on comes back down.
//!
//! **Why the review is monthly and not every tick.** Not for the cost —
//! levelling up does not touch the coverage, so it is cheap. It is because
//! a gap on its own only protects against oscillation if the thresholds
//! are far apart, whereas an infrequent cadence makes it **structural**: thirty
//! ticks pass between two decisions, and a house cannot go up and down more
//! than twelve times a year by construction. Satisfaction keeps accumulating
//! every tick (step 6.1): it is only the *decision* that is monthly.
//!
//! **What does not happen here, and why the coverage is not touched.** Levelling
//! up needs no gate. A provider's capacity is consumed by the
//! **residents present**, and going up a level brings nobody in — it brings
//! permission to hold more of them, which is a different thing. The pressure
//! arrives when the demographics fill the space (phase 14): then demand grows,
//! the provider fills up, and some house falls out of the coverage. That is not
//! a pathology to switch off, it is the game loop.

use crate::data::Rules;
use crate::event::Event;
use crate::ids::Level;
use crate::tick::StepReport;
use crate::world::{House, World};

/// Step 6.2 — the monthly review: decay, then levelling up.
///
/// **One pass over the houses, at most one jump each.** The plan described two
/// passes, decay and then levelling up; a single pass is the same thing with
/// the guarantee made structural instead of inherited from the data. With two
/// passes, "a house that has just come down must not go back up in the same
/// review" holds only *because* the gap is validated — true today,
/// and exactly the kind of dependency that goes quiet when a table changes.
///
/// **Decay is evaluated first.** A house on its way down must not be able to
/// rise on one leftover requirement: evaluating the worse condition first makes
/// the transition monotone.
///
/// The houses are walked in `HouseId` order, like every other pass in the core.
pub(crate) fn review(world: &mut World, r: &mut StepReport) {
    // Splitting the fields is what lets the houses be written while the tables
    // are read and the totals are updated: they are disjoint fields of the same
    // `World`, but the borrow checker only sees that if they are named
    // separately. Naming `data` here is what replaced a refcount bump that was
    // there for the same reason and cost an atomic.
    let World {
        data,
        houses,
        population,
        ..
    } = world;
    let rules = &data.rules;
    let top = rules.top_house_level();
    let mut anyone_evicted = false;

    for (house, h) in houses.iter_mut() {
        let from = h.level;
        let step = if decays(h, rules) {
            from.previous()
        } else if rises(h, rules, top) {
            from.next()
        } else {
            continue;
        };
        // `decays` and `rises` each guarantee the level exists — level 1 never
        // decays, nothing rises past the top — so `None` cannot come out of
        // here. Skipping is the answer that leaves the table intact the day it
        // does, which is more than the saturating arithmetic this replaced
        // could say: that one moved the house to a level it had just been told
        // was not there.
        let Some(to) = step else {
            continue;
        };
        h.level = to;

        if to < from {
            // **Decay evicts.** The house has shrunk, and whoever no longer
            // fits leaves. It is the only point in this phase where `residents`
            // changes, and therefore the only one that has to invalidate the
            // coverage: capacity is counted on the residents present.
            //
            // A destination the table does not contain evicts **nobody**, and
            // that is the point of the `if let`. `decays()` deliberately sends
            // a house at an unknown level down (see there), so `to` can itself
            // be off the table — and a fallback capacity of zero would empty
            // the house in one review. That is a population wipe, not the
            // convergence the descent is for: leave the residents alone until
            // the house reaches a level that exists, then apply its capacity.
            if let Some(capacity) = rules.max_residents(to)
                && h.residents > capacity
            {
                population.evicted += u64::from(h.residents - capacity);
                h.residents = capacity;
                anyone_evicted = true;
            }
            // Two variants and not one with `to < from`: the renderer hangs two
            // different animations off them, and discriminating on a numeric
            // comparison is the kind of thing you get wrong once but for ever.
            r.events.push(Event::HouseDegraded { house, from, to });
        } else {
            r.events.push(Event::HouseEvolved { house, from, to });
        }
    }

    if anyone_evicted {
        world.dirty.invalidate_coverage();
    }
}

/// Whether a house has stopped meeting the demands of the level it is at.
///
/// Read against its **own** level: those are the services it has to keep in
/// order to stay. Level 1 never decays — there is no level 0, and a house that
/// empties out is phase 14's business, not this one's.
///
/// A house standing at a level the table no longer contains comes down. It
/// cannot happen through play — the dataset hash travels in the replay header,
/// so a shortened table is a different game — and coming down is the
/// conservative answer: it converges on a level that exists instead of freezing
/// there.
fn decays(house: &House, rules: &Rules) -> bool {
    if house.level == Level::FIRST {
        return false;
    }
    let Some(def) = rules.house_level(house.level) else {
        return true;
    };
    def.required_services
        .iter()
        .any(|k| house.satisfaction[k.index()] < def.decay_threshold)
}

/// Whether a house meets the demands of the level **above** it.
///
/// Evaluated against the destination and not the current level: that way you
/// never rise into a requirement that is not met.
///
/// A destination that requires nothing would be reached for free, `all` over an
/// empty list being true. It is validation that keeps that case from existing
/// (`rules.house_levels[i].required_services` may not be empty) rather than a
/// special case here: a level nobody has to earn is a table mistake, and it
/// would be silently unreachable if this function pretended otherwise.
fn rises(house: &House, rules: &Rules, top: Option<Level>) -> bool {
    // No top is an empty table, which validation refuses: there is nowhere to
    // rise to, and nowhere the house could be standing either.
    if top.is_none_or(|top| house.level >= top) {
        return false;
    }
    let Some(def) = house.level.next().and_then(|next| rules.house_level(next)) else {
        return false;
    };
    def.required_services
        .iter()
        .all(|k| house.satisfaction[k.index()] >= def.level_up_threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::HouseLevelDef;
    use crate::data::{DemographicsRules, SatisfactionRules};
    use crate::ids::TilePos;
    use crate::service::{ServiceFlags, ServiceKind};
    use crate::units::{Coins, Milli};

    /// Three levels, all wanting water: enough to exercise the levels without
    /// bringing the per-level requirements into it, which is tested elsewhere.
    fn rules() -> Rules {
        let def = |max_residents, level_up_threshold, decay_threshold| HouseLevelDef {
            max_residents,
            required_services: vec![ServiceKind::Water],
            level_up_threshold,
            decay_threshold,
            taxable_per_resident: Milli::ZERO,
        };
        Rules {
            ticks_per_month: 30,
            months_per_year: 12,
            starting_treasury: Coins::ZERO,
            house_levels: vec![def(4, 0, 0), def(8, 50, 25), def(12, 90, 60)],
            food_per_resident: Milli::ZERO,
            demographics: DemographicsRules {
                births_per_thousand_per_month: 1,
                deaths_per_thousand_per_month: 0,
                deaths_per_thousand_per_month_when_unserved: 1,
                unserved_threshold: 25,
                birth_threshold: 60,
                jitter_per_thousand: 0,
            },
            satisfaction: SatisfactionRules {
                max: 100,
                step_up: 4,
                step_down: 10,
                mood_thresholds: [25, 50, 75],
            },
        }
    }

    /// A level from its number, so the tests below can go on talking in levels.
    fn lvl(number: u8) -> Level {
        Level::new(number).expect("levels count from 1")
    }

    fn house(level: u8, water: u8) -> House {
        let mut h = House {
            origin: TilePos::new(0, 0),
            level: lvl(level),
            residents: 4,
            served: ServiceFlags::empty(),
            satisfaction: [0; ServiceKind::COUNT],
        };
        h.satisfaction[ServiceKind::Water.index()] = water;
        h
    }

    /// The band between the two thresholds is where nothing happens, whichever
    /// side the house came from. It is the shape of the gap, read off the two
    /// predicates rather than off a running simulation.
    #[test]
    fn inside_the_band_nothing_moves() {
        let r = rules();
        let top = r.top_house_level();
        for water in 25..50 {
            let at_two = house(2, water);
            assert!(!decays(&at_two, &r), "level 2 decays at {water}");
            let at_one = house(1, water);
            assert!(!rises(&at_one, &r, top), "level 1 rises at {water}");
        }
    }

    /// The edges of the band, which is where an off-by-one lives: the
    /// thresholds are inclusive at the bottom, like the mood bands.
    #[test]
    fn the_thresholds_are_inclusive_at_the_bottom() {
        let r = rules();
        let top = r.top_house_level();
        assert!(rises(&house(1, 50), &r, top), "50 is level 2's threshold");
        assert!(!rises(&house(1, 49), &r, top));
        assert!(decays(&house(2, 24), &r));
        assert!(!decays(&house(2, 25), &r), "25 is level 2's floor");
    }

    /// Level 1 has nowhere to fall, the top level has nowhere to rise.
    #[test]
    fn the_ends_of_the_table_hold() {
        let r = rules();
        let top = r.top_house_level();
        assert!(!decays(&house(1, 0), &r), "there is no level 0");
        assert!(!rises(&house(3, 100), &r, top), "there is no level 4");
        assert!(
            decays(&house(9, 100), &r),
            "a level off the table comes down"
        );
    }
}
