//! Numeric quantities held in the state. No floats (D4).
//!
//! Two distinct types that cannot be converted into one another:
//! - [`Milli`] for fractional quantities (food, labour, wear), in thousandths;
//! - [`Coins`] for money, which has no in-game fractions.

use std::fmt;

/// Thousandths in one unit.
const MILLI_PER_UNIT: i32 = 1000;

/// A quantity in thousandths of a unit. No floats in the state.
///
/// Covers roughly +/- 2,147,483 units: enough for food and population.
/// Money is deliberately left out, see [`Coins`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Milli(i32);

impl Milli {
    /// Zero units.
    pub const ZERO: Self = Self(0);

    /// Builds from whole units. `None` if the value does not fit.
    pub const fn from_units(units: i32) -> Option<Self> {
        match units.checked_mul(MILLI_PER_UNIT) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Builds directly from thousandths.
    pub const fn from_millis(millis: i32) -> Self {
        Self(millis)
    }

    /// The raw value in thousandths.
    pub const fn to_millis(self) -> i32 {
        self.0
    }

    /// The whole part, truncated towards zero.
    pub const fn to_units_trunc(self) -> i32 {
        self.0 / MILLI_PER_UNIT
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

    /// Saturating addition. Allowed only where saturating **is** the intended
    /// game rule (e.g. a stock that stops at the granary's maximum), never as
    /// a shortcut against overflow: that case needs `checked_add`.
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction. Same warning as [`Milli::saturating_add`].
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Integer division **truncated towards zero**: `-1501 / 2` is `-750`, not
    /// `-751`. `None` if the divisor is zero or if the result is not
    /// representable (`i32::MIN / -1`).
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
    /// Prints `12.500` — for snapshots and for the ASCII map meant for the LLM.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = i64::from(self.0);
        let sign = if v < 0 { "-" } else { "" };
        let abs = v.unsigned_abs();
        let units = abs / 1000;
        let millis = abs % 1000;
        write!(f, "{sign}{units}.{millis:03}")
    }
}

impl fmt::Debug for Milli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Milli({self})")
    }
}

/// Money. A whole number, with no thousandths: the treasury has no in-game
/// fractions.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
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

    /// Same warning as [`Milli::saturating_add`]: only where saturating is the
    /// game rule.
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

    /// D4: no public type of the core may expose a float.
    /// A minimal sentinel, not a proof: the `clippy::float_arithmetic` lint is
    /// the real constraint.
    #[test]
    fn milli_is_not_a_float() {
        assert_eq!(size_of::<Milli>(), 4);
        assert_eq!(size_of::<Coins>(), 4);
    }

    #[test]
    fn from_units_is_checked() {
        assert_eq!(Milli::from_units(3), Some(Milli::from_millis(3000)));
        assert_eq!(Milli::from_units(3_000_000), None);
        assert_eq!(Milli::from_units(-3_000_000), None);
    }

    /// `div_int` truncates towards zero for negative operands too.
    #[test]
    fn div_int_truncates_towards_zero() {
        let cases = [
            (1500, 2, Some(750)),
            (1501, 2, Some(750)),
            (-1500, 2, Some(-750)),
            (-1501, 2, Some(-750)),
            (-1, 2, Some(0)),
            (1000, 0, None),
            (i32::MIN, -1, None),
        ];
        for (v, d, expected) in cases {
            assert_eq!(
                Milli::from_millis(v).div_int(d),
                expected.map(Milli::from_millis),
                "div_int({v}, {d})"
            );
        }
    }

    #[test]
    fn display_prints_the_thousandths() {
        assert_eq!(Milli::from_millis(12_500).to_string(), "12.500");
        assert_eq!(Milli::from_millis(0).to_string(), "0.000");
        assert_eq!(Milli::from_millis(7).to_string(), "0.007");
        assert_eq!(Milli::from_millis(-12_500).to_string(), "-12.500");
        assert_eq!(Milli::from_millis(-7).to_string(), "-0.007");
        assert_eq!(Milli::from_millis(i32::MIN).to_string(), "-2147483.648");
    }

    proptest! {
        /// The checked operations never panic and agree with the i64 oracle.
        #[test]
        fn milli_checked_never_panics(a: i32, b: i32) {
            let (ma, mb) = (Milli::from_millis(a), Milli::from_millis(b));
            let oracle = |v: i64| {
                if v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX) {
                    Some(Milli::from_millis(v as i32))
                } else {
                    None
                }
            };
            prop_assert_eq!(ma.checked_add(mb), oracle(i64::from(a) + i64::from(b)));
            prop_assert_eq!(ma.checked_sub(mb), oracle(i64::from(a) - i64::from(b)));
            prop_assert_eq!(ma.checked_mul_int(b), oracle(i64::from(a) * i64::from(b)));
        }

        /// Saturating never leaves the range and preserves the ordering.
        #[test]
        fn milli_saturating_stays_in_range(a: i32, b: i32) {
            let (ma, mb) = (Milli::from_millis(a), Milli::from_millis(b));
            let sum = i64::from(a) + i64::from(b);
            let expected = sum.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            prop_assert_eq!(ma.saturating_add(mb), Milli::from_millis(expected));
        }

        /// div_int always truncates towards zero, never floors.
        #[test]
        fn milli_div_int_never_panics(a: i32, d: i32) {
            let r = Milli::from_millis(a).div_int(d);
            if d == 0 || (a == i32::MIN && d == -1) {
                prop_assert_eq!(r, None);
            } else {
                let expected = i64::from(a) / i64::from(d);
                prop_assert_eq!(r, Some(Milli::from_millis(expected as i32)));
            }
        }

        #[test]
        fn coins_checked_never_panics(a: i32, b: i32) {
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
