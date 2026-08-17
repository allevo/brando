//! Aggregate service coverage (D2).
//!
//! A provider serves the houses within a range measured **along the road
//! network**, until it runs out of a capacity counted in **residents served**
//! ([`pick_within_capacity`]).

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
    /// HouseId -> `[BuildingId]` that serve the house.
    ///
    /// NB: size_of::<Option<BuildingId>>() == size_of::<BuildingId>().
    /// See below.
    served_by: BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]>,
    /// How many times coverage has been recomputed.
    #[cfg(feature = "counters")]
    recomputes: u32,
}

const _: () = assert!(size_of::<Option<BuildingId>>() == size_of::<BuildingId>());

impl Coverage {
    /// The provider serving this house for that service. `None` if none does.
    pub fn served_by(&self, house: HouseId, kind: ServiceKind) -> Option<BuildingId> {
        self.served_by.get(&house)?[kind.index()]
    }

    #[cfg(feature = "counters")]
    pub const fn recomputes(&self) -> u32 {
        self.recomputes
    }
}

#[cfg(feature = "test-util")]
impl Coverage {
    pub fn is_served(&self, house: HouseId, kind: ServiceKind) -> bool {
        self.served_by(house, kind).is_some()
    }

    /// The assignments, without the diagnostic counter, as the
    /// incremental-versus-from-scratch test compares them.
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

#[cfg(feature = "from-scratch")]
impl Coverage {
    /// Coverage recomputed from scratch on the current state, ignoring the dirty flags.
    pub fn from_scratch(world: &World) -> Self {
        compute_from_scratch(world)
    }
}

/// From a road tile to the houses that face onto it.
///
/// The houses of tile `t` are `houses[offsets[t]..offsets[t + 1]]`.
///
/// **At most four houses per tile**, because a tile has four orthogonal
/// neighbours (`Grid::neighbors4`) and each holds at most one occupant.
struct HousesByTile {
    /// The offset of each [`HousesByTile::houses`] group
    offsets: Vec<u32>,
    /// The houses list, ordered by tile.
    houses: Vec<HouseId>,
}

impl HousesByTile {
    fn new(world: &World) -> Self {
        let mut pairs: Vec<(TileIndex, HouseId)> = Vec::new();
        let mut entrances = Vec::new();
        for (id, _) in world.houses() {
            world.house_entrances_into(id, &mut entrances);
            for t in &entrances {
                pairs.push((*t, id));
            }
        }
        Self::from_entrances(world.grid().len() as usize, &pairs)
    }

    /// A counting sort over the `(tile, house)` pairs, in three passes: count,
    /// running sum, fill.
    ///
    /// Linear in the number of tiles plus the number of entrances, with no
    /// comparisons at all.
    fn from_entrances(tiles: usize, pairs: &[(TileIndex, HouseId)]) -> Self {
        // `offsets` carries three meanings in turn, one per pass below, and
        // reusing the one array for all three is what makes the sort read oddly.
        // Five tiles, with the pairs arriving as (2,A) (0,B) (2,C) (4,D) (2,E):
        //
        //   index         0   1   2   3   4   5
        //   after count   1   0   3   0   1   0   how many houses face the tile
        //   after sum     1   1   4   4   5   5   the end of the tile's group
        //   after fill    0   1   1   4   4   5   its start, which `get` reads
        //
        //   houses        B   E   C   A   D
        //   index         0   1   2   3   4
        //
        // Follow tile 2: its slot starts at 4 and is stepped down to 3, 2, 1 as
        // A, C and E are written behind it. Stepped down once per house of its
        // own tile, it lands on the start of its run. Tile 1 has no houses, is
        // never touched, and keeps what the sum left there — which is both the
        // end of tile 0's group and the start of its own empty one.
        //
        // Reading back: `get(2)` is `houses[1..4]`, the three houses of tile 2;
        // `get(1)` is `houses[1..1]`, empty because start and end coincide.

        let mut offsets = vec![0u32; tiles + 1];

        // Count: each entry becomes the number of houses facing its tile.
        for (t, _) in pairs {
            if let Some(count) = offsets.get_mut(t.as_usize()) {
                *count += 1;
            }
        }

        // Running sum: each entry becomes the end of its tile's group.
        let mut running = 0u32;
        for o in &mut offsets {
            running += *o;
            *o = running;
        }

        // Fill: each house is written behind those already placed for its tile,
        // leaving the entry on the start of the group.
        let mut houses = vec![HouseId::default(); running as usize];
        for (t, h) in pairs {
            let Some(cursor) = offsets.get_mut(t.as_usize()) else {
                continue;
            };
            // A cursor cannot run past the start of its own group, because the
            // pass above counted the very same pairs. It is written as a check
            // rather than a subtraction because nothing in the core panics.
            let Some(start) = cursor.checked_sub(1) else {
                continue;
            };
            *cursor = start;
            if let Some(slot) = houses.get_mut(start as usize) {
                *slot = *h;
            }
        }

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
fn compute_from_scratch(world: &World) -> Coverage {
    // The reverse map from road tile to the houses facing onto it. Built once
    // per recomputation instead of once per provider.
    let houses_by_tile = HousesByTile::new(world);

    // The assignments accumulate here, not in the `Coverage`, and for a reason
    // of cost: during the computation they have to be **queried** once per
    // candidate of every provider — ~100,000 times at the reference scale,
    // because `pick_within_capacity` walks every candidate and does not stop
    // when it fills up (it skips whoever does not fit). The `Coverage` is
    // materialised at the end, once.
    let mut assigned: SecondaryMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]> =
        SecondaryMap::new();

    // Re-used Map across buildings to avoid multiple allocations.
    // It could be an HashSet if not shared across the buildings.
    // If the value matches, the house id is already seen.
    let mut seen: SecondaryMap<HouseId, u32> = SecondaryMap::new();

    // (distance, tile_index, house_id)
    // Cleared for each building.
    let mut candidates: Vec<(u16, TileIndex, HouseId)> = Vec::new();

    // All the entrances of the building.
    // Cleared for each building.
    let mut entrances: Vec<TileIndex> = Vec::new();

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
    for (index, (provider, b)) in world.buildings().enumerate() {
        // We use index as `epoch`. `epoch` is used to distinguish iterations,
        // so we can re-use same objects, like `seen` and `visited`.
        let epoch = index as u32;
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
            // A provider not hooked up to the network serves nobody.
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
                    // we already proceed this house for the "epoch" building.
                    continue;
                }
                seen.insert(*h, epoch);
                let Some(house) = world.house(*h) else {
                    continue;
                };
                let Some(idx) = world.grid().index(house.origin) else {
                    continue;
                };
                candidates.push((d, idx, *h));
            }
        });

        // **Priority order**, game semantics: when the candidates exceed the
        // capacity, the nearest ones are served; at equal distance the smaller
        // `TileIndex` wins (just to have a stable sort).
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
    // The counter is carried across by hand because the recomputation replaces
    // the whole structure, and it is carried only where it exists: without
    // `counters` these two statements compile to nothing, which is the point of
    // putting it behind a feature.
    #[cfg(feature = "counters")]
    let recomputes = world.coverage.recomputes;
    world.coverage = compute_from_scratch(world);
    #[cfg(feature = "counters")]
    {
        world.coverage.recomputes = recomputes.wrapping_add(1);
    }
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
    for (id, services) in &coverage.served_by {
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
    use slotmap::SlotMap;

    use super::{HousesByTile, pick_within_capacity};
    use crate::grid::TileIndex;
    use crate::ids::HouseId;

    /// The counting sort, against the grouping done the obvious way.
    ///
    /// The recordings already cover this, but only as "a hash moved": a cursor
    /// off by one files a house under the neighbouring tile, and every provider
    /// on that street then serves the wrong set of houses. This says which tile.
    ///
    /// The two sides are compared **sorted**, because the order within a group
    /// is deliberately not promised — see [`HousesByTile::from_entrances`].
    #[test]
    fn a_group_holds_exactly_the_houses_of_its_tile() {
        let mut ids: SlotMap<HouseId, ()> = SlotMap::with_key();
        let h: Vec<HouseId> = (0..8).map(|_| ids.insert(())).collect();
        let at = |t: u16, i: usize| (TileIndex::new(t), h[i]);

        // The first tile, the last one, a tile carrying the full four, empty
        // tiles either side, and a walk whose pairs do not arrive in tile order.
        let cases: &[(usize, Vec<(TileIndex, HouseId)>)] = &[
            (9, Vec::new()),
            (1, vec![at(0, 0)]),
            (
                9,
                vec![
                    at(4, 0),
                    at(0, 1),
                    at(8, 2),
                    at(4, 3),
                    at(4, 4),
                    at(0, 5),
                    at(4, 6),
                    at(8, 7),
                ],
            ),
        ];

        for (tiles, pairs) in cases {
            let index = HousesByTile::from_entrances(*tiles, pairs);

            let mut total = 0;
            for t in 0..*tiles {
                let mut expected: Vec<HouseId> = pairs
                    .iter()
                    .filter(|(p, _)| p.as_usize() == t)
                    .map(|(_, id)| *id)
                    .collect();
                let mut got = index.get(TileIndex::new(t as u16)).to_vec();
                expected.sort_unstable();
                got.sort_unstable();
                assert_eq!(got, expected, "tile {t} of {tiles}");
                total += got.len();
            }
            // Nothing is stranded in the backing array outside every group.
            // The loop above cannot see that: a running sum that started
            // somewhere other than zero would leave a gap no tile reaches,
            // while each group it does reach still held the right houses.
            assert_eq!(total, index.houses.len(), "{tiles} tiles");
        }
    }

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
