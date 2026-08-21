//! A map: the ground truth [`crate::grid::Grid::from_map`] builds a grid from.
//!
//! Loaded and validated by `sim-data` (`RawMap`, `validate_map`) — the same
//! division of labour as the four balancing tables: I/O and validation stay
//! out of the core, and `MapDef` is the shape the core consumes. It lives here
//! and not in `sim-data` because `sim-core` cannot depend on `sim-data` (see
//! the crate's own module doc), and `Grid::from_map` is a method on this
//! crate's `Grid`.

use crate::grid::Terrain;

/// Hash prefix: keeps this hash apart from the dataset's and the state's.
/// Changing it moves every recording that names a map by id and hash.
const PREFIX: &[u8] = b"brando/map/v1";

/// An already-validated map.
///
/// `sim_data::validate_map` is the one path that guarantees `terrain.len() ==
/// ground.len() == width as usize * height as usize`, that every `ground`
/// entry is at most [`crate::grid::Tile::MAX_GROUND_HEIGHT`], and that the
/// walkable tiles form one connected region — [`crate::grid::Grid::from_map`]
/// trusts all three without re-checking them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapDef {
    pub id: String,
    pub width: u16,
    pub height: u16,
    /// Row-major, `y * width + x`.
    pub terrain: Vec<Terrain>,
    /// Row-major, same order and length as `terrain`.
    pub ground: Vec<u8>,
    /// blake3 of the fields above, computed by [`MapDef::new`] so it can
    /// never fall out of step with them — the same discipline `DataSet::new`
    /// holds for the balancing tables. Feeds `MapSpec::File` at recording
    /// time and is checked again at replay time.
    pub hash: [u8; 32],
}

impl MapDef {
    pub fn new(
        id: String,
        width: u16,
        height: u16,
        terrain: Vec<Terrain>,
        ground: Vec<u8>,
    ) -> Self {
        let hash = map_hash(&id, width, height, &terrain, &ground);
        Self {
            id,
            width,
            height,
            terrain,
            ground,
            hash,
        }
    }

    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }
}

fn map_hash(id: &str, width: u16, height: u16, terrain: &[Terrain], ground: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(PREFIX);
    h.update(&(id.len() as u64).to_le_bytes());
    h.update(id.as_bytes());
    h.update(&width.to_le_bytes());
    h.update(&height.to_le_bytes());
    h.update(&(terrain.len() as u64).to_le_bytes());
    for (t, g) in terrain.iter().zip(ground) {
        h.update(&[t.index() as u8, *g]);
    }
    *h.finalize().as_bytes()
}
