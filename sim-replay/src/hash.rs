//! Hash canonico dello stato.
//!
//! Scritto a mano, non delegato a serde (A3): con serde l'hash dipenderebbe
//! dall'ordine di dichiarazione dei campi e dal formato, quindi spostare un
//! campo in una struct — refactoring senza conseguenze semantiche —
//! invaliderebbe tutti i golden. E' esattamente il falso positivo che rende
//! inutile il test piu' prezioso del progetto.
//!
//! Il costo e' che aggiungere un campo allo stato richiede di aggiungerlo qui
//! a mano. Se lo si dimentica, l'hash diventa cieco su quel campo e i golden
//! smettono di proteggerlo: e' il rischio noto di A3, mitigato dal test
//! `l_hash_copre_tutto_lo_stato`.

use sim_core::{RngDomain, ServiceKind, World};

/// Prefisso di dominio: separa questo hash da quello del dataset.
/// Cambiarlo rigenera tutti i golden.
const DOMINIO: &[u8] = b"brando/world/v1";

/// Hash canonico dello stato, in un ordine fissato **qui** e non altrove.
///
/// Cosa entra: tick, hash del dataset, dimensioni della griglia, i tile in
/// ordine di `TileIdx`, gli edifici e le case in ordine di id, l'economia e la
/// posizione di ogni stream RNG.
///
/// Cosa **non** entra: `RoadNetwork`, `Coverage`, `DirtyFlags`, `FoodLedger`.
/// Sono strutture derivate o diagnostiche; se entrassero, un bug di
/// ricostruzione si presenterebbe come divergenza di hash, mentre il test che
/// deve coglierlo e' l'equivalenza incrementale/da-zero della fase 06 — che
/// dice anche *dove* e' il problema.
///
/// `House::servita` invece **entra**, pur essendo calcolata dalla copertura:
/// e' un campo dello stato ed e' l'input da cui M1 fara' evolvere o degradare
/// le case. Se restasse fuori, su M0 un golden non si accorgerebbe quasi di
/// nulla — cambiare la regola di assegnazione dei servizi non sposterebbe un
/// bit.
pub fn hash_world(w: &World) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMINIO);

    h.update(&w.tick().to_le_bytes());
    h.update(&w.data().hash);

    let grid = w.grid();
    h.update(&grid.width().to_le_bytes());
    h.update(&grid.height().to_le_bytes());

    // --- tile, in ordine di TileIdx ---
    for idx in grid.indices() {
        let Some(t) = grid.get(idx) else { continue };
        h.update(&[t.terrain as u8, t.flags.bits()]);
        // I flag dicono gia' se c'e' un occupante; l'origine si aggiunge solo
        // quando c'e', per non hashare byte privi di significato.
        if let Some(occ) = t.occupante() {
            h.update(&occ.origin.get().to_le_bytes());
        }
    }

    // --- edifici, in ordine di id ---
    // L'iterazione di uno SlotMap e' per indice di slot, deterministica a
    // parita' di sequenza di inserimenti e rimozioni — garantita dal log dei
    // comandi (D4).
    h.update(&(w.n_edifici() as u64).to_le_bytes());
    for (_, b) in w.buildings() {
        h.update(&b.kind.get().to_le_bytes());
        h.update(&[b.origin.x, b.origin.y, b.level]);
        h.update(&b.stock.to_millis().to_le_bytes());
    }

    // --- case, in ordine di id ---
    h.update(&(w.n_case() as u64).to_le_bytes());
    for (_, c) in w.houses() {
        h.update(&[c.origin.x, c.origin.y, c.level]);
        h.update(&c.abitanti.to_le_bytes());
        h.update(&[c.servita.bits()]);
    }

    // --- economia ---
    h.update(&w.economy().tesoro.get().to_le_bytes());

    // --- posizione degli stream RNG ---
    // Uno stato in cui Events ha consumato 5 valori non e' lo stesso in cui ne
    // ha consumati 6, anche se tutto il resto coincide: senza questo, una
    // divergenza si manifesterebbe molti tick dopo, dove e' quasi impossibile
    // da attribuire.
    for d in RngDomain::TUTTI {
        h.update(&w.rng().draws(d).to_le_bytes());
    }

    // Presidio contro un cambio silenzioso del numero di servizi: se ne
    // arrivasse uno nuovo, `servita.bits()` cambierebbe significato.
    h.update(&[ServiceKind::COUNT as u8]);

    *h.finalize().as_bytes()
}

pub fn hash_hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}
