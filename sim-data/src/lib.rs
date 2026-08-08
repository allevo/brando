#![forbid(unsafe_code)]

//! Tabelle di bilanciamento in RON, caricate e validate all'avvio.
//!
//! Tutto cio' che e' numerico nel gioco vive qui e non nel codice
//! (CLAUDE.md, convenzioni). Il caricamento e' I/O e per questo sta qui,
//! mai dentro `sim-core` (D4).

pub mod load;
pub mod raw;
pub mod validate;

pub use load::{LoadError, from_ron_str, load_from_dir};
/// Le definizioni del dataset vivono in `sim-core` (il `World` le tiene in
/// un `Arc`, A2); qui restano parsing, validazione e I/O. Ri-esportate per
/// comodita' di chi carica le tabelle.
pub use sim_core::data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef};
pub use validate::{ValidationError, ValidationErrorKind, ValidationReport, validate};

/// Directory delle tabelle di produzione, risolta a compile time.
///
/// Evita che ogni chiamante debba sapere dove sta `sim-data/data`. Resta un
/// percorso su disco e non un `include_str!`: le tabelle si modificano senza
/// ricompilare, che e' il punto di averle in RON.
pub fn dir_dati_di_produzione() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// Carica le tabelle di produzione.
pub fn load_default() -> Result<DataSet, LoadError> {
    load_from_dir(&dir_dati_di_produzione())
}
