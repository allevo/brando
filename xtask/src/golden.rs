//! Registrazione e rigenerazione dei golden replay.

use std::path::Path;
use std::sync::Arc;

use sim_core::DataSet;
use sim_replay::{CHECKPOINT_OGNI, GridSpec, Header, Recording, checkpoints};

use crate::scenario::{self, Scenario};

/// Quanti tick copre ogni golden: un anno di gioco (A6).
pub fn tick_del_golden(data: &DataSet) -> u32 {
    data.rules.tick_per_anno()
}

pub fn registra(sc: &Scenario, data: &DataSet) -> Recording {
    Recording {
        header: Header {
            format_version: sim_replay::FORMAT_VERSION,
            seed: sc.seed,
            grid: GridSpec {
                width: sc.lato,
                height: sc.lato,
                terrain: sim_core::Terrain::Pianura,
            },
            dataset_hash: data.hash_hex(),
        },
        commands: sc.comandi.clone(),
    }
}

/// Rigenera `.ron` e `.hashes` di tutti gli scenari.
///
/// Con `check` non scrive niente e riporta i file che sarebbero cambiati: e'
/// la differenza tra golden che significano qualcosa e golden che si
/// rigenerano per abitudine ogni volta che sono rossi.
pub fn regen(data: &Arc<DataSet>, check: bool) -> Result<Vec<String>, String> {
    let dir = sim_replay::dir_golden();
    if !check {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let mut differenze = Vec::new();
    for nome in scenario::NOMI {
        let sc = scenario::per_nome(nome, data)
            .ok_or_else(|| format!("scenario sconosciuto: {nome}"))?;
        let rec = registra(&sc, data);

        let ron = rec
            .to_ron()
            .map_err(|e| format!("serializzazione di {nome}: {e}"))?;
        confronta_o_scrivi(
            &dir.join(format!("{nome}.ron")),
            &ron,
            check,
            &mut differenze,
        )?;

        let fino_a = tick_del_golden(data);
        let cps = checkpoints(&rec, Arc::clone(data), fino_a, CHECKPOINT_OGNI)
            .map_err(|e| format!("replay di {nome}: {e}"))?;
        let hashes = sim_replay::golden::rendi(&cps, nome, CHECKPOINT_OGNI);
        confronta_o_scrivi(
            &dir.join(format!("{nome}.hashes")),
            &hashes,
            check,
            &mut differenze,
        )?;
    }
    Ok(differenze)
}

fn confronta_o_scrivi(
    path: &Path,
    contenuto: &str,
    check: bool,
    differenze: &mut Vec<String>,
) -> Result<(), String> {
    let attuale = std::fs::read_to_string(path).ok();
    if attuale.as_deref() == Some(contenuto) {
        return Ok(());
    }
    let nome = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    if check {
        differenze.push(match attuale {
            None => format!("{nome} (assente)"),
            Some(_) => format!("{nome} (diverso)"),
        });
        return Ok(());
    }
    std::fs::write(path, contenuto).map_err(|e| format!("{}: {e}", path.display()))?;
    differenze.push(nome);
    Ok(())
}
