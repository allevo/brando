//! Copertura aggregata dei servizi (D2).
//!
//! Un provider serve le case entro un raggio misurato **sulla rete stradale**,
//! fino a esaurire una capacita' contata in **abitanti serviti**
//! ([`scelte_entro_capacita`]). Non ci sono walker: i portatori d'acqua che il
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

        // **Ordine di priorita'**, semantica di gioco: quando le candidate
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

        // Contesa: in M0 la casa e' servita e basta, vince il primo provider
        // nell'ordine di iterazione. Le case gia' prese si tolgono **prima**
        // del riempimento, non dentro: una casa contesa non deve consumare la
        // capacita' di chi arriva secondo.
        let libere = ordinate
            .iter()
            .filter(|(_, _, h)| cov.provider(*h, kind).is_none())
            .filter_map(|(_, _, h)| Some((*h, world.house(*h)?.abitanti)));
        let scelte: Vec<HouseId> = scelte_entro_capacita(libere, capacita).collect();

        for h in scelte {
            cov.served_by.entry(h).or_default()[kind.index()] = Some(provider);
        }
    }

    cov
}

/// Riempie la capacita' di un provider scorrendo le candidate **gia' ordinate
/// per priorita'**, e restituisce quelle servite.
///
/// La capacita' si conta in **abitanti**, non in case: con i livelli delle
/// case (M1) la popolazione varia da casa a casa, e una capacita' in case non
/// direbbe piu' quanta gente il provider riesce davvero a servire.
///
/// **Semantica di gioco, due regole in una.**
///
/// *Niente assegnazione parziale.* Una casa entra con tutti i suoi abitanti o
/// non entra: mezza casa servita non e' uno stato che il gioco sappia
/// rappresentare. E' la stessa scelta del consumo di cibo (fase 07).
///
/// *Chi non ci sta viene saltato, non fa da barriera.* Se restano due posti e
/// la candidata ne chiede quattro, la scansione prosegue con la successiva,
/// che puo' essere piu' lontana. L'alternativa — fermarsi alla prima che non
/// ci sta — terrebbe la distanza come priorita' assoluta, ma lascerebbe posti
/// inutilizzati: un provider dichiarato per venti abitanti ne servirebbe
/// sedici, e la capacita' in tabella smetterebbe di dire il vero. Peggio, una
/// casa grande costruita vicino al provider taglierebbe fuori *tutte* quelle
/// oltre, pur restando dei posti liberi.
///
/// La distanza resta comunque l'ordine di priorita': nessuna candidata viene
/// scavalcata per preferenza, solo per impossibilita'.
fn scelte_entro_capacita<T>(
    candidate: impl IntoIterator<Item = (T, u16)>,
    capacita: u16,
) -> impl Iterator<Item = T> {
    let mut residua = capacita;
    candidate.into_iter().filter_map(move |(id, abitanti)| {
        residua = residua.checked_sub(abitanti)?;
        Some(id)
    })
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

#[cfg(test)]
mod tests {
    use super::scelte_entro_capacita;

    /// La regola di riempimento, riga per riga.
    ///
    /// In M0 tutte le case hanno gli stessi abitanti, quindi le righe con
    /// popolazioni miste non sono ancora osservabili in una partita: la scelta
    /// va fissata **adesso** perche' e' oggi che la si puo' fare senza
    /// rigenerare niente, e i livelli delle case (M1) la renderanno visibile.
    /// Capacita', candidate in ordine di priorita' come `(id, abitanti)`, e
    /// gli id che devono risultare serviti.
    type Caso = (u16, &'static [(u8, u16)], &'static [u8]);

    #[test]
    fn il_riempimento_salta_chi_non_ci_sta_invece_di_fermarsi() {
        let casi: &[Caso] = &[
            (0, &[(1, 4)], &[]),
            (8, &[(1, 4), (2, 4)], &[1, 2]),
            // Oltre la capacita' non si serve nessuno.
            (8, &[(1, 4), (2, 4), (3, 4)], &[1, 2]),
            // Niente parziale: restano due posti, la candidata ne chiede
            // quattro e resta fuori intera.
            (6, &[(1, 4), (2, 4)], &[1]),
            // Chi non ci sta viene saltato: la terza, piu' lontana ma piu'
            // piccola, prende i due posti che la seconda non poteva usare.
            (6, &[(1, 4), (2, 4), (3, 2)], &[1, 3]),
            // E la scansione prosegue anche dopo piu' salti di fila.
            (6, &[(1, 4), (2, 4), (3, 3), (4, 1), (5, 1)], &[1, 4, 5]),
            // Una candidata piu' grande dell'intera capacita' non blocca nulla.
            (4, &[(1, 9), (2, 4)], &[2]),
        ];

        for (capacita, candidate, servite) in casi {
            let scelte: Vec<u8> =
                scelte_entro_capacita(candidate.iter().copied(), *capacita).collect();
            assert_eq!(
                scelte, *servite,
                "capacita' {capacita}, candidate {candidate:?}"
            );
        }
    }
}
