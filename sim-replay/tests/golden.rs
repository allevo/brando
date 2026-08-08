//! Fase 08 — il test piu' prezioso del progetto.
//!
//! Da qui in avanti ogni fonte di non-determinismo introdotta per distrazione
//! si manifesta come un hash che cambia, entro un commit da quando e' stata
//! introdotta, invece che come un bug di bilanciamento inspiegabile sei mesi
//! dopo.

use std::sync::Arc;

use sim_core::{Coins, DataSet, TilePos, World};
use sim_replay::{CHECKPOINT_OGNI, Recording, ReplayError, checkpoints, hash_hex, hash_world};

const SCENARI: [&str; 2] = ["minimo", "fame"];

fn dati() -> Arc<DataSet> {
    Arc::new(sim_data::load_default().expect("le tabelle di produzione devono caricare"))
}

fn recording(nome: &str) -> Recording {
    let p = sim_replay::dir_golden().join(format!("{nome}.ron"));
    Recording::load(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn hashes_committati(nome: &str) -> Vec<(u32, String)> {
    let p = sim_replay::dir_golden().join(format!("{nome}.hashes"));
    let testo = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    sim_replay::golden::leggi(&testo).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn fino_a(data: &DataSet) -> u32 {
    data.rules.tick_per_anno()
}

// --- 1. determinismo intra-processo ------------------------------------------

#[test]
fn lo_stesso_replay_due_volte_da_gli_stessi_hash() {
    let data = dati();
    for nome in SCENARI {
        let rec = recording(nome);
        let a = checkpoints(&rec, Arc::clone(&data), fino_a(&data), CHECKPOINT_OGNI)
            .expect("replay valido");
        let b = checkpoints(&rec, Arc::clone(&data), fino_a(&data), CHECKPOINT_OGNI)
            .expect("replay valido");
        assert_eq!(a, b, "scenario {nome}");
        assert!(!a.is_empty(), "lo scenario {nome} non produce checkpoint");
    }
}

// --- 2. determinismo inter-processo (il golden vero e proprio) ---------------

/// Coglie cio' che il test 1 non puo': la dipendenza da indirizzi di memoria,
/// da `RandomState`, dall'ordine di iterazione di una collezione hash.
///
/// Se fallisce senza che sia cambiato il bilanciamento, non rigenerare: e'
/// stata introdotta una fonte di non-determinismo.
#[test]
fn gli_hash_coincidono_con_quelli_committati() {
    let data = dati();
    for nome in SCENARI {
        let rec = recording(nome);
        let calcolati = checkpoints(&rec, Arc::clone(&data), fino_a(&data), CHECKPOINT_OGNI)
            .expect("replay valido");
        let attesi = hashes_committati(nome);

        assert_eq!(
            calcolati.len(),
            attesi.len(),
            "scenario {nome}: numero di checkpoint diverso"
        );
        for (c, (tick, hex)) in calcolati.iter().zip(attesi) {
            assert_eq!(c.tick, tick, "scenario {nome}: tick disallineati");
            assert_eq!(
                hash_hex(&c.hash),
                hex,
                "scenario {nome}: hash diverso al tick {tick}"
            );
        }
    }
}

// --- 3. determinismo dell'esecuzione parziale --------------------------------

/// Fermarsi a meta' e riprendere deve dare lo stesso stato di una corsa unica.
/// Coglie gli stati "nascosti" ricostruiti male al riavvio — la rete stradale
/// e la copertura, che non stanno nel salvataggio.
#[test]
fn fermarsi_a_meta_e_riprendere_da_lo_stesso_stato() {
    let data = dati();
    for nome in SCENARI {
        let rec = recording(nome);
        let meta = fino_a(&data) / 2;

        let unica = sim_replay::replay(&rec, Arc::clone(&data), fino_a(&data)).expect("replay");

        let mut spezzata = sim_replay::replay(&rec, Arc::clone(&data), meta).expect("replay");
        sim_replay::avanza(&mut spezzata, &rec, fino_a(&data));

        assert_eq!(
            hash_hex(&hash_world(&unica)),
            hash_hex(&hash_world(&spezzata)),
            "scenario {nome}: riprendere a meta' cambia lo stato"
        );
    }
}

// --- 4. mismatch del dataset -------------------------------------------------

/// Alterato un numero nel dataset, il replay deve fallire **subito** e con il
/// motivo giusto, non divergere silenziosamente al tick 200.
#[test]
fn un_dataset_diverso_fallisce_con_datasetmismatch() {
    let dir = sim_data::dir_dati_di_produzione();
    let leggi = |n: &str| std::fs::read_to_string(dir.join(n)).expect("tabella leggibile");
    let buildings = leggi("buildings.ron").replacen(
        "produzione_per_tick: Some(400)",
        "produzione_per_tick: Some(401)",
        1,
    );
    let alterato = Arc::new(
        sim_data::from_ron_str(&leggi("rules.ron"), &leggi("terrain.ron"), &buildings)
            .expect("tabella alterata comunque valida"),
    );

    let rec = recording("minimo");
    match sim_replay::replay(&rec, alterato, 10) {
        Err(ReplayError::DatasetMismatch { atteso, trovato }) => {
            assert_eq!(atteso, rec.header.dataset_hash);
            assert_ne!(trovato, atteso);
        }
        altro => panic!("atteso DatasetMismatch, trovato {altro:?}"),
    }
}

#[test]
fn una_versione_di_formato_ignota_e_un_errore() {
    let data = dati();
    let mut rec = recording("minimo");
    rec.header.format_version = sim_replay::FORMAT_VERSION + 1;
    assert!(matches!(
        sim_replay::replay(&rec, data, 1),
        Err(ReplayError::FormatoNonSupportato { .. })
    ));
}

// --- 6. l'hash copre tutto lo stato ------------------------------------------

/// Mitigazione del rischio noto di A3: l'hash e' scritto a mano, quindi un
/// campo nuovo puo' restare fuori senza che nessuno se ne accorga — e da quel
/// momento i golden sono **ciechi** su quel campo.
///
/// Se questo test fallisce, `hash_world` ha smesso di coprire un campo dello
/// stato. Non silenziarlo: aggiungere il campo a `hash_world`.
#[test]
fn l_hash_copre_tutto_lo_stato() {
    let data = dati();
    let rec = recording("minimo");
    let base = sim_replay::replay(&rec, Arc::clone(&data), 100).expect("replay");
    let h0 = hash_world(&base);

    /// Una perturbazione di un singolo campo dello stato.
    type Perturbazione = (&'static str, fn(&mut World));

    let perturbazioni: Vec<Perturbazione> = vec![
        ("tick", |w| {
            sim_core::step(w, &[]);
        }),
        ("un tile", |w| {
            w.set_terrain(TilePos::new(0, 0), sim_core::Terrain::Roccia);
        }),
        ("un edificio", |w| {
            let id = w.buildings().next().map(|(id, _)| id).expect("un edificio");
            w.building_mut(id).expect("vivo").level += 1;
        }),
        ("la giacenza di un edificio", |w| {
            let id = w.buildings().next().map(|(id, _)| id).expect("un edificio");
            let b = w.building_mut(id).expect("vivo");
            b.stock = b.stock.saturating_add(sim_core::Milli::from_millis(1));
        }),
        ("una casa", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("una casa");
            w.house_mut(id).expect("viva").abitanti += 1;
        }),
        ("i servizi di una casa", |w| {
            let id = w.houses().next().map(|(id, _)| id).expect("una casa");
            let h = w.house_mut(id).expect("viva");
            let attuale = h.servita.get(sim_core::ServiceKind::Acqua);
            h.servita.set(sim_core::ServiceKind::Acqua, !attuale);
        }),
        ("il tesoro", |w| {
            w.economy_mut().tesoro = w.economy().tesoro.saturating_add(Coins::new(1));
        }),
    ];

    for (cosa, perturba) in perturbazioni {
        let mut w = base.clone();
        perturba(&mut w);
        assert_ne!(
            hash_hex(&hash_world(&w)),
            hash_hex(&h0),
            "l'hash non copre {cosa}: da qui in poi i golden sono ciechi su quel campo"
        );
    }

    // La posizione di ogni stream RNG, un dominio alla volta.
    for d in sim_core::RngDomain::TUTTI {
        let mut w = base.clone();
        w.consuma_rng(d);
        assert_ne!(
            hash_hex(&hash_world(&w)),
            hash_hex(&h0),
            "l'hash non copre la posizione dello stream {d:?}"
        );
    }
}

/// Le strutture derivate **non** devono entrare nell'hash: se ci entrassero,
/// un bug di ricostruzione incrementale si presenterebbe come divergenza di
/// hash invece che come test di equivalenza fallito (fase 06).
#[test]
fn le_strutture_derivate_restano_fuori_dall_hash() {
    let data = dati();
    let rec = recording("minimo");
    let a = sim_replay::replay(&rec, Arc::clone(&data), 100).expect("replay");

    // Stesso stato, ma con contatori diagnostici diversi: la rete e' stata
    // ricostruita piu' volte perche' la partita e' stata spezzata.
    let mut b = sim_replay::replay(&rec, Arc::clone(&data), 50).expect("replay");
    sim_replay::avanza(&mut b, &rec, 100);

    assert_eq!(hash_hex(&hash_world(&a)), hash_hex(&hash_world(&b)));
}

// --- 7. sensibilita' al seed (atteso rosso in M0) ----------------------------

/// Seed diverso, stessi comandi ⇒ hash diverso, **appena** un dominio RNG
/// viene usato.
///
/// In M0 nessun sistema estrae dall'RNG: migrazione ed eventi casuali sono
/// M1. Il test e' quindi atteso fallire ed e' scritto ora perche' ora si
/// capisce perche' serve. Da riattivare con la migrazione (M1, fase 2), che e'
/// il primo uso reale di `RngDomain::Migration`.
#[test]
#[ignore = "in M0 nessun sistema usa l'RNG: si riattiva con la migrazione (M1)"]
fn seed_diversi_danno_hash_diversi() {
    let data = dati();
    let rec = recording("minimo");
    let mut altro = rec.clone();
    altro.header.seed = rec.header.seed + 1;

    let a = sim_replay::replay(&rec, Arc::clone(&data), fino_a(&data)).expect("replay");
    let b = sim_replay::replay(&altro, Arc::clone(&data), fino_a(&data)).expect("replay");
    assert_ne!(hash_hex(&hash_world(&a)), hash_hex(&hash_world(&b)));
}

// --- forma dei golden --------------------------------------------------------

#[test]
fn i_golden_sono_coerenti_con_il_dataset_e_leggibili() {
    let data = dati();
    for nome in SCENARI {
        let rec = recording(nome);
        assert_eq!(rec.header.dataset_hash, data.hash_hex(), "scenario {nome}");
        assert_eq!(rec.header.format_version, sim_replay::FORMAT_VERSION);
        assert!(!rec.commands.is_empty(), "scenario {nome} senza comandi");

        let mut precedente = 0;
        for (t, _) in &rec.commands {
            assert!(*t >= precedente, "scenario {nome}: tick non ordinati");
            precedente = *t;
        }

        let hashes = hashes_committati(nome);
        assert!(!hashes.is_empty(), "scenario {nome} senza checkpoint");
        for (_, hex) in &hashes {
            assert_eq!(hex.len(), 64, "hash malformato in {nome}");
        }
    }
}
