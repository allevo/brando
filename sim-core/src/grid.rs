//! Griglia dei tile e tipi che la compongono.
//!
//! Vincolo di memoria: `Tile` sta in 4 byte, quindi 40.000 tile occupano
//! 160 KB e restano in cache (CLAUDE.md, modello dello stato). Ogni campo
//! aggiunto qui va pesato contro quel budget, che un test presidia.

use serde::{Deserialize, Serialize};

use crate::ids::{TileIdx, TilePos};

/// Lato massimo della griglia (A4). Oltre, `TileIdx(u16)` non basterebbe.
pub const LATO_MAX: u16 = 256;

/// Tipo di terreno. I numeri associati (costruibile? costo della strada?)
/// non stanno qui: arrivano dalle tabelle di `sim-data` (D6).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Terrain {
    #[default]
    Pianura,
    Acqua,
    Roccia,
}

impl Terrain {
    /// Tutte le varianti, in ordine stabile. Serve a `sim-data` per validare
    /// che la tabella dei terreni sia completa.
    pub const TUTTI: [Terrain; 3] = [Terrain::Pianura, Terrain::Acqua, Terrain::Roccia];
}

/// Bit di stato del tile. Scritto a mano invece che con `bitflags` per non
/// aggiungere una dipendenza a `sim-core` (D1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct TileFlags(u8);

impl TileFlags {
    const HAS_ROAD: u8 = 1 << 0;
    /// Se l'occupante e' una casa; altrimenti, se presente, e' un edificio.
    const OCCUPANT_IS_HOUSE: u8 = 1 << 1;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn has_road(self) -> bool {
        self.0 & Self::HAS_ROAD != 0
    }

    pub const fn set_road(&mut self, on: bool) {
        self.set(Self::HAS_ROAD, on);
    }

    pub const fn occupant_is_house(self) -> bool {
        self.0 & Self::OCCUPANT_IS_HOUSE != 0
    }

    pub const fn set_occupant_is_house(&mut self, on: bool) {
        self.set(Self::OCCUPANT_IS_HOUSE, on);
    }

    const fn set(&mut self, bit: u8, on: bool) {
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

impl std::fmt::Debug for TileFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TileFlags(road={}, house={})",
            self.has_road(),
            self.occupant_is_house()
        )
    }
}

/// Riferimento compatto all'occupante di un tile.
///
/// Un `Option<Box<...>>` costerebbe 8 byte e una indirezione: qui c'e' un
/// `u16` con sentinella. Il tipo dell'occupante (casa o edificio) sta nei
/// [`TileFlags`]; la mappa da slot a `BuildingId`/`HouseId` sta nel `World`,
/// non nel tile.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OccupantSlot(u16);

impl OccupantSlot {
    const VUOTO: u16 = u16::MAX;

    pub const EMPTY: Self = Self(Self::VUOTO);

    /// `None` se l'indice coincide con la sentinella di "vuoto".
    pub const fn new(index: u16) -> Option<Self> {
        if index == Self::VUOTO {
            None
        } else {
            Some(Self(index))
        }
    }

    pub const fn is_empty(self) -> bool {
        self.0 == Self::VUOTO
    }

    pub const fn index(self) -> Option<u16> {
        if self.is_empty() { None } else { Some(self.0) }
    }
}

impl Default for OccupantSlot {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Budget: 4 byte. 40.000 tile ⇒ 160 KB, sta in L2.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Tile {
    pub terrain: Terrain,
    pub flags: TileFlags,
    pub occupant: OccupantSlot,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GridError {
    #[error("dimensione della griglia non valida: {width}x{height}, ammesso 1..={max} per lato")]
    DimensioneNonValida { width: u16, height: u16, max: u16 },
}

/// Griglia densa di tile, indicizzata `y * width + x`.
///
/// `width` e `height` sono `u16` e non `u8` perche' il lato massimo e' 256,
/// che in un `u8` non ci starebbe: i valori ammessi sono `1..=256`. Le
/// coordinate restano `u8` (0..=255).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Grid {
    width: u16,
    height: u16,
    tiles: Vec<Tile>,
}

impl Grid {
    /// Griglia uniforme. Rifiuta lati a 0 o oltre [`LATO_MAX`].
    pub fn new(width: u16, height: u16, terrain: Terrain) -> Result<Self, GridError> {
        if width == 0 || height == 0 || width > LATO_MAX || height > LATO_MAX {
            return Err(GridError::DimensioneNonValida {
                width,
                height,
                max: LATO_MAX,
            });
        }
        let len = usize::from(width) * usize::from(height);
        let tile = Tile {
            terrain,
            ..Tile::default()
        };
        Ok(Self {
            width,
            height,
            tiles: vec![tile; len],
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Numero di tile. `u32` e non `usize`: il massimo e' 65.536.
    pub const fn len(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    /// Sempre `false`: una griglia con un lato a zero non e' costruibile.
    /// Esiste solo per non lasciare `len()` senza il suo compagno.
    pub const fn is_empty(&self) -> bool {
        false
    }

    pub const fn in_bounds(&self, pos: TilePos) -> bool {
        (pos.x as u16) < self.width && (pos.y as u16) < self.height
    }

    /// Indice lineare della posizione, `None` se fuori dalla mappa.
    pub const fn idx(&self, pos: TilePos) -> Option<TileIdx> {
        if !self.in_bounds(pos) {
            return None;
        }
        let i = pos.y as u32 * self.width as u32 + pos.x as u32;
        // Invariante: in_bounds implica i < width*height <= 65_536, quindi i
        // sta in u16 (l'ultimo indice valido e' 65_535).
        Some(TileIdx::new(i as u16))
    }

    /// Posizione corrispondente all'indice, `None` se fuori dalla mappa.
    pub const fn pos(&self, idx: TileIdx) -> Option<TilePos> {
        let i = idx.get() as u32;
        if i >= self.len() {
            return None;
        }
        let w = self.width as u32;
        Some(TilePos::new((i % w) as u8, (i / w) as u8))
    }

    pub fn get(&self, idx: TileIdx) -> Option<&Tile> {
        self.tiles.get(idx.as_usize())
    }

    pub fn get_mut(&mut self, idx: TileIdx) -> Option<&mut Tile> {
        self.tiles.get_mut(idx.as_usize())
    }

    pub fn at(&self, pos: TilePos) -> Option<&Tile> {
        self.get(self.idx(pos)?)
    }

    pub fn at_mut(&mut self, pos: TilePos) -> Option<&mut Tile> {
        let idx = self.idx(pos)?;
        self.get_mut(idx)
    }

    /// Tutti gli indici validi, in ordine crescente. E' l'ordine di scansione
    /// canonico: la ricostruzione della rete stradale (fase 05) ci si appoggia.
    pub fn indices(&self) -> impl Iterator<Item = TileIdx> {
        (0..self.len()).map(|i| TileIdx::new(i as u16))
    }

    /// I quattro vicini ortogonali, **senza wraparound**: il vicino "a destra"
    /// di `x = width - 1` non esiste, non e' il primo della riga sotto.
    ///
    /// Ordine di emissione: nord, ovest, est, sud — cioe' `TileIdx` crescente.
    /// L'ordine e' parte del contratto di determinismo del BFS (D4).
    pub fn neighbors4(&self, idx: TileIdx) -> impl Iterator<Item = TileIdx> {
        let mut out = [None; 4];
        if let Some(p) = self.pos(idx) {
            let i = idx.get();
            if p.y > 0 {
                out[0] = Some(TileIdx::new(i - self.width));
            }
            if p.x > 0 {
                out[1] = Some(TileIdx::new(i - 1));
            }
            if u16::from(p.x) + 1 < self.width {
                out[2] = Some(TileIdx::new(i + 1));
            }
            if u16::from(p.y) + 1 < self.height {
                out[3] = Some(TileIdx::new(i + self.width));
            }
        }
        out.into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn tile_sta_nel_budget() {
        assert_eq!(
            size_of::<Tile>(),
            4,
            "Tile e' cresciuto: 40.000 tile devono stare in cache"
        );
    }

    #[test]
    fn new_rifiuta_dimensioni_fuori_range() {
        for (w, h) in [(0, 10), (10, 0), (257, 10), (10, 257)] {
            assert_eq!(
                Grid::new(w, h, Terrain::Pianura),
                Err(GridError::DimensioneNonValida {
                    width: w,
                    height: h,
                    max: LATO_MAX
                })
            );
        }
        assert!(Grid::new(256, 256, Terrain::Pianura).is_ok());
        assert!(Grid::new(1, 1, Terrain::Pianura).is_ok());
    }

    #[test]
    fn griglia_massima_indicizza_l_ultimo_tile() {
        let g = Grid::new(256, 256, Terrain::Pianura).expect("256x256 e' valida");
        assert_eq!(g.len(), 65_536);
        let ultimo = g.idx(TilePos::new(255, 255)).expect("in bounds");
        assert_eq!(ultimo.get(), u16::MAX);
        assert_eq!(g.pos(ultimo), Some(TilePos::new(255, 255)));
    }

    #[test]
    fn occupant_slot_distingue_vuoto_da_indice() {
        assert!(OccupantSlot::EMPTY.is_empty());
        assert_eq!(OccupantSlot::EMPTY.index(), None);
        assert_eq!(OccupantSlot::new(0).and_then(OccupantSlot::index), Some(0));
        assert_eq!(OccupantSlot::new(u16::MAX), None);
        assert_eq!(OccupantSlot::default(), OccupantSlot::EMPTY);
    }

    #[test]
    fn niente_wraparound_ai_bordi() {
        let g = Grid::new(4, 4, Terrain::Pianura).expect("griglia valida");
        // Il tile a destra della prima riga non e' vicino del primo della seconda.
        let destra = g.idx(TilePos::new(3, 0)).expect("in bounds");
        let sinistra_riga_dopo = g.idx(TilePos::new(0, 1)).expect("in bounds");
        let vicini: Vec<_> = g.neighbors4(destra).collect();
        assert!(!vicini.contains(&sinistra_riga_dopo));
    }

    #[test]
    fn conteggio_vicini_per_posizione() {
        let g = Grid::new(5, 4, Terrain::Pianura).expect("griglia valida");
        let n = |x, y| {
            g.neighbors4(g.idx(TilePos::new(x, y)).expect("in bounds"))
                .count()
        };
        assert_eq!(n(0, 0), 2, "angolo");
        assert_eq!(n(4, 3), 2, "angolo opposto");
        assert_eq!(n(2, 0), 3, "bordo");
        assert_eq!(n(2, 2), 4, "interno");
    }

    /// Griglie di dimensione arbitraria e una posizione valida al loro interno.
    fn griglia_e_pos() -> impl Strategy<Value = (Grid, TilePos)> {
        (1u16..=LATO_MAX, 1u16..=LATO_MAX).prop_flat_map(|(w, h)| {
            let g = Grid::new(w, h, Terrain::Pianura).expect("dimensioni in range");
            (Just(g), 0..w, 0..h).prop_map(|(g, x, y)| (g, TilePos::new(x as u8, y as u8)))
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Biiezione posizione -> indice -> posizione.
        #[test]
        fn pos_idx_e_biiettiva((g, p) in griglia_e_pos()) {
            let i = g.idx(p).expect("pos generata in bounds");
            prop_assert_eq!(g.pos(i), Some(p));
        }

        /// E nell'altro verso: indice -> posizione -> indice.
        #[test]
        fn idx_pos_e_biiettiva((g, p) in griglia_e_pos()) {
            let i = g.idx(p).expect("pos generata in bounds");
            let p2 = g.pos(i).expect("indice valido");
            prop_assert_eq!(g.idx(p2), Some(i));
        }

        /// Fuori dai bordi non esiste indice.
        #[test]
        fn fuori_bordo_niente_indice((g, _p) in griglia_e_pos()) {
            if g.width() < LATO_MAX {
                let fuori = TilePos::new(g.width() as u8, 0);
                prop_assert_eq!(g.idx(fuori), None);
                prop_assert!(!g.in_bounds(fuori));
            }
            prop_assert_eq!(g.pos(TileIdx::new(u16::MAX)).is_some(), g.len() == 65_536);
        }

        /// Ogni vicino e' in mappa e a distanza di Manhattan 1; il numero di
        /// vicini dipende solo da quanti bordi tocca il tile.
        #[test]
        fn neighbors4_non_esce_e_non_wrappa((g, p) in griglia_e_pos()) {
            let i = g.idx(p).expect("pos generata in bounds");
            let vicini: Vec<_> = g.neighbors4(i).collect();

            for &v in &vicini {
                let pv = g.pos(v).expect("vicino in mappa");
                prop_assert_eq!(p.manhattan(pv), 1);
            }

            let sul_bordo_x = u16::from(p.x) == 0 || u16::from(p.x) + 1 == g.width();
            let sul_bordo_y = u16::from(p.y) == 0 || u16::from(p.y) + 1 == g.height();
            let attesi = if g.width() == 1 { 0 } else if sul_bordo_x { 1 } else { 2 }
                + if g.height() == 1 { 0 } else if sul_bordo_y { 1 } else { 2 };
            prop_assert_eq!(vicini.len(), attesi);

            // I vicini escono in ordine di TileIdx crescente (contratto BFS).
            let mut ordinati = vicini.clone();
            ordinati.sort_unstable();
            prop_assert_eq!(vicini, ordinati);
        }

        /// La relazione di vicinanza e' simmetrica.
        #[test]
        fn neighbors4_e_simmetrica((g, p) in griglia_e_pos()) {
            let i = g.idx(p).expect("pos generata in bounds");
            for v in g.neighbors4(i) {
                prop_assert!(g.neighbors4(v).any(|w| w == i));
            }
        }
    }
}
