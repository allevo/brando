#![forbid(unsafe_code)]

//! The balancing tables in RON, loaded and validated at startup.
//!
//! Everything numeric in the game — costs, ranges, requirements — lives in these
//! tables and not in the code (D6). Loading is I/O and that is why it lives here,
//! never inside `sim-core` (D4).

pub mod load;
pub mod raw;
pub mod validate;

pub use load::{LoadError, from_ron_str, load_from_dir};
/// The dataset definitions live in `sim-core` (the `World` holds them in an
/// `Arc`); what stays here is parsing, validation and I/O. Re-exported for
/// the convenience of whoever loads the tables.
pub use sim_core::data::{BuildingDef, DataSet, DifficultyDef, DifficultyId, Rules, ServiceDef};
pub use validate::{ValidationError, ValidationErrorKind, ValidationReport, validate};

/// The directory of the production tables, resolved at compile time.
///
/// It saves every caller from having to know where `sim-data/data` is. It stays
/// a path on disk and not an `include_str!`: the tables can be edited without
/// recompiling, which is the whole point of having them in RON.
pub fn production_data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// Loads the production tables.
pub fn load_default() -> Result<DataSet, LoadError> {
    load_from_dir(&production_data_dir())
}
