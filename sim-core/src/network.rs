//! La rete stradale: componenti connesse e distanze percorse.
//!
//! Struttura **derivata**: ricostruibile in qualunque momento dalla `Grid`.
//! Non entra nell'hash canonico dello stato — se ci entrasse, un bug nella
//! ricostruzione si mostrerebbe come divergenza di hash invece che come test
//! di equivalenza fallito, e la divergenza non direbbe *dove* e' il problema.
//!
//! Ogni tile strada costa 1: nessun costo di attraversamento variabile, nessun
//! livello di strada, nessun senso di marcia. Sono cose di M1 o oltre.

use crate::grid::Grid;
use crate::ids::TileIdx;

/// Identificatore di componente connessa: il `TileIdx` **minimo** tra i suoi
/// tile.
///
/// Non un contatore incrementale. Costa uguale — la scansione e' gia' in
/// ordine crescente — e rende l'etichettatura una funzione del solo insieme
/// di strade, non della storia degli inserimenti. Senza, due partite che
/// costruiscono le stesse strade in ordine diverso avrebbero stati diversi.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ComponentId(TileIdx);

impl ComponentId {
    pub const fn tile(self) -> TileIdx {
        self.0
    }
}

/// Etichettatura delle strade in componenti connesse.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoadNetwork {
    /// Per ogni tile: la componente, o `None` se non e' strada.
    ///
    /// `Option<ComponentId>` costa 4 byte per tile invece di 2, cioe' 256 KB
    /// sulla mappa massima. E' una struttura derivata e diagnostica, non lo
    /// stato: il budget stretto e' quello di `Tile`, non questo.
    component: Vec<Option<ComponentId>>,
    /// Quante ricostruzioni complete sono state eseguite. Serve ai test del
    /// dirty flag, non al gioco.
    rebuilds: u32,
}

impl RoadNetwork {
    pub fn new(tiles: u32) -> Self {
        Self {
            component: vec![None; tiles as usize],
            rebuilds: 0,
        }
    }

    pub fn component(&self, idx: TileIdx) -> Option<ComponentId> {
        self.component.get(idx.as_usize()).copied().flatten()
    }

    pub const fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    pub fn e_strada(&self, idx: TileIdx) -> bool {
        self.component(idx).is_some()
    }

    /// Due tile strada sono connessi se stanno nella stessa componente.
    pub fn connessi(&self, a: TileIdx, b: TileIdx) -> bool {
        match (self.component(a), self.component(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    /// Numero di componenti distinte.
    pub fn n_componenti(&self) -> usize {
        let mut viste: Vec<ComponentId> = self.component.iter().flatten().copied().collect();
        viste.sort_unstable();
        viste.dedup();
        viste.len()
    }

    /// Rietichetta tutto da zero, scandendo i tile in ordine di `TileIdx`
    /// crescente.
    ///
    /// Ricostruzione completa e non incrementale, deliberatamente: 40.000 tile
    /// scanditi una volta sono irrilevanti finche' non lo dice un profiler, e
    /// il dirty flag evita gia' di farlo a ogni tick. Cio' che serviva subito
    /// era il *flag*, non l'algoritmo furbo.
    pub(crate) fn rebuild(&mut self, grid: &Grid) {
        self.component.clear();
        self.component.resize(grid.len() as usize, None);
        self.rebuilds = self.rebuilds.wrapping_add(1);

        let mut coda: Vec<TileIdx> = Vec::new();
        for seme in grid.indices() {
            if !e_strada(grid, seme) || self.component(seme).is_some() {
                continue;
            }
            // Il seme e' il primo tile non ancora etichettato in ordine
            // crescente, quindi e' il minimo della sua componente: l'id
            // canonico esce dalla scansione, senza un secondo passaggio.
            let id = ComponentId(seme);
            self.component[seme.as_usize()] = Some(id);
            coda.clear();
            coda.push(seme);
            while let Some(t) = coda.pop() {
                for v in grid.neighbors4(t) {
                    if e_strada(grid, v) && self.component(v).is_none() {
                        self.component[v.as_usize()] = Some(id);
                        coda.push(v);
                    }
                }
            }
        }
    }
}

fn e_strada(grid: &Grid, idx: TileIdx) -> bool {
    grid.get(idx).is_some_and(|t| t.flags.has_road())
}

/// BFS troncato sulla rete stradale.
///
/// Parte dai tile in `start` a distanza 0 e visita solo tile strada, fermandosi
/// oltre `max`. Il troncamento non e' un'ottimizzazione opzionale: senza, un
/// raggio 12 su una citta' grande visiterebbe tutta la rete.
///
/// `visita` riceve ogni tile raggiunto **una sola volta**, con la distanza
/// minima. L'ordine di visita e' per distanza crescente e, a parita' di
/// distanza, per `TileIdx` crescente: e' un ordine totale, e chi ci costruisce
/// sopra una regola di gioco (fase 06) non dipende da dettagli del BFS.
pub fn bfs_strade(grid: &Grid, start: &[TileIdx], max: u16, mut visita: impl FnMut(TileIdx, u16)) {
    let mut distanza: Vec<Option<u16>> = vec![None; grid.len() as usize];

    let mut livello: Vec<TileIdx> = start
        .iter()
        .copied()
        .filter(|t| e_strada(grid, *t))
        .collect();
    livello.sort_unstable();
    livello.dedup();
    for t in &livello {
        distanza[t.as_usize()] = Some(0);
    }

    let mut d = 0u16;
    let mut prossimo: Vec<TileIdx> = Vec::new();
    while !livello.is_empty() {
        for t in &livello {
            visita(*t, d);
        }
        if d == max {
            break;
        }
        prossimo.clear();
        for t in &livello {
            for v in grid.neighbors4(*t) {
                if e_strada(grid, v) && distanza[v.as_usize()].is_none() {
                    distanza[v.as_usize()] = Some(d + 1);
                    prossimo.push(v);
                }
            }
        }
        prossimo.sort_unstable();
        std::mem::swap(&mut livello, &mut prossimo);
        d += 1;
    }
}
