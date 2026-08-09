//! Loading the tables from disk.
//!
//! It is the only place in the project that does I/O on game data: `sim-core`
//! does none (D4).

use std::path::{Path, PathBuf};

use crate::raw::{RawBuildingTable, RawDataSet, RawRules, RawTerrainTable};
use crate::validate::{ValidationReport, validate};
use sim_core::data::DataSet;

const FILE_RULES: &str = "rules.ron";
const FILE_TERRAIN: &str = "terrain.ron";
const FILE_BUILDINGS: &str = "buildings.ron";

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{file} is not valid RON: {source}")]
    Ron {
        file: &'static str,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("{0}")]
    Validation(#[from] ValidationReport),
}

/// Loads the three tables from a directory.
pub fn load_from_dir(dir: &Path) -> Result<DataSet, LoadError> {
    let read = |name: &str| -> Result<String, LoadError> {
        let path = dir.join(name);
        std::fs::read_to_string(&path).map_err(|source| LoadError::Io { path, source })
    };
    let rules = read(FILE_RULES)?;
    let terrain = read(FILE_TERRAIN)?;
    let buildings = read(FILE_BUILDINGS)?;
    from_ron_str(&rules, &terrain, &buildings)
}

/// Like [`load_from_dir`], but starting from content already read.
///
/// It serves the tests (which put together valid and broken tables without
/// duplicating files on disk) and whoever will want to embed the tables in the
/// binary.
pub fn from_ron_str(rules: &str, terrain: &str, buildings: &str) -> Result<DataSet, LoadError> {
    let raw = RawDataSet {
        rules: parse::<RawRules>(FILE_RULES, rules)?,
        terrain: parse::<RawTerrainTable>(FILE_TERRAIN, terrain)?,
        buildings: parse::<RawBuildingTable>(FILE_BUILDINGS, buildings)?,
    };
    Ok(validate(&raw)?)
}

fn parse<T: serde::de::DeserializeOwned>(file: &'static str, s: &str) -> Result<T, LoadError> {
    ron::from_str(s).map_err(|source| LoadError::Ron { file, source })
}
