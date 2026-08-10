//! Production and consumption (step 4 of the tick).
//!
//! In M0 the farm is a coverage provider just like the well: it produces into a
//! local stock and the houses it covers consume from there (A5). It is not the
//! final production chain — warehouses and real logistics walkers are M3 (D3) —
//! but it is the smallest version that closes an observable
//! production→consumption loop, and it introduces no concept that will have to
//! be removed: coverage stays valid, what changes in M3 is only *where* the
//! goods come from.

use std::sync::Arc;

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
    // The Arc is cloned so the tables can be read while the buildings are
    // mutated: it is a refcount bump, not a copy of the dataset.
    let data = Arc::clone(&world.data);

    // --- 1. the farms produce ---
    for (_, b) in world.buildings.iter_mut() {
        let Some(def) = data.def(b.kind) else {
            continue;
        };
        let (Some(output), Some(max)) = (def.output_per_tick, def.max_stock) else {
            continue;
        };

        let gross = b.stock.saturating_add(output);
        // Saturating at the maximum stock is **game semantics**: the granary is
        // full and the rest is lost. The `lost` term exists precisely because
        // without it conservation would not be an equality.
        let capped = gross.min(max);
        let lost = i64::from(gross.to_millis()) - i64::from(capped.to_millis());

        world.food.produced += i64::from(output.to_millis());
        world.food.lost_to_full_stock += lost;
        b.stock = capped;
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
    let houses: Vec<HouseId> = world.houses.keys().collect();
    for h in houses {
        let Some(house) = world.houses.get(h) else {
            continue;
        };
        let residents = i32::from(house.residents);
        let Some(provider) = world.coverage.provider(h, ServiceKind::Food) else {
            continue;
        };

        let needed = data
            .rules
            .food_per_resident
            .checked_mul_int(residents)
            .unwrap_or(Milli::ZERO);
        if !take_from_stock(world, provider, needed) {
            world.food.covered_but_unfed += 1;
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
