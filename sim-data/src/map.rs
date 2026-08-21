//! Loading and validating a named map (phase 16).
//!
//! Not part of [`crate::load::load_from_dir`]: the four balancing tables are
//! always loaded together, but a map is loaded by name, one at a time, from
//! `sim-data/maps/<id>.ron` — a sibling of `sim-data/data/`, not inside it.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sim_core::{Grid, MapDef, Terrain, Tile};

use crate::validate::{ValidationErrorKind, ValidationReport};

/// The raw shape of a map, exactly as it sits in the RON file.
///
/// `terrain` and `ground` are read as characters rather than as `Terrain`
/// values or numbers directly, for the same reason `raw.rs`'s fields are bare
/// integers and strings: a bad character has to produce a validation error
/// naming the row and the column it is in, not an obscure deserialisation
/// one, and the file stays readable by eye — one character per tile, laid out
/// as the map itself is.
#[derive(Debug, Clone, Deserialize)]
pub struct RawMap {
    pub id: String,
    pub width: u16,
    pub height: u16,
    pub terrain: Vec<String>,
    pub ground: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MapLoadError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not valid RON: {source}")]
    Ron {
        path: PathBuf,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("{0}")]
    Validation(#[from] ValidationReport),
}

/// The directory named maps are loaded from — a sibling of the balancing
/// tables' own directory, since a map is optional and loaded by name rather
/// than always loaded together with the other four.
pub fn maps_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("maps")
}

/// Loads and validates the map named `id` from [`maps_dir`].
pub fn load_map_by_id(id: &str) -> Result<MapDef, MapLoadError> {
    let path = maps_dir().join(format!("{id}.ron"));
    let text = std::fs::read_to_string(&path).map_err(|source| MapLoadError::Io {
        path: path.clone(),
        source,
    })?;
    let raw: RawMap = ron::from_str(&text).map_err(|source| MapLoadError::Ron { path, source })?;
    Ok(validate_map(&raw)?)
}

/// Validates a raw map and builds the [`MapDef`] the core consumes.
///
/// Runs in stages, each gating the next: a shape fault (stage A) makes
/// "row-major array" undefined, so the character/range checks (stage B) do
/// not run on garbage, and the connectivity check (stage C) needs a `Grid`
/// that stage B has already guaranteed is buildable. The same gating
/// [`crate::validate::validate`] already uses between per-table and
/// cross-table checks.
pub fn validate_map(raw: &RawMap) -> Result<MapDef, ValidationReport> {
    let mut rep = ValidationReport::default();

    // --- stage A: shape ---
    if let Err(e) = Grid::new(raw.width, raw.height, Terrain::Plain) {
        let sim_core::GridError::InvalidSize { width, height, max } = e;
        rep.push(
            "width/height",
            ValidationErrorKind::InvalidMapSize { width, height, max },
        );
    }
    check_row_shape(raw, "terrain", &raw.terrain, &mut rep);
    check_row_shape(raw, "ground", &raw.ground, &mut rep);
    if !rep.is_empty() {
        return Err(rep);
    }

    // --- stage B: characters and range ---
    let terrain = decode_terrain(&raw.terrain, &mut rep);
    let ground = decode_ground(&raw.ground, &mut rep);
    if !rep.is_empty() {
        return Err(rep);
    }

    let def = MapDef::new(raw.id.clone(), raw.width, raw.height, terrain, ground);

    // --- stage C: connectivity ---
    // A river drawn across the map severs it; until bridges exist (D3-adjacent
    // future phase 21), that would silently strand half of it. The same shape
    // as `Inconsistency::NoGap`: a fact about a data file that the data file
    // itself is made to prove.
    let grid = Grid::from_map(&def);
    let regions = grid.walkable_regions();
    if regions.len() > 1 {
        let mut sizes = regions;
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        rep.push("terrain", ValidationErrorKind::SeveredMap { sizes });
    }

    if rep.is_empty() { Ok(def) } else { Err(rep) }
}

fn check_row_shape(raw: &RawMap, block: &'static str, rows: &[String], rep: &mut ValidationReport) {
    if rows.len() != raw.height as usize {
        rep.push(
            block,
            ValidationErrorKind::WrongMapRowCount {
                block,
                expected: raw.height,
                found: rows.len(),
            },
        );
        return;
    }
    for (row, line) in rows.iter().enumerate() {
        let found = line.chars().count();
        if found != raw.width as usize {
            rep.push(
                format!("{block}[{row}]"),
                ValidationErrorKind::WrongMapRowLength {
                    block,
                    row,
                    expected: raw.width,
                    found,
                },
            );
        }
    }
}

fn decode_terrain(rows: &[String], rep: &mut ValidationReport) -> Vec<Terrain> {
    let mut out = Vec::with_capacity(rows.iter().map(|r| r.chars().count()).sum());
    for (row, line) in rows.iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            let terrain = match c {
                '.' => Terrain::Plain,
                '~' => Terrain::Water,
                '#' => Terrain::Rock,
                _ => {
                    rep.push(
                        format!("terrain[{row}][{col}]"),
                        ValidationErrorKind::UnclaimedMapCharacter {
                            block: "terrain",
                            row,
                            col,
                            found: c,
                            known: ". (plain), ~ (water), # (rock)",
                        },
                    );
                    Terrain::Plain
                }
            };
            out.push(terrain);
        }
    }
    out
}

/// Decodes the ground-height block, base 32: `'0'..='9'` then `'a'..='v'`.
///
/// A digit is accepted through the wider `'0'..='9', 'a'..='z'` alphabet
/// (`char::to_digit(36)`) so that "is this a digit at all" and "is it in
/// range" are two different, independently-triggerable faults: with the
/// exact 32-symbol alphabet the field can hold, every claimed character
/// already decodes in range, and a height-out-of-range table could never be
/// written to exercise the check that exists for it.
fn decode_ground(rows: &[String], rep: &mut ValidationReport) -> Vec<u8> {
    let mut out = Vec::with_capacity(rows.iter().map(|r| r.chars().count()).sum());
    for (row, line) in rows.iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            let height = match c.to_digit(36) {
                Some(v) if v > u32::from(Tile::MAX_GROUND_HEIGHT) => {
                    rep.push(
                        format!("ground[{row}][{col}]"),
                        ValidationErrorKind::GroundHeightBeyondMax {
                            row,
                            col,
                            found: v as u8,
                            max: Tile::MAX_GROUND_HEIGHT,
                        },
                    );
                    0
                }
                Some(v) => v as u8,
                None => {
                    rep.push(
                        format!("ground[{row}][{col}]"),
                        ValidationErrorKind::UnclaimedMapCharacter {
                            block: "ground",
                            row,
                            col,
                            found: c,
                            known: "0-9, a-v (base 32)",
                        },
                    );
                    0
                }
            };
            out.push(height);
        }
    }
    out
}
