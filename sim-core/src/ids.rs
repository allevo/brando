//! Newtype identifiers. A bare `usize` never appears in a public signature
//! (CLAUDE.md, conventions).

use serde::{Deserialize, Serialize};

slotmap::new_key_type! {
    /// Id of a building in the `World`'s `SlotMap`.
    pub struct BuildingId;
    /// Id of a house in the `World`'s `SlotMap`.
    pub struct HouseId;
}

/// Linear tile index: `y * width + x`.
///
/// The map is at most 256x256 (A4), so 65,536 tiles: the last index is
/// `u16::MAX` and fits exactly.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TileIdx(u16);

impl TileIdx {
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

/// A kind of building, as an index into the `DataSet`'s `buildings` table.
///
/// Not an enum: building kinds are data, not code (D6). An id that resolves to
/// no row of the table is a command error, not a panic — the LLM will produce
/// some (CLAUDE.md, AI interface).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct BuildingKindId(u16);

impl BuildingKindId {
    pub const fn new(v: u16) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// A position on the grid. `x` and `y` are `u8` because the maximum side is
/// 256 (A4): valid coordinates run from 0 to 255.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_is_symmetric_and_does_not_wrap() {
        let a = TilePos::new(0, 0);
        let b = TilePos::new(255, 255);
        assert_eq!(a.manhattan(b), 510);
        assert_eq!(b.manhattan(a), 510);
        assert_eq!(a.manhattan(a), 0);
    }
}
