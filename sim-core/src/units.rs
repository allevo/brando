//! Quantita' numeriche dello stato. Nessun float (D4).
//!
//! Due tipi distinti e non convertibili tra loro:
//! - [`Milli`] per le quantita' frazionarie (cibo, lavoro, usura), in millesimi;
//! - [`Coins`] per il denaro, che non ha frazioni di gioco (A1).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Millesimi in una unita'.
const MILLI_PER_UNITA: i32 = 1000;

/// Quantita' in millesimi di unita'. Niente float nello stato (D4).
///
/// Copre circa +/- 2.147.483 unita': abbastanza per cibo e popolazione al
/// target di scala di D5. Il denaro sta fuori di proposito, vedi [`Coins`].
///
/// Non implementa `Add`/`Sub`: l'operatore invita a ignorare l'overflow, e in
/// un core che non puo' panicare l'overflow silenzioso e' peggio del rumore
/// visivo di `checked_add`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize)]
pub struct Milli(i32);

impl Milli {
    /// Zero unita'.
    pub const ZERO: Self = Self(0);

    /// Costruisce da unita' intere. `None` se il valore non ci sta:
    /// 3.000.000 di unita' non sono rappresentabili.
    pub const fn from_units(units: i32) -> Option<Self> {
        match units.checked_mul(MILLI_PER_UNITA) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Costruisce direttamente da millesimi.
    pub const fn from_millis(millis: i32) -> Self {
        Self(millis)
    }

    /// Valore grezzo in millesimi.
    pub const fn to_millis(self) -> i32 {
        self.0
    }

    /// Parte intera, troncata verso zero.
    pub const fn to_units_trunc(self) -> i32 {
        self.0 / MILLI_PER_UNITA
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn checked_mul_int(self, k: i32) -> Option<Self> {
        match self.0.checked_mul(k) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Somma saturante. Ammessa solo dove la saturazione **e'** la semantica di
    /// gioco voluta (es. una giacenza che si ferma al massimo del granaio), mai
    /// come scorciatoia contro l'overflow: in quel caso serve `checked_add`.
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Sottrazione saturante. Stessa avvertenza di [`Milli::saturating_add`].
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Divisione intera con **troncamento verso zero**: `-1501 / 2` fa `-750`,
    /// non `-751`. `None` se il divisore e' zero o se il risultato non e'
    /// rappresentabile (`i32::MIN / -1`).
    pub const fn div_int(self, d: i32) -> Option<Self> {
        match self.0.checked_div(d) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }

    pub const fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }
}

impl fmt::Display for Milli {
    /// Stampa `12.500` — per gli snapshot e la mappa ASCII destinata all'LLM.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = i64::from(self.0);
        let segno = if v < 0 { "-" } else { "" };
        let abs = v.unsigned_abs();
        let unita = abs / 1000;
        let resto = abs % 1000;
        write!(f, "{segno}{unita}.{resto:03}")
    }
}

impl fmt::Debug for Milli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Milli({self})")
    }
}

/// Denaro. Intero, senza millesimi: il tesoro non ha frazioni di gioco e
/// usare i millesimi dimezzerebbe il range utile per niente (A1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize)]
pub struct Coins(i32);

impl Coins {
    pub const ZERO: Self = Self(0);

    pub const fn new(v: i32) -> Self {
        Self(v)
    }

    pub const fn get(self) -> i32 {
        self.0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn checked_mul_int(self, k: i32) -> Option<Self> {
        match self.0.checked_mul(k) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Stessa avvertenza di [`Milli::saturating_add`]: solo dove saturare e'
    /// la regola di gioco.
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for Coins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Coins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Coins({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// D4: nessun tipo pubblico del core deve esporre float.
    /// Sentinella minima, non una prova: la lint `clippy::float_arithmetic`
    /// e' il vero vincolo.
    #[test]
    fn milli_non_e_un_float() {
        assert_eq!(size_of::<Milli>(), 4);
        assert_eq!(size_of::<Coins>(), 4);
    }

    #[test]
    fn from_units_e_checked() {
        assert_eq!(Milli::from_units(3), Some(Milli::from_millis(3000)));
        assert_eq!(Milli::from_units(3_000_000), None);
        assert_eq!(Milli::from_units(-3_000_000), None);
    }

    /// `div_int` tronca verso zero anche per operandi negativi.
    #[test]
    fn div_int_tronca_verso_zero() {
        let casi = [
            (1500, 2, Some(750)),
            (1501, 2, Some(750)),
            (-1500, 2, Some(-750)),
            (-1501, 2, Some(-750)),
            (-1, 2, Some(0)),
            (1000, 0, None),
            (i32::MIN, -1, None),
        ];
        for (v, d, atteso) in casi {
            assert_eq!(
                Milli::from_millis(v).div_int(d),
                atteso.map(Milli::from_millis),
                "div_int({v}, {d})"
            );
        }
    }

    #[test]
    fn display_stampa_i_millesimi() {
        assert_eq!(Milli::from_millis(12_500).to_string(), "12.500");
        assert_eq!(Milli::from_millis(0).to_string(), "0.000");
        assert_eq!(Milli::from_millis(7).to_string(), "0.007");
        assert_eq!(Milli::from_millis(-12_500).to_string(), "-12.500");
        assert_eq!(Milli::from_millis(-7).to_string(), "-0.007");
        assert_eq!(Milli::from_millis(i32::MIN).to_string(), "-2147483.648");
    }

    proptest! {
        /// Le operazioni checked non panicano mai e coincidono con l'oracolo i64.
        #[test]
        fn milli_checked_mai_panic(a: i32, b: i32) {
            let (ma, mb) = (Milli::from_millis(a), Milli::from_millis(b));
            let oracolo = |v: i64| {
                if v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX) {
                    Some(Milli::from_millis(v as i32))
                } else {
                    None
                }
            };
            prop_assert_eq!(ma.checked_add(mb), oracolo(i64::from(a) + i64::from(b)));
            prop_assert_eq!(ma.checked_sub(mb), oracolo(i64::from(a) - i64::from(b)));
            prop_assert_eq!(ma.checked_mul_int(b), oracolo(i64::from(a) * i64::from(b)));
        }

        /// La saturazione non esce mai dal range e conserva l'ordine.
        #[test]
        fn milli_saturating_resta_nel_range(a: i32, b: i32) {
            let (ma, mb) = (Milli::from_millis(a), Milli::from_millis(b));
            let somma = i64::from(a) + i64::from(b);
            let atteso = somma.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            prop_assert_eq!(ma.saturating_add(mb), Milli::from_millis(atteso));
        }

        /// div_int e' sempre troncamento verso zero, mai floor.
        #[test]
        fn milli_div_int_mai_panic(a: i32, d: i32) {
            let r = Milli::from_millis(a).div_int(d);
            if d == 0 || (a == i32::MIN && d == -1) {
                prop_assert_eq!(r, None);
            } else {
                let atteso = i64::from(a) / i64::from(d);
                prop_assert_eq!(r, Some(Milli::from_millis(atteso as i32)));
            }
        }

        #[test]
        fn coins_checked_mai_panic(a: i32, b: i32) {
            let (ca, cb) = (Coins::new(a), Coins::new(b));
            prop_assert_eq!(
                ca.checked_add(cb).map(Coins::get),
                a.checked_add(b)
            );
            prop_assert_eq!(
                ca.checked_sub(cb).map(Coins::get),
                a.checked_sub(b)
            );
        }
    }
}
