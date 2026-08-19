//! Steps 6.4 and 6.6 — emigration and immigration.
//!
//! The two flows this module owns are half of the determinism contract
//! `demographics.rs` opens with: deaths' jitter and choice of house, then this
//! module's emigration jitter and choice, then births', then this module's
//! immigration jitter and choice — the complete order lives once, in
//! `tick::houses_and_migration`, and is not repeated here.
//!
//! **Where the destination gate is *not* put, and it is a corrected plan and
//! not a free choice.** The phase 15 plan as first written made the eligible
//! set for immigration "the served houses" — the same filter levelling up
//! reads. Slot 14.5, closed after that plan and before this module, answered a
//! narrower question — whether an empty house consumes provider capacity — and
//! in doing so ruled the wider one out explicitly: *"Immigration must not make
//! coverage a precondition for moving into a particular house. Coverage drives
//! the rate at which a city attracts people ... if it also decided which house
//! they enter, an emptied house could never be refilled — uncovered because
//! empty, empty because uncovered."* A `hard`-profile city is exactly that
//! state at tick zero: every house holds nobody and is therefore served by
//! nobody, so a coverage-gated destination would leave `hard` with nowhere for
//! its first immigrant to go, for ever. The eligible set here is every house
//! with room, full stop — coverage still decides the *rate*, through
//! [`attractiveness`], which is where slot 14.5 says it belongs.
//!
//! **Why immigration has two regimes rather than one rate.** The intuitive
//! model is a rate against the population the city already has: a bigger,
//! equally attractive city should draw people faster than a small one, which is
//! word of mouth and is what [`immigration`] does once the city is past
//! `founding_population_threshold`. But that model has the same bootstrap
//! problem the destination gate above had, and for the same reason: a
//! `hard`-profile city starts at zero residents, and zero residents times any
//! rate is zero, for ever. So below the threshold the rate is counted against
//! the **free places** instead, which exist the moment a house is built whatever
//! its occupancy, and the city has something to grow from.
//!
//! The hand-off is a **hard cutover and not a blend**, the same shape as
//! `emigration_threshold` and `birth_threshold`: a second kind of curve is a
//! second thing to reason about, and a kink in the growth curve is balancing to
//! watch in the dump rather than a reason to smooth it away.
//!
//! **Why the threshold counts residents and not ticks.** The other way to
//! re-solve the bootstrap is a window — immigration behaves differently for the
//! first so many ticks — and it is worse, because it is tied to the calendar
//! regardless of what the player has actually built: a slow start could burn the
//! whole window before a single resident exists to receive anybody. A population
//! threshold is tied to the city's own state, so it behaves the same whether the
//! player is fast or slow, and it hands over the moment a city has enough people
//! for word of mouth to mean anything.

use crate::demographics::{DIVISOR, Flow, StepReportEvents, jitter, level_of, mature};
use crate::event::Event;
use crate::ids::HouseId;
use crate::rng::RngKind;
use crate::world::{House, World};

/// How much the city attracts, in thousandths — the weighted sum of two
/// whole-number shares: the average satisfaction, weighted by residents, and
/// the free places in the houses the coverage currently reaches.
///
/// A **pure** function of the state: no RNG in here. The jitter is in the
/// flow, not in the index, which is what lets this be printed in the dump and
/// compared between two games — it is the number the semantic observation for
/// the LLM (M3) will show in place of the serialised state. Phase 16 adds the
/// tax rate as a third term, and not before: coupling the two phases would
/// mean balancing migration and taxes together, which is two problems in one.
///
/// **An empty city reads as maximally attractive, on both shares, and that is
/// deliberate rather than a fallback for a division by zero.** With no
/// residents there is nothing to weight the satisfaction average by, and with
/// no house yet served there is nothing to divide the free places by either —
/// both denominators are genuinely zero, not merely close to it. Reading
/// either share as zero there would mean a freshly founded `hard` city, whose
/// houses are all born empty, computes an attractiveness of zero for ever:
/// nothing would ever draw its first immigrant, which is the one thing this
/// profile depends on this function for. Both shares default to the ceiling
/// instead — the reading a *just-satisfied, wide-open* city would report —
/// so the first immigrants do arrive.
pub fn attractiveness(world: &World) -> i32 {
    let rules = &world.data.rules;
    // Guarded rather than trusted: a validated dataset always has
    // `satisfaction.max >= 1`, but this function stays total on a hand-built
    // one that does not, the same discipline `demographics::births` already
    // holds for the same division.
    let max = i64::from(rules.satisfaction.max).max(1);

    let satisfaction_share = if world.population() == 0 {
        1000
    } else {
        i64::from(crate::demographics::average_satisfaction(world)) * 1_000 / max
    };

    let (free, places) = free_places_in_served_houses(world);
    let free_places_share = if places == 0 {
        1_000
    } else {
        free * 1_000 / places
    };

    let m = &rules.migration;
    let weighted = satisfaction_share * i64::from(m.satisfaction_weight)
        + free_places_share * i64::from(m.free_places_weight);
    i32::try_from(weighted / 1_000).unwrap_or(i32::MAX)
}

/// The free places in houses the coverage **currently reaches** on every
/// service their own level requires, and the total places in those same
/// houses — the numerator and denominator of `attractiveness`'s second share.
///
/// Restricted to served houses on purpose: it is what makes the term read
/// coverage rather than raw capacity, so two cities with the same houses and
/// different coverage report different attractiveness. It is *not* the set
/// [`immigration`] draws its destinations from — that one is every house with
/// room, served or not, for the reason the module doc comment gives.
fn free_places_in_served_houses(world: &World) -> (i64, i64) {
    let rules = &world.data.rules;
    let mut free = 0i64;
    let mut places = 0i64;
    for (_, h) in world.houses() {
        if !fully_served(h, world) {
            continue;
        }
        let Some(max) = rules.max_residents(h.level) else {
            continue;
        };
        places += i64::from(max);
        free += i64::from(max.saturating_sub(h.residents));
    }
    (free, places)
}

/// Whether a house is covered, this tick, on **every** service its own level
/// requires — not merely one of them, and not the accumulated satisfaction,
/// which is a different question asked by `demographics::worst_required`.
fn fully_served(house: &House, world: &World) -> bool {
    world
        .data
        .rules
        .required_at(house.level)
        .iter()
        .all(|k| house.served.get(*k))
}

// --- 6.4 emigration ----------------------------------------------------------

/// Residents of a house below `emigration_threshold` on the worst service its
/// own level requires leave, at `emigration_per_thousand_per_month_unhappy`.
///
/// The same shape as `demographics::deaths`' raised rate, with one threshold
/// and one rate instead of two: emigration has no "served" counterpart the way
/// deaths has a base rate, because a house that is not going without has no
/// reason to leave at all.
///
/// This is the channel by which a city that decays empties out **even without
/// deaths** — the difference between "people are starving to death" and
/// "people are leaving", which read differently in the totals on purpose: a
/// house evicted by decay (phase 13) already flows into `evicted`, so this
/// counts only residents who left a house still standing, of their own accord.
pub(crate) fn emigration(world: &mut World, r: &mut StepReportEvents<'_>) -> bool {
    let m = world.data.rules.migration.clone();

    let mut eligible_residents = 0i64;
    for (_, h) in world.houses.iter() {
        if crate::demographics::worst_required(world, h) < m.emigration_threshold {
            eligible_residents += i64::from(h.residents);
        }
    }
    if eligible_residents == 0 {
        return false;
    }

    let j = jitter(world, m.jitter_per_thousand, RngKind::Migration);
    let rate = eligible_residents * i64::from(m.emigration_per_thousand_per_month_unhappy);
    let events = mature(world, Flow::Emigration, rate * (1_000 + j), DIVISOR);
    if events == 0 {
        return false;
    }

    // Built after the rate is decided and not before: unlike births' "room"
    // condition, going without does not change as residents leave, so the set
    // built once here stays exactly the set the rate above was counted
    // against — the same eligibility, read once.
    let mut eligible: Vec<HouseId> = world
        .houses
        .iter()
        .filter(|(_, h)| {
            h.residents > 0
                && crate::demographics::worst_required(world, h) < m.emigration_threshold
        })
        .map(|(id, _)| id)
        .collect();

    let mut moved = false;
    for _ in 0..events {
        if eligible.is_empty() {
            break;
        }
        let n = eligible.len() as u64;
        let at = world.rng.get(RngKind::Migration).below(n) as usize;
        let house = eligible[at];
        let Some(h) = world.houses.get_mut(house) else {
            break;
        };
        h.residents = h.residents.saturating_sub(1);
        world.population.emigrated += 1;
        moved = true;
        if h.residents == 0 {
            // The same event a death empties a house with: the renderer cares
            // that nobody lives there any more, not which flow did it.
            r.push(Event::HouseAbandoned { house });
            eligible.remove(at);
        }
    }
    moved
}

// --- 6.6 immigration -----------------------------------------------------------

/// Fills the free places of every house that has one, at a rate the city's
/// [`attractiveness`] scales and the regime's own basis counts — the module doc
/// comment is where the two regimes are argued for.
///
/// **Zero free places ⇒ zero immigrants, at any attractiveness — not a reduced
/// rate, an exact one.** Houses are the only container for population, and with
/// none free there is nowhere for anybody to go. Below the founding threshold
/// that is arithmetic, because the rate is a multiple of the free places; at or
/// above it the rate reads the population instead, so it is the **placement**
/// that stops rather than the wanting, and everybody the city could not house is
/// counted into `PopulationTotals::turned_away` and then forgotten — each tick's
/// demand is worked out afresh from that tick's own state, the same rule the
/// granary's lid follows, and nobody waits outside for room to appear.
///
/// **The gate that is *not* here: providers' remaining capacity.** It would be
/// easy and it would look prudent — let nobody in if the well is at its limit
/// — but it would mean the services precede the population instead of chasing
/// it, and the city would stop growing on its own without the player ever
/// noticing there was a problem to fix. Without that gate, what has to happen
/// does: the city fills up, outgrows its own services, the coverage starts
/// falling short, satisfaction drops, emigration switches on. The player sees
/// the shortfall and builds. The overshoot is the signal, not a bug.
pub(crate) fn immigration(world: &mut World, r: &mut StepReportEvents<'_>) -> bool {
    // Read before anything moves: `attractiveness` is the state at the top of
    // this sub-step, which by 6.6 already reflects this tick's deaths,
    // emigration and births.
    let attractiveness = i64::from(attractiveness(world));
    let m = world.data.rules.migration.clone();

    // Every house with room, served or not — see the module doc comment for
    // why coverage does not narrow this set.
    let mut eligible: Vec<HouseId> = Vec::new();
    let mut free_places = 0i64;
    for (id, h) in world.houses.iter() {
        let Some(max) = world.data.rules.max_residents(h.level) else {
            continue;
        };
        if h.residents < max {
            eligible.push(id);
            free_places += i64::from(max - h.residents);
        }
    }

    // Which basis the rate is counted against is the whole of the difference
    // between the two regimes; everything after this point is shared.
    let population = i64::from(world.population());
    let (basis, per_thousand) = if population < i64::from(m.founding_population_threshold) {
        (free_places, m.founding_immigration_per_thousand_per_month)
    } else {
        (population, m.immigration_per_thousand_per_month)
    };
    // Nothing to count a rate against is nothing to draw for, so the jitter
    // below is not drawn either — the rule the whole of step 6 keeps, which is
    // what makes the number of draws a function of the city. In the founding
    // regime this is the "no free places, no immigration" gate; past the
    // threshold the basis is a population validation keeps at one or more, so it
    // never fires there.
    if basis == 0 {
        return false;
    }

    let j = jitter(world, m.jitter_per_thousand, RngKind::Migration);
    // `attractiveness` folds into the numerator and its own thousandths base
    // into the divisor, the same shape `births` uses for the satisfaction
    // scaling: multiplying it in first and dividing once at the end is what
    // keeps the whole curve instead of truncating it to a whole number early.
    let rate = basis * i64::from(per_thousand) * attractiveness;
    let events = mature(
        world,
        Flow::Immigration,
        rate * (1_000 + j),
        DIVISOR * 1_000,
    );
    if events == 0 {
        return false;
    }

    let mut placed = 0u64;
    for _ in 0..events {
        if eligible.is_empty() {
            break;
        }
        let n = eligible.len() as u64;
        let at = world.rng.get(RngKind::Migration).below(n) as usize;
        let house = eligible[at];
        let (arrived_in_an_empty_house, full) = {
            let Some(h) = world.houses.get_mut(house) else {
                break;
            };
            let was_empty = h.residents == 0;
            h.residents = h.residents.saturating_add(1);
            (was_empty, h.residents)
        };
        world.population.immigrated += 1;
        placed += 1;
        if arrived_in_an_empty_house {
            r.push(Event::HouseRepopulated { house });
        }
        let level = level_of(world, house);
        if world
            .data
            .rules
            .max_residents(level)
            .is_some_and(|mx| full >= mx)
        {
            eligible.remove(at);
        }
    }

    // Everyone the loop above could not seat, which is nobody at all in the
    // founding regime: the rate is a multiple of the free places down there, so
    // a city with none matures no events to turn away. Saturating rather than
    // plain, unlike the flows next to it: a full city past the threshold turns
    // people away in crowds per tick, not one at a time.
    world.population.turned_away = world.population.turned_away.saturating_add(events - placed);

    placed > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        BuildingDef, BuildingRole, DataSet, DemographicsRules, DifficultyDef, DifficultyId,
        HouseLevelDef, MigrationRules, Rules, SatisfactionRules,
    };
    use crate::grid::{Grid, Terrain};
    use crate::service::ServiceKind;
    use crate::units::{Coins, Milli};

    fn rules() -> Rules {
        Rules {
            starting_treasury: Coins::ZERO,
            house_levels: vec![HouseLevelDef {
                max_residents: 4,
                required_services: vec![ServiceKind::Water],
                level_up_threshold: 50,
                decay_threshold: 25,
                taxable_per_resident: Milli::ZERO,
            }],
            food_per_resident: Milli::ZERO,
            satisfaction: SatisfactionRules {
                max: 100,
                step_up: 4,
                step_down: 10,
                mood_thresholds: [25, 50, 75],
            },
            demographics: DemographicsRules {
                births_per_thousand_per_month: 1,
                deaths_per_thousand_per_month: 0,
                deaths_per_thousand_per_month_when_unserved: 1,
                unserved_threshold: 25,
                birth_threshold: 60,
                jitter_per_thousand: 0,
            },
            migration: MigrationRules {
                satisfaction_weight: 700,
                free_places_weight: 300,
                founding_immigration_per_thousand_per_month: 30,
                founding_population_threshold: 20,
                immigration_per_thousand_per_month: 30,
                emigration_per_thousand_per_month_unhappy: 40,
                emigration_threshold: 25,
                jitter_per_thousand: 0,
            },
        }
    }

    fn a_house_kind() -> BuildingDef {
        BuildingDef {
            id: "house".into(),
            size: (1, 1),
            cost: Coins::ZERO,
            levels: 1,
            role: BuildingRole::House {
                required_services: vec![ServiceKind::Water],
            },
            production: None,
        }
    }

    fn an_empty_world() -> World {
        let data = std::sync::Arc::new(DataSet::new(
            rules(),
            [Coins::ZERO; Terrain::COUNT],
            vec![a_house_kind()],
            vec![DifficultyDef {
                id: "easy".into(),
                starting_residents_per_house: 0,
            }],
        ));
        World::new(
            Grid::new(4, 4, Terrain::Plain).expect("valid dimensions"),
            data,
            1,
            DifficultyId::new(0),
        )
    }

    /// An empty city — no houses at all — reads as maximally attractive on
    /// both shares: the module doc comment's whole argument, pinned to a
    /// number. It is the reading that lets a freshly founded `hard` city draw
    /// its first immigrant at all.
    #[test]
    fn an_empty_city_is_maximally_attractive() {
        let w = an_empty_world();
        assert_eq!(w.population(), 0);
        assert_eq!(attractiveness(&w), 1_000);
    }
}
