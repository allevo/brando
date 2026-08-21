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

/// Everything that can be wrong with the **text** of a map.
///
/// One type for both directions: "what `load_map_by_id` would refuse" and
/// "what [`save_map`] will not write" are the same question asked twice, and a
/// second type for the second half could only ever come to disagree with this
/// one. It carries no path, because text has none — the two callers that do
/// have one wrap it with theirs.
#[derive(Debug, thiserror::Error)]
pub enum MapParseError {
    #[error("not valid RON: {0}")]
    Ron(#[from] ron::error::SpannedError),

    #[error("{0}")]
    Validation(#[from] ValidationReport),
}

#[derive(Debug, thiserror::Error)]
pub enum MapLoadError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: MapParseError,
    },
}

/// Why a map was not written. `Refused` is the interesting one: the text the
/// renderer produced is text the loader would not accept, so nothing was
/// written at all.
#[derive(Debug, thiserror::Error)]
pub enum MapSaveError {
    #[error("cannot write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("refused to write {path}: {source}")]
    Refused {
        path: PathBuf,
        #[source]
        source: MapParseError,
    },
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
    parse_map(&text).map_err(|source| MapLoadError::Parse { path, source })
}

/// Parses map text into a validated [`MapDef`].
///
/// Lifted out of the middle of [`load_map_by_id`], which now calls it: reading
/// a file and reading the text in it are two jobs, and only the second one is
/// wanted by something that has just produced the text itself. The same split
/// [`crate::load::from_ron_str`] already has from [`crate::load::load_from_dir`].
pub fn parse_map(text: &str) -> Result<MapDef, MapParseError> {
    let raw: RawMap = ron::from_str(text)?;
    Ok(validate_map(&raw)?)
}

/// Renders a map back into the text [`parse_map`] reads, legend comments and
/// all: a character table is documented where it is defined, and the file the
/// tool writes has to be as readable by eye as the one a person wrote.
pub fn render_map(def: &MapDef) -> String {
    let terrain = rows(&def.terrain, def.width, |&t| terrain_char(t));
    let ground = rows(&def.ground, def.width, |&h| ground_char(h));

    let mut out = String::new();
    out.push_str("(\n");
    // Debug for a string is the escaping RON reads back, so an id needs no
    // quoting rule of ours.
    out.push_str(&format!("    id: {:?},\n", def.id));
    out.push_str(&format!("    width: {},\n", def.width));
    out.push_str(&format!("    height: {},\n", def.height));
    push_block(&mut out, "terrain", TERRAIN_LEGEND, &terrain);
    push_block(&mut out, "ground", GROUND_LEGEND, &ground);
    out.push_str(")\n");
    out
}

/// Renders `def` and writes it to `path`, refusing to write anything
/// [`load_map_by_id`] would not accept.
///
/// The check is on the rendered **text**, never on `def`: a fault in the
/// alphabet or in the length of a row exists only once the text exists, and a
/// writer that checked the struct it started from would be marking its own
/// homework. Nothing reaches the disk until the real loader has accepted it.
/// `note` is prepended as comment lines — the caller says where the map came
/// from, this function says what the format is.
pub fn save_map(def: &MapDef, note: &str, path: &Path) -> Result<(), MapSaveError> {
    let mut text = String::new();
    for line in note.lines() {
        // An empty line of the note becomes a bare `//` rather than `// `:
        // trailing spaces in a committed file are noise every later diff has
        // to carry.
        out_comment(&mut text, line);
    }
    text.push_str(&render_map(def));

    parse_map(&text).map_err(|source| MapSaveError::Refused {
        path: path.to_path_buf(),
        source,
    })?;
    std::fs::write(path, &text).map_err(|source| MapSaveError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn out_comment(out: &mut String, line: &str) {
    out.push_str("//");
    if !line.is_empty() {
        out.push(' ');
        out.push_str(line);
    }
    out.push('\n');
}

/// One block: its legend comment, then one quoted row per line.
fn push_block(out: &mut String, name: &str, legend: &str, rows: &[String]) {
    out.push_str("    ");
    out_comment(out, legend);
    out.push_str(&format!("    {name}: [\n"));
    for row in rows {
        out.push_str(&format!("        {row:?},\n"));
    }
    out.push_str("    ],\n");
}

/// A block's cells cut into rows of `width` characters.
///
/// A width of zero writes no rows rather than panicking in `chunks`: a
/// [`MapDef`] that did not come from [`validate_map`] can hold one, and it has
/// to survive as far as the text, which is where [`save_map`] looks for it.
fn rows<T>(cells: &[T], width: u16, to_char: impl Fn(&T) -> char) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    cells
        .chunks(width as usize)
        .map(|row| row.iter().map(&to_char).collect())
        .collect()
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

/// The line written above the terrain block, and the same alphabet
/// [`decode_terrain`] claims below.
const TERRAIN_LEGEND: &str = "'.' plain   '~' water   '#' rock";

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

/// The character a terrain is written as — the exact inverse of the match in
/// [`decode_terrain`] above, and it sits beside it so the two cannot be
/// changed apart. A fourth terrain that reached only one of them would be a
/// map the tool writes and the loader refuses.
const fn terrain_char(t: Terrain) -> char {
    match t {
        Terrain::Plain => '.',
        Terrain::Water => '~',
        Terrain::Rock => '#',
    }
}

/// The line written above the ground block, and the same alphabet
/// [`decode_ground`] claims below.
const GROUND_LEGEND: &str = "ground height in base 32: '0'..'9' then 'a'..'v'";

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

/// The character a ground height is written as — std's own base-32 alphabet,
/// which is the exact inverse of [`decode_ground`]'s `to_digit` above, so
/// there is no second table of ours to fall out of step with the first.
///
/// A height base 32 has no digit for comes out as `'?'`, which neither block
/// claims, so [`parse_map`] refuses the text naming the row and the column.
/// Rendering neither panics nor repairs: a fault in a [`MapDef`] that did not
/// come from [`validate_map`] has to survive as far as the text, which is
/// where [`save_map`] is looking for it.
fn ground_char(h: u8) -> char {
    char::from_digit(u32::from(h), 32).unwrap_or('?')
}
