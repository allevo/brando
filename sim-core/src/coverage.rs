//! Copertura aggregata dei servizi (D2).
//!
//! Un provider serve le case entro un raggio misurato **sulla rete stradale**,
//! con capacita' limitata. Non ci sono walker: i portatori d'acqua che il
//! giocatore vedra' camminare sono decorativi, vivono nel renderer e sono
//! derivati da questa struttura. **Se un `Walker` compare in questo modulo,
//! e' un bug.**
//!
//! Come `RoadNetwork`, `Coverage` e' derivata e non entra nell'hash canonico
//! dello stato: cio' che la presidia e' il test di equivalenza
//! incrementale/da-zero, che dice anche *dove* e' il problema.

use std::collections::BTreeMap;

use crate::ids::{BuildingId, HouseId, TileIdx};
use crate::network::bfs_strade;
use crate::service::ServiceKind;
use crate::world::World;

/// Per ogni casa e per ogni servizio, chi la serve.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Coverage {
    served_by: BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]>,
    /// Quante volte la copertura e' stata ricalcolata. Serve ai test del
    /// dirty flag, non al gioco.
    ricalcoli: u32,
}

impl Coverage {
    pub fn provider(&self, house: HouseId, kind: ServiceKind) -> Option<BuildingId> {
        self.served_by.get(&house)?[kind.index()]
    }

    pub fn e_servita(&self, house: HouseId, kind: ServiceKind) -> bool {
        self.provider(house, kind).is_some()
    }

    pub const fn ricalcoli(&self) -> u32 {
        self.ricalcoli
    }

    /// Le assegnazioni, senza il contatore diagnostico: e' cio' che va
    /// confrontato tra copertura incrementale e copertura da zero.
    pub const fn assegnazioni(
        &self,
    ) -> &BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]> {
        &self.served_by
    }

    /// Le case servite da un provider, in ordine di `HouseId`.
    pub fn case_servite_da(&self, provider: BuildingId) -> Vec<HouseId> {
        self.served_by
            .iter()
            .filter(|(_, s)| s.contains(&Some(provider)))
            .map(|(h, _)| *h)
            .collect()
    }
}

/// Ricalcola la copertura da zero sullo stato corrente.
///
/// In M0 il passo 3 del tick chiama proprio questa, per tutti i provider,
/// ogni volta che `dirty.coverage` non e' vuoto: `CLAUDE.md` autorizza
/// l'ingenuita' qui, purche' i flag esistano. Cio' che il dirty flag protegge
/// oggi non e' il costo del ricalcolo ma la sua **assenza** quando nulla e'
/// cambiato; e cio' che il test di equivalenza coglie e' un'invalidazione
/// dimenticata, non un errore di algoritmo.
pub fn calcola_da_zero(world: &World) -> Coverage {
    let mut cov = Coverage::default();

    // Mappa inversa tile strada -> case che vi si affacciano. Costruita una
    // volta per ricalcolo invece che una volta per provider.
    let mut case_per_tile: BTreeMap<TileIdx, Vec<HouseId>> = BTreeMap::new();
    for (id, _) in world.houses() {
        for t in world.ingressi_casa(id) {
            case_per_tile.entry(t).or_default().push(id);
        }
    }

    // I provider si scorrono in ordine di BuildingId: e' l'ordine che decide
    // chi vince una casa contesa, quindi e' semantica di gioco.
    for (provider, b) in world.buildings() {
        let Some(def) = world.data().def(b.kind) else {
            continue;
        };
        let Some(servizio) = def.servizio.as_ref() else {
            continue;
        };
        let (Some(raggio), Some(capacita)) = (servizio.raggio(b.level), servizio.capacita(b.level))
        else {
            continue;
        };
        let kind = servizio.kind;

        let ingressi = world.ingressi_edificio(provider);
        if ingressi.is_empty() {
            // Un provider non agganciato alla rete non serve nessuno: una casa
            // non adiacente a nessuna strada non e' raggiungibile, a qualunque
            // distanza.
            continue;
        }

        // Candidate: casa -> distanza minima a cui e' stata raggiunta.
        let mut candidate: BTreeMap<HouseId, u16> = BTreeMap::new();
        bfs_strade(world.grid(), &ingressi, raggio, |tile, d| {
            let Some(case) = case_per_tile.get(&tile) else {
                return;
            };
            for h in case {
                candidate
                    .entry(*h)
                    .and_modify(|best| {
                        if d < *best {
                            *best = d;
                        }
                    })
                    .or_insert(d);
            }
        });

        // **Regola di assegnazione**, semantica di gioco: quando le candidate
        // superano la capacita' si servono le piu' vicine; a parita' di
        // distanza vince il `TileIdx` minore. E' un ordine totale — senza il
        // secondo criterio due case equidistanti sarebbero ordinate
        // dall'ordine di visita del BFS, cioe' da un dettaglio implementativo,
        // e il golden replay diventerebbe fragile.
        let mut ordinate: Vec<(u16, TileIdx, HouseId)> = candidate
            .into_iter()
            .filter_map(|(h, d)| {
                let origin = world.house(h)?.origin;
                let idx = world.grid().idx(origin)?;
                Some((d, idx, h))
            })
            .collect();
        ordinate.sort_unstable();

        let mut assegnate = 0u16;
        for (_, _, h) in ordinate {
            if assegnate >= capacita {
                break;
            }
            let slot = &mut cov.served_by.entry(h).or_default()[kind.index()];
            if slot.is_some() {
                // Contesa: in M0 la casa e' servita e basta, vince il primo
                // provider nell'ordine di iterazione. Semplificazione da
                // riaprire se in M1 la capacita' diventera' "abitanti
                // serviti" invece di "case servite".
                continue;
            }
            *slot = Some(provider);
            assegnate += 1;
        }
    }

    cov
}

/// Passo 3 del tick.
pub(crate) fn propagate_coverage(world: &mut World) {
    if !world.dirty.coverage_da_rivedere() {
        return;
    }
    let ricalcoli = world.coverage.ricalcoli;
    world.coverage = calcola_da_zero(world);
    world.coverage.ricalcoli = ricalcoli.wrapping_add(1);
    world.dirty.coverage.clear();
    world.dirty.coverage_invalidata = false;

    // Le case portano una copia dei flag: e' cio' che leggeranno evoluzione e
    // degrado (M1) senza dover interrogare la `Coverage`.
    let flags: Vec<(HouseId, crate::service::ServiceFlags)> = world
        .houses()
        .map(|(id, _)| {
            let mut f = crate::service::ServiceFlags::empty();
            for k in ServiceKind::TUTTI {
                f.set(k, world.coverage.e_servita(id, k));
            }
            (id, f)
        })
        .collect();
    for (id, f) in flags {
        if let Some(h) = world.houses.get_mut(id) {
            h.servita = f;
        }
    }
}
