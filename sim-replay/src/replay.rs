//! Rigioca una registrazione.

use std::sync::Arc;

use sim_core::{DataSet, Grid, World};

use crate::hash::hash_world;
use crate::recording::Recording;

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error(
        "il dataset non corrisponde a quello con cui e' stata registrata la partita:\n  \
         atteso  {atteso}\n  trovato {trovato}\n\
         se il cambio di bilanciamento era voluto, rigenera i golden con \
         `cargo xtask regen-golden`"
    )]
    DatasetMismatch { atteso: String, trovato: String },

    #[error("versione del formato non supportata: {trovata}, questa build legge la {attesa}")]
    FormatoNonSupportato { trovata: u16, attesa: u16 },

    #[error("griglia non valida nell'header: {0}")]
    GrigliaNonValida(#[from] sim_core::GridError),
}

/// Un punto di controllo: il tick e l'hash dello stato a quel tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub tick: u32,
    pub hash: [u8; 32],
}

/// Crea il mondo iniziale descritto dall'header, verificando che il dataset
/// sia quello giusto.
pub fn mondo_iniziale(rec: &Recording, data: Arc<DataSet>) -> Result<World, ReplayError> {
    if rec.header.format_version != crate::recording::FORMAT_VERSION {
        return Err(ReplayError::FormatoNonSupportato {
            trovata: rec.header.format_version,
            attesa: crate::recording::FORMAT_VERSION,
        });
    }
    let trovato = data.hash_hex();
    if rec.header.dataset_hash != trovato {
        return Err(ReplayError::DatasetMismatch {
            atteso: rec.header.dataset_hash.clone(),
            trovato,
        });
    }
    let g = &rec.header.grid;
    let grid = Grid::new(g.width, g.height, g.terrain)?;
    Ok(World::new(grid, data, rec.header.seed))
}

/// Rigioca fino al tick `until` (escluso: dopo la chiamata `world.tick()` vale
/// `until`).
pub fn replay(rec: &Recording, data: Arc<DataSet>, until: u32) -> Result<World, ReplayError> {
    let mut w = mondo_iniziale(rec, data)?;
    avanza(&mut w, rec, until);
    Ok(w)
}

/// Porta avanti un mondo gia' avviato fino al tick `until`.
///
/// Separata da [`replay`] perche' e' cio' che rende verificabile il
/// determinismo dell'esecuzione **parziale**: fermarsi a meta' e riprendere
/// deve dare lo stesso stato di una corsa unica.
pub fn avanza(world: &mut World, rec: &Recording, until: u32) {
    while world.tick() < until {
        let cmds = rec.comandi_al_tick(world.tick());
        sim_core::step(world, &cmds);
    }
}

/// Rigioca calcolando un hash ogni `ogni` tick.
///
/// Il checkpoint al tick 0 non c'e': e' lo stato iniziale, che l'header
/// descrive gia' per intero.
pub fn checkpoints(
    rec: &Recording,
    data: Arc<DataSet>,
    until: u32,
    ogni: u32,
) -> Result<Vec<Checkpoint>, ReplayError> {
    let mut w = mondo_iniziale(rec, data)?;
    let mut out = Vec::new();
    let passo = ogni.max(1);
    while w.tick() < until {
        let prossimo = (w.tick() / passo + 1) * passo;
        avanza(&mut w, rec, prossimo.min(until));
        if w.tick() % passo == 0 || w.tick() == until {
            out.push(Checkpoint {
                tick: w.tick(),
                hash: hash_world(&w),
            });
        }
    }
    Ok(out)
}
