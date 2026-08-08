//! Hash canonico del dataset.
//!
//! Alimenta l'hasher campo per campo in un ordine scritto qui esplicitamente,
//! invece di serializzare con serde (A3): con serde l'hash dipenderebbe
//! dall'ordine di dichiarazione dei campi, e spostare un campo — refactoring
//! senza conseguenze semantiche — invaliderebbe tutti i golden.
//!
//! Corollario: aggiungere un campo alle tabelle richiede di aggiungerlo qui a
//! mano. Se non lo si fa, un cambio di bilanciamento su quel campo non fara'
//! fallire i replay. E' il costo consapevole di A3.

use std::collections::BTreeMap;

use sim_core::Terrain;

use crate::dataset::{BuildingDef, Rules, TerrainDef};

/// Prefisso di dominio: separa questo hash da qualunque altro blake3 del
/// progetto. Cambiarlo rigenera tutti i golden.
const DOMINIO: &[u8] = b"brando/dataset/v1";

pub(crate) fn canonical_hash(
    rules: &Rules,
    terrain: &BTreeMap<Terrain, TerrainDef>,
    buildings: &[BuildingDef],
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMINIO);

    // --- rules ---
    h.update(&rules.tick_per_mese.to_le_bytes());
    h.update(&rules.mesi_per_anno.to_le_bytes());
    h.update(&rules.tesoro_iniziale.get().to_le_bytes());
    h.update(&(rules.abitanti_per_livello_casa.len() as u64).to_le_bytes());
    for a in &rules.abitanti_per_livello_casa {
        h.update(&a.to_le_bytes());
    }
    h.update(&rules.consumo_cibo_per_abitante.to_millis().to_le_bytes());

    // --- terrain, in ordine di Terrain (il BTreeMap lo garantisce) ---
    h.update(&(terrain.len() as u64).to_le_bytes());
    for (t, d) in terrain {
        h.update(&[*t as u8]);
        h.update(&[u8::from(d.costruibile), u8::from(d.attraversabile)]);
        h.update(&d.costo_strada.get().to_le_bytes());
    }

    // --- buildings, in ordine di BuildingKindId ---
    h.update(&(buildings.len() as u64).to_le_bytes());
    for b in buildings {
        // La lunghezza prima del contenuto: senza, "ab"+"c" e "a"+"bc"
        // darebbero lo stesso hash.
        h.update(&(b.id.len() as u64).to_le_bytes());
        h.update(b.id.as_bytes());
        h.update(&[b.footprint.0, b.footprint.1, b.livelli]);
        h.update(&b.costo.get().to_le_bytes());

        match &b.servizio {
            None => {
                h.update(&[0u8]);
            }
            Some(s) => {
                h.update(&[1u8]);
                h.update(&[s.kind.index() as u8]);
                hash_u16_slice(&mut h, &s.raggio_per_livello);
                hash_u16_slice(&mut h, &s.capacita_per_livello);
            }
        }

        h.update(&(b.servizi_richiesti.len() as u64).to_le_bytes());
        for s in &b.servizi_richiesti {
            h.update(&[s.index() as u8]);
        }

        hash_opt_milli(&mut h, b.produzione_per_tick);
        hash_opt_milli(&mut h, b.giacenza_max);
    }

    *h.finalize().as_bytes()
}

fn hash_u16_slice(h: &mut blake3::Hasher, v: &[u16]) {
    h.update(&(v.len() as u64).to_le_bytes());
    for x in v {
        h.update(&x.to_le_bytes());
    }
}

fn hash_opt_milli(h: &mut blake3::Hasher, v: Option<sim_core::Milli>) {
    match v {
        None => {
            h.update(&[0u8]);
        }
        Some(m) => {
            h.update(&[1u8]);
            h.update(&m.to_millis().to_le_bytes());
        }
    }
}
