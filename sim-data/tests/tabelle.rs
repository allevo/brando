//! Test delle tabelle: la fixture di produzione carica, quelle rotte
//! producono un report **completo**, e l'hash distingue un cambio di
//! bilanciamento da una riformattazione.

use std::path::Path;

use sim_core::{Coins, Milli, ServiceKind, Terrain};
use sim_data::{DataSet, LoadError, ValidationErrorKind};

fn rules_valide() -> String {
    leggi_dati("rules.ron")
}

fn terrain_valido() -> String {
    leggi_dati("terrain.ron")
}

fn leggi_dati(nome: &str) -> String {
    let p = sim_data::dir_dati_di_produzione().join(nome);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn fixture_rotta(nome: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rotte")
        .join(nome);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Carica le tabelle valide sostituendo solo `buildings.ron`.
fn con_buildings(buildings: &str) -> Result<DataSet, LoadError> {
    sim_data::from_ron_str(&rules_valide(), &terrain_valido(), buildings)
}

// --- 1. la fixture di produzione ---

#[test]
fn le_tabelle_di_produzione_caricano() {
    let d = sim_data::load_default().expect("le tabelle di produzione devono caricare");

    assert!(d.rules.tick_per_mese >= 1);
    assert_eq!(d.rules.tick_per_anno(), d.rules.tick_per_mese * 12);
    assert_eq!(d.rules.tesoro_iniziale, Coins::new(1000));

    for t in Terrain::TUTTI {
        assert!(d.terrain(t).is_some(), "terreno {t:?} assente");
    }

    let casa = d.kind_by_id("casa").expect("la casa esiste");
    let pozzo = d.kind_by_id("pozzo").expect("il pozzo esiste");
    let fattoria = d.kind_by_id("fattoria").expect("la fattoria esiste");
    assert_eq!(d.kind_by_id("piramide"), None);

    let casa = d.def(casa).expect("def della casa");
    assert!(casa.e_una_casa());
    assert!(!casa.e_un_produttore());
    assert_eq!(
        casa.servizi_richiesti,
        [ServiceKind::Acqua, ServiceKind::Cibo]
    );

    let pozzo = d.def(pozzo).expect("def del pozzo");
    let s = pozzo
        .servizio
        .as_ref()
        .expect("il pozzo fornisce un servizio");
    assert_eq!(s.kind, ServiceKind::Acqua);
    assert_eq!(s.raggio(1), Some(12));
    assert_eq!(s.capacita(1), Some(8));
    assert_eq!(s.raggio(2), None, "il pozzo ha un solo livello in M0");
    assert_eq!(s.raggio(0), None, "i livelli partono da 1");

    let fattoria = d.def(fattoria).expect("def della fattoria");
    assert!(fattoria.e_un_produttore());
    assert_eq!(fattoria.tile_occupati(), 4);
    assert_eq!(fattoria.produzione_per_tick, Some(Milli::from_millis(400)));
    assert!(fattoria.giacenza_max > Some(Milli::ZERO));
}

/// Il bilanciamento deve rendere raggiungibile sia la sazieta' sia la fame:
/// se una fattoria bastasse per qualunque numero di case, lo scenario "fame"
/// della fase 08 non esisterebbe.
#[test]
fn una_fattoria_non_sostiene_la_propria_capacita_massima() {
    let d = sim_data::load_default().expect("tabelle valide");
    let f = d
        .def(d.kind_by_id("fattoria").expect("fattoria"))
        .expect("def");
    let s = f.servizio.as_ref().expect("servizio");

    let abitanti = i32::from(d.rules.abitanti_per_livello_casa[0]);
    let per_casa = d
        .rules
        .consumo_cibo_per_abitante
        .checked_mul_int(abitanti)
        .expect("consumo di una casa");
    let capacita = i32::from(s.capacita(1).expect("capacita"));
    let domanda_max = per_casa.checked_mul_int(capacita).expect("domanda massima");
    let prodotto = f.produzione_per_tick.expect("produzione");

    assert!(
        prodotto > per_casa,
        "una fattoria deve sostenere almeno una casa"
    );
    assert!(
        prodotto < domanda_max,
        "a piena capacita' la fattoria deve andare in deficit: {prodotto} >= {domanda_max}"
    );
}

// --- 2. fixture rotte, una per controllo ---

fn errori(buildings: &str) -> Vec<(String, ValidationErrorKind)> {
    match con_buildings(buildings) {
        Err(LoadError::Validazione(r)) => r.errors.into_iter().map(|e| (e.path, e.kind)).collect(),
        Err(altro) => panic!("atteso un errore di validazione, trovato: {altro}"),
        Ok(_) => panic!("la tabella rotta ha superato la validazione"),
    }
}

#[test]
fn id_duplicato() {
    let e = errori(&fixture_rotta("id_duplicato.ron"));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].0, "buildings[1].id");
    assert!(matches!(e[0].1, ValidationErrorKind::IdDuplicato { .. }));
}

#[test]
fn livelli_senza_raggi_corrispondenti() {
    let e = errori(&fixture_rotta("livelli_senza_raggi.ron"));
    assert_eq!(e.len(), 2, "manca un valore sia in raggio sia in capacita'");
    assert_eq!(e[0].0, "buildings[0].servizio.raggio_per_livello");
    assert_eq!(e[1].0, "buildings[0].servizio.capacita_per_livello");
}

#[test]
fn costo_negativo() {
    let e = errori(&fixture_rotta("costo_negativo.ron"));
    assert_eq!(
        e,
        [(
            "buildings[0].costo".to_string(),
            ValidationErrorKind::Negativo { trovato: -10 }
        )]
    );
}

#[test]
fn servizio_sconosciuto() {
    let e = errori(&fixture_rotta("servizio_sconosciuto.ron"));
    assert_eq!(e.len(), 2, "kind del servizio e servizio richiesto");
    assert_eq!(e[0].0, "buildings[0].servizio.kind");
    assert_eq!(e[1].0, "buildings[0].servizi_richiesti[0]");
}

#[test]
fn campo_mancante_e_ron_invalido_falliscono_al_parse() {
    for nome in ["campo_mancante.ron", "ron_invalido.ron"] {
        match con_buildings(&fixture_rotta(nome)) {
            Err(LoadError::Ron { file, .. }) => assert_eq!(file, "buildings.ron"),
            altro => panic!("{nome}: atteso un errore di parse, trovato {altro:?}"),
        }
    }
}

// --- 3. il report e' completo ---

/// Distingue una validazione vera da un `?` sul primo controllo.
#[test]
fn il_report_riporta_tutti_gli_errori() {
    let e = errori(&fixture_rotta("tre_errori.ron"));
    let paths: Vec<_> = e.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        [
            "buildings[0].id",
            "buildings[0].footprint.0",
            "buildings[1].giacenza_max",
        ],
        "tre errori distinti devono comparire tutti e tre"
    );
}

// --- 4. messaggi stabili ---

#[test]
fn messaggi_del_report_stabili() {
    let r = match con_buildings(&fixture_rotta("tre_errori.ron")) {
        Err(LoadError::Validazione(r)) => r,
        altro => panic!("atteso un report, trovato {altro:?}"),
    };
    insta::assert_snapshot!("report_tre_errori", r.to_string());

    let r = match con_buildings(&fixture_rotta("servizio_sconosciuto.ron")) {
        Err(LoadError::Validazione(r)) => r,
        altro => panic!("atteso un report, trovato {altro:?}"),
    };
    insta::assert_snapshot!("report_servizio_sconosciuto", r.to_string());
}

// --- 5. hash ---

#[test]
fn hash_stabile_tra_due_caricamenti() {
    let a = sim_data::load_default().expect("tabelle valide");
    let b = sim_data::load_default().expect("tabelle valide");
    assert_eq!(a.hash, b.hash);
    assert_eq!(a.hash_hex().len(), 64);
}

/// Riformattare il RON o aggiungere un commento non deve invalidare i golden:
/// l'hash e' sul contenuto validato, non sui byte del file.
#[test]
fn hash_insensibile_alla_riformattazione() {
    let originale = leggi_dati("buildings.ron");
    let riformattato = format!(
        "// commento aggiunto\n\n{}\n\n// e un altro in fondo\n",
        originale.replace('\n', "\n   ")
    );

    let a = con_buildings(&originale).expect("originale valido");
    let b = con_buildings(&riformattato).expect("riformattato valido");
    assert_eq!(
        a.hash, b.hash,
        "una riformattazione non e' un cambio di bilanciamento"
    );
}

/// Ma un numero cambiato si': e' cio' che fa fallire il replay subito e per
/// il motivo giusto (A2).
#[test]
fn hash_sensibile_al_bilanciamento() {
    let originale = leggi_dati("buildings.ron");
    let modificato = originale.replacen("costo: 10", "costo: 11", 1);
    assert_ne!(originale, modificato, "la sostituzione deve aver morso");

    let a = con_buildings(&originale).expect("originale valido");
    let b = con_buildings(&modificato).expect("modificato valido");
    assert_ne!(a.hash, b.hash);
}

/// Anche le rules e i terreni entrano nell'hash: se ci finisse solo
/// `buildings`, cambiare il consumo di cibo non farebbe fallire nessun golden.
#[test]
fn hash_copre_tutte_le_tabelle() {
    let base = sim_data::load_default().expect("tabelle valide");

    let rules_mod = rules_valide().replacen(
        "consumo_cibo_per_abitante: 20",
        "consumo_cibo_per_abitante: 21",
        1,
    );
    let a = sim_data::from_ron_str(&rules_mod, &terrain_valido(), &leggi_dati("buildings.ron"))
        .expect("valido");
    assert_ne!(base.hash, a.hash, "le rules devono entrare nell'hash");

    let terrain_mod = terrain_valido().replacen("costo_strada: 2", "costo_strada: 3", 1);
    let b = sim_data::from_ron_str(&rules_valide(), &terrain_mod, &leggi_dati("buildings.ron"))
        .expect("valido");
    assert_ne!(base.hash, b.hash, "i terreni devono entrare nell'hash");
}
