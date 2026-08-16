//! Aggregate service coverage (D2).
//!
//! A provider serves the houses within a range measured **along the road
//! network**, until it runs out of a capacity counted in **residents served**
//! ([`pick_within_capacity`]). There are no walkers: the water carriers the
//! player will see wandering around are decorative, live in the renderer and
//! are derived from this structure. **If a `Walker` shows up in this module,
//! it is a bug.**
//!
//! Like `RoadNetwork`, `Coverage` is derived and does not enter the state hash:
//! what guards it is the incremental-versus-from-scratch equivalence test,
//! which also says *where* the problem is.

use std::collections::BTreeMap;

use slotmap::SecondaryMap;

use crate::grid::TileIndex;
use crate::ids::{BuildingId, HouseId};
use crate::network::{Visited, bfs_roads};
use crate::service::ServiceKind;
use crate::world::World;

/// For every house and every service, who serves it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Coverage {
    served_by: BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]>,
    /// How many times coverage has been recomputed. It serves the dirty-flag
    /// tests, not the game.
    recomputes: u32,
}

impl Coverage {
    pub fn provider(&self, house: HouseId, kind: ServiceKind) -> Option<BuildingId> {
        self.served_by.get(&house)?[kind.index()]
    }

    pub fn is_served(&self, house: HouseId, kind: ServiceKind) -> bool {
        self.provider(house, kind).is_some()
    }

    pub const fn recomputes(&self) -> u32 {
        self.recomputes
    }

    /// The assignments, without the diagnostic counter: this is what gets
    /// compared between incremental coverage and coverage from scratch.
    pub const fn assignments(
        &self,
    ) -> &BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]> {
        &self.served_by
    }

    /// The houses served by a provider, in `HouseId` order.
    pub fn houses_served_by(&self, provider: BuildingId) -> Vec<HouseId> {
        self.served_by
            .iter()
            .filter(|(_, s)| s.contains(&Some(provider)))
            .map(|(h, _)| *h)
            .collect()
    }
}

/// From a road tile to the houses that face onto it.
///
/// In CSR form: the houses of tile `t` are `houses[offsets[t]..offsets[t + 1]]`.
/// `TileIndex` is a **dense** index over the grid, so indexing it directly costs
/// one access, against the ~13 comparisons with pointer chasing of a
/// `BTreeMap`. That is not a detail: it is the most frequent operation of the
/// whole recomputation — one per reached tile, per provider, i.e. ~100,000
/// times at the reference scale.
///
/// **At most four houses per tile**, because a tile has four orthogonal
/// neighbours (`Grid::neighbors4`) and each holds at most one occupant. The
/// bound holds even when houses grow beyond 1x1 in M1: it comes from the tile's
/// neighbours, not from the size of the house.
struct HousesByTile {
    /// Length `grid.len() + 1`.
    offsets: Vec<u32>,
    /// The houses, grouped by tile and in `HouseId` order within each group.
    houses: Vec<HouseId>,
}

impl HousesByTile {
    /// A counting sort in three passes: count, prefix-sum, fill.
    ///
    /// Linear in the number of tiles plus the number of entrances, with no
    /// comparisons at all — against the implicit ordering of a `BTreeMap`,
    /// which would pay `log n` on every insertion and allocate a node per tile.
    fn new(world: &World) -> Self {
        let tiles = world.grid().len() as usize;
        let mut offsets = vec![0u32; tiles + 1];

        // The pairs are collected in `HouseId` order: that is the order that
        // then shows up inside each group.
        let mut pairs: Vec<(TileIndex, HouseId)> = Vec::new();
        let mut entrances = Vec::new();
        for (id, _) in world.houses() {
            world.house_entrances_into(id, &mut entrances);
            for t in &entrances {
                pairs.push((*t, id));
                offsets[t.as_usize()] += 1;
            }
        }

        // Exclusive prefix sum: `offsets[t]` becomes the start of the group.
        let mut running = 0u32;
        for o in &mut offsets {
            let count = *o;
            *o = running;
            running += count;
        }

        // Forward fill, with `offsets[t]` acting as a cursor. At the end every
        // cursor has arrived at the end of its own group, i.e. at the start of
        // the next one: shifting everything one place to the right is enough.
        let mut houses = vec![HouseId::default(); pairs.len()];
        for (t, h) in pairs {
            let i = t.as_usize();
            if let Some(slot) = houses.get_mut(offsets[i] as usize) {
                *slot = h;
            }
            offsets[i] += 1;
        }
        for i in (1..=tiles).rev() {
            offsets[i] = offsets[i - 1];
        }
        offsets[0] = 0;

        Self { offsets, houses }
    }

    fn get(&self, t: TileIndex) -> &[HouseId] {
        let i = t.as_usize();
        let (Some(&from), Some(&to)) = (self.offsets.get(i), self.offsets.get(i + 1)) else {
            return &[];
        };
        self.houses.get(from as usize..to as usize).unwrap_or(&[])
    }
}

/// Recomputes coverage from scratch on the current state.
///
/// In M0 step 3 of the tick calls exactly this, for every provider, each time
/// `dirty.coverage` is not empty. Implementing this step naively is allowed as
/// long as the flags exist. What the dirty flag protects today is not the cost
/// of the recomputation but its **absence** when nothing has changed; and what
/// the equivalence test catches is a forgotten invalidation, not an algorithm
/// mistake.
pub fn compute_from_scratch(world: &World) -> Coverage {
    // The reverse map from road tile to the houses facing onto it. Built once
    // per recomputation instead of once per provider.
    let houses_by_tile = HousesByTile::new(world);

    // The assignments accumulate here, not in the `Coverage`, and for a reason
    // of cost: during the computation they have to be **queried** once per
    // candidate of every provider — ~100,000 times at the reference scale,
    // because `pick_within_capacity` walks every candidate and does not stop
    // when it fills up (it skips whoever does not fit). A `BTreeMap` with
    // 3,750 keys would pay ~13 comparisons each time; a `SecondaryMap` is dense
    // over the slot index, so it costs one access. The `Coverage` is
    // materialised at the end, once.
    let mut assigned: SecondaryMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]> =
        SecondaryMap::new();
    // A time marker per candidate, so a house the BFS reaches from more than
    // one tile is not counted twice: the value is the index of the current
    // provider, so there is no need to clear it between providers.
    let mut seen: SecondaryMap<HouseId, u32> = SecondaryMap::new();
    let mut candidates: Vec<(u16, TileIndex, HouseId)> = Vec::new();
    let mut entrances: Vec<TileIndex> = Vec::new();
    // One per recomputation, reused by every provider: allocating it per
    // provider was the last per-provider cost proportional to the map.
    let mut visited = Visited::new(world.grid().len());

    // The providers are walked in BuildingId order: it is the order that
    // decides who wins a contested house, so it is game semantics.
    //
    // **A dependency to keep an eye on.** That `SlotMap`'s iteration coincides
    // with the increasing order of the keys is true today — `KeyData` derives
    // `Ord` with `idx` before `version`, and iteration walks the slots by index
    // — but `slotmap` documents the key order as *unspecified*. So a dependency
    // update could change which provider wins a contested house. It would not
    // be silent: the recordings would diverge, and it is exactly the case
    // described by the message of `the_hashes_match_the_committed_ones` —
    // different hashes without a balance change means stop and find the source,
    // not regenerate.
    for (epoch, (provider, b)) in world.buildings().enumerate() {
        let epoch = epoch as u32;
        let Some(def) = world.data().def(b.kind) else {
            continue;
        };
        let Some(service) = def.service() else {
            continue;
        };
        let (Some(range), Some(capacity)) = (service.range(b.level), service.capacity(b.level))
        else {
            continue;
        };
        let kind = service.kind;

        world.building_entrances_into(provider, &mut entrances);
        if entrances.is_empty() {
            // A provider not hooked up to the network serves nobody: a house
            // not adjacent to any road is unreachable, at any distance.
            continue;
        }

        // The candidates, with the smallest distance they were reached at.
        //
        // The minimum does not need searching for: `bfs_roads` visits by
        // increasing distance, so **the first sighting of a house is already
        // its minimum**. It is enough to ignore later sightings, which arrive
        // when the house faces onto more than one reached tile.
        candidates.clear();
        bfs_roads(world.grid(), &entrances, range, &mut visited, |tile, d| {
            for h in houses_by_tile.get(tile) {
                if seen.get(*h) == Some(&epoch) {
                    continue;
                }
                seen.insert(*h, epoch);
                let Some(house) = world.house(*h) else {
                    continue;
                };
                let Some(idx) = world.grid().idx(house.origin) else {
                    continue;
                };
                candidates.push((d, idx, *h));
            }
        });

        // **Priority order**, game semantics: when the candidates exceed the
        // capacity, the nearest ones are served; at equal distance the smaller
        // `TileIndex` wins. It is a total order — without the second criterion
        // two equidistant houses would be ordered by the BFS's visit order,
        // i.e. by an implementation detail, and the recorded replay would
        // become fragile.
        candidates.sort_unstable();

        // Contention: in M0 a house is either served or not, and the first
        // provider in iteration order wins. Houses already taken are removed
        // **before** the filling, not inside it: a contested house must not
        // consume the capacity of whoever comes second.
        let free = candidates.iter().filter_map(|(_, _, h)| {
            let taken = assigned
                .get(*h)
                .is_some_and(|services| services[kind.index()].is_some());
            if taken {
                return None;
            }
            Some((*h, world.house(*h)?.residents))
        });
        let picked: Vec<HouseId> = pick_within_capacity(free, capacity).collect();

        for h in picked {
            if assigned.get(h).is_none() {
                assigned.insert(h, [None; ServiceKind::COUNT]);
            }
            if let Some(services) = assigned.get_mut(h) {
                services[kind.index()] = Some(provider);
            }
        }
    }

    // Final materialisation. **Only** houses with at least one service get in:
    // it is the same condition as before, when the entry was created by the
    // `entry().or_default()` inside the loop over the picks. Inserting houses
    // without services too would change the set of keys returned by
    // `assignments()` and `houses_served_by()` — no test would catch it,
    // because both sides of the comparison would change.
    let mut cov = Coverage::default();
    for (h, services) in &assigned {
        if services.iter().any(Option::is_some) {
            cov.served_by.insert(h, *services);
        }
    }
    cov
}

/// Fills up a provider's capacity by walking the candidates **already sorted by
/// priority**, and returns the ones that get served.
///
/// The capacity is counted in **residents**, not houses: with house levels (M1)
/// the population varies from house to house, and a capacity in houses would no
/// longer say how many people the provider can really serve.
///
/// **Game semantics, two rules in one.**
///
/// *No partial assignment.* A house gets in with all of its residents or it
/// does not get in: half a house served is not a state the game can represent.
/// It is the same choice as food consumption (phase 07).
///
/// *Whoever does not fit is skipped, and does not act as a barrier.* If two
/// places are left and the candidate asks for four, the scan carries on with
/// the next one, which may be further away. The alternative — stopping at the
/// first one that does not fit — would keep distance as an absolute priority,
/// but would leave places unused: a provider declared for twenty residents
/// would serve sixteen, and the capacity in the table would stop telling the
/// truth. For a producer that shortfall would also break the consistency
/// between capacity and output that [`CapacityBeyondOutput`] guards. Worse, one
/// large house built near the provider would cut out *all* those beyond it,
/// while places sat free.
///
/// Distance is still the priority order: no candidate is jumped over out of
/// preference, only out of impossibility.
///
/// [`CapacityBeyondOutput`]: crate::data::Inconsistency::CapacityBeyondOutput
fn pick_within_capacity<T>(
    candidates: impl IntoIterator<Item = (T, u16)>,
    capacity: u16,
) -> impl Iterator<Item = T> {
    let mut left = capacity;
    candidates.into_iter().filter_map(move |(id, residents)| {
        left = left.checked_sub(residents)?;
        Some(id)
    })
}

/// Step 3 of the tick.
pub(crate) fn propagate_coverage(world: &mut World) {
    if !world.dirty.coverage_needs_recompute() {
        return;
    }
    let recomputes = world.coverage.recomputes;
    world.coverage = compute_from_scratch(world);
    world.coverage.recomputes = recomputes.wrapping_add(1);
    world.dirty.coverage.clear();
    world.dirty.coverage_invalidated = false;

    // The houses carry a copy of the flags: it is what levelling up and decay
    // (M1) will read without having to query the `Coverage`.
    //
    // Splitting the fields is what lets the houses be written while the
    // coverage is read: they are disjoint fields of the same `World`, but the
    // borrow checker only sees that if they are named separately. The
    // alternative — collecting the flags into a `Vec` and walking it again —
    // cost one extra pass and one allocation per recomputation.
    let World {
        houses, coverage, ..
    } = world;
    for (_, h) in houses.iter_mut() {
        h.served = crate::service::ServiceFlags::empty();
    }
    for (id, services) in coverage.assignments() {
        let Some(h) = houses.get_mut(*id) else {
            continue;
        };
        for k in ServiceKind::ALL {
            h.served.set(k, services[k.index()].is_some());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pick_within_capacity;

    /// The filling rule, row by row.
    ///
    /// In M0 every house has the same number of residents, so the rows with
    /// mixed populations are not yet observable in a game: the choice has to be
    /// fixed **now** because now is when it can be made without regenerating
    /// anything, and house levels (M1) will make it visible.
    /// Capacity, candidates in priority order as `(id, residents)`, and the ids
    /// that must end up served.
    type Case = (u16, &'static [(u8, u16)], &'static [u8]);

    #[test]
    fn filling_skips_whoever_does_not_fit_instead_of_stopping() {
        let cases: &[Case] = &[
            (0, &[(1, 4)], &[]),
            (8, &[(1, 4), (2, 4)], &[1, 2]),
            // Beyond the capacity nobody is served.
            (8, &[(1, 4), (2, 4), (3, 4)], &[1, 2]),
            // Nothing partial: two places are left, the candidate asks for four
            // and stays out whole.
            (6, &[(1, 4), (2, 4)], &[1]),
            // Whoever does not fit is skipped: the third one, further away but
            // smaller, takes the two places the second could not use.
            (6, &[(1, 4), (2, 4), (3, 2)], &[1, 3]),
            // And the scan carries on even after several skips in a row.
            (6, &[(1, 4), (2, 4), (3, 3), (4, 1), (5, 1)], &[1, 4, 5]),
            // A candidate larger than the whole capacity blocks nothing.
            (4, &[(1, 9), (2, 4)], &[2]),
        ];

        for (capacity, candidates, served) in cases {
            let picked: Vec<u8> =
                pick_within_capacity(candidates.iter().copied(), *capacity).collect();
            assert_eq!(
                picked, *served,
                "capacity {capacity}, candidates {candidates:?}"
            );
        }
    }
}
