//! Il dataset di bilanciamento: la forma validata che il core consuma.
//!
//! Le **definizioni** stanno qui e non in `sim-data` per la direzione delle
//! dipendenze: il `World` tiene un `Arc<DataSet>` (A2), e sim-core non puo'
//! dipendere da sim-data. In `sim-data` restano il parsing RON, la
//! validazione e l'I/O — cioe' tutto cio' che il core non deve fare (D4).

use std::collections::BTreeMap;

use crate::data_hash::canonical_hash;
use crate::grid::Terrain;
use crate::ids::BuildingKindId;
use crate::service::ServiceKind;
use crate::units::{Coins, Milli};

/// Costanti globali di simulazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub tick_per_mese: u32,
    pub mesi_per_anno: u32,
    pub tesoro_iniziale: Coins,
    /// Indicizzato per livello di casa (livello 1 = indice 0).
    pub abitanti_per_livello_casa: Vec<u16>,
    pub consumo_cibo_per_abitante: Milli,
}

impl Rules {
    /// Tick in un anno di gioco. Gli obiettivi di scenario si esprimono in
    /// mesi e anni, mai in tick (M1).
    pub const fn tick_per_anno(&self) -> u32 {
        self.tick_per_mese * self.mesi_per_anno
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainDef {
    pub costruibile: bool,
    pub attraversabile: bool,
    pub costo_strada: Coins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDef {
    pub kind: ServiceKind,
    /// Raggio in tile percorsi sulla rete stradale (D2), uno per livello.
    pub raggio_per_livello: Vec<u16>,
    /// **Abitanti** serviti contemporaneamente, uno per livello.
    ///
    /// Non case: con i livelli delle case (M1) la popolazione varia da casa a
    /// casa, e una capacita' in case non direbbe piu' quanta gente il provider
    /// riesce davvero a servire.
    pub capacita_per_livello: Vec<u16>,
}

impl ServiceDef {
    /// Raggio al livello dato (livello 1 = indice 0), `None` fuori range.
    pub fn raggio(&self, livello: u8) -> Option<u16> {
        self.raggio_per_livello
            .get(usize::from(livello).checked_sub(1)?)
            .copied()
    }

    /// Capacita' in abitanti al livello dato, `None` fuori range.
    pub fn capacita(&self, livello: u8) -> Option<u16> {
        self.capacita_per_livello
            .get(usize::from(livello).checked_sub(1)?)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// (larghezza, altezza) in tile.
    pub footprint: (u8, u8),
    pub costo: Coins,
    pub livelli: u8,
    pub servizio: Option<ServiceDef>,
    pub servizi_richiesti: Vec<ServiceKind>,
    pub produzione_per_tick: Option<Milli>,
    pub giacenza_max: Option<Milli>,
}

impl BuildingDef {
    /// Un edificio e' una casa se richiede servizi invece di fornirne.
    /// Regola strutturale, non un numero: sta nel codice di proposito.
    pub fn e_una_casa(&self) -> bool {
        self.servizio.is_none() && !self.servizi_richiesti.is_empty()
    }

    pub fn e_un_produttore(&self) -> bool {
        self.produzione_per_tick.is_some()
    }

    /// Numero di tile occupati.
    pub const fn tile_occupati(&self) -> u16 {
        self.footprint.0 as u16 * self.footprint.1 as u16
    }
}

/// Tabelle validate, pronte per il core.
///
/// Vive dietro un `Arc` dentro il `World` (A2): il caricamento e' I/O e resta
/// fuori dal core, ma i sistemi hanno bisogno delle tabelle a ogni tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSet {
    pub rules: Rules,
    /// `BTreeMap` e non `HashMap`: l'ordine di iterazione e' un contratto (D4).
    pub terrain: BTreeMap<Terrain, TerrainDef>,
    /// Indicizzato per [`BuildingKindId`].
    pub buildings: Vec<BuildingDef>,
    /// blake3 del contenuto **validato**, non dei byte dei file: riformattare
    /// un RON o aggiungere un commento non cambia l'hash, cambiare un numero
    /// si. Entra nell'hash dello stato (A2). Lo calcola [`DataSet::new`].
    pub hash: [u8; 32],
}

impl DataSet {
    /// Costruisce e calcola l'hash canonico. L'unico modo di ottenere un
    /// `DataSet`: cosi' l'hash non puo' essere fuori sincrono col contenuto.
    pub fn new(
        rules: Rules,
        terrain: BTreeMap<Terrain, TerrainDef>,
        buildings: Vec<BuildingDef>,
    ) -> Self {
        let hash = canonical_hash(&rules, &terrain, &buildings);
        Self {
            rules,
            terrain,
            buildings,
            hash,
        }
    }

    pub fn def(&self, kind: BuildingKindId) -> Option<&BuildingDef> {
        self.buildings.get(usize::from(kind.get()))
    }

    /// Risolve l'id testuale usato nelle tabelle e negli scenari.
    pub fn kind_by_id(&self, id: &str) -> Option<BuildingKindId> {
        let pos = self.buildings.iter().position(|b| b.id == id)?;
        u16::try_from(pos).ok().map(BuildingKindId::new)
    }

    pub fn terrain(&self, t: Terrain) -> Option<&TerrainDef> {
        self.terrain.get(&t)
    }

    /// Hash in esadecimale, per i messaggi di errore e gli header di replay.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// I provider di cibo che dichiarano piu' capacita' di quanta la loro
    /// produzione ne sostenga.
    ///
    /// **Perche' e' un controllo e non una convenzione nei commenti.** Se un
    /// provider di cibo puo' assegnarsi piu' abitanti di quanti ne sfami, le
    /// case in eccedenza restano assegnate a lui — la contesa la vince il
    /// primo provider — e non mangiano piu': la fame diventa uno stato
    /// **assorbente**, che nemmeno costruire una seconda fattoria scioglie.
    /// Con questo controllo verde vale invece l'implicazione inversa, ed e' un
    /// invariante del gioco: *una casa coperta dal servizio cibo mangia
    /// sempre*.
    ///
    /// Il conto e' sul caso peggiore, giacenza a zero all'inizio del tick:
    /// cio' che il provider puo' distribuire e' il minore fra la produzione di
    /// un tick e quanto il granaio riesce a tenere.
    ///
    /// **Vive quanto A5.** E' la regola giusta finche' la fattoria produce
    /// nella propria giacenza; in M3 la merce arrivera' da un magazzino con
    /// walker logistici reali (D3), la capacita' smettera' di dipendere dalla
    /// produzione locale e questo controllo va tolto insieme alla
    /// semplificazione che presidia.
    pub fn capacita_cibo_insostenibile(&self) -> Vec<CapacitaInsostenibile> {
        let mut out = Vec::new();
        let consumo = self.rules.consumo_cibo_per_abitante.to_millis();
        if consumo <= 0 {
            // Cibo gratis: qualunque capacita' e' sostenibile. Non e' questo
            // controllo a dire che il dataset non ha senso.
            return out;
        }

        for (building, def) in self.buildings.iter().enumerate() {
            let Some(servizio) = def.servizio.as_ref() else {
                continue;
            };
            if servizio.kind != ServiceKind::Cibo {
                continue;
            }
            let (Some(produzione), Some(giacenza_max)) =
                (def.produzione_per_tick, def.giacenza_max)
            else {
                continue;
            };

            let distribuibile = produzione.to_millis().min(giacenza_max.to_millis());
            let sostenibili = u16::try_from(distribuibile / consumo).unwrap_or(u16::MAX);
            for (i, &capacita) in servizio.capacita_per_livello.iter().enumerate() {
                if capacita > sostenibili {
                    out.push(CapacitaInsostenibile {
                        building,
                        livello: u8::try_from(i + 1).unwrap_or(u8::MAX),
                        capacita,
                        sostenibili,
                    });
                }
            }
        }
        out
    }
}

/// Un provider di cibo la cui capacita' supera cio' che la sua produzione
/// sostiene. Lo trova [`DataSet::capacita_cibo_insostenibile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacitaInsostenibile {
    /// Indice in [`DataSet::buildings`].
    pub building: usize,
    /// Livello a cui la capacita' sfora, da 1.
    pub livello: u8,
    /// Abitanti dichiarati in tabella.
    pub capacita: u16,
    /// Abitanti che la produzione sostiene davvero.
    pub sostenibili: u16,
}
