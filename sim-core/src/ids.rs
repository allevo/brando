//! Identificatori newtype. Nelle firme pubbliche non compare mai un `usize`
//! nudo (CLAUDE.md, convenzioni).

use serde::{Deserialize, Serialize};

slotmap::new_key_type! {
    /// Id di un edificio nello `SlotMap` del `World`.
    pub struct BuildingId;
    /// Id di una casa nello `SlotMap` del `World`.
    pub struct HouseId;
}

/// Indice lineare di tile: `y * width + x`.
///
/// La mappa e' al massimo 256x256 (A4), quindi 65.536 tile: l'ultimo indice e'
/// `u16::MAX` e ci sta esatto.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TileIdx(u16);

impl TileIdx {
    pub const fn new(v: u16) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Tipo di edificio, come indice nella tabella `buildings` del `DataSet`.
///
/// Non e' un enum: i tipi di edificio sono dati, non codice (D6). Un id che
/// non risolve a nessuna riga della tabella e' un errore di comando, non un
/// panic — l'LLM ne produrra' (CLAUDE.md, interfaccia AI).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct BuildingKindId(u16);

impl BuildingKindId {
    pub const fn new(v: u16) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Posizione su griglia. `x` e `y` stanno in `u8` perche' il lato massimo e'
/// 256 (A4): le coordinate valide vanno da 0 a 255.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TilePos {
    pub x: u8,
    pub y: u8,
}

impl TilePos {
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    /// Distanza di Manhattan. In linea d'aria: **non** e' la distanza usata
    /// dalla copertura dei servizi, che si misura sulla rete stradale (D2).
    pub const fn manhattan(self, other: Self) -> u16 {
        let dx = self.x.abs_diff(other.x) as u16;
        let dy = self.y.abs_diff(other.y) as u16;
        dx + dy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_e_simmetrica_e_non_wrappa() {
        let a = TilePos::new(0, 0);
        let b = TilePos::new(255, 255);
        assert_eq!(a.manhattan(b), 510);
        assert_eq!(b.manhattan(a), 510);
        assert_eq!(a.manhattan(a), 0);
    }
}
