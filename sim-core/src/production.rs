//! Production and consumption (step 4 of the tick).
//!
//! In M0 the farm is a coverage provider just like the well: it produces into a
//! local stock and the houses it covers consume from there (A5). It is not the
//! final production chain — warehouses and real logistics walkers are M3 (D3) —
//! but it is the smallest version that closes an observable
//! production→consumption loop, and it introduces no concept that will have to
//! be removed: coverage stays valid, what changes in M3 is only *where* the
//! goods come from.

use crate::ids::{BuildingId, HouseId};
use crate::service::ServiceKind;
use crate::units::Milli;
use crate::world::World;

/// Running totals for food.
///
/// They serve to check conservation, and M2's evaluator. They influence no game
/// decision and do not enter the state hash.
///
/// The fields are `i64` of thousandths and not `Milli`: they are running
/// totals, which over a long game leave the range of `Milli(i32)`. Saturating
/// in there would silently break the very equality these fields exist to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FoodTotals {
    pub produced: i64,
    pub consumed: i64,
    /// Food that evaporated because the granary was full.
    pub lost_to_full_stock: i64,
    /// Food that evaporated along with the farm that held it.
    ///
    /// It exists for the same reason as the field above: without it,
    /// demolishing a full producer would break the conservation equality, and
    /// the phase's most important test would report a bug that is not there.
    pub lost_to_demolition: i64,
    /// House-ticks in which a house had a food provider assigned and still did
    /// not eat. **It has to stay zero.**
    ///
    /// It is the observable form of *a house covered by food always eats* (A5),
    /// and it exists because phase 12 took away the other way of checking it:
    /// until then `House::served`'s food bit was written by step 4 and meant
    /// "it ate", so comparing it against the coverage was a real question. Now
    /// step 3 writes both bits and both mean "covered" (A9), and that
    /// comparison would be `x == x`.
    ///
    /// The check that replaces it is this counter, and it is worth more than
    /// what it replaces: it holds over the whole history rather than at the
    /// moment it is looked at.
    pub covered_but_unfed: u64,
}

impl FoodTotals {
    /// How much should be sitting in the stocks of the farms still standing.
    pub const fn expected_stock(&self) -> i64 {
        self.produced - self.consumed - self.lost_to_full_stock - self.lost_to_demolition
    }
}

pub(crate) fn production(world: &mut World) {
    // --- 1. the farms produce ---
    // The fields are named separately so the tables can be read while the
    // buildings and the totals are written: they are disjoint fields of the
    // same `World`, and the borrow checker only sees that if the destructuring
    // says so. It replaces a refcount bump that was there for the same reason
    // and cost an atomic. Scoped to this step, because step 2 below calls
    // functions that want the whole `&mut World`.
    {
        let World {
            data,
            buildings,
            food,
            ..
        } = &mut *world;

        for (_, b) in buildings.iter_mut() {
            let Some(def) = data.def(b.kind) else {
                continue;
            };
            let Some(p) = def.production.as_ref() else {
                continue;
            };
            let (output, max) = (p.output_per_tick, p.max_stock);

            let gross = b.stock.saturating_add(output);
            // Saturating at the maximum stock is **game semantics**: the granary
            // is full and the rest is lost. The `lost` term exists precisely
            // because without it conservation would not be an equality.
            let capped = gross.min(max);
            let lost = i64::from(gross.to_millis()) - i64::from(capped.to_millis());

            food.produced += i64::from(output.to_millis());
            food.lost_to_full_stock += lost;
            b.stock = capped;
        }
    }

    // --- 2. the houses eat ---
    // In HouseId order: it is a deterministic order and it is a game rule, like
    // the (distance, TileIdx) ordering of phase 06.
    //
    // **Step 4 does not write `House::served` any more** (phase 12, A9). That
    // bit means "covered", for water and for food alike, and step 3 is the only
    // one that writes it: reading a field with two meanings depending on the
    // bit was the trap A9 announced, and satisfaction is the first system to
    // read it. What used to be said by rewriting the bit is now said by
    // `covered_but_unfed`, which has to stay at zero.
    // Copied out before the loop rather than read through `world` inside it:
    // `take_from_stock` wants the whole `&mut World`, so no borrow of the
    // tables can be held across it. A `Milli` is four bytes and `Copy`, which
    // is why this needs no refcount bump of the dataset.
    let food_per_resident = world.data.rules.food_per_resident;
    let houses: Vec<HouseId> = world.houses.keys().collect();
    for h in houses {
        let Some(house) = world.houses.get(h) else {
            continue;
        };
        let residents = i32::from(house.residents);
        let Some(provider) = world.coverage.provider(h, ServiceKind::Food) else {
            continue;
        };

        let needed = food_per_resident
            .checked_mul_int(residents)
            .unwrap_or(Milli::ZERO);
        if !take_from_stock(world, provider, needed) {
            // Nothing ever decrements this. It counts house-ticks — events
            // already past — and a tick that goes well does not undo one that
            // did not: that is the whole point, a single assertion at the end
            // of a run covers every tick of it. "How many houses are hungry
            // right now" is a different question, and `served`'s food bit
            // already answers it.
            //
            // Saturating and not wrapping: the value has to stay zero, so any
            // increment at all is a bug already — and wrapping back round to
            // zero would make `covered_houses_are_fed` pass by overflow.
            world.food.covered_but_unfed = world.food.covered_but_unfed.saturating_add(1);
        }
    }
}

/// Takes `amount` out of the provider's stock.
///
/// Returns `false` without touching anything if the stock is not enough: **no
/// partial consumption**. The choice is deliberate — it makes conservation
/// checkable with an exact equality instead of an inequality, and in M1 it will
/// give a clean binary signal for "the house ate this month".
fn take_from_stock(world: &mut World, provider: BuildingId, amount: Milli) -> bool {
    let Some(b) = world.buildings.get_mut(provider) else {
        return false;
    };
    if b.stock < amount {
        return false;
    }
    let Some(left) = b.stock.checked_sub(amount) else {
        return false;
    };
    b.stock = left;
    world.food.consumed += i64::from(amount.to_millis());
    true
}
