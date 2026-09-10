//! `gen-map`: draws a map from a seed and a list of sources, says what it
//! drew, and writes it out.
//!
//! Thin on purpose: the arithmetic belongs to `map-gen` and the writing to
//! `sim-data`. The one thing this front door owns is the shape a list of
//! sources comes in — either a small RON file (`--sources <path>`) or one
//! `--source <x>,<y>,<kind>,<strength>` flag per source, typed straight on
//! the command line — because nothing else in the tree ever reads or writes
//! either shape, so it belongs here rather than costing `map-gen` a
//! `serde`/`ron` dependency of its own. The principle is the crate's own:
//! the crate that consumes a format for a purpose owns it, the way
//! `sim-data` owns the map format itself.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
struct RawSources {
    sources: Vec<RawSource>,
}

#[derive(Deserialize)]
struct RawSource {
    x: u8,
    y: u8,
    kind: RawSourceKind,
    strength: u16,
}

#[derive(Deserialize)]
enum RawSourceKind {
    Mountain,
    Land,
    Sea,
}

pub fn gen_map(args: &[String]) -> Result<(), String> {
    let seed: u64 = wanted(args, "--seed", "a number")?;
    let width: u16 = wanted(args, "--width", "a number of tiles")?;
    let height: u16 = wanted(args, "--height", "a number of tiles")?;
    let min_height: u8 = wanted(args, "--min-height", "a number")?;
    let max_height: u8 = wanted(args, "--max-height", "a number")?;
    let id = super::flag(args, "--id").ok_or("--id <name> is required")?;
    let out = super::flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| sim_data::maps_dir().join(format!("{id}.ron")));

    let (sources, sources_flags) = read_sources(args)?;
    let data = sim_data::load_default().map_err(|e| format!("tables: {e}"))?;
    let drawn = map_gen::draw(&map_gen::DrawInput {
        seed,
        sources,
        width,
        height,
        min_height,
        max_height,
        id: id.clone(),
    })
    .map_err(|e| e.to_string())?;

    print!("{}", map_gen::report(&drawn, &data));
    if args.iter().any(|a| a == "--print") {
        println!();
        print!("{}", sim_data::render_map(&drawn.def));
    }

    sim_data::save_map(&drawn.def, &note(seed, &sources_flags, &id), &out)
        .map_err(|e| e.to_string())?;
    println!("\nwrote {}", out.display());
    Ok(())
}

/// The sources a map grows from, from **either** a RON file (`--sources
/// <path>`) **or** one `--source <x>,<y>,<kind>,<strength>` flag per source
/// — never both, and never neither: an ambiguous combination is a clean
/// error naming the ambiguity, not a silent choice of which one wins.
///
/// Returns the parsed sources alongside the exact flag text that named
/// them, so [`note`] can record the run that really happened rather than
/// always assuming a file.
fn read_sources(args: &[String]) -> Result<(Vec<map_gen::Source>, String), String> {
    let path = super::flag(args, "--sources");
    let inline = super::flags(args, "--source");

    match (path, inline.is_empty()) {
        (Some(_), false) => Err(
            "give sources either as --sources <path> or as one --source \
             <x>,<y>,<kind>,<strength> per source, not both"
                .to_string(),
        ),
        (None, true) => Err("sources are required: --sources <path>, or one --source \
             <x>,<y>,<kind>,<strength> per source"
            .to_string()),
        (Some(path), true) => {
            let sources = read_sources_file(&path)?;
            Ok((sources, format!("--sources {path}")))
        }
        (None, false) => {
            let sources = inline
                .iter()
                .map(|s| parse_source(s))
                .collect::<Result<Vec<_>, _>>()?;
            let flags = inline
                .iter()
                .map(|s| format!("--source {s}"))
                .collect::<Vec<_>>()
                .join(" ");
            Ok((sources, flags))
        }
    }
}

fn read_sources_file(path: &str) -> Result<Vec<map_gen::Source>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let raw: RawSources = ron::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(raw
        .sources
        .into_iter()
        .map(|s| map_gen::Source {
            point: sim_core::TilePos::new(s.x, s.y),
            kind: match s.kind {
                RawSourceKind::Mountain => map_gen::SourceKind::Mountain,
                RawSourceKind::Land => map_gen::SourceKind::Land,
                RawSourceKind::Sea => map_gen::SourceKind::Sea,
            },
            strength: s.strength,
        })
        .collect())
}

/// One `--source` value, `<x>,<y>,<kind>,<strength>`.
///
/// `kind` is read case-insensitively: a RON file already has to spell the
/// enum's exact declared name (`Mountain`, not `mountain`), and a flag
/// typed by hand into a shell should not carry the same requirement for no
/// reason the shell would ever care about.
fn parse_source(raw: &str) -> Result<map_gen::Source, String> {
    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    let [x, y, kind, strength] = parts.as_slice() else {
        return Err(format!(
            "--source wants <x>,<y>,<kind>,<strength>, found '{raw}'"
        ));
    };
    let x: u8 = x
        .parse()
        .map_err(|_| format!("--source '{raw}': x wants a number, found '{x}'"))?;
    let y: u8 = y
        .parse()
        .map_err(|_| format!("--source '{raw}': y wants a number, found '{y}'"))?;
    let strength: u16 = strength
        .parse()
        .map_err(|_| format!("--source '{raw}': strength wants a number, found '{strength}'"))?;
    let kind = match kind.to_ascii_lowercase().as_str() {
        "mountain" => map_gen::SourceKind::Mountain,
        "land" => map_gen::SourceKind::Land,
        "sea" => map_gen::SourceKind::Sea,
        _ => {
            return Err(format!(
                "--source '{raw}': kind wants mountain, land or sea, found '{kind}'"
            ));
        }
    };
    Ok(map_gen::Source {
        point: sim_core::TilePos::new(x, y),
        kind,
        strength,
    })
}

/// The command that drew the file, and the sentence that stops that line
/// being read as a contract: the header is not a promise that the same
/// command would draw the file again, since the generator is a tool and is
/// free to improve, where a committed map is an asset the recordings hash —
/// which is the whole reason the generator is not core code.
/// `sources_flags` is whichever of `--sources <path>` or the `--source ...`
/// flags actually named the sources, so the line records the run that
/// really happened.
fn note(seed: u64, sources_flags: &str, id: &str) -> String {
    format!(
        "Drawn by `cargo xtask gen-map --seed {seed} {sources_flags} --id {id}`,\n\
         then read by eye and committed. That line records how this file was\n\
         first drawn; it is not a promise that the same command would draw it\n\
         again. The generator is a tool and is free to improve, where this file\n\
         is an asset the recordings hash — which is the whole reason the\n\
         generator is not core code."
    )
}

/// A required flag, read as the number the argument really is.
fn wanted<T: std::str::FromStr>(args: &[String], name: &str, what: &str) -> Result<T, String> {
    let v = super::flag(args, name).ok_or_else(|| format!("{name} <{what}> is required"))?;
    v.parse()
        .map_err(|_| format!("{name} wants {what}, found '{v}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_parses_its_four_fields() {
        let s = parse_source("4,28,Sea,5").expect("well formed");
        assert_eq!(s.point, sim_core::TilePos::new(4, 28));
        assert_eq!(s.kind, map_gen::SourceKind::Sea);
        assert_eq!(s.strength, 5);
    }

    #[test]
    fn kind_is_read_case_insensitively() {
        for text in ["mountain", "Mountain", "MOUNTAIN", "MoUnTaIn"] {
            let s = parse_source(&format!("0,0,{text},1")).expect(text);
            assert_eq!(s.kind, map_gen::SourceKind::Mountain, "{text}");
        }
    }

    #[test]
    fn each_malformed_source_is_refused_by_name() {
        assert!(parse_source("4,28,Sea").is_err(), "too few fields");
        assert!(parse_source("4,28,Sea,5,1").is_err(), "too many fields");
        assert!(parse_source("x,28,Sea,5").is_err(), "x is not a number");
        assert!(parse_source("4,y,Sea,5").is_err(), "y is not a number");
        assert!(
            parse_source("4,28,Sea,strong").is_err(),
            "strength is not a number"
        );
        assert!(parse_source("4,28,Ocean,5").is_err(), "unknown kind");
    }

    #[test]
    fn giving_both_sources_and_source_is_refused() {
        let args: Vec<String> = ["--sources", "a.ron", "--source", "0,0,Land,1"]
            .into_iter()
            .map(String::from)
            .collect();
        assert!(read_sources(&args).is_err());
    }

    #[test]
    fn giving_neither_sources_nor_source_is_refused() {
        assert!(read_sources(&[]).is_err());
    }

    #[test]
    fn source_flags_alone_are_read_into_the_same_shape_a_file_would_give() {
        let args: Vec<String> = ["--source", "4,4,Mountain,3", "--source", "44,28,Sea,5"]
            .into_iter()
            .map(String::from)
            .collect();
        let (sources, flags) = read_sources(&args).expect("two well-formed sources");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].kind, map_gen::SourceKind::Mountain);
        assert_eq!(sources[1].kind, map_gen::SourceKind::Sea);
        assert_eq!(flags, "--source 4,4,Mountain,3 --source 44,28,Sea,5");
    }
}
