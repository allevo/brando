//! Step 1: sources compete for tiles.
//!
//! Each source keeps a set `A` of tiles it has claimed (seeded with its own
//! input point) and a set `B` of `A`'s untransformed neighbours, not yet
//! claimed by anyone. Sources act in the order they were given; a source
//! with strength `s` among a list whose greatest strength is `max_strength`
//! gets a turn only on rounds where `round % (max_strength - s + 1) == 0` —
//! the weaker a source, the longer between its turns. On its turn a source
//! with a non-empty `B` draws one tile from it uniformly at random, moves it
//! into `A`, pushes every unclaimed neighbour of that tile into its own
//! `B`, and removes the tile from every other source's `B` — it is no
//! longer anyone else's to claim. This repeats until every tile has an
//! owner.
//!
//! Termination is provable rather than assumed: the grid is fully connected
//! and has no obstacles yet, so whenever tiles remain unclaimed the
//! boundary of the claimed region is non-empty, and a boundary tile sits in
//! some source's `B` by construction. Every source's turn period is finite,
//! so the loop ends within `tiles * (max_strength + 1)` rounds.

use sim_core::{Grid, TileIndex};

use crate::{Source, dice};

/// Which source owns a tile. A newtype rather than a bare index, so a
/// source's position in the input list is never mixed up with a tile's
/// position on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceId(usize);

impl SourceId {
    pub(crate) const fn new(v: usize) -> Self {
        Self(v)
    }
}

/// The result of step 1.
pub(crate) struct AssignResult {
    /// `owner[i]` — which source tile `i` belongs to. Always `Some` once
    /// this function returns: the loop does not stop until every tile has
    /// one, and this one array *is* every source's `A` at once.
    pub(crate) owner: Vec<Option<SourceId>>,
    /// `claimed[i]` — source `i`'s own `A`, in the order it claimed each
    /// tile. `claimed[i][0]` is always that source's input point. Step 2
    /// reads this to pick a uniformly random origin and to know `|A_i|`.
    pub(crate) claimed: Vec<Vec<TileIndex>>,
}

/// One source's `B`: a list of pending tiles, plus a same-length index
/// giving each tile's position in the list, so removing a specific tile
/// ("it is now taken") is O(1) rather than a scan, and pushing a tile
/// already queued is a no-op rather than a duplicate entry.
struct Frontier {
    tiles: Vec<TileIndex>,
    position: Vec<Option<u32>>,
}

impl Frontier {
    fn new(tile_count: usize) -> Self {
        Self {
            tiles: Vec::new(),
            position: vec![None; tile_count],
        }
    }

    fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    fn push(&mut self, t: TileIndex) {
        let i = usize::from(t.get());
        if self.position[i].is_none() {
            self.position[i] = Some(self.tiles.len() as u32);
            self.tiles.push(t);
        }
    }

    fn remove(&mut self, t: TileIndex) {
        let i = usize::from(t.get());
        if let Some(at) = self.position[i].take() {
            let removed = self.tiles.swap_remove(at as usize);
            debug_assert_eq!(removed, t, "position must point at the tile it names");
            if let Some(&replacement) = self.tiles.get(at as usize) {
                self.position[usize::from(replacement.get())] = Some(at);
            }
        }
    }
}

/// Runs step 1: every tile ends up owned by exactly one source.
///
/// `sources` is already validated by `pipeline::draw` — every point in
/// bounds, no two coinciding — and this function trusts both.
pub(crate) fn run(seed: u64, sources: &[Source], grid: &Grid, max_strength: u16) -> AssignResult {
    let n = grid.len() as usize;
    let mut owner: Vec<Option<SourceId>> = vec![None; n];
    let mut claimed: Vec<Vec<TileIndex>> = vec![Vec::new(); sources.len()];
    let mut frontiers: Vec<Frontier> = (0..sources.len()).map(|_| Frontier::new(n)).collect();

    // Every source's own point is claimed first, in one pass, before any
    // frontier is built — so a frontier never mistakes a neighbour's own
    // starting point for ground still open to claim.
    for (i, s) in sources.iter().enumerate() {
        let idx = grid
            .index(s.point)
            .expect("source point validated in bounds");
        owner[usize::from(idx.get())] = Some(SourceId::new(i));
        claimed[i].push(idx);
    }
    for (i, s) in sources.iter().enumerate() {
        let idx = grid
            .index(s.point)
            .expect("source point validated in bounds");
        for nb in grid.neighbors4(idx) {
            if owner[usize::from(nb.get())].is_none() {
                frontiers[i].push(nb);
            }
        }
    }

    let mut rng = dice::stream(seed, "assign");
    let mut remaining = n - sources.len();
    let round_cap = (n as u64).saturating_mul(u64::from(max_strength) + 1) + 1;
    let mut round: u64 = 0;

    while remaining > 0 {
        for (i, s) in sources.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            let period = u64::from(max_strength - s.strength) + 1;
            if !round.is_multiple_of(period) || frontiers[i].is_empty() {
                continue;
            }

            let pick = dice::below(&mut rng, frontiers[i].tiles.len() as u64) as usize;
            let tile = frontiers[i].tiles[pick];
            frontiers[i].remove(tile);
            owner[usize::from(tile.get())] = Some(SourceId::new(i));
            claimed[i].push(tile);
            remaining -= 1;

            for nb in grid.neighbors4(tile) {
                if owner[usize::from(nb.get())].is_none() {
                    frontiers[i].push(nb);
                }
            }
            for (j, frontier) in frontiers.iter_mut().enumerate() {
                if j != i {
                    frontier.remove(tile);
                }
            }
        }
        round += 1;
        // Backed by the termination proof in the module doc; a safety net
        // on it, not a substitute for it.
        debug_assert!(
            round < round_cap,
            "step 1 did not converge — a bug, not a seed"
        );
    }

    AssignResult { owner, claimed }
}

#[cfg(test)]
mod tests {
    use sim_core::{Grid, Terrain, TilePos};

    use super::*;
    use crate::SourceKind;

    fn grid(w: u16, h: u16) -> Grid {
        Grid::new(w, h, Terrain::Plain).expect("valid size")
    }

    fn sources(points: &[(u8, u8)]) -> Vec<Source> {
        points
            .iter()
            .map(|&(x, y)| Source {
                point: TilePos::new(x, y),
                kind: SourceKind::Land,
                strength: 1,
            })
            .collect()
    }

    #[test]
    fn every_tile_ends_up_with_exactly_one_owner() {
        let g = grid(8, 8);
        let s = sources(&[(0, 0), (7, 7), (0, 7)]);
        let result = run(1, &s, &g, 1);
        assert!(result.owner.iter().all(Option::is_some));
        assert_eq!(result.owner.len(), 64);
    }

    #[test]
    fn a_sources_claimed_tiles_are_always_disjoint_from_every_others() {
        let g = grid(10, 10);
        let s = sources(&[(1, 1), (8, 8), (1, 8), (8, 1)]);
        let result = run(2, &s, &g, 1);
        let mut seen = [false; 100];
        for claims in &result.claimed {
            for t in claims {
                let i = usize::from(t.get());
                assert!(!seen[i], "tile {i} claimed twice");
                seen[i] = true;
            }
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn every_source_claims_its_own_starting_point_first() {
        let g = grid(6, 6);
        let s = sources(&[(2, 2), (4, 4)]);
        let result = run(3, &s, &g, 1);
        assert_eq!(result.claimed[0][0], g.index(TilePos::new(2, 2)).unwrap());
        assert_eq!(result.claimed[1][0], g.index(TilePos::new(4, 4)).unwrap());
    }

    #[test]
    fn a_weak_source_still_eventually_claims_tiles_around_it() {
        let g = grid(20, 20);
        let s = vec![
            Source {
                point: TilePos::new(0, 0),
                kind: SourceKind::Land,
                strength: 100,
            },
            Source {
                point: TilePos::new(19, 19),
                kind: SourceKind::Land,
                strength: 1,
            },
        ];
        let result = run(4, &s, &g, 100);
        assert!(
            result.claimed[1].len() > 1,
            "the weak source never got a single extra tile"
        );
    }

    #[test]
    fn assign_is_deterministic_for_the_same_seed_and_differs_for_a_different_one() {
        let g = grid(12, 12);
        let s = sources(&[(1, 1), (10, 10), (1, 10)]);
        let a = run(9, &s, &g, 1).owner;
        let b = run(9, &s, &g, 1).owner;
        assert_eq!(a, b);
        let c = run(10, &s, &g, 1).owner;
        assert_ne!(a, c);
    }
}
