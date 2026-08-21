#![forbid(unsafe_code)]

//! Saving as `seed + Vec<Command>`, and hashing the state (D4).
//!
//! A save file is not a dump of the state: it is the seed plus the command log,
//! replayed by the core. The recorded replays live here too, and they are the
//! project's most valuable test — from here on, every source of
//! non-determinism introduced by accident shows up as a hash that changes,
//! within one commit of being introduced.

pub mod expected;
pub mod hash;
pub mod recording;
pub mod replay;

pub use expected::ExpectedError;
pub use hash::{hash_hex, hash_world};
pub use recording::{FORMAT_VERSION, Header, MapSpec, Recording, RecordingError};
pub use replay::{Checkpoint, ReplayError, advance, checkpoints, initial_world, replay};

/// How many ticks apart the checkpoints are in the committed recordings.
pub const CHECKPOINT_EVERY: u32 = 30;

/// The directory of the recorded replays, resolved at compile time.
pub fn expected_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected")
}
