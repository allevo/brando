//! Newtype identifiers. A bare `usize` never appears in a public signature
//! (CLAUDE.md, conventions).

use std::fmt;

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

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// One rung of a per-level table: `rules.house_levels` for the house ladder,
/// `range_per_level` and `capacity_per_level` for a provider's.
///
/// **It stores the index and shows the number.** A level counts from 1 in the
/// tables, in the messages and in the state hash, and from 0 in every `Vec` that
/// holds one; before this type the `±1` between the two was written out by hand
/// at seven sites across three crates, in three different spellings
/// (`checked_sub`, `saturating_sub`, `index + 1`). [`Level::as_usize`] is total
/// precisely because the subtraction happens here, once.
///
/// One type for both ladders, and not a `HouseLevel` beside a `BuildingLevel`:
/// they are the same shape, the checks in [`DataSet::inconsistencies`] report on
/// both, and a provider's ladder stays one rung long for the whole of M1 (D6 —
/// the second type is invented until something needs to tell them apart).
///
/// Not `Serialize`, like [`DifficultyId`]: what travels in a save file is
/// `seed + Vec<Command>` (D4), never a level. Not `Default` either — the four
/// places that mean the bottom rung say [`Level::FIRST`].
///
/// [`DataSet::inconsistencies`]: crate::data::DataSet::inconsistencies
/// [`DifficultyId`]: crate::data::DifficultyId
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(u8);

impl Level {
    /// The bottom rung. Every house and every building is born here.
    pub const FIRST: Self = Self(0);

    /// The last index a `Level` may hold, so that [`Level::get`] cannot
    /// overflow. A table of 255 rungs is not a case this game has: the bound is
    /// what a total function owes, not a limit anybody will meet.
    const LAST_INDEX: u8 = u8::MAX - 1;

    /// The rung with this number, counting from 1. `None` for zero: there is no
    /// level 0, and this is the one place that has to say so.
    pub const fn new(number: u8) -> Option<Self> {
        match number.checked_sub(1) {
            Some(index) => Some(Self(index)),
            None => None,
        }
    }

    /// The rung at this position in a per-level table (index 0 = level 1).
    pub const fn from_index(index: usize) -> Self {
        if index > Self::LAST_INDEX as usize {
            Self(Self::LAST_INDEX)
        } else {
            Self(index as u8)
        }
    }

    /// The number the tables, the messages and the state hash use.
    pub const fn get(self) -> u8 {
        self.0 + 1
    }

    /// Its position in a per-level table.
    ///
    /// `pub`, where `TileIdx::as_usize` and `BuildingKindId::as_usize` are
    /// `pub(crate)`: `sim-data` builds the RON path of a rung
    /// (`rules.house_levels[i]`) out of it, and it is the index, not the number,
    /// that names the entry somebody has to go and fix.
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// The rung above, `None` at the ceiling of the type.
    pub const fn next(self) -> Option<Self> {
        if self.0 < Self::LAST_INDEX {
            Some(Self(self.0 + 1))
        } else {
            None
        }
    }

    /// The rung below, `None` at [`Level::FIRST`] — there is no level 0.
    pub const fn previous(self) -> Option<Self> {
        match self.0.checked_sub(1) {
            Some(index) => Some(Self(index)),
            None => None,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

/// Written by hand, and it earns its lines: the derived one would print the
/// index, so a failing `assert_eq!` on `Event::HouseEvolved` would report a
/// house rising from `Level(0)` to `Level(1)` and cost somebody an afternoon.
impl fmt::Debug for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Level({})", self.get())
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

    /// The one thing this type exists for: the two conversions are each
    /// other's inverse, and neither can be got wrong at a call site.
    #[test]
    fn the_number_and_the_index_agree() {
        assert_eq!(Level::FIRST.get(), 1);
        assert_eq!(Level::FIRST.as_usize(), 0);
        for index in 0..8usize {
            let level = Level::from_index(index);
            assert_eq!(level.as_usize(), index);
            assert_eq!(Level::new(level.get()), Some(level));
        }
    }

    #[test]
    fn the_ladder_has_two_ends() {
        assert_eq!(Level::new(0), None, "there is no level 0");
        assert_eq!(Level::FIRST.previous(), None);
        assert_eq!(Level::FIRST.next(), Level::new(2));
        // The clamp is what keeps `get` from overflowing: an index past the end
        // of the type saturates instead of wrapping to level 1.
        let last = Level::from_index(usize::MAX);
        assert_eq!(last.get(), u8::MAX);
        assert_eq!(last.next(), None);
    }

    #[test]
    fn it_prints_the_number_not_the_index() {
        let two = Level::new(2).expect("levels count from 1");
        assert_eq!(two.to_string(), "2");
        assert_eq!(format!("{two:?}"), "Level(2)");
    }

    #[test]
    fn manhattan_is_symmetric_and_does_not_wrap() {
        let a = TilePos::new(0, 0);
        let b = TilePos::new(255, 255);
        assert_eq!(a.manhattan(b), 510);
        assert_eq!(b.manhattan(a), 510);
        assert_eq!(a.manhattan(a), 0);
    }
}
