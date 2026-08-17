//! The road network: connected components and walked distances.
//!
//! A **derived** structure: rebuildable at any moment from the `Grid`. It does
//! not enter the state hash — if it did, a bug in the rebuild would show up as
//! a diverging hash instead of a failing equivalence test, and the divergence
//! would not say *where* the problem is.
//!
//! Every road tile costs 1: no variable crossing cost, no road levels, no
//! one-way streets. Those are M1 or later.

use crate::grid::{Grid, TileIndex};

/// Identifier of a connected component: the **smallest** `TileIndex` among its
/// tiles.
///
/// Not a running counter. It costs the same — the scan is already in
/// increasing order — and it makes the labelling a function of the set of
/// roads alone, not of the order they were placed in. Without it, two games
/// that build the same roads in a different order would have different states.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ComponentId(TileIndex);

impl ComponentId {
    pub const fn tile(self) -> TileIndex {
        self.0
    }
}

/// The roads labelled into connected components.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoadNetwork {
    /// For each tile: its component, or `None` if it is not a road.
    ///
    /// `Option<ComponentId>` costs 4 bytes per tile instead of 2, i.e. 256 KB
    /// on the largest map. This is a derived, diagnostic structure, not the
    /// state: the tight budget is `Tile`'s, not this one's.
    component: Vec<Option<ComponentId>>,
    /// How many full rebuilds have been performed. It serves the dirty-flag
    /// tests, not the game.
    #[cfg(feature = "counters")]
    rebuilds: u32,
}

impl RoadNetwork {
    pub fn new(tiles: u32) -> Self {
        Self {
            component: vec![None; tiles as usize],
            #[cfg(feature = "counters")]
            rebuilds: 0,
        }
    }

    pub fn component(&self, idx: TileIndex) -> Option<ComponentId> {
        self.component.get(idx.as_usize()).copied().flatten()
    }

    #[cfg(feature = "counters")]
    pub const fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    pub fn is_road(&self, idx: TileIndex) -> bool {
        self.component(idx).is_some()
    }

    /// Two road tiles are connected if they are in the same component.
    pub fn connected(&self, a: TileIndex, b: TileIndex) -> bool {
        match (self.component(a), self.component(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    /// Number of distinct components.
    pub fn component_count(&self) -> usize {
        let mut seen: Vec<ComponentId> = self.component.iter().flatten().copied().collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    /// Relabels everything from scratch, scanning the tiles in increasing
    /// `TileIndex` order.
    ///
    /// A full rebuild rather than an incremental one, deliberately: scanning
    /// 40,000 tiles once is irrelevant until a profiler says otherwise, and the
    /// dirty flag already avoids doing it every tick. What was needed straight
    /// away was the *flag*, not the clever algorithm.
    pub(crate) fn rebuild(&mut self, grid: &Grid) {
        self.component.clear();
        self.component.resize(grid.len() as usize, None);
        #[cfg(feature = "counters")]
        {
            self.rebuilds = self.rebuilds.wrapping_add(1);
        }

        let mut queue: Vec<TileIndex> = Vec::new();
        for root in grid.indices() {
            if !is_road(grid, root) || self.component(root).is_some() {
                continue;
            }
            // The root is the first tile not yet labelled in increasing order,
            // so it is the smallest of its component: the id falls out of
            // the scan itself, with no second pass.
            let id = ComponentId(root);
            self.component[root.as_usize()] = Some(id);
            queue.clear();
            queue.push(root);
            while let Some(t) = queue.pop() {
                for v in grid.neighbors4(t) {
                    if is_road(grid, v) && self.component(v).is_none() {
                        self.component[v.as_usize()] = Some(id);
                        queue.push(v);
                    }
                }
            }
        }
    }
}

fn is_road(grid: &Grid, idx: TileIndex) -> bool {
    grid.get(idx).is_some_and(|t| t.has_road())
}

/// The set of tiles already visited by a BFS, reusable between one call and
/// the next.
///
/// It is neither state nor a derived structure: it is a temporary note, and it
/// lives in the caller. It exists because `bfs_roads` used to allocate a
/// buffer the size of the grid **on every call**, i.e. once per provider:
/// 156 KB for 1,219 providers at the reference scale, on memory the BFS then
/// touches 1% of.
///
/// The generation marker saves having to clear it: "visited" means
/// `epochs[i] == current`, so an entry left over from the previous round is
/// invisible **by construction**. The alternative — clearing only the tiles
/// touched — costs less memory but depends on the invariant "marked ≡ visited",
/// which holds today and might stop holding tomorrow after an innocuous change
/// to the loop below. And it would be a bug the equivalence test would **not**
/// catch, because `compute_from_scratch` would use a shared scratch buffer too
/// and would get it wrong in the same way: the oracle would go blind on
/// exactly this class of bug.
#[derive(Debug, Clone, Default)]
pub struct Visited {
    epochs: Vec<u32>,
    current: u32,
}

impl Visited {
    pub fn new(tiles: u32) -> Self {
        Self {
            epochs: vec![0; tiles as usize],
            current: 0,
        }
    }

    /// Opens a fresh round: from here on no tile counts as visited.
    fn begin(&mut self, tiles: usize) {
        if self.epochs.len() != tiles {
            self.epochs.clear();
            self.epochs.resize(tiles, 0);
            self.current = 0;
        }
        // On wrap it resets and restarts from 1: `current` is never 0, which is
        // the value the array is born with.
        self.current = match self.current.checked_add(1) {
            Some(c) => c,
            None => {
                self.epochs.fill(0);
                1
            }
        };
    }

    fn is_seen(&self, i: usize) -> bool {
        self.epochs.get(i) == Some(&self.current)
    }

    fn mark(&mut self, i: usize) {
        if let Some(e) = self.epochs.get_mut(i) {
            *e = self.current;
        }
    }
}

/// A BFS over the road network, cut off at a maximum distance.
///
/// It starts from the tiles in `start` at distance 0 and visits only road
/// tiles, stopping beyond `max`. The cutoff is not an optional optimisation:
/// without it, a range of 12 in a large city would visit the whole network.
///
/// `visit` receives all the tiles at one distance **in a single call**, each
/// tile exactly once and at the smallest distance it can be reached at. The
/// calls come in increasing distance, and within a call the tiles are in
/// increasing `TileIndex`: it is a total order, so whoever builds a game rule
/// on top of it (phase 06) does not depend on BFS internals.
///
/// **A whole distance at a time, and not a tile at a time.** A caller that has
/// to break ties among what it finds does so on something of its own — the
/// coverage orders the *houses* facing the tiles, by the house's origin, which
/// is not the order the tiles arrive in. Handing over one tile at a time would
/// let it act on a partial distance, and the answer would depend on which road
/// tile a house happened to be reached from.
///
/// Returning `false` ends the walk after that distance, and the tiles beyond it
/// are never touched. It is what lets a provider that has run out of capacity
/// stop instead of walking to its full range.
///
/// `visited` is opened in a fresh round on every call: its previous contents do
/// not affect the result, and passing in one already used is the intended way
/// to use it.
pub fn bfs_roads(
    grid: &Grid,
    start: &[TileIndex],
    max: u16,
    visited: &mut Visited,
    mut visit: impl FnMut(&[TileIndex], u16) -> bool,
) {
    visited.begin(grid.len() as usize);

    let mut level: Vec<TileIndex> = start
        .iter()
        .copied()
        .filter(|t| is_road(grid, *t))
        .collect();
    level.sort_unstable();
    level.dedup();
    for t in &level {
        visited.mark(t.as_usize());
    }

    let mut d = 0u16;
    let mut next: Vec<TileIndex> = Vec::new();
    while !level.is_empty() {
        if !visit(&level, d) {
            break;
        }
        if d == max {
            break;
        }
        next.clear();
        for t in &level {
            for v in grid.neighbors4(*t) {
                if is_road(grid, v) && !visited.is_seen(v.as_usize()) {
                    visited.mark(v.as_usize());
                    next.push(v);
                }
            }
        }
        next.sort_unstable();
        std::mem::swap(&mut level, &mut next);
        d += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{Visited, bfs_roads};
    use crate::grid::{Grid, Terrain, TileIndex, TilePos};

    /// An 8x8 grid with a horizontal road along row `y`.
    fn with_road(y: u8) -> Grid {
        let mut g = Grid::new(8, 8, Terrain::Plain).expect("valid grid");
        for x in 0..8u8 {
            let idx = g.index(TilePos::new(x, y)).expect("on the map");
            if let Some(t) = g.get_mut(idx) {
                t.set_road(true);
            }
        }
        g
    }

    fn reached(grid: &Grid, from: TileIndex, max: u16, v: &mut Visited) -> Vec<(TileIndex, u16)> {
        let mut out = Vec::new();
        bfs_roads(grid, &[from], max, v, |tiles, d| {
            out.extend(tiles.iter().map(|t| (*t, d)));
            true
        });
        out
    }

    /// **The test that guards the way a reused scratch buffer can break.**
    ///
    /// If one round left traces visible to the next, the second BFS would
    /// believe tiles were already visited when they were not, and would skip
    /// part of them. There would be no panic: only silently smaller coverage.
    /// And `coverage_equivalence` would not catch it, because
    /// `compute_from_scratch` also reuses one scratch buffer across its
    /// providers and would get it wrong the same way — the oracle would be
    /// blind on this class, and this test is what covers it.
    #[test]
    fn reusing_the_scratch_gives_the_same_result_as_a_fresh_one() {
        let grid = with_road(3);
        let a = grid.index(TilePos::new(0, 3)).expect("on the map");
        let b = grid.index(TilePos::new(7, 3)).expect("on the map");

        let expected_a = reached(&grid, a, 4, &mut Visited::new(grid.len()));
        let expected_b = reached(&grid, b, 4, &mut Visited::new(grid.len()));
        assert!(!expected_a.is_empty() && !expected_b.is_empty());

        // The same two BFS runs, back to back on the same scratch, for three
        // rounds: one alone would not tell "it cleans up" from "it has not
        // dirtied anything yet".
        let mut reused = Visited::new(grid.len());
        for round in 0..3 {
            assert_eq!(
                reached(&grid, a, 4, &mut reused),
                expected_a,
                "round {round}"
            );
            assert_eq!(
                reached(&grid, b, 4, &mut reused),
                expected_b,
                "round {round}"
            );
        }
    }

    /// Wrapping the generation counter does not make old traces visible again.
    /// Without the reset on overflow, the epoch would come back to a value
    /// already written into the array and some tiles would look visited.
    #[test]
    fn wrapping_the_counter_does_not_revive_old_traces() {
        let grid = with_road(3);
        let a = grid.index(TilePos::new(0, 3)).expect("on the map");
        let expected = reached(&grid, a, 4, &mut Visited::new(grid.len()));

        let mut at_the_limit = Visited::new(grid.len());
        // One real round, to leave a mark in the array...
        let _ = reached(&grid, a, 4, &mut at_the_limit);
        // ...then bring the counter one step short of wrapping.
        at_the_limit.current = u32::MAX;
        assert_eq!(
            reached(&grid, a, 4, &mut at_the_limit),
            expected,
            "at the wrap"
        );
        assert_eq!(
            reached(&grid, a, 4, &mut at_the_limit),
            expected,
            "after the wrap"
        );
    }

    /// Stopping at a distance leaves everything beyond it untouched, and what
    /// came before it is exactly what a walk cut off there would have given.
    ///
    /// Two halves, and the second is the one worth having: a stop that merely
    /// hid the far tiles from the caller while still walking them would pass
    /// the first assertion and buy nothing.
    #[test]
    fn stopping_at_a_distance_walks_no_further() {
        let grid = with_road(3);
        let a = grid.index(TilePos::new(0, 3)).expect("on the map");

        for stop_at in 0..4u16 {
            let mut seen = Vec::new();
            let mut marked = 0;
            let mut v = Visited::new(grid.len());
            bfs_roads(&grid, &[a], 7, &mut v, |tiles, d| {
                seen.extend(tiles.iter().map(|t| (*t, d)));
                d < stop_at
            });
            for i in 0..grid.len() as usize {
                marked += usize::from(v.is_seen(i));
            }

            let expected = reached(&grid, a, stop_at, &mut Visited::new(grid.len()));
            assert_eq!(seen, expected, "stopping at {stop_at}");
            assert_eq!(
                marked,
                expected.len(),
                "stopping at {stop_at}: no tile beyond it is even marked"
            );
        }
    }

    /// A scratch born for a different grid must not be used blindly: `begin`
    /// notices from its length and starts clean.
    #[test]
    fn a_wrongly_sized_scratch_gets_rebuilt() {
        let grid = with_road(3);
        let a = grid.index(TilePos::new(0, 3)).expect("on the map");
        let expected = reached(&grid, a, 4, &mut Visited::new(grid.len()));

        let mut someone_elses = Visited::new(4);
        assert_eq!(reached(&grid, a, 4, &mut someone_elses), expected);
    }
}
