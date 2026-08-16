//! Replays a recording.

use std::sync::Arc;

use sim_core::{DataSet, Grid, World};

use crate::hash::hash_world;
use crate::recording::Recording;

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error(
        "the dataset does not match the one the game was recorded with:\n  \
         expected {expected}\n  found    {found}\n\
         if the balance change was intended, regenerate the recordings with \
         `cargo xtask regen-expected`"
    )]
    DatasetMismatch { expected: String, found: String },

    #[error("unsupported format version: {found}, this build reads {expected}")]
    UnsupportedFormat { found: u16, expected: u16 },

    /// The header's profile does not exist in the table.
    ///
    /// A distinct error from [`ReplayError::DatasetMismatch`] because the cause
    /// is different: there the balancing has changed, here a profile has been
    /// removed or renamed.
    #[error("unknown difficulty profile: {found:?} (known: {known})")]
    UnknownDifficulty { found: String, known: String },

    #[error("invalid grid in the header: {0}")]
    InvalidGrid(#[from] sim_core::GridError),
}

/// A checkpoint: the tick and the state hash at that tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub tick: u32,
    pub hash: [u8; 32],
}

/// Creates the initial world described by the header, checking that the dataset
/// is the right one.
pub fn initial_world(rec: &Recording, data: Arc<DataSet>) -> Result<World, ReplayError> {
    if rec.header.format_version != crate::recording::FORMAT_VERSION {
        return Err(ReplayError::UnsupportedFormat {
            found: rec.header.format_version,
            expected: crate::recording::FORMAT_VERSION,
        });
    }
    let found = data.hash_hex();
    if rec.header.dataset_hash != found {
        return Err(ReplayError::DatasetMismatch {
            expected: rec.header.dataset_hash.clone(),
            found,
        });
    }
    // The header names the profile, so a save always replays on the difficulty
    // it was played on. There is no fallback: a default here is how a recording
    // would silently change game halfway through the project.
    let difficulty = data
        .difficulty_by_id(&rec.header.difficulty)
        .ok_or_else(|| ReplayError::UnknownDifficulty {
            found: rec.header.difficulty.clone(),
            known: data.difficulty_ids(),
        })?;

    let g = &rec.header.grid;
    let grid = Grid::new(g.width, g.height, g.terrain)?;
    Ok(World::new(grid, data, rec.header.seed, difficulty))
}

/// Replays up to tick `until` (exclusive: after the call `world.tick()` equals
/// `until`).
pub fn replay(rec: &Recording, data: Arc<DataSet>, until: u32) -> Result<World, ReplayError> {
    let mut w = initial_world(rec, data)?;
    advance(&mut w, rec, until);
    Ok(w)
}

/// Carries an already started world forward to tick `until`.
///
/// Kept separate from [`replay`] because it is what makes the determinism of
/// **partial** execution checkable: stopping halfway and picking back up has to
/// give the same state as one single run.
pub fn advance(world: &mut World, rec: &Recording, until: u32) {
    while world.tick() < until {
        let cmds = rec.commands_at_tick(world.tick());
        sim_core::step(world, &cmds);
    }
}

/// Replays, computing a hash every `every` ticks.
///
/// There is no checkpoint at tick 0: that is the initial state, which the
/// header already describes in full.
pub fn checkpoints(
    rec: &Recording,
    data: Arc<DataSet>,
    until: u32,
    every: u32,
) -> Result<Vec<Checkpoint>, ReplayError> {
    let mut w = initial_world(rec, data)?;
    let mut out = Vec::new();
    let stride = every.max(1);
    while w.tick() < until {
        let next = (w.tick() / stride + 1) * stride;
        advance(&mut w, rec, next.min(until));
        if w.tick() % stride == 0 || w.tick() == until {
            out.push(Checkpoint {
                tick: w.tick(),
                hash: hash_world(&w),
            });
        }
    }
    Ok(out)
}
