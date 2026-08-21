//! Value noise, in whole numbers.
//!
//! Floats are denied across this workspace because two machines can disagree
//! about them, and there is nothing one would buy here anyway: the field ends
//! up five bits wide, seen through a water threshold and a rock threshold.
//!
//! Simplex noise is the tempting alternative and it is refused. It needs a
//! table of gradients and a skew factor that is not a ratio of small whole
//! numbers, so it costs either the float ban or a constant fudged until it
//! looks close enough — and its one real advantage, that it leaves no pattern
//! lined up with the axes, is invisible at five bits. It would be a real
//! improvement on a map with two hundred and fifty-six levels of height. This
//! is not that map.

use rand::{RngCore, SeedableRng};
use rand_pcg::Pcg64;

/// The scale every value in this module is held at: a value of `UNIT` means
/// one. Sixteen bits is far more than the five the field ends up in, so the
/// rounding each layer suffers never reaches the output.
pub const UNIT: i64 = 1 << 16;

/// Keeps this crate's dice apart from every other stream drawn from a seed.
const SALT: &str = "brando/map-gen/v1";

/// Draws one field of noise over `width * height` tiles, row-major.
///
/// Every value comes back in `0..UNIT`, and it is in that range **by
/// construction** rather than by measurement — which is what lets the caller
/// cut the field into steps against a fixed divisor instead of against the
/// range this particular seed happened to reach.
///
/// `name` says which field this is. It costs one argument on a function that
/// exists, and it buys that a second field added later — outcrops of rock
/// scattered on the plain, trees, what the soil is worth — cannot shift the
/// one drawn first. The core's `RngKind` is deliberately not reused for it:
/// that enum's declaration order enters every recorded hash, so a fifth entry
/// for a tool that never ticks would move every recording in the tree to buy
/// nothing.
pub fn field(seed: u64, name: &str, width: u16, height: u16, corner_spacing: u16) -> Vec<i64> {
    let mut out = vec![0i64; width as usize * height as usize];
    let mut spacing = corner_spacing.max(1);
    let mut weight = UNIT;
    let mut total = 0i64;

    // Each layer is half the strength and twice the detail of the one before:
    // the big layers make the landforms and the small ones the roughness.
    // Those two ratios are written into this loop and given no names, because
    // nobody tunes what a layer *is* — two knobs no one would ever turn are
    // two knobs to document, validate and regret.
    for layer in 0u32.. {
        let cols = (width as usize).div_ceil(spacing as usize) + 1;
        let rows = (height as usize).div_ceil(spacing as usize) + 1;
        let corners = corner_values(seed, name, layer, cols * rows);
        add_layer(&mut out, &corners, width, height, spacing, weight);
        total += weight;
        if spacing == 1 || weight == 1 {
            break;
        }
        spacing = (spacing / 2).max(1);
        weight /= 2;
    }

    for v in &mut out {
        *v /= total;
    }
    out
}

/// Adds one layer: the value at each tile is eased between the four corners
/// of the square of the grid it sits in, and added at `weight`.
fn add_layer(out: &mut [i64], corners: &[i64], width: u16, height: u16, spacing: u16, weight: i64) {
    let (w, h, s) = (width as usize, height as usize, spacing as usize);
    let cols = w.div_ceil(s) + 1;

    for y in 0..h {
        let (cy, ey) = (y / s, ease(part_of_the_way(y % s, s)));
        for x in 0..w {
            let (cx, ex) = (x / s, ease(part_of_the_way(x % s, s)));
            let top = between(corners[cy * cols + cx], corners[cy * cols + cx + 1], ex);
            let bottom = between(
                corners[(cy + 1) * cols + cx],
                corners[(cy + 1) * cols + cx + 1],
                ex,
            );
            out[y * w + x] += between(top, bottom, ey) * weight;
        }
    }
}

/// One value per corner, drawn in `0..UNIT`.
///
/// The stream is seeded by a blake3 of the seed, this crate's salt, the
/// field's name and the layer — the same device the core uses to keep its four
/// streams apart, and no mixing constant invented here. The value is a slice
/// of raw bits rather than a remainder, so there is no bias to argue about.
fn corner_values(seed: u64, name: &str, layer: u32, n: usize) -> Vec<i64> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(SALT.as_bytes());
    // The name's length goes in front of it, so "ab" followed by layer 1 and
    // "a" followed by something else cannot hash to the same bytes.
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&layer.to_le_bytes());

    let mut rng = Pcg64::from_seed(*hasher.finalize().as_bytes());
    (0..n).map(|_| i64::from(rng.next_u32() >> 16)).collect()
}

/// How far through the gap between two corners a tile sits, in `0..=UNIT`.
fn part_of_the_way(offset: usize, spacing: usize) -> i64 {
    (offset as i64) * UNIT / (spacing as i64)
}

/// The curve between two corners, `3t² − 2t³`.
///
/// Without it the corner grid shows through as straight creases lined up with
/// the axes: the curve is flat where it meets a corner, so two neighbouring
/// squares of the grid meet smoothly rather than at an angle.
fn ease(t: i64) -> i64 {
    (t * t / UNIT) * (3 * UNIT - 2 * t) / UNIT
}

/// `t` of the way from `a` to `b`, where `t` is in `0..=UNIT`.
fn between(a: i64, b: i64, t: i64) -> i64 {
    a + (b - a) * t / UNIT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The range the whole pipeline leans on, checked at the two ends and in
    /// the middle: it is a fact about the arithmetic, and a later change to
    /// the arithmetic is exactly what would break it.
    #[test]
    fn the_ease_runs_from_nothing_to_everything_and_is_half_way_at_the_middle() {
        assert_eq!(ease(0), 0);
        assert_eq!(ease(UNIT), UNIT);
        assert_eq!(ease(UNIT / 2), UNIT / 2);
    }

    #[test]
    fn a_field_stays_inside_the_range_it_promises() {
        for seed in 0..8u64 {
            for &spacing in &[1u16, 3, 12, 40] {
                let f = field(seed, "relief", 37, 23, spacing);
                assert_eq!(f.len(), 37 * 23);
                assert!(
                    f.iter().all(|v| (0..UNIT).contains(v)),
                    "seed {seed}, spacing {spacing}"
                );
            }
        }
    }

    /// Two fields drawn from the same seed differ, which is the whole reason
    /// the field's name is an argument.
    #[test]
    fn two_fields_of_the_same_seed_are_not_the_same_field() {
        let relief = field(7, "relief", 24, 24, 8);
        let other = field(7, "rock", 24, 24, 8);
        assert_ne!(relief, other);
    }
}
