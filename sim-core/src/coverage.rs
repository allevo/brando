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

use slotmap::SecondaryMap;

use crate::ids::{BuildingId, HouseId, TileIdx};
use crate::network::{Visitati, bfs_strade};
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

/// Da tile strada alle case che vi si affacciano.
///
/// Forma CSR: le case del tile `t` sono `case[offsets[t]..offsets[t + 1]]`.
/// `TileIdx` e' un indice **denso** sulla griglia, quindi indicizzarlo
/// direttamente costa un accesso, contro i ~13 confronti con pointer chasing di
/// un `BTreeMap`. Non e' un dettaglio: e' l'operazione piu' frequente dell'intero
/// ricalcolo — una per tile raggiunto, per ogni provider, cioe' ~100.000 volte
/// alla scala di riferimento.
///
/// **Al massimo quattro case per tile**, perche' un tile ha quattro vicini
/// ortogonali (`Grid::neighbors4`) e ognuno ospita al piu' un occupante. Il
/// limite regge anche se in M1 le case diventeranno piu' grandi di 1x1: viene
/// dai vicini del tile, non dalla dimensione della casa.
struct CasePerTile {
    /// Lungo `grid.len() + 1`.
    offsets: Vec<u32>,
    /// Le case, raggruppate per tile e in ordine di `HouseId` dentro il gruppo.
    case: Vec<HouseId>,
}

impl CasePerTile {
    /// Counting sort in tre passate: conta, somma i prefissi, riempi.
    ///
    /// Lineare nel numero di tile piu' quello degli ingressi, senza nessun
    /// confronto — contro l'ordinamento implicito di un `BTreeMap`, che
    /// pagherebbe `log n` a ogni inserimento e allocherebbe un nodo per tile.
    fn nuova(world: &World) -> Self {
        let tiles = world.grid().len() as usize;
        let mut offsets = vec![0u32; tiles + 1];

        // Le coppie si raccolgono in ordine di `HouseId`: e' quell'ordine che
        // si ritrova poi dentro ogni gruppo.
        let mut coppie: Vec<(TileIdx, HouseId)> = Vec::new();
        let mut ingressi = Vec::new();
        for (id, _) in world.houses() {
            world.ingressi_casa_in(id, &mut ingressi);
            for t in &ingressi {
                coppie.push((*t, id));
                offsets[t.as_usize()] += 1;
            }
        }

        // Somma prefissa esclusiva: `offsets[t]` diventa l'inizio del gruppo.
        let mut somma = 0u32;
        for o in &mut offsets {
            let conteggio = *o;
            *o = somma;
            somma += conteggio;
        }

        // Riempimento in avanti, con `offsets[t]` che fa da cursore. Alla fine
        // ogni cursore e' arrivato sulla fine del proprio gruppo, cioe'
        // sull'inizio del successivo: basta traslare di uno a destra.
        let mut case = vec![HouseId::default(); coppie.len()];
        for (t, h) in coppie {
            let i = t.as_usize();
            if let Some(slot) = case.get_mut(offsets[i] as usize) {
                *slot = h;
            }
            offsets[i] += 1;
        }
        for i in (1..=tiles).rev() {
            offsets[i] = offsets[i - 1];
        }
        offsets[0] = 0;

        Self { offsets, case }
    }

    fn get(&self, t: TileIdx) -> &[HouseId] {
        let i = t.as_usize();
        let (Some(&da), Some(&a)) = (self.offsets.get(i), self.offsets.get(i + 1)) else {
            return &[];
        };
        self.case.get(da as usize..a as usize).unwrap_or(&[])
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
    // Mappa inversa tile strada -> case che vi si affacciano. Costruita una
    // volta per ricalcolo invece che una volta per provider.
    let case_per_tile = CasePerTile::nuova(world);

    // Le assegnazioni si accumulano qui, non nella `Coverage`, e per una
    // ragione di costo: durante il calcolo vanno **interrogate** una volta per
    // candidata di ogni provider — ~100.000 volte alla scala di riferimento,
    // perche' `scelte_entro_capacita` scorre tutte le candidate e non si ferma
    // alla saturazione (salta chi non ci sta, A5). Un `BTreeMap` con 3.750
    // chiavi pagherebbe ~13 confronti ogni volta; una `SecondaryMap` e' densa
    // sull'indice di slot, quindi costa un accesso. La `Coverage` si
    // materializza alla fine, una volta sola.
    let mut assegnate: SecondaryMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]> =
        SecondaryMap::new();
    // Marca temporale per candidata, per non contare due volte una casa che il
    // BFS raggiunge da piu' tile: il valore e' l'indice del provider corrente,
    // cosi' non serve ripulire fra un provider e l'altro.
    let mut vista: SecondaryMap<HouseId, u32> = SecondaryMap::new();
    let mut ordinate: Vec<(u16, TileIdx, HouseId)> = Vec::new();
    let mut ingressi: Vec<TileIdx> = Vec::new();
    // Uno solo per ricalcolo, riusato da tutti i provider: allocarlo per
    // provider era l'ultimo costo per-provider proporzionale alla mappa.
    let mut visitati = Visitati::nuovo(world.grid().len());

    // I provider si scorrono in ordine di BuildingId: e' l'ordine che decide
    // chi vince una casa contesa, quindi e' semantica di gioco.
    //
    // **Dipendenza da tenere d'occhio.** Che l'iterazione dello `SlotMap`
    // coincida con l'ordine crescente delle chiavi e' vero oggi — `KeyData`
    // deriva `Ord` con `idx` prima di `version`, e l'iterazione scorre gli
    // slot per indice — ma `slotmap` documenta l'ordine delle chiavi come
    // *unspecified*. Un aggiornamento della dipendenza potrebbe quindi
    // cambiare quale provider vince una casa contesa. Non sarebbe silenzioso:
    // i golden divergerebbero, ed e' esattamente il caso descritto dal
    // messaggio di `gli_hash_coincidono_con_quelli_committati` — hash diversi
    // senza che il bilanciamento sia cambiato significa fermarsi e cercare la
    // fonte, non rigenerare.
    for (epoca, (provider, b)) in world.buildings().enumerate() {
        let epoca = epoca as u32;
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

        world.ingressi_edificio_in(provider, &mut ingressi);
        if ingressi.is_empty() {
            // Un provider non agganciato alla rete non serve nessuno: una casa
            // non adiacente a nessuna strada non e' raggiungibile, a qualunque
            // distanza.
            continue;
        }

        // Candidate, con la distanza minima a cui sono state raggiunte.
        //
        // Il minimo non va cercato: `bfs_strade` visita per distanza crescente,
        // quindi **il primo avvistamento di una casa e' gia' il suo minimo**.
        // Basta ignorare gli avvistamenti successivi, che arrivano quando la
        // casa si affaccia su piu' di un tile raggiunto.
        ordinate.clear();
        bfs_strade(world.grid(), &ingressi, raggio, &mut visitati, |tile, d| {
            for h in case_per_tile.get(tile) {
                if vista.get(*h) == Some(&epoca) {
                    continue;
                }
                vista.insert(*h, epoca);
                let Some(casa) = world.house(*h) else {
                    continue;
                };
                let Some(idx) = world.grid().idx(casa.origin) else {
                    continue;
                };
                ordinate.push((d, idx, *h));
            }
        });

        // **Ordine di priorita'**, semantica di gioco: quando le candidate
        // superano la capacita' si servono le piu' vicine; a parita' di
        // distanza vince il `TileIdx` minore. E' un ordine totale — senza il
        // secondo criterio due case equidistanti sarebbero ordinate
        // dall'ordine di visita del BFS, cioe' da un dettaglio implementativo,
        // e il golden replay diventerebbe fragile.
        ordinate.sort_unstable();

        // Contesa: in M0 la casa e' servita e basta, vince il primo provider
        // nell'ordine di iterazione. Le case gia' prese si tolgono **prima**
        // del riempimento, non dentro: una casa contesa non deve consumare la
        // capacita' di chi arriva secondo.
        let libere = ordinate.iter().filter_map(|(_, _, h)| {
            let presa = assegnate
                .get(*h)
                .is_some_and(|servizi| servizi[kind.index()].is_some());
            if presa {
                return None;
            }
            Some((*h, world.house(*h)?.abitanti))
        });
        let scelte: Vec<HouseId> = scelte_entro_capacita(libere, capacita).collect();

        for h in scelte {
            if assegnate.get(h).is_none() {
                assegnate.insert(h, [None; ServiceKind::COUNT]);
            }
            if let Some(servizi) = assegnate.get_mut(h) {
                servizi[kind.index()] = Some(provider);
            }
        }
    }

    // Materializzazione finale. Entrano **solo** le case con almeno un
    // servizio: e' la stessa condizione di prima, quando la voce nasceva
    // dall'`entry().or_default()` dentro il ciclo delle scelte. Inserire anche
    // le case senza servizi cambierebbe l'insieme delle chiavi restituito da
    // `assegnazioni()` e `case_servite_da()` — nessun test lo coglierebbe,
    // perche' cambierebbero entrambi i lati del confronto.
    let mut cov = Coverage::default();
    for (h, servizi) in &assegnate {
        if servizi.iter().any(Option::is_some) {
            cov.served_by.insert(h, *servizi);
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
/// sedici, e la capacita' in tabella smetterebbe di dire il vero. Per un
/// produttore quel divario romperebbe anche la coerenza fra capacita' e
/// produzione che [`capacita_cibo_insostenibile`] presidia. Peggio, una casa
/// grande costruita vicino al provider taglierebbe fuori *tutte* quelle oltre,
/// pur restando dei posti liberi.
///
/// La distanza resta comunque l'ordine di priorita': nessuna candidata viene
/// scavalcata per preferenza, solo per impossibilita'.
///
/// [`capacita_cibo_insostenibile`]: crate::data::DataSet::capacita_cibo_insostenibile
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
    //
    // Lo split dei campi serve a scrivere le case mentre si legge la
    // copertura: sono campi disgiunti dello stesso `World`, ma il borrow
    // checker lo vede solo se glieli si nomina separatamente. L'alternativa —
    // raccogliere i flag in un `Vec` e riscorrerlo — costava una passata e
    // un'allocazione per ogni ricalcolo.
    let World {
        houses, coverage, ..
    } = world;
    for (_, h) in houses.iter_mut() {
        h.servita = crate::service::ServiceFlags::empty();
    }
    for (id, servizi) in coverage.assegnazioni() {
        let Some(h) = houses.get_mut(*id) else {
            continue;
        };
        for k in ServiceKind::TUTTI {
            h.servita.set(k, servizi[k.index()].is_some());
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
