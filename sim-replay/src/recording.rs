//! Il salvataggio: `seed + Vec<Command>`, non un dump dello stato (D4).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sim_core::{Command, Terrain};

/// Versione del formato. Si incrementa quando cambia la forma del file, non
/// quando cambia il bilanciamento: per quello c'e' `dataset_hash`.
pub const FORMAT_VERSION: u16 = 1;

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
    pub grid: GridSpec,
    /// blake3 del `DataSet`, in esadecimale per restare leggibile nel file.
    ///
    /// Se il bilanciamento cambia, il replay fallisce subito e con il motivo
    /// giusto invece di divergere dieci tick dopo per un effetto secondario
    /// (A2).
    pub dataset_hash: String,
}

/// Una partita registrata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recording {
    pub header: Header,
    /// Ordinato per tick crescente. Piu' comandi nello stesso tick mantengono
    /// l'ordine di inserimento: quell'ordine e' parte del contratto di
    /// determinismo, non un dettaglio del file.
    pub commands: Vec<(u32, Command)>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    #[error("impossibile leggere o scrivere {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} non e' un RON valido: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("impossibile serializzare la registrazione: {0}")]
    Serialize(#[from] ron::Error),
}

impl Recording {
    pub fn load(path: &Path) -> Result<Self, RecordingError> {
        let testo = std::fs::read_to_string(path).map_err(|source| RecordingError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        ron::from_str(&testo).map_err(|source| RecordingError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Scrive il file. Il formato e' volutamente prolisso e indentato: un
    /// golden che nessuno riesce a leggere non aiuta a capire perche' e'
    /// cambiato.
    pub fn save(&self, path: &Path) -> Result<(), RecordingError> {
        let testo = self.to_ron()?;
        std::fs::write(path, testo).map_err(|source| RecordingError::Io {
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

    /// L'ultimo tick in cui c'e' un comando.
    pub fn ultimo_tick(&self) -> u32 {
        self.commands.iter().map(|(t, _)| *t).max().unwrap_or(0)
    }

    /// I comandi di un tick, nell'ordine di inserimento.
    pub fn comandi_al_tick(&self, tick: u32) -> Vec<Command> {
        self.commands
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| *c)
            .collect()
    }
}
