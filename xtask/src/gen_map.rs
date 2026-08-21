//! `gen-map`: draws a map from a seed, says what it drew, and writes it out.
//!
//! Thin on purpose. The arithmetic belongs to `map-gen` and the writing to
//! `sim-data`, which owns the format; what is left here is four flags and a
//! report printed for a person to read. It gains no RON dependency of its own,
//! because `sim_data::save_map` means it never needs one.

use std::path::PathBuf;

pub fn gen_map(args: &[String]) -> Result<(), String> {
    let seed: u64 = wanted(args, "--seed", "a number")?;
    let width: u16 = wanted(args, "--width", "a number of tiles")?;
    let height: u16 = wanted(args, "--height", "a number of tiles")?;
    let id = super::flag(args, "--id").ok_or("--id <name> is required")?;
    let out = super::flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| sim_data::maps_dir().join(format!("{id}.ron")));

    let data = sim_data::load_default().map_err(|e| format!("tables: {e}"))?;
    let drawn = map_gen::draw(seed, width, height, &id).map_err(|e| e.to_string())?;

    print!("{}", map_gen::report(&drawn, &data));
    if args.iter().any(|a| a == "--print") {
        println!();
        print!("{}", sim_data::render_map(&drawn.def));
    }

    sim_data::save_map(&drawn.def, &note(seed, width, height, &id), &out)
        .map_err(|e| e.to_string())?;
    println!("\nwrote {}", out.display());
    Ok(())
}

/// The command that drew the file, and the sentence that stops that line being
/// read as a contract.
///
/// The obvious next thought, once there is a generator and committed maps, is
/// a `--check` that redraws every one of them and compares. It looks like
/// rigour and it is the opposite: it would make the generator's output a
/// frozen contract, so improving the noise would fail the check, and the way
/// to make it pass would be to redraw the maps — and redrawing the maps moves
/// every recording that names one. The generator would be back inside the
/// determinism contract by the back door, which is exactly what keeping it out
/// of the core was for.
fn note(seed: u64, width: u16, height: u16, id: &str) -> String {
    format!(
        "Drawn by `cargo xtask gen-map --seed {seed} --width {width} \
         --height {height} --id {id}`,\n\
         then read by eye and committed. That line records how this file was\n\
         first drawn; it is not a promise that the same command would draw it\n\
         again. The generator is a tool and is free to improve, where this file\n\
         is an asset the recordings hash — which is the whole reason the\n\
         generator is not core code."
    )
}

/// A required flag, read as the number the argument really is.
///
/// It is not `super::number`, which answers in `u32`: a width is a `u16` and a
/// seed is a `u64`, and narrowing a `u32` after the fact would turn a size the
/// grid could never hold into some other size it might.
fn wanted<T: std::str::FromStr>(args: &[String], name: &str, what: &str) -> Result<T, String> {
    let v = super::flag(args, name).ok_or_else(|| format!("{name} <{what}> is required"))?;
    v.parse()
        .map_err(|_| format!("{name} wants {what}, found '{v}'"))
}
