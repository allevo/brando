#![forbid(unsafe_code)]

//! Salvataggio come `seed + Vec<Command>` e hashing canonico dello stato (D4).
//!
//! Un salvataggio non e' un dump dello stato: e' il seed piu' il log dei
//! comandi, rigiocato dal core. Qui vivono anche i golden replay, che sono il
//! test piu' prezioso del progetto — da qui in avanti ogni fonte di
//! non-determinismo introdotta per distrazione si manifesta come un hash che
//! cambia, entro un commit da quando e' stata introdotta.

pub mod golden;
pub mod hash;
pub mod recording;
pub mod replay;

pub use golden::GoldenError;
pub use hash::{hash_hex, hash_world};
pub use recording::{FORMAT_VERSION, GridSpec, Header, Recording, RecordingError};
pub use replay::{Checkpoint, ReplayError, avanza, checkpoints, mondo_iniziale, replay};

/// Ogni quanti tick si prende un checkpoint nei golden committati.
pub const CHECKPOINT_OGNI: u32 = 30;

/// Directory dei golden, risolta a compile time.
pub fn dir_golden() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}
