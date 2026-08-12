//! The state hash.
//!
//! Written by hand, not delegated to serde (A3): with serde the hash would
//! depend on the order the fields are declared in and on the format, so moving
//! a field inside a struct — a refactor with no semantic consequences — would
//! invalidate every recording. That is exactly the false positive that makes
//! the project's most valuable test useless.
//!
//! The cost is that adding a field to the state requires adding it here by
//! hand. If that is forgotten, the hash goes blind on that field and the
//! recordings stop protecting it: it is the known risk of A3, mitigated by the
//! `the_hash_covers_the_whole_state` test.

use sim_core::{Building, Economy, House, RngDomain, ServiceKind, World};

/// Domain prefix: keeps this hash apart from the dataset's.
/// Changing it regenerates every recording.
const DOMAIN: &[u8] = b"brando/world/v1";

/// The state hash, in an order fixed **here** and nowhere else.
///
/// What goes in: the tick, the dataset hash, the difficulty, the grid's
/// dimensions, the tiles in `TileIdx` order, the buildings and the houses in id
/// order, the economy and the position of every RNG stream.
///
/// What does **not** go in: `RoadNetwork`, `Coverage`, `DirtyFlags`,
/// `FoodTotals`. They are derived or diagnostic structures; if they went in, a
/// rebuild bug would show up as a hash divergence, while the test meant to
/// catch it is the incremental-versus-from-scratch equivalence of phase 06 —
/// which also says *where* the problem is.
///
/// `House::served` on the other hand **does** go in, even though it is computed
/// from the coverage: it is a field of the state and it is the input M1 will
/// use to level houses up or let them decay. If it stayed out, a recording on
/// M0 would barely notice anything — changing the service assignment rule
/// would not move a single bit.
pub fn hash_world(w: &World) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMAIN);

    h.update(&w.tick().to_le_bytes());
    h.update(&w.data().hash);
    // The difficulty is state, not a parameter of the run: it changes the
    // simulation, so it enters the hash from tick 0 (A13). Two games with the
    // same seed and the same commands on different profiles are different games
    // and must not be confusable.
    h.update(&[w.difficulty().get()]);

    let grid = w.grid();
    h.update(&grid.width().to_le_bytes());
    h.update(&grid.height().to_le_bytes());

    // --- tiles, in TileIdx order ---
    for idx in grid.indices() {
        let Some(t) = grid.get(idx) else { continue };
        h.update(&[t.terrain as u8, t.flags.bits()]);
        // The flags already say whether there is an occupant; the origin is
        // added only when there is one, so no meaningless bytes get hashed.
        if let Some(occ) = t.occupant() {
            h.update(&occ.origin.get().to_le_bytes());
        }
    }

    // --- buildings, in id order ---
    // Iterating a SlotMap goes by slot index, deterministic given the same
    // sequence of insertions and removals — guaranteed by the command log (D4).
    //
    // Destructured and not accessed field by field, here and below: the
    // exhaustive pattern stops compiling the moment a field is added, which is
    // the same compile-time canary `World::field_canary` gives the state as a
    // whole — free wherever the fields are already `pub`.
    h.update(&(w.building_count() as u64).to_le_bytes());
    for (_, b) in w.buildings() {
        let Building {
            kind,
            origin,
            level,
            stock,
        } = b;
        h.update(&kind.get().to_le_bytes());
        h.update(&[origin.x, origin.y, level.get()]);
        h.update(&stock.to_millis().to_le_bytes());
    }

    // --- houses, in id order ---
    h.update(&(w.house_count() as u64).to_le_bytes());
    for (_, c) in w.houses() {
        let House {
            origin,
            level,
            residents,
            served,
            satisfaction,
        } = c;
        h.update(&[origin.x, origin.y, level.get()]);
        h.update(&residents.to_le_bytes());
        h.update(&[served.bits()]);
        h.update(satisfaction);
    }

    // --- economy ---
    let Economy { treasury } = w.economy();
    h.update(&treasury.get().to_le_bytes());

    // --- position of the RNG streams ---
    // A state in which Events has consumed 5 values is not the same as one in
    // which it has consumed 6, even if everything else matches: without this, a
    // divergence would show up many ticks later, where it is almost impossible
    // to attribute.
    for d in RngDomain::ALL {
        h.update(&w.rng().draws(d).to_le_bytes());
    }

    // A guard against a silent change in the number of services: if a new one
    // arrived, `served.bits()` would change meaning.
    h.update(&[ServiceKind::COUNT as u8]);

    *h.finalize().as_bytes()
}

pub fn hash_hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}
