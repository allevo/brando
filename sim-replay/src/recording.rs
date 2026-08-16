//! The save file: `seed + Vec<Command>`, not a dump of the state (D4).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sim_core::{Command, Terrain};

/// Version of the format. It goes up when the shape of the file changes, not
/// when the balancing does: that is what `dataset_hash` is for.
///
/// 2 since phase 11: the header carries the difficulty profile.
pub const FORMAT_VERSION: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSpec {
    pub width: u16,
    pub height: u16,
    pub terrain: Terrain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub format_version: u16,
    pub seed: u64,
    /// The difficulty profile's **textual** id, not its index.
    ///
    /// A recording saying `difficulty: 1` cannot be read, and reordering the
    /// table would silently change the meaning of every save file already
    /// written. The cost is one lookup on opening; the return is that the file
    /// stays what a recording has to be, which is readable.
    pub difficulty: String,
    pub grid: GridSpec,
    /// blake3 of the `DataSet`, in hexadecimal so it stays readable in the file.
    ///
    /// If the balancing changes, the replay fails immediately and for the right
    /// reason instead of diverging ten ticks later through a side effect.
    pub dataset_hash: String,
}

/// A recorded game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recording {
    pub header: Header,
    /// Sorted by increasing tick. Several commands within the same tick keep
    /// the order they were added in: that order is part of the determinism
    /// contract, not a detail of the file.
    pub commands: Vec<(u32, Command)>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    #[error("cannot read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not valid RON: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("cannot serialise the recording: {0}")]
    Serialize(#[from] ron::Error),
}

impl Recording {
    pub fn load(path: &Path) -> Result<Self, RecordingError> {
        let text = std::fs::read_to_string(path).map_err(|source| RecordingError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        ron::from_str(&text).map_err(|source| RecordingError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Writes the file. The format is deliberately verbose and indented: a
    /// recording nobody can read does not help anyone work out why it changed.
    pub fn save(&self, path: &Path) -> Result<(), RecordingError> {
        let text = self.to_ron()?;
        std::fs::write(path, text).map_err(|source| RecordingError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn to_ron(&self) -> Result<String, RecordingError> {
        let cfg = ron::ser::PrettyConfig::new()
            .indentor("  ")
            .struct_names(true);
        Ok(format!("{}\n", ron::ser::to_string_pretty(self, cfg)?))
    }

    /// The last tick that holds a command.
    pub fn last_tick(&self) -> u32 {
        self.commands.iter().map(|(t, _)| *t).max().unwrap_or(0)
    }

    /// The commands of one tick, in the order they were added.
    pub fn commands_at_tick(&self, tick: u32) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| *c)
            .collect()
    }
}
