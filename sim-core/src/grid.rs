//! The tile grid and the types that make it up.
//!
//! Memory constraint: `Tile` fits in 4 bytes, so 40,000 tiles take 160 KB and
//! stay in cache (CLAUDE.md, state model). Every field added here has to be
//! weighed against that budget, which a test guards.

use serde::{Deserialize, Serialize};

use crate::ids::{TileIdx, TilePos};

/// Maximum side of the grid (A4). Beyond it, `TileIdx(u16)` would not be enough.
pub const MAX_SIDE: u16 = 256;

/// The kind of ground on a tile. The numbers that go with it (buildable? road
/// cost?) do not live here: they come from the `sim-data` tables (D6).
///
/// The three kinds, in plain terms:
/// - `Plain` — ordinary flat ground. You can build on it and lay roads on it.
///   It is the default the whole map starts as.
/// - `Water` — rivers and sea. Nothing can be built and no road can cross it.
/// - `Rock` — rocky ground. No building fits on it, but a road can be cut
///   through, at a higher cost than on `Plain`.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Terrain {
    #[default]
    Plain,
    Water,
    Rock,
}

impl Terrain {
    /// Every variant, in a stable order. `sim-data` uses it to check that the
    /// terrain table is complete.
    pub const ALL: [Terrain; 3] = [Terrain::Plain, Terrain::Water, Terrain::Rock];
}

/// Status bits of a tile. Hand-written instead of using `bitflags` so as not
/// to add a dependency to `sim-core` (D1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct TileFlags(u8);

impl TileFlags {
    const HAS_ROAD: u8 = 1 << 0;
    /// Whether the tile is occupied. It needs a bit of its own because the
    /// occupant's index no longer has a sentinel value: any `TileIdx`,
    /// `u16::MAX` included, is a legitimate origin for a building on a 256x256
    /// map.
    const HAS_OCCUPANT: u8 = 1 << 1;
    /// Whether the occupant is a house; otherwise it is a building.
    const OCCUPANT_IS_HOUSE: u8 = 1 << 2;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn has_road(self) -> bool {
        self.0 & Self::HAS_ROAD != 0
    }

    pub const fn set_road(&mut self, on: bool) {
        self.set(Self::HAS_ROAD, on);
    }

    pub const fn has_occupant(self) -> bool {
        self.0 & Self::HAS_OCCUPANT != 0
    }

    pub const fn occupant_is_house(self) -> bool {
        self.0 & Self::OCCUPANT_IS_HOUSE != 0
    }

    pub(crate) const fn set(&mut self, bit: u8, on: bool) {
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

impl std::fmt::Debug for TileFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TileFlags(road={}, occupied={}, house={})",
            self.has_road(),
            self.has_occupant(),
            self.occupant_is_house()
        )
    }
}

/// Whoever occupies a tile, as a compact reference.
///
/// The occupant is identified by the **origin tile** of its area, not by a
/// running index handed out at build time: this way the state of a tile is a
/// function of the city and not of the order things were placed in, and no
/// free list of slots has to be kept consistent. The map from origin to
/// `BuildingId`/`HouseId` lives in the `World`, not in the tile.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TileOccupant {
    pub origin: TileIdx,
    pub is_house: bool,
}

/// Budget: 4 bytes. 40,000 tiles ⇒ 160 KB, which fits in L2.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Tile {
    pub terrain: Terrain,
    pub flags: TileFlags,
    /// Only valid if `flags.has_occupant()`. Private: reading it without
    /// checking the flag would give the origin of an occupant already removed.
    occupant_origin: TileIdx,
}

impl Tile {
    /// The tile's occupant, if there is one.
    pub const fn occupant(&self) -> Option<TileOccupant> {
        if self.flags.has_occupant() {
            Some(TileOccupant {
                origin: self.occupant_origin,
                is_house: self.flags.occupant_is_house(),
            })
        } else {
            None
        }
    }

    pub const fn is_free(&self) -> bool {
        !self.flags.has_occupant() && !self.flags.has_road()
    }

    pub const fn set_occupant(&mut self, occ: TileOccupant) {
        self.occupant_origin = occ.origin;
        self.flags.set(TileFlags::HAS_OCCUPANT, true);
        self.flags.set(TileFlags::OCCUPANT_IS_HOUSE, occ.is_house);
    }

    pub const fn clear_occupant(&mut self) {
        self.flags.set(TileFlags::HAS_OCCUPANT, false);
        self.flags.set(TileFlags::OCCUPANT_IS_HOUSE, false);
        self.occupant_origin = TileIdx::new(0);
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GridError {
    #[error("invalid grid size: {width}x{height}, each side must be 1..={max}")]
    InvalidSize { width: u16, height: u16, max: u16 },
}

/// A dense grid of tiles, indexed `y * width + x`.
///
/// `width` and `height` are `u16` rather than `u8` because the maximum side is
/// 256, which would not fit in a `u8`: the allowed values are `1..=256`. The
/// coordinates stay `u8` (0..=255).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Grid {
    width: u16,
    height: u16,
    tiles: Vec<Tile>,
}

impl Grid {
    /// A uniform grid. Rejects sides of 0 or beyond [`MAX_SIDE`].
    pub fn new(width: u16, height: u16, terrain: Terrain) -> Result<Self, GridError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(GridError::InvalidSize {
                width,
                height,
                max: MAX_SIDE,
            });
        }
        let len = usize::from(width) * usize::from(height);
        let tile = Tile {
            terrain,
            ..Tile::default()
        };
        Ok(Self {
            width,
            height,
            tiles: vec![tile; len],
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Number of tiles. `u32` and not `usize`: the maximum is 65,536.
    pub const fn len(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    /// Always `false`: a grid with a side of zero cannot be built.
    /// It exists only so `len()` is not left without its companion.
    pub const fn is_empty(&self) -> bool {
        false
    }

    pub const fn in_bounds(&self, pos: TilePos) -> bool {
        (pos.x as u16) < self.width && (pos.y as u16) < self.height
    }

    /// Linear index of the position, `None` if off the map.
    pub const fn idx(&self, pos: TilePos) -> Option<TileIdx> {
        if !self.in_bounds(pos) {
            return None;
        }
        let i = pos.y as u32 * self.width as u32 + pos.x as u32;
        // Invariant: in_bounds implies i < width*height <= 65_536, so i fits in
        // a u16 (the last valid index is 65_535).
        Some(TileIdx::new(i as u16))
    }

    /// The position matching an index, `None` if off the map.
    pub const fn pos(&self, idx: TileIdx) -> Option<TilePos> {
        let i = idx.get() as u32;
        if i >= self.len() {
            return None;
        }
        let w = self.width as u32;
        Some(TilePos::new((i % w) as u8, (i / w) as u8))
    }

    pub fn get(&self, idx: TileIdx) -> Option<&Tile> {
        self.tiles.get(idx.as_usize())
    }

    pub fn get_mut(&mut self, idx: TileIdx) -> Option<&mut Tile> {
        self.tiles.get_mut(idx.as_usize())
    }

    pub fn at(&self, pos: TilePos) -> Option<&Tile> {
        self.get(self.idx(pos)?)
    }

    pub fn at_mut(&mut self, pos: TilePos) -> Option<&mut Tile> {
        let idx = self.idx(pos)?;
        self.get_mut(idx)
    }

    /// Every valid index, in increasing order. It is the scan order the rest of
    /// the core assumes: rebuilding the road network (phase 05) relies on it.
    pub fn indices(&self) -> impl Iterator<Item = TileIdx> {
        (0..self.len()).map(|i| TileIdx::new(i as u16))
    }

    /// The four orthogonal neighbours, **without wraparound**: the neighbour
    /// "to the right" of `x = width - 1` does not exist, it is not the first
    /// tile of the row below.
    ///
    /// Emission order: north, west, east, south — that is, increasing
    /// `TileIdx`. The order is part of the BFS determinism contract (D4).
    pub fn neighbors4(&self, idx: TileIdx) -> impl Iterator<Item = TileIdx> {
        let mut out = [None; 4];
        if let Some(p) = self.pos(idx) {
            let i = idx.get();
            if p.y > 0 {
                out[0] = Some(TileIdx::new(i - self.width));
            }
            if p.x > 0 {
                out[1] = Some(TileIdx::new(i - 1));
            }
            if u16::from(p.x) + 1 < self.width {
                out[2] = Some(TileIdx::new(i + 1));
            }
            if u16::from(p.y) + 1 < self.height {
                out[3] = Some(TileIdx::new(i + self.width));
            }
        }
        out.into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn tile_stays_within_budget() {
        assert_eq!(
            size_of::<Tile>(),
            4,
            "Tile has grown: 40,000 tiles have to fit in cache"
        );
    }

    #[test]
    fn new_rejects_sizes_out_of_range() {
        for (w, h) in [(0, 10), (10, 0), (257, 10), (10, 257)] {
            assert_eq!(
                Grid::new(w, h, Terrain::Plain),
                Err(GridError::InvalidSize {
                    width: w,
                    height: h,
                    max: MAX_SIDE
                })
            );
        }
        assert!(Grid::new(256, 256, Terrain::Plain).is_ok());
        assert!(Grid::new(1, 1, Terrain::Plain).is_ok());
    }

    #[test]
    fn the_largest_grid_can_index_its_last_tile() {
        let g = Grid::new(256, 256, Terrain::Plain).expect("256x256 is valid");
        assert_eq!(g.len(), 65_536);
        let last = g.idx(TilePos::new(255, 255)).expect("in bounds");
        assert_eq!(last.get(), u16::MAX);
        assert_eq!(g.pos(last), Some(TilePos::new(255, 255)));
    }

    /// Even the last tile of a 256x256 map (`TileIdx` = u16::MAX) can be the
    /// origin of an occupant: that is why presence lives in a flag rather than
    /// in a sentinel value.
    #[test]
    fn the_occupant_is_identified_by_its_origin_tile() {
        let mut t = Tile::default();
        assert_eq!(t.occupant(), None);
        assert!(t.is_free());

        let occ = TileOccupant {
            origin: TileIdx::new(u16::MAX),
            is_house: true,
        };
        t.set_occupant(occ);
        assert_eq!(t.occupant(), Some(occ));
        assert!(!t.is_free());

        t.clear_occupant();
        assert_eq!(t.occupant(), None);
        assert_eq!(t, Tile::default(), "clearing also resets the origin");
    }

    #[test]
    fn no_wraparound_at_the_edges() {
        let g = Grid::new(4, 4, Terrain::Plain).expect("valid grid");
        // The tile to the right of the first row is not a neighbour of the
        // first tile of the second row.
        let right = g.idx(TilePos::new(3, 0)).expect("in bounds");
        let left_of_next_row = g.idx(TilePos::new(0, 1)).expect("in bounds");
        let neighbors: Vec<_> = g.neighbors4(right).collect();
        assert!(!neighbors.contains(&left_of_next_row));
    }

    #[test]
    fn neighbor_count_by_position() {
        let g = Grid::new(5, 4, Terrain::Plain).expect("valid grid");
        let n = |x, y| {
            g.neighbors4(g.idx(TilePos::new(x, y)).expect("in bounds"))
                .count()
        };
        assert_eq!(n(0, 0), 2, "corner");
        assert_eq!(n(4, 3), 2, "opposite corner");
        assert_eq!(n(2, 0), 3, "edge");
        assert_eq!(n(2, 2), 4, "interior");
    }

    /// Grids of arbitrary size, plus a valid position inside them.
    fn grid_and_pos() -> impl Strategy<Value = (Grid, TilePos)> {
        (1u16..=MAX_SIDE, 1u16..=MAX_SIDE).prop_flat_map(|(w, h)| {
            let g = Grid::new(w, h, Terrain::Plain).expect("sizes in range");
            (Just(g), 0..w, 0..h).prop_map(|(g, x, y)| (g, TilePos::new(x as u8, y as u8)))
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Position -> index -> position gets you back where you started.
        #[test]
        fn pos_to_idx_round_trips((g, p) in grid_and_pos()) {
            let i = g.idx(p).expect("generated pos is in bounds");
            prop_assert_eq!(g.pos(i), Some(p));
        }

        /// And the other way round: index -> position -> index.
        #[test]
        fn idx_to_pos_round_trips((g, p) in grid_and_pos()) {
            let i = g.idx(p).expect("generated pos is in bounds");
            let p2 = g.pos(i).expect("valid index");
            prop_assert_eq!(g.idx(p2), Some(i));
        }

        /// Outside the edges there is no index.
        #[test]
        fn outside_the_edge_there_is_no_index((g, _p) in grid_and_pos()) {
            if g.width() < MAX_SIDE {
                let outside = TilePos::new(g.width() as u8, 0);
                prop_assert_eq!(g.idx(outside), None);
                prop_assert!(!g.in_bounds(outside));
            }
            prop_assert_eq!(g.pos(TileIdx::new(u16::MAX)).is_some(), g.len() == 65_536);
        }

        /// Every neighbour is on the map and at Manhattan distance 1; how many
        /// there are depends only on how many edges the tile touches.
        #[test]
        fn neighbors4_stays_on_the_map_and_does_not_wrap((g, p) in grid_and_pos()) {
            let i = g.idx(p).expect("generated pos is in bounds");
            let neighbors: Vec<_> = g.neighbors4(i).collect();

            for &v in &neighbors {
                let pv = g.pos(v).expect("neighbour on the map");
                prop_assert_eq!(p.manhattan(pv), 1);
            }

            let on_edge_x = u16::from(p.x) == 0 || u16::from(p.x) + 1 == g.width();
            let on_edge_y = u16::from(p.y) == 0 || u16::from(p.y) + 1 == g.height();
            let expected = if g.width() == 1 { 0 } else if on_edge_x { 1 } else { 2 }
                + if g.height() == 1 { 0 } else if on_edge_y { 1 } else { 2 };
            prop_assert_eq!(neighbors.len(), expected);

            // Neighbours come out in increasing TileIdx order (BFS contract).
            let mut sorted = neighbors.clone();
            sorted.sort_unstable();
            prop_assert_eq!(neighbors, sorted);
        }

        /// Being neighbours is a symmetric relation.
        #[test]
        fn neighbors4_is_symmetric((g, p) in grid_and_pos()) {
            let i = g.idx(p).expect("generated pos is in bounds");
            for v in g.neighbors4(i) {
                prop_assert!(g.neighbors4(v).any(|w| w == i));
            }
        }
    }
}
