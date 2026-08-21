//! The tile grid and the types that make it up.
//!
//! Memory constraint: `Tile` fits in 4 bytes, so 40,000 tiles take 160 KB and
//! stay in cache. `u16` indices, no pointers, no `Option<Box<...>>`: every field
//! added here has to be weighed against that budget, which a test guards.

use serde::{Deserialize, Serialize};

/// Maximum side of the grid. Beyond it, `TileIndex(u16)` would not be enough.
pub const MAX_SIDE: u16 = 256;

/// Linear tile index: `y * width + x`.
///
/// The map is at most 256x256, so 65,536 tiles: the last index is
/// `u16::MAX` and fits exactly.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TileIndex(u16);

impl TileIndex {
    pub const fn new(v: u16) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// A position on the grid. `x` and `y` are `u8` because the maximum side is
/// 256: valid coordinates run from 0 to 255.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TilePos {
    pub x: u8,
    pub y: u8,
}

impl TilePos {
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    /// Manhattan distance, as the crow flies. This is **not** the distance
    /// used by service coverage, which is measured along the road network (D2).
    pub const fn manhattan(self, other: Self) -> u16 {
        let dx = self.x.abs_diff(other.x) as u16;
        let dy = self.y.abs_diff(other.y) as u16;
        dx + dy
    }
}

/// The kind of ground on a tile.
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
    /// Every variant, in a stable order.
    pub const ALL: [Terrain; 3] = [Terrain::Plain, Terrain::Water, Terrain::Rock];

    /// How many kinds of ground there are, for the arrays indexed by one.
    pub const COUNT: usize = Self::ALL.len();

    /// Position in the arrays indexed by terrain.
    pub const fn index(self) -> usize {
        match self {
            Self::Plain => 0,
            Self::Water => 1,
            Self::Rock => 2,
        }
    }

    /// Whether a building may stand on this terrain.
    pub const fn is_buildable(self) -> bool {
        matches!(self, Terrain::Plain)
    }

    /// Whether a road may be laid on this terrain.
    ///
    /// A separate question from [`Terrain::is_buildable`], and the terrains
    /// answer the two differently on purpose: a road can be cut through `Rock`
    /// where no building fits — you cross a mountain, you do not settle on it —
    /// and `Water` takes neither.
    pub const fn is_walkable(self) -> bool {
        matches!(self, Terrain::Plain | Terrain::Rock)
    }
}

/// What a tile carries: a road, an occupant, and whether that occupant is a
/// house — a road is not an occupant, and no tile ever has both.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct TileFlags(u8);

impl TileFlags {
    /// Whether a road runs over the tile.
    const HAS_ROAD: u8 = 1 << 0;
    /// Whether the tile is occupied.
    const HAS_OCCUPANT: u8 = 1 << 1;
    /// Whether the occupant is a house; otherwise it is a building.
    const OCCUPANT_IS_HOUSE: u8 = 1 << 2;

    /// The flags.
    const fn bits(self) -> u8 {
        self.0
    }

    /// Whether a road runs over the tile.
    const fn has_road(self) -> bool {
        self.0 & Self::HAS_ROAD != 0
    }

    /// Whether a building or a house stands on the tile.
    const fn has_occupant(self) -> bool {
        self.0 & Self::HAS_OCCUPANT != 0
    }

    /// Whether the occupant is a house rather than a building, meaningful only
    /// while `has_occupant` is set.
    const fn occupant_is_house(self) -> bool {
        self.0 & Self::OCCUPANT_IS_HOUSE != 0
    }

    /// Turns a raw bit mask on or off.
    const fn set(&mut self, bit: u8, on: bool) {
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
    pub origin: TileIndex,
    pub is_house: bool,
}

// Bit layout of `Tile::packed`. Three fields share one `u16` rather than each
// taking a byte of their own, because a byte each makes `Tile` six bytes with
// the alignment, and 40,000 tiles then take 240 KB instead of 160 KB — the
// budget the assert below guards.
//
// terrain 0..4 (4 bits, sixteen kinds fit, three exist) · flags 4..8 (4 bits,
// `TileFlags` uses three of them) · ground height 8..13 (5 bits, 0..=31) ·
// 3 bits left spare on purpose: a bridge flag, a fourth occupant kind, a
// second height for water depth are each one bit, and re-packing later moves
// every recorded hash — leaving room now is the only time it is free.
const TERRAIN_SHIFT: u16 = 0;
const TERRAIN_BITS: u16 = 4;
const TERRAIN_MASK: u16 = (1 << TERRAIN_BITS) - 1;

const FLAGS_SHIFT: u16 = TERRAIN_SHIFT + TERRAIN_BITS;
const FLAGS_BITS: u16 = 4;
const FLAGS_MASK: u16 = (1 << FLAGS_BITS) - 1;

const HEIGHT_SHIFT: u16 = FLAGS_SHIFT + FLAGS_BITS;
const HEIGHT_BITS: u16 = 5;
const HEIGHT_MASK: u16 = (1 << HEIGHT_BITS) - 1;

/// NB: Budget: 4 bytes. 40,000 tiles ⇒ 160 KB, which fits in L2.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Tile {
    packed: u16,
    occupant_origin: TileIndex,
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tile")
            .field("terrain", &self.terrain())
            .field("ground_height", &self.ground_height())
            .field("flags", &self.flags())
            .field("occupant_origin", &self.occupant_origin)
            .finish()
    }
}

impl Tile {
    /// The highest ground height the field can hold: five bits, `0..=31`.
    pub const MAX_GROUND_HEIGHT: u8 = HEIGHT_MASK as u8;

    /// The kind of ground on the tile.
    pub const fn terrain(&self) -> Terrain {
        match self.packed & TERRAIN_MASK {
            0 => Terrain::Plain,
            1 => Terrain::Water,
            2 => Terrain::Rock,
            // Provable invariant: `set_terrain` only ever writes
            // `Terrain::index()`'s range, 0..=2 — the other thirteen codes
            // the 4-bit field could hold are never produced.
            _ => unreachable!(),
        }
    }

    /// How high the ground is on this tile, counted in steps, `0..=31`.
    ///
    /// **The grid's own `height` is its size in tiles**, which is a different
    /// thing entirely — hence the longer name here. A step is a unit of
    /// gameplay, not of length: the renderer multiplies it by a scale of its
    /// own to get pixels.
    pub const fn ground_height(&self) -> u8 {
        ((self.packed >> HEIGHT_SHIFT) & HEIGHT_MASK) as u8
    }

    const fn flags(&self) -> TileFlags {
        TileFlags(((self.packed >> FLAGS_SHIFT) & FLAGS_MASK) as u8)
    }

    const fn set_flags(&mut self, f: TileFlags) {
        let bits = (f.0 as u16) & FLAGS_MASK;
        self.packed = (self.packed & !(FLAGS_MASK << FLAGS_SHIFT)) | (bits << FLAGS_SHIFT);
    }

    /// Whether a road runs over the tile.
    pub const fn has_road(&self) -> bool {
        self.flags().has_road()
    }

    /// The flag byte, which is what the state hash reads.
    pub const fn flag_bits(&self) -> u8 {
        self.flags().bits()
    }

    /// The tile's occupant, if there is one.
    pub const fn occupant(&self) -> Option<TileOccupant> {
        let f = self.flags();
        if f.has_occupant() {
            Some(TileOccupant {
                origin: self.occupant_origin,
                is_house: f.occupant_is_house(),
            })
        } else {
            None
        }
    }

    pub const fn is_free(&self) -> bool {
        let f = self.flags();
        !f.has_occupant() && !f.has_road()
    }

    /// Sets the kind of ground, which only scenario setup does.
    pub(crate) const fn set_terrain(&mut self, terrain: Terrain) {
        let bits = (terrain.index() as u16) & TERRAIN_MASK;
        self.packed = (self.packed & !TERRAIN_MASK) | bits;
    }

    /// Sets the ground height, which only scenario setup does — like
    /// `set_terrain`, gameplay never changes the ground under a placed
    /// building. The caller guarantees `height <= Tile::MAX_GROUND_HEIGHT`;
    /// a larger value is masked to its low five bits rather than checked
    /// here, the same trust `set_terrain` already places in its one caller.
    pub(crate) const fn set_ground_height(&mut self, height: u8) {
        let bits = (height as u16) & HEIGHT_MASK;
        self.packed = (self.packed & !(HEIGHT_MASK << HEIGHT_SHIFT)) | (bits << HEIGHT_SHIFT);
    }

    /// Puts a road on the tile or takes it away.
    pub(crate) const fn set_road(&mut self, on: bool) {
        let mut f = self.flags();
        f.set(TileFlags::HAS_ROAD, on);
        self.set_flags(f);
    }

    pub(crate) const fn set_occupant(&mut self, occ: TileOccupant) {
        self.occupant_origin = occ.origin;
        let mut f = self.flags();
        f.set(TileFlags::HAS_OCCUPANT, true);
        f.set(TileFlags::OCCUPANT_IS_HOUSE, occ.is_house);
        self.set_flags(f);
    }

    pub(crate) const fn clear_occupant(&mut self) {
        let mut f = self.flags();
        f.set(TileFlags::HAS_OCCUPANT, false);
        f.set(TileFlags::OCCUPANT_IS_HOUSE, false);
        self.set_flags(f);
        self.occupant_origin = TileIndex::new(0);
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GridError {
    #[error("invalid grid size: {width}x{height}, each side must be 1..={max}")]
    InvalidSize { width: u16, height: u16, max: u16 },
}

/// A dense grid of tiles, indexed `y * width + x`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grid {
    width: u16,
    height: u16,
    tiles: Box<[Tile]>,
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
        let mut tile = Tile::default();
        tile.set_terrain(terrain);
        Ok(Self {
            width,
            height,
            tiles: vec![tile; len].into_boxed_slice(),
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
    pub const fn index(&self, pos: TilePos) -> Option<TileIndex> {
        if !self.in_bounds(pos) {
            return None;
        }
        let i = pos.y as u32 * self.width as u32 + pos.x as u32;
        // Invariant: in_bounds implies i < width*height <= 65_536, so i fits in
        // a u16 (the last valid index is 65_535).
        Some(TileIndex::new(i as u16))
    }

    /// The position matching an index, `None` if off the map.
    pub const fn pos(&self, idx: TileIndex) -> Option<TilePos> {
        let i = idx.get() as u32;
        if i >= self.len() {
            return None;
        }
        let w = self.width as u32;
        Some(TilePos::new((i % w) as u8, (i / w) as u8))
    }

    pub fn get(&self, idx: TileIndex) -> Option<&Tile> {
        self.tiles.get(idx.as_usize())
    }

    pub(crate) fn get_mut(&mut self, idx: TileIndex) -> Option<&mut Tile> {
        self.tiles.get_mut(idx.as_usize())
    }

    pub fn at(&self, pos: TilePos) -> Option<&Tile> {
        self.get(self.index(pos)?)
    }

    pub(crate) fn at_mut(&mut self, pos: TilePos) -> Option<&mut Tile> {
        let idx = self.index(pos)?;
        self.get_mut(idx)
    }

    /// Every valid index, in increasing order. It is the scan order the rest of
    /// the core assumes: rebuilding the road network (phase 05) relies on it.
    pub fn indices(&self) -> impl Iterator<Item = TileIndex> {
        (0..self.len()).map(|i| TileIndex::new(i as u16))
    }

    /// The four orthogonal neighbours, **without wraparound**: the neighbour
    /// "to the right" of `x = width - 1` does not exist, it is not the first
    /// tile of the row below.
    ///
    /// Emission order: north, west, east, south — that is, increasing
    /// `TileIndex`.
    pub fn neighbors4(&self, idx: TileIndex) -> impl Iterator<Item = TileIndex> {
        let mut out = [None; 4];
        if let Some(p) = self.pos(idx) {
            let i = idx.get();
            if p.y > 0 {
                out[0] = Some(TileIndex::new(i - self.width));
            }
            if p.x > 0 {
                out[1] = Some(TileIndex::new(i - 1));
            }
            if u16::from(p.x) + 1 < self.width {
                out[2] = Some(TileIndex::new(i + 1));
            }
            if u16::from(p.y) + 1 < self.height {
                out[3] = Some(TileIndex::new(i + self.width));
            }
        }
        out.into_iter().flatten()
    }

    /// The slope over a set of tiles: the highest ground height among them
    /// minus the lowest. Zero for a single tile or a level region — there is
    /// nothing to flatten over one height.
    ///
    /// It is **never stored on the tile**.
    pub fn slope_over(&self, tiles: &[TileIndex]) -> u8 {
        // A stored slope would be a second copy of a fact the heights already
        // carry, and the two would drift apart the first time a height
        // changed without it — the reason a save file is `seed +
        // Vec<Command>` and not a dump of the state (D4), in miniature.
        let mut range: Option<(u8, u8)> = None;
        for &idx in tiles {
            // An index the grid cannot resolve is skipped, not reported — the
            // same convention the state hash's tile loop already follows.
            let Some(h) = self.get(idx).map(Tile::ground_height) else {
                continue;
            };
            range = Some(match range {
                None => (h, h),
                Some((lo, hi)) => (lo.min(h), hi.max(h)),
            });
        }
        range.map_or(0, |(lo, hi)| hi - lo)
    }

    /// The walkable tiles (`Terrain::is_walkable`), grouped into connected
    /// components under [`Grid::neighbors4`], one entry per component holding
    /// its tile count.
    ///
    /// A plain flood fill, not `RoadNetwork`'s generation-tagged `Visited`:
    /// that structure is tuned for being rebuilt on every road change, on the
    /// hot path. This runs once, when a map is loaded, and simplicity is the
    /// only thing worth optimising for here. The map loader is its one
    /// caller: until bridges exist, a river severing the walkable ground is
    /// refused at load rather than discovered later as an unreachable
    /// district.
    pub fn walkable_regions(&self) -> Vec<usize> {
        let mut seen = vec![false; self.len() as usize];
        let mut sizes = Vec::new();
        let walkable = |g: &Grid, i: TileIndex| g.get(i).is_some_and(|t| t.terrain().is_walkable());

        for root in self.indices() {
            if seen[root.as_usize()] || !walkable(self, root) {
                continue;
            }
            let mut count = 0usize;
            let mut queue = vec![root];
            seen[root.as_usize()] = true;
            while let Some(t) = queue.pop() {
                count += 1;
                for v in self.neighbors4(t) {
                    if !seen[v.as_usize()] && walkable(self, v) {
                        seen[v.as_usize()] = true;
                        queue.push(v);
                    }
                }
            }
            sizes.push(count);
        }
        sizes
    }

    /// Builds a grid from an already-validated map. Infallible: `MapDef`'s
    /// own invariants — dimensions in range, row lengths matching, heights in
    /// range, one connected walkable region — are `sim_data::validate_map`'s
    /// job, not this constructor's.
    pub fn from_map(def: &crate::map::MapDef) -> Self {
        let tiles: Vec<Tile> = def
            .terrain
            .iter()
            .zip(&def.ground)
            .map(|(&terrain, &height)| {
                let mut t = Tile::default();
                t.set_terrain(terrain);
                t.set_ground_height(height);
                t
            })
            .collect();
        Self {
            width: def.width,
            height: def.height,
            tiles: tiles.into_boxed_slice(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The whole truth table, written out. It lives here because this is where
    /// the answers live: a match arm edited by hand should have to change a
    /// test, and before these facts left the tables the only thing pinning them
    /// was a `.ron` file nobody compiled.
    #[test]
    fn what_each_terrain_allows() {
        assert!(Terrain::Plain.is_buildable());
        assert!(Terrain::Plain.is_walkable());

        assert!(!Terrain::Water.is_buildable());
        assert!(!Terrain::Water.is_walkable());

        // The asymmetric one: crossed, never settled on.
        assert!(!Terrain::Rock.is_buildable());
        assert!(Terrain::Rock.is_walkable());
    }

    /// `index` gives back the position the variant is declared at.
    ///
    /// Two statements of one order — the declaration and the numbers in
    /// `index` — and the hashes depend on them agreeing. Reordering the enum
    /// without reordering the numbers would file a terrain's road cost under
    /// another terrain's name, and nothing else would say so.
    #[test]
    fn index_is_the_position_in_the_declaration() {
        for (i, t) in Terrain::ALL.into_iter().enumerate() {
            assert_eq!(t.index(), i, "{t:?} is declared at {i}");
        }
    }

    /// Anything a building can stand on is something a road can reach, and the
    /// placement rule leans on it: a building with no entrance is off the
    /// network, so buildable ground that no road could touch would be a trap.
    #[test]
    fn every_buildable_terrain_is_walkable() {
        for t in Terrain::ALL {
            assert!(
                !t.is_buildable() || t.is_walkable(),
                "{t:?} can be built on but no road can reach it"
            );
        }
    }

    #[test]
    fn manhattan_is_symmetric_and_does_not_wrap() {
        let a = TilePos::new(0, 0);
        let b = TilePos::new(255, 255);
        assert_eq!(a.manhattan(b), 510);
        assert_eq!(b.manhattan(a), 510);
        assert_eq!(a.manhattan(a), 0);
    }

    #[test]
    fn tile_stays_within_budget() {
        assert_eq!(
            size_of::<Tile>(),
            4,
            "Tile has grown: 40,000 tiles have to fit in cache"
        );
    }

    /// The packing round-trips over the whole field, not just the terrain
    /// codes actually in use: `Terrain::ALL` only has three members, but the
    /// 4-bit field the packing gives it has sixteen, and a shift/mask bug in
    /// an unused code would sit undetected until a fourth terrain used it.
    #[test]
    fn the_packing_round_trips() {
        for terrain_bits in 0u16..16 {
            for height in 0u8..=Tile::MAX_GROUND_HEIGHT {
                for flags in 0u16..16 {
                    let packed = (terrain_bits & TERRAIN_MASK)
                        | ((flags & FLAGS_MASK) << FLAGS_SHIFT)
                        | ((u16::from(height) & HEIGHT_MASK) << HEIGHT_SHIFT);
                    let t = Tile {
                        packed,
                        occupant_origin: TileIndex::new(u16::MAX),
                    };
                    assert_eq!(t.packed & TERRAIN_MASK, terrain_bits);
                    assert_eq!(t.ground_height(), height);
                    assert_eq!(u16::from(t.flag_bits()), flags);
                    assert_eq!(t.occupant_origin, TileIndex::new(u16::MAX));
                }
            }
        }
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
        let last = g.index(TilePos::new(255, 255)).expect("in bounds");
        assert_eq!(last.get(), u16::MAX);
        assert_eq!(g.pos(last), Some(TilePos::new(255, 255)));
    }

    /// Even the last tile of a 256x256 map (`TileIndex` = u16::MAX) can be the
    /// origin of an occupant: that is why presence lives in a flag rather than
    /// in a sentinel value.
    #[test]
    fn the_occupant_is_identified_by_its_origin_tile() {
        let mut t = Tile::default();
        assert_eq!(t.occupant(), None);
        assert!(t.is_free());

        let occ = TileOccupant {
            origin: TileIndex::new(u16::MAX),
            is_house: true,
        };
        t.set_occupant(occ);
        assert_eq!(t.occupant(), Some(occ));
        assert!(!t.is_free());

        t.clear_occupant();
        assert_eq!(t.occupant(), None);
        assert_eq!(t, Tile::default(), "clearing also resets the origin");
    }

    /// The flags go into the state hash as one byte per tile, so moving one of
    /// these numbers would change every hash with nothing about it looking like
    /// an error — the constants are private, so the check goes through the
    /// accessors that set them.
    #[test]
    fn the_bit_numbers_are_frozen() {
        let mut t = Tile::default();
        assert_eq!(t.flag_bits(), 0);

        t.set_road(true);
        assert_eq!(t.flag_bits(), 0b001);
        t.set_road(false);

        let origin = TileIndex::new(0);
        t.set_occupant(TileOccupant {
            origin,
            is_house: false,
        });
        assert_eq!(t.flag_bits(), 0b010);

        t.set_occupant(TileOccupant {
            origin,
            is_house: true,
        });
        assert_eq!(t.flag_bits(), 0b110);
    }

    /// The type enforces no rule about which flags may be set together — that
    /// is `tick`'s job — but one bit must never disturb another.
    #[test]
    fn the_flags_are_independent() {
        let mut t = Tile::default();
        t.set_road(true);
        t.set_occupant(TileOccupant {
            origin: TileIndex::new(7),
            is_house: true,
        });
        assert!(t.has_road(), "taking the tile left the road alone");

        t.clear_occupant();
        assert!(t.has_road(), "clearing the occupant left the road alone");
        assert_eq!(t.occupant(), None);

        t.set_road(false);
        assert_eq!(t, Tile::default());
    }

    #[test]
    fn no_wraparound_at_the_edges() {
        let g = Grid::new(4, 4, Terrain::Plain).expect("valid grid");
        // The tile to the right of the first row is not a neighbour of the
        // first tile of the second row.
        let right = g.index(TilePos::new(3, 0)).expect("in bounds");
        let left_of_next_row = g.index(TilePos::new(0, 1)).expect("in bounds");
        let neighbors: Vec<_> = g.neighbors4(right).collect();
        assert!(!neighbors.contains(&left_of_next_row));
    }

    #[test]
    fn neighbor_count_by_position() {
        let g = Grid::new(5, 4, Terrain::Plain).expect("valid grid");
        let n = |x, y| {
            g.neighbors4(g.index(TilePos::new(x, y)).expect("in bounds"))
                .count()
        };
        assert_eq!(n(0, 0), 2, "corner");
        assert_eq!(n(4, 3), 2, "opposite corner");
        assert_eq!(n(2, 0), 3, "edge");
        assert_eq!(n(2, 2), 4, "interior");
    }

    #[test]
    fn slope_over_a_single_tile_is_always_zero() {
        let mut g = Grid::new(3, 3, Terrain::Plain).expect("valid grid");
        let idx = g.index(TilePos::new(1, 1)).expect("in bounds");
        g.get_mut(idx).expect("tile").set_ground_height(17);
        assert_eq!(g.slope_over(&[idx]), 0);
    }

    #[test]
    fn slope_over_a_level_region_is_zero() {
        let mut g = Grid::new(3, 3, Terrain::Plain).expect("valid grid");
        let tiles: Vec<TileIndex> = g.indices().collect();
        for &idx in &tiles {
            g.get_mut(idx).expect("tile").set_ground_height(5);
        }
        assert_eq!(g.slope_over(&tiles), 0);
    }

    #[test]
    fn slope_over_is_the_highest_minus_the_lowest() {
        let mut g = Grid::new(2, 2, Terrain::Plain).expect("valid grid");
        let heights = [2u8, 7, 3, 9];
        let tiles: Vec<TileIndex> = g.indices().collect();
        for (&idx, &h) in tiles.iter().zip(&heights) {
            g.get_mut(idx).expect("tile").set_ground_height(h);
        }
        assert_eq!(g.slope_over(&tiles), 9 - 2);
    }

    #[test]
    fn walkable_regions_of_a_uniform_grid_is_one() {
        let g = Grid::new(4, 4, Terrain::Plain).expect("valid grid");
        assert_eq!(g.walkable_regions(), vec![16]);
    }

    /// A strip of water down the middle of a 1-tall row severs it into two
    /// regions, the shape a river drawn across a map takes.
    #[test]
    fn a_water_strip_severs_the_walkable_region() {
        let mut g = Grid::new(5, 1, Terrain::Plain).expect("valid grid");
        let water = g.index(TilePos::new(2, 0)).expect("in bounds");
        g.get_mut(water).expect("tile").set_terrain(Terrain::Water);

        let mut regions = g.walkable_regions();
        regions.sort_unstable();
        assert_eq!(regions, vec![2, 2]);
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
        fn pos_to_index_round_trips((g, p) in grid_and_pos()) {
            let i = g.index(p).expect("generated pos is in bounds");
            prop_assert_eq!(g.pos(i), Some(p));
        }

        /// And the other way round: index -> position -> index.
        #[test]
        fn index_to_pos_round_trips((g, p) in grid_and_pos()) {
            let i = g.index(p).expect("generated pos is in bounds");
            let p2 = g.pos(i).expect("valid index");
            prop_assert_eq!(g.index(p2), Some(i));
        }

        /// Every index the grid hands out resolves to a tile, and one past the
        /// end does not.
        ///
        /// The tile count is stated twice — once by `len`, as `width * height`,
        /// and once by the slice `get` reads — and this is the property that
        /// says they agree. Failing it is unreachable today, because the slice
        /// is sized from `len` and nothing can resize it afterwards, and the
        /// property is written down anyway: a `get` answering `None` here would
        /// be swallowed in silence by the state hash, which skips an index it
        /// cannot resolve rather than reporting one.
        #[test]
        fn every_index_the_grid_hands_out_resolves((g, _p) in grid_and_pos()) {
            prop_assert!(g.indices().all(|i| g.get(i).is_some()));
            // On the largest map the next index wraps to 0, which is a real
            // tile; anywhere else it is off the end.
            prop_assert_eq!(
                g.get(TileIndex::new(g.len() as u16)).is_some(),
                g.len() == 65_536
            );
        }

        /// Outside the edges there is no index.
        #[test]
        fn outside_the_edge_there_is_no_index((g, _p) in grid_and_pos()) {
            if g.width() < MAX_SIDE {
                let outside = TilePos::new(g.width() as u8, 0);
                prop_assert_eq!(g.index(outside), None);
                prop_assert!(!g.in_bounds(outside));
            }
            prop_assert_eq!(g.pos(TileIndex::new(u16::MAX)).is_some(), g.len() == 65_536);
        }

        /// Every neighbour is on the map and at Manhattan distance 1; how many
        /// there are depends only on how many edges the tile touches.
        #[test]
        fn neighbors4_stays_on_the_map_and_does_not_wrap((g, p) in grid_and_pos()) {
            let i = g.index(p).expect("generated pos is in bounds");
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

            // Neighbours come out in increasing TileIndex order (BFS contract).
            let mut sorted = neighbors.clone();
            sorted.sort_unstable();
            prop_assert_eq!(neighbors, sorted);
        }

        /// Being neighbours is a symmetric relation.
        #[test]
        fn neighbors4_is_symmetric((g, p) in grid_and_pos()) {
            let i = g.index(p).expect("generated pos is in bounds");
            for v in g.neighbors4(i) {
                prop_assert!(g.neighbors4(v).any(|w| w == i));
            }
        }
    }
}
