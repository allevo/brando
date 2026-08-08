//! Validazione delle tabelle grezze.
//!
//! Il report raccoglie **tutti** gli errori, non solo il primo: chi corregge
//! una tabella vuole vedere tutti i problemi in un giro, non ricompilare sei
//! volte. Un `?` sul primo controllo sarebbe una validazione finta.

use std::collections::BTreeMap;
use std::fmt;

use sim_core::{Coins, Milli, ServiceKind, Terrain};

use crate::raw::{RawBuildingDef, RawDataSet};
use sim_core::data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef};

/// Un singolo problema, con il percorso logico del campo che lo causa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Es. `buildings[1].servizio.raggio_per_livello`.
    pub path: String,
    pub kind: ValidationErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationErrorKind {
    #[error("valore vuoto")]
    Vuoto,

    #[error("id duplicato, gia' usato da {precedente}")]
    IdDuplicato { precedente: String },

    #[error("atteso almeno {min}, trovato {trovato}")]
    TroppoPiccolo { min: i64, trovato: i64 },

    #[error("valore negativo: {trovato}")]
    Negativo { trovato: i64 },

    #[error("atteso un valore per livello: livelli = {livelli}, valori = {trovati}")]
    LunghezzaPerLivello { livelli: u8, trovati: usize },

    #[error("servizio sconosciuto: {nome:?} (noti: {noti})")]
    ServizioSconosciuto { nome: String, noti: String },

    #[error("un produttore deve avere giacenza_max > 0")]
    ProduttoreSenzaGiacenza,

    #[error("giacenza_max su un edificio che non produce nulla")]
    GiacenzaSenzaProduzione,

    #[error(
        "capacita' {capacita} abitanti, ma la produzione ne sostiene {sostenibili}: \
         le case in eccesso resterebbero assegnate a un provider che non le sfama"
    )]
    CapacitaOltreLaProduzione { capacita: u16, sostenibili: u16 },

    #[error("terreno assente dalla tabella: {terrain:?}")]
    TerrenoMancante { terrain: Terrain },

    #[error("terreno gia' dichiarato")]
    TerrenoDuplicato,

    #[error("troppi edifici in tabella: il massimo e' {max}")]
    TroppiEdifici { max: usize },
}

/// L'insieme dei problemi trovati in un giro di validazione.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

impl ValidationReport {
    fn push(&mut self, path: impl Into<String>, kind: ValidationErrorKind) {
        self.errors.push(ValidationError {
            path: path.into(),
            kind,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} error{} di validazione nelle tabelle:",
            self.errors.len(),
            if self.errors.len() == 1 { "e" } else { "i" }
        )?;
        for e in &self.errors {
            writeln!(f, "  - {}: {}", e.path, e.kind)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationReport {}

/// Valida e costruisce in un solo passaggio.
///
/// Il piano prevedeva `validate(&RawDataSet) -> Result<(), ValidationReport>`
/// con la conversione a parte: farlo in due passaggi obbligherebbe a
/// ricontrollare gli stessi invarianti durante la conversione (o a fidarsi
/// con degli `expect`). Un passaggio solo, che restituisce il dataset
/// costruito, e' la stessa garanzia senza la duplicazione.
pub fn validate(raw: &RawDataSet) -> Result<DataSet, ValidationReport> {
    let mut rep = ValidationReport::default();

    let rules = valida_rules(raw, &mut rep);
    let terrain = valida_terrain(raw, &mut rep);
    let buildings = valida_buildings(raw, &mut rep);

    if !rep.is_empty() {
        return Err(rep);
    }

    // I controlli **fra** tabelle vengono dopo, e solo se le singole tabelle
    // sono sane: la coerenza fra la capacita' di un provider di cibo e cio'
    // che la sua produzione sostiene incrocia `rules` e `buildings`, e su una
    // tabella gia' rotta produrrebbe rumore invece che informazione.
    let data = DataSet::new(rules, terrain, buildings);
    for v in data.capacita_cibo_insostenibile() {
        rep.push(
            format!("buildings[{}].servizio.capacita_per_livello", v.building),
            ValidationErrorKind::CapacitaOltreLaProduzione {
                capacita: v.capacita,
                sostenibili: v.sostenibili,
            },
        );
    }
    if !rep.is_empty() {
        return Err(rep);
    }

    Ok(data)
}

fn valida_rules(raw: &RawDataSet, rep: &mut ValidationReport) -> Rules {
    let r = &raw.rules;

    if r.tick_per_mese < 1 {
        rep.push(
            "rules.tick_per_mese",
            ValidationErrorKind::TroppoPiccolo {
                min: 1,
                trovato: i64::from(r.tick_per_mese),
            },
        );
    }
    if r.mesi_per_anno < 1 {
        rep.push(
            "rules.mesi_per_anno",
            ValidationErrorKind::TroppoPiccolo {
                min: 1,
                trovato: i64::from(r.mesi_per_anno),
            },
        );
    }
    if r.tick_per_mese.checked_mul(r.mesi_per_anno).is_none() {
        rep.push(
            "rules.mesi_per_anno",
            ValidationErrorKind::TroppoPiccolo { min: 1, trovato: 0 },
        );
    }
    if r.abitanti_per_livello_casa.is_empty() {
        rep.push(
            "rules.abitanti_per_livello_casa",
            ValidationErrorKind::Vuoto,
        );
    }
    if r.consumo_cibo_per_abitante < 0 {
        rep.push(
            "rules.consumo_cibo_per_abitante",
            ValidationErrorKind::Negativo {
                trovato: i64::from(r.consumo_cibo_per_abitante),
            },
        );
    }

    Rules {
        tick_per_mese: r.tick_per_mese,
        mesi_per_anno: r.mesi_per_anno,
        tesoro_iniziale: Coins::new(r.tesoro_iniziale),
        abitanti_per_livello_casa: r.abitanti_per_livello_casa.clone(),
        consumo_cibo_per_abitante: Milli::from_millis(r.consumo_cibo_per_abitante),
    }
}

fn valida_terrain(raw: &RawDataSet, rep: &mut ValidationReport) -> BTreeMap<Terrain, TerrainDef> {
    let mut out = BTreeMap::new();

    for (i, t) in raw.terrain.terrains.iter().enumerate() {
        let path = format!("terrains[{i}]");
        if t.costo_strada < 0 {
            rep.push(
                format!("{path}.costo_strada"),
                ValidationErrorKind::Negativo {
                    trovato: i64::from(t.costo_strada),
                },
            );
        }
        let def = TerrainDef {
            costruibile: t.costruibile,
            attraversabile: t.attraversabile,
            costo_strada: Coins::new(t.costo_strada),
        };
        if out.insert(t.terrain, def).is_some() {
            rep.push(
                format!("{path}.terrain"),
                ValidationErrorKind::TerrenoDuplicato,
            );
        }
    }

    // La tabella deve coprire tutte le varianti: un terreno mancante
    // diventerebbe un `unwrap` in un hot path.
    for t in Terrain::TUTTI {
        if !out.contains_key(&t) {
            rep.push(
                "terrains",
                ValidationErrorKind::TerrenoMancante { terrain: t },
            );
        }
    }

    out
}

fn valida_buildings(raw: &RawDataSet, rep: &mut ValidationReport) -> Vec<BuildingDef> {
    let defs = &raw.buildings.buildings;

    if defs.len() > usize::from(u16::MAX) {
        rep.push(
            "buildings",
            ValidationErrorKind::TroppiEdifici {
                max: usize::from(u16::MAX),
            },
        );
    }

    let mut out = Vec::with_capacity(defs.len());
    for (i, b) in defs.iter().enumerate() {
        let path = format!("buildings[{i}]");

        if b.id.trim().is_empty() {
            rep.push(format!("{path}.id"), ValidationErrorKind::Vuoto);
        } else if let Some(prec) = defs.iter().take(i).position(|o| o.id == b.id) {
            rep.push(
                format!("{path}.id"),
                ValidationErrorKind::IdDuplicato {
                    precedente: format!("buildings[{prec}].id"),
                },
            );
        }

        if b.livelli < 1 {
            rep.push(
                format!("{path}.livelli"),
                ValidationErrorKind::TroppoPiccolo {
                    min: 1,
                    trovato: i64::from(b.livelli),
                },
            );
        }
        if b.costo < 0 {
            rep.push(
                format!("{path}.costo"),
                ValidationErrorKind::Negativo {
                    trovato: i64::from(b.costo),
                },
            );
        }
        if b.footprint.0 < 1 {
            rep.push(
                format!("{path}.footprint.0"),
                ValidationErrorKind::TroppoPiccolo {
                    min: 1,
                    trovato: i64::from(b.footprint.0),
                },
            );
        }
        if b.footprint.1 < 1 {
            rep.push(
                format!("{path}.footprint.1"),
                ValidationErrorKind::TroppoPiccolo {
                    min: 1,
                    trovato: i64::from(b.footprint.1),
                },
            );
        }

        let servizio = valida_servizio(b, &path, rep);
        let servizi_richiesti = valida_servizi_richiesti(b, &path, rep);
        valida_produzione(b, &path, rep);

        out.push(BuildingDef {
            id: b.id.clone(),
            footprint: b.footprint,
            costo: Coins::new(b.costo),
            livelli: b.livelli,
            servizio,
            servizi_richiesti,
            produzione_per_tick: b.produzione_per_tick.map(Milli::from_millis),
            giacenza_max: b.giacenza_max.map(Milli::from_millis),
        });
    }

    out
}

fn valida_servizio(
    b: &RawBuildingDef,
    path: &str,
    rep: &mut ValidationReport,
) -> Option<ServiceDef> {
    let s = b.servizio.as_ref()?;

    let kind = match ServiceKind::from_id(&s.kind) {
        Some(k) => Some(k),
        None => {
            rep.push(
                format!("{path}.servizio.kind"),
                ValidationErrorKind::ServizioSconosciuto {
                    nome: s.kind.clone(),
                    noti: servizi_noti(),
                },
            );
            None
        }
    };

    // Un valore per livello: e' l'errore piu' probabile quando in M1 i livelli
    // diventeranno piu' di uno.
    for (campo, valori) in [
        ("raggio_per_livello", &s.raggio_per_livello),
        ("capacita_per_livello", &s.capacita_per_livello),
    ] {
        if valori.len() != usize::from(b.livelli) {
            rep.push(
                format!("{path}.servizio.{campo}"),
                ValidationErrorKind::LunghezzaPerLivello {
                    livelli: b.livelli,
                    trovati: valori.len(),
                },
            );
        }
    }

    kind.map(|kind| ServiceDef {
        kind,
        raggio_per_livello: s.raggio_per_livello.clone(),
        capacita_per_livello: s.capacita_per_livello.clone(),
    })
}

fn valida_servizi_richiesti(
    b: &RawBuildingDef,
    path: &str,
    rep: &mut ValidationReport,
) -> Vec<ServiceKind> {
    let mut out = Vec::with_capacity(b.servizi_richiesti.len());
    for (j, nome) in b.servizi_richiesti.iter().enumerate() {
        match ServiceKind::from_id(nome) {
            Some(k) => out.push(k),
            None => rep.push(
                format!("{path}.servizi_richiesti[{j}]"),
                ValidationErrorKind::ServizioSconosciuto {
                    nome: nome.clone(),
                    noti: servizi_noti(),
                },
            ),
        }
    }
    out
}

fn valida_produzione(b: &RawBuildingDef, path: &str, rep: &mut ValidationReport) {
    match (b.produzione_per_tick, b.giacenza_max) {
        (Some(p), giacenza) => {
            if p < 0 {
                rep.push(
                    format!("{path}.produzione_per_tick"),
                    ValidationErrorKind::Negativo {
                        trovato: i64::from(p),
                    },
                );
            }
            if giacenza.is_none_or(|g| g <= 0) {
                rep.push(
                    format!("{path}.giacenza_max"),
                    ValidationErrorKind::ProduttoreSenzaGiacenza,
                );
            }
        }
        (None, Some(_)) => rep.push(
            format!("{path}.giacenza_max"),
            ValidationErrorKind::GiacenzaSenzaProduzione,
        ),
        (None, None) => {}
    }
}

fn servizi_noti() -> String {
    ServiceKind::TUTTI
        .iter()
        .map(|k| k.as_id())
        .collect::<Vec<_>>()
        .join(", ")
}
