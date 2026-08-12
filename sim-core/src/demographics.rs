//! Steps 6.3 and 6.5 — deaths and births (phase 14).
//!
//! Aggregated, never a die per house: D5 makes the house the unit of
//! simulation, and the rule this module implements is ***the rate is random,
//! the distribution is deterministic***. A rate gets one jittered draw per
//! tick; which house an event falls on is a draw per event. Fifteen thousand
//! residents cost four draws and a handful more, not fifteen thousand.
//!
//! **The order of the draws inside a tick is a determinism contract**, like the
//! order of the ten steps, and it is written here rather than left to the
//! reading order of the code:
//!
//! 1. deaths' jitter
//! 2. deaths' choice of house, one draw per death
//! 3. births' jitter
//! 4. births' choice of house, one draw per birth
//!
//! Departures come before arrivals for two reasons. One technical: after a
//! decay there can be a house full to its limit, and freeing up first keeps
//! `residents <= max_residents` true at every observable instant and not merely
//! at the end of the step. One of gameplay: a house that loses somebody can win
//! them back in the same tick, which makes the population responsive instead of
//! jerky.
//!
//! **Nothing is drawn when there is nothing to draw for.** A flow with no
//! eligible residents takes no jitter, which keeps `draws` a readable function
//! of the city rather than of the calendar — and it is what lets
//! `an_empty_tick_only_advances_the_tick` go on asserting that a world with no
//! houses touches no stream, the same sentence it has asserted since phase 04,
//! now true for a reason instead of by absence.

use crate::data::Rules;
use crate::event::Event;
use crate::ids::HouseId;
use crate::rng::RngKind;
use crate::world::World;

/// The four flows of population.
///
/// **The declaration order is frozen.** [`Demographics::remainder`] is indexed
/// by it and that array goes into the state hash, so reordering the variants
/// silently reassigns every accumulated fraction to a different flow. It is the
/// same hazard as [`RngKind`]'s order and `Terrain`'s, and it is listed in
/// `GLOSSARY.md` next to them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Flow {
    Births,
    Deaths,
    Immigration,
    Emigration,
}

impl Flow {
    pub const ALL: [Self; 4] = [
        Self::Births,
        Self::Deaths,
        Self::Immigration,
        Self::Emigration,
    ];
    pub const COUNT: usize = Self::ALL.len();

    /// The slot in [`Demographics::remainder`]. Assigned per variant, not by
    /// position in [`Flow::ALL`], for the reason [`RngKind::index`] gives.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Births => 0,
            Self::Deaths => 1,
            Self::Immigration => 2,
            Self::Emigration => 3,
        }
    }
}

/// The fractions of an event not yet matured, one slot per flow.
///
/// **State**, and in the state hash: these decide the following ticks. Two of
/// the four slots do nothing until phase 15 and are hashed as zeros on purpose
/// — the array is sized for all four flows now so that adding migration moves
/// no recording.
///
/// **Why an accumulator at all.** A rate of 0.4 births a tick truncated to a
/// whole number is zero on *every* tick, so a small city would never grow. The
/// truncation is not a rounding error, it is a cliff.
///
/// **What the slot actually holds** is the undivided numerator, not thousandths
/// of an event. Dividing into thousandths once per tick throws away up to a
/// thousandth per flow per tick, which over five years is a systematic
/// *downward* bias of a couple of events — small, invisible, and exactly what
/// `the_jitter_does_not_move_the_mean` exists to catch. Keeping the numerator
/// whole means nothing is truncated at all: the modulo below leaves the
/// accumulator under the divisor and the discarded part is never discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Demographics {
    pub(crate) remainder: [i64; Flow::COUNT],
}

impl Demographics {
    /// The fraction of an event pending for a flow, for the hash and for tests.
    pub const fn remainder(&self, flow: Flow) -> i64 {
        self.remainder[flow.index()]
    }
}

/// Running totals for the population, one per flow plus the two that are not
/// flows of the demographics but still move people.
///
/// Diagnostic like [`FoodTotals`]: they influence no game decision and stay out
/// of the state hash. `u64` because they are running totals over a whole game,
/// not a quantity the state holds.
///
/// They exist so that population conservation can be an **exact equality**
/// rather than an inequality — the same job [`FoodTotals`] does for food, and
/// the reason both `settled_on_construction` and `lost_to_demolition` are here:
/// a house appears with people already in it and disappears with people still
/// in it, and neither is a birth or a death.
///
/// [`FoodTotals`]: crate::production::FoodTotals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PopulationTotals {
    /// Residents who arrived with a newly-built house ([A13]: how full a house
    /// is born is the difficulty's one knob).
    ///
    /// The inflow mirror of `lost_to_demolition`, and the third time this shape
    /// has been needed — the first two were a demolished farm's stock (phase
    /// 07) and a demolished house's residents.
    ///
    /// [A13]: crate::data::DifficultyDef
    pub settled_on_construction: u64,
    pub born: u64,
    pub died: u64,
    /// Phase 15. In the conservation equation from now, so that phase adds no
    /// term to it.
    pub immigrated: u64,
    /// Phase 15, like `immigrated`.
    pub emigrated: u64,
    /// Residents sent away because decay shrank the house below its occupancy
    /// (phase 13, written by [`levels::review`]).
    ///
    /// [`levels::review`]: crate::levels
    pub evicted: u64,
    /// Residents who vanished with a demolished house.
    pub lost_to_demolition: u64,
}

impl PopulationTotals {
    /// Everyone who ever arrived, minus everyone who ever left.
    ///
    /// The right-hand side of the conservation equality; the left-hand side is
    /// the sum of `residents` over the living houses. `i64` and not `u64`
    /// because the invariant has to be able to *observe* a negative balance in
    /// order to report it, not saturate and hide it.
    pub const fn balance(&self) -> i64 {
        let arrived = self.settled_on_construction + self.born + self.immigrated;
        let left = self.died + self.emigrated + self.evicted + self.lost_to_demolition;
        arrived as i64 - left as i64
    }
}

/// Steps 6.3 and 6.5. Returns whether anybody moved.
///
/// The caller invalidates the coverage on `true` and only on `true`: capacity
/// is counted on the residents present ([A12]), so an assignment made at step 3
/// with yesterday's population is stale the moment anyone is born or dies —
/// but in a full or empty city nobody moves, no recomputation is needed, and
/// the tick goes back to costing what it did before this phase.
///
/// A birth and a death in the same tick net to no change in `population` and
/// still have to invalidate: the coverage is decided by the residents of each
/// house, not by the city's total. That is why this returns a flag instead of
/// the caller comparing populations.
///
/// [A12]: crate::coverage
pub(crate) fn run(world: &mut World, r: &mut StepReportEvents<'_>) -> bool {
    let divisor = divisor(&world.data.rules);
    if divisor == 0 {
        // `ticks_per_month == 0`: a table can say it, and the core does not
        // panic on data. The same guard `Rules::is_month_boundary` has.
        return false;
    }
    let deaths = deaths(world, divisor, r);
    let births = births(world, divisor);
    deaths || births
}

/// `1000` (rates are per thousand) × `1000` (the jitter's base) × the ticks in
/// a month. A function of the tables, so it is computed and not written down.
fn divisor(rules: &Rules) -> i64 {
    i64::from(rules.ticks_per_month) * 1_000 * 1_000
}

/// The jitter on a rate: one draw, symmetric about zero, in `-J..=J`.
///
/// Symmetric because an asymmetric jitter shifts the whole balancing without
/// showing up anywhere — which is what `the_jitter_does_not_move_the_mean`
/// checks over ten thousand ticks.
fn jitter(world: &mut World, span: u16) -> i64 {
    if span == 0 {
        // Still a draw: the cost of a flow's jitter must not depend on a table
        // value, or `draws` stops being a function of the city.
        world.rng.get(RngKind::Demographics).below(1);
        return 0;
    }
    let width = u64::from(span) * 2 + 1;
    let drawn = world.rng.get(RngKind::Demographics).below(width);
    drawn as i64 - i64::from(span)
}

/// Turns an accumulated numerator into whole events, keeping the remainder.
fn mature(world: &mut World, flow: Flow, numerator: i64, divisor: i64) -> u64 {
    let slot = &mut world.demographics.remainder[flow.index()];
    *slot = slot.saturating_add(numerator);
    let events = *slot / divisor;
    *slot %= divisor;
    u64::try_from(events).unwrap_or(0)
}

// --- 6.3 deaths -------------------------------------------------------------

/// Everybody can die; the rate is the raised one for the residents of a house
/// that is going without.
///
/// **What "going without" means, and it is not "hungry".** The obvious reading
/// — food satisfaction under a threshold — breaks the early game. Since phase
/// 13 the production table's level 1 is a hut that requires water only, on
/// purpose: it is what makes the first rung reachable in a city that has no
/// farm yet. Read hunger off the food accumulator regardless of level and every
/// hut in a farmless city dies at ten times the base rate, so the opening of
/// every game is a slow bleed by construction and the level table would have to
/// be rebalanced to compensate for a rule nobody chose.
///
/// The reading that fits is the one the rest of step 6 already uses: the
/// **worst service the house's own level requires**, the same `required_at`
/// that the mood and the decay check read. A hut with water is fine; a level-2
/// house without food is not. One threshold, no new concept, and the raised
/// rate generalises to every service a rung ever demands instead of being wired
/// to food.
fn deaths(world: &mut World, divisor: i64, r: &mut StepReportEvents<'_>) -> bool {
    let rules = world.data.rules.clone();
    let d = &rules.demographics;

    let mut served = 0i64;
    let mut unserved = 0i64;
    for (_, h) in world.houses.iter() {
        let residents = i64::from(h.residents);
        if worst_required(world, h) < d.unserved_threshold {
            unserved += residents;
        } else {
            served += residents;
        }
    }
    if served + unserved == 0 {
        return false;
    }

    let j = jitter(world, d.jitter_per_thousand);
    let rate = served * i64::from(d.deaths_per_thousand_per_month)
        + unserved * i64::from(d.deaths_per_thousand_per_month_when_unserved);
    let events = mature(world, Flow::Deaths, rate * (1_000 + j), divisor);
    if events == 0 {
        return false;
    }

    // The eligible set in `HouseId` order: the choice is a draw over an index
    // into it, so the order is what makes the same seed pick the same house.
    let mut eligible: Vec<HouseId> = world
        .houses
        .iter()
        .filter(|(_, h)| h.residents > 0)
        .map(|(id, _)| id)
        .collect();

    let mut moved = false;
    for _ in 0..events {
        if eligible.is_empty() {
            break;
        }
        let n = eligible.len() as u64;
        let at = world.rng.get(RngKind::Demographics).below(n) as usize;
        let house = eligible[at];
        let Some(h) = world.houses.get_mut(house) else {
            break;
        };
        h.residents = h.residents.saturating_sub(1);
        world.population.died += 1;
        moved = true;
        if h.residents == 0 {
            // The only state change worth an event: everything else about the
            // demographics is an aggregate, and one event per house per tick is
            // the polling the renderer boundary forbids.
            r.push(Event::HouseAbandoned { house });
            // `remove` and not `swap_remove`: the set stays in `HouseId` order,
            // which is what the next draw indexes into.
            eligible.remove(at);
        }
    }
    moved
}

// --- 6.5 births -------------------------------------------------------------

/// Three conditions, and the rate is scaled by how well the city as a whole is
/// doing.
///
/// `residents > 0` (you need people to make people), room under
/// `max_residents(level)`, and the house's own worst required service above
/// `birth_threshold`. The base rate is then scaled by the city's
/// residents-weighted average satisfaction: that is the "tied to overall
/// wellbeing" half of A14 — a city that is struggling does not have children,
/// even in the houses that are doing well.
///
/// **The rate is counted against the eligible residents, not the population**,
/// and that is what makes the plateau structural rather than a consequence of
/// the numbers. A city whose houses are all full has nobody eligible, so the
/// rate is zero and the population stops — and it stops for a reason a reader
/// can point at instead of because two constants happen to cancel.
fn births(world: &mut World, divisor: i64) -> bool {
    let rules = world.data.rules.clone();
    let d = &rules.demographics;

    let mut eligible: Vec<HouseId> = Vec::new();
    let mut eligible_residents = 0i64;
    for (id, h) in world.houses.iter() {
        let room = rules
            .max_residents(h.level)
            .is_some_and(|max| h.residents < max);
        if h.residents > 0 && room && worst_required(world, h) >= d.birth_threshold {
            eligible.push(id);
            eligible_residents += i64::from(h.residents);
        }
    }
    if eligible_residents == 0 {
        return false;
    }

    let j = jitter(world, d.jitter_per_thousand);
    // The satisfaction scaling folds into the numerator, and `satisfaction.max`
    // into the divisor: scaling the rate first would truncate it to a whole
    // number and throw away most of the curve.
    let max = i64::from(rules.satisfaction.max);
    if max == 0 {
        return false;
    }
    let rate = eligible_residents
        * i64::from(d.births_per_thousand_per_month)
        * i64::from(average_satisfaction(world));
    let events = mature(world, Flow::Births, rate * (1_000 + j), divisor * max);
    if events == 0 {
        return false;
    }

    let mut moved = false;
    for _ in 0..events {
        if eligible.is_empty() {
            break;
        }
        let n = eligible.len() as u64;
        let at = world.rng.get(RngKind::Demographics).below(n) as usize;
        let house = eligible[at];
        let full = {
            let Some(h) = world.houses.get_mut(house) else {
                break;
            };
            h.residents = h.residents.saturating_add(1);
            world.population.born += 1;
            moved = true;
            h.residents
        };
        if rules.max_residents(level_of(world, house)).is_some_and(|m| full >= m) {
            eligible.remove(at);
        }
    }
    moved
}

// --- shared readings --------------------------------------------------------

/// The worst of the accumulators for the services **this house's own level**
/// requires; `satisfaction.max` if the rung requires nothing.
///
/// The same reading `Mood::of` and the decay check use, which is the point: one
/// threshold and no new concept.
fn worst_required(world: &World, house: &crate::world::House) -> u8 {
    let rules = &world.data.rules;
    rules
        .required_at(house.level)
        .iter()
        .map(|k| house.satisfaction[k.index()])
        .min()
        // A rung that requires nothing is going without nothing. Validation
        // keeps that case from existing in a real table; the reading here has
        // to be total all the same.
        .unwrap_or(rules.satisfaction.max)
}

fn level_of(world: &World, house: HouseId) -> crate::ids::Level {
    world
        .houses
        .get(house)
        .map_or(crate::ids::Level::FIRST, |h| h.level)
}

/// The city's satisfaction, weighted by residents.
///
/// Zero for an empty city: a city with no residents is a perfectly normal
/// state, and `no_panic_on_ten_thousand_commands` produces them constantly.
pub(crate) fn average_satisfaction(world: &World) -> u8 {
    let mut total = 0u64;
    let mut residents = 0u64;
    for (_, h) in world.houses.iter() {
        let r = u64::from(h.residents);
        total += u64::from(worst_required(world, h)) * r;
        residents += r;
    }
    if residents == 0 {
        return 0;
    }
    u8::try_from(total / residents).unwrap_or(u8::MAX)
}

/// A borrow of just the event sink of `StepReport`.
///
/// The demographics need to push events while holding `&mut World`, and
/// `StepReport` lives in `tick`. Passing the vector alone keeps this module
/// from depending on the shape of the report.
pub(crate) struct StepReportEvents<'a> {
    pub(crate) events: &'a mut Vec<Event>,
}

impl StepReportEvents<'_> {
    fn push(&mut self, e: Event) {
        self.events.push(e);
    }
}
