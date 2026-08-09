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

/// Insieme dei tile gia' visitati da un BFS, riusabile fra una chiamata e
/// l'altra.
///
/// Non e' stato e non e' una struttura derivata: e' un appunto temporaneo, e
/// vive nel chiamante. Esiste perche' `bfs_strade` allocava un buffer grande
/// quanto la griglia **a ogni chiamata**, cioe' una volta per provider: 156 KB
/// per 1.219 provider alla scala di riferimento, su memoria che il BFS poi
/// tocca all'1%.
///
/// La marca di generazione evita di doverlo ripulire: "visitato" significa
/// `epoche[i] == corrente`, quindi una voce rimasta dal giro precedente e'
/// invisibile **per costruzione**. L'alternativa — ripulire i soli tile toccati
/// — costa meno memoria ma dipende dall'invariante "marcati ≡ visitati", che
/// oggi vale e domani potrebbe non valere piu' per una modifica innocua al
/// ciclo qui sotto. E sarebbe un bug che il test di equivalenza **non**
/// coglierebbe, perche' anche `calcola_da_zero` userebbe uno scratch condiviso
/// e sbaglierebbe allo stesso modo: l'oracolo diventerebbe cieco proprio su
/// questa classe.
#[derive(Debug, Clone, Default)]
pub struct Visitati {
    epoche: Vec<u32>,
    corrente: u32,
}

impl Visitati {
    pub fn nuovo(tiles: u32) -> Self {
        Self {
            epoche: vec![0; tiles as usize],
            corrente: 0,
        }
    }

    /// Apre un giro nuovo: da qui in poi nessun tile risulta visitato.
    fn apri(&mut self, tiles: usize) {
        if self.epoche.len() != tiles {
            self.epoche.clear();
            self.epoche.resize(tiles, 0);
            self.corrente = 0;
        }
        // Al wrap si riazzera e si riparte da 1: `corrente` non e' mai 0, che
        // e' il valore con cui l'array nasce.
        self.corrente = match self.corrente.checked_add(1) {
            Some(c) => c,
            None => {
                self.epoche.fill(0);
                1
            }
        };
    }

    fn visto(&self, i: usize) -> bool {
        self.epoche.get(i) == Some(&self.corrente)
    }

    fn segna(&mut self, i: usize) {
        if let Some(e) = self.epoche.get_mut(i) {
            *e = self.corrente;
        }
    }
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
///
/// `visitati` viene aperto in un giro nuovo a ogni chiamata: il suo contenuto
/// precedente non influenza il risultato, e passarne uno gia' usato e' il modo
/// previsto di usarlo.
pub fn bfs_strade(
    grid: &Grid,
    start: &[TileIdx],
    max: u16,
    visitati: &mut Visitati,
    mut visita: impl FnMut(TileIdx, u16),
) {
    visitati.apri(grid.len() as usize);

    let mut livello: Vec<TileIdx> = start
        .iter()
        .copied()
        .filter(|t| e_strada(grid, *t))
        .collect();
    livello.sort_unstable();
    livello.dedup();
    for t in &livello {
        visitati.segna(t.as_usize());
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
                if e_strada(grid, v) && !visitati.visto(v.as_usize()) {
                    visitati.segna(v.as_usize());
                    prossimo.push(v);
                }
            }
        }
        prossimo.sort_unstable();
        std::mem::swap(&mut livello, &mut prossimo);
        d += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{Visitati, bfs_strade};
    use crate::grid::{Grid, Terrain};
    use crate::ids::{TileIdx, TilePos};

    /// Griglia 8x8 con una strada orizzontale sulla riga `y`.
    fn con_strada(y: u8) -> Grid {
        let mut g = Grid::new(8, 8, Terrain::Pianura).expect("griglia valida");
        for x in 0..8u8 {
            let idx = g.idx(TilePos::new(x, y)).expect("in mappa");
            if let Some(t) = g.get_mut(idx) {
                t.flags.set_road(true);
            }
        }
        g
    }

    fn raggiunti(grid: &Grid, da: TileIdx, max: u16, v: &mut Visitati) -> Vec<(TileIdx, u16)> {
        let mut out = Vec::new();
        bfs_strade(grid, &[da], max, v, |t, d| out.push((t, d)));
        out
    }

    /// **Il test che presidia il modo in cui lo scratch riusato puo' rompersi.**
    ///
    /// Se un giro lasciasse tracce visibili al successivo, il secondo BFS
    /// crederebbe gia' visitati dei tile che non lo sono e ne salterebbe una
    /// parte. Non ci sarebbe nessun panic: solo una copertura silenziosamente
    /// piu' piccola. E `equivalenza_copertura` non lo coglierebbe, perche'
    /// anche `calcola_da_zero` riusa uno scratch fra i suoi provider e
    /// sbaglierebbe allo stesso modo — l'oracolo sarebbe cieco su questa
    /// classe, e questo test e' cio' che la copre.
    #[test]
    fn riusare_lo_scratch_da_lo_stesso_risultato_di_uno_nuovo() {
        let grid = con_strada(3);
        let a = grid.idx(TilePos::new(0, 3)).expect("in mappa");
        let b = grid.idx(TilePos::new(7, 3)).expect("in mappa");

        let atteso_a = raggiunti(&grid, a, 4, &mut Visitati::nuovo(grid.len()));
        let atteso_b = raggiunti(&grid, b, 4, &mut Visitati::nuovo(grid.len()));
        assert!(!atteso_a.is_empty() && !atteso_b.is_empty());

        // Gli stessi due BFS, in fila sullo stesso scratch, e per tre giri:
        // uno solo non distinguerebbe "pulisce" da "non sporca ancora".
        let mut riusato = Visitati::nuovo(grid.len());
        for giro in 0..3 {
            assert_eq!(
                raggiunti(&grid, a, 4, &mut riusato),
                atteso_a,
                "giro {giro}"
            );
            assert_eq!(
                raggiunti(&grid, b, 4, &mut riusato),
                atteso_b,
                "giro {giro}"
            );
        }
    }

    /// Il wrap del contatore di generazione non rende visibili le tracce
    /// vecchie. Senza il riazzeramento all'overflow, l'epoca tornerebbe su un
    /// valore gia' scritto nell'array e dei tile risulterebbero visitati.
    #[test]
    fn il_wrap_del_contatore_non_resuscita_le_tracce() {
        let grid = con_strada(3);
        let a = grid.idx(TilePos::new(0, 3)).expect("in mappa");
        let atteso = raggiunti(&grid, a, 4, &mut Visitati::nuovo(grid.len()));

        let mut al_limite = Visitati::nuovo(grid.len());
        // Un giro vero, per lasciare una marca nell'array...
        let _ = raggiunti(&grid, a, 4, &mut al_limite);
        // ...poi si porta il contatore a un passo dal wrap.
        al_limite.corrente = u32::MAX;
        assert_eq!(raggiunti(&grid, a, 4, &mut al_limite), atteso, "al wrap");
        assert_eq!(
            raggiunti(&grid, a, 4, &mut al_limite),
            atteso,
            "dopo il wrap"
        );
    }

    /// Uno scratch nato per una griglia diversa non va usato a caso: `apri` se
    /// ne accorge dalla lunghezza e riparte pulito.
    #[test]
    fn uno_scratch_di_taglia_sbagliata_viene_rifatto() {
        let grid = con_strada(3);
        let a = grid.idx(TilePos::new(0, 3)).expect("in mappa");
        let atteso = raggiunti(&grid, a, 4, &mut Visitati::nuovo(grid.len()));

        let mut altrui = Visitati::nuovo(4);
        assert_eq!(raggiunti(&grid, a, 4, &mut altrui), atteso);
    }
}
