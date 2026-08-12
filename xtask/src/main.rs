//! The headless runner: single games, batches, regenerating the recordings.
//!
//! This is where the game runs with no graphics dependency at all: `sim-core`
//! and nothing else. If Bevy were needed here, it would be an architectural
//! mistake.

mod bench;
mod expected;
mod scenario;

use std::process::ExitCode;
use std::sync::Arc;

use sim_core::{ServiceKind, World};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);

    match command {
        Some("run") => exit_code(run(&args[1..])),
        Some("record") => exit_code(record(&args[1..])),
        Some("regen-expected") => exit_code(regen_expected(&args[1..])),
        Some("bench") => exit_code(bench::bench(&args[1..])),
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}

fn exit_code(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("usage: cargo xtask <command>");
    eprintln!();
    eprintln!("  run --scenario <name> --ticks <n> [--dump-every <n>] [--difficulty <id>]");
    eprintln!("  record --scenario <name> --out <file.ron>");
    eprintln!("  regen-expected [--check]");
    eprintln!("  bench [--side <n>] [--residents <n>] [--reps <n>]   (use --release)");
    eprintln!();
    eprintln!("scenarios: {}", scenario::NAMES.join(", "));
}

/// Records a scenario into a `.ron` file of your choosing.
///
/// The recordings of the known scenarios are written by `regen-expected`; this
/// is for pulling one out at an arbitrary path, to inspect it or to build a new
/// case on top of it.
fn record(args: &[String]) -> Result<(), String> {
    let name = flag(args, "--scenario").ok_or("--scenario <name> is required")?;
    let out = flag(args, "--out").ok_or("--out <file.ron> is required")?;

    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tables: {e}"))?);
    let sc = scenario::by_name(&name, &data).ok_or_else(|| format!("unknown scenario: {name}"))?;
    let rec = expected::record(&sc, &data);

    let path = std::path::Path::new(&out);
    rec.save(path).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out} ({} commands)", rec.commands.len());
    Ok(())
}

/// Regenerates the recordings, or checks there would be nothing to regenerate.
fn regen_expected(args: &[String]) -> Result<(), String> {
    let check = args.iter().any(|a| a == "--check");
    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tables: {e}"))?);
    let changed = expected::regen(&data, check)?;

    if changed.is_empty() {
        println!("recordings up to date, nothing to do");
        return Ok(());
    }
    if check {
        eprintln!("these recordings would have changed:");
        for d in &changed {
            eprintln!("  - {d}");
        }
        return Err(
            "the recordings are out of date. If the change was intended, run \n  \
             cargo xtask regen-expected\n\
             and commit the diff; if it was not, there is a source of non-determinism to find"
                .to_string(),
        );
    }
    println!("regenerated:");
    for d in &changed {
        println!("  - {d}");
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    let name = flag(args, "--scenario").unwrap_or_else(|| "minimal".to_string());
    let ticks: u32 = number(args, "--ticks")?.unwrap_or(120);
    let dump_every: u32 = number(args, "--dump-every")?.unwrap_or(30);

    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tables: {e}"))?);
    let mut sc =
        scenario::by_name(&name, &data).ok_or_else(|| format!("unknown scenario: {name}"))?;

    // The scenario proposes a profile, the flag overrides it. An unknown id is
    // an error and not a fallback: playing another game than the one asked for
    // is worse than not playing.
    let profile =
        flag(args, "--difficulty").unwrap_or_else(|| scenario::RECORDED_DIFFICULTY.into());
    sc.difficulty = data.difficulty_by_id(&profile).ok_or_else(|| {
        format!(
            "unknown difficulty profile: {profile:?} (known: {})",
            data.difficulty_ids()
        )
    })?;

    println!("scenario '{}' — {}", sc.name, sc.description);
    println!(
        "seed {}, grid {}x{}, difficulty '{}', dataset {}",
        sc.seed,
        sc.side,
        sc.side,
        profile,
        &data.hash_hex()[..16]
    );
    println!();
    table_header();

    let mut w = sc.world(Arc::clone(&data));
    let mut rejected = 0usize;
    for t in 0..ticks {
        let cmds = sc.commands_at_tick(t);
        let r = sim_core::step(&mut w, &cmds);
        rejected += r.rejected.len();
        for (i, e) in &r.rejected {
            eprintln!("  tick {t}: command {i} rejected: {e}");
        }
        if dump_every > 0 && (t + 1) % dump_every == 0 {
            row(&w);
        }
    }

    println!();
    println!("{} commands rejected in total", rejected);
    Ok(())
}

fn table_header() {
    println!(
        "{:>6}  {:>6}  {:>6}  {:>5}  {:>9}  {:>7}  {:>7}  {:>8}  {:>7}  {:>10}",
        "tick", "months", "houses", "res.", "stock", "water", "food", "treasury", "sat.", "levels"
    );
}

fn row(w: &World) {
    let months = w.tick() / w.data().rules.ticks_per_month;
    let with_water = w
        .houses()
        .filter(|(_, h)| h.served.get(ServiceKind::Water))
        .count();
    let with_food = w
        .houses()
        .filter(|(_, h)| h.served.get(ServiceKind::Food))
        .count();
    let houses = w.house_count();
    let max = w.data().rules.satisfaction.max;
    println!(
        "{:>6}  {:>6}  {:>6}  {:>5}  {:>9}  {:>3}/{:<3}  {:>3}/{:<3}  {:>8}  {:>3}/{:<3}  {:>10}",
        w.tick(),
        months,
        houses,
        w.population(),
        milli(w.total_stock()),
        with_water,
        houses,
        with_food,
        houses,
        w.economy().treasury,
        average_satisfaction(w),
        max,
        level_distribution(w)
    );
}

/// How many houses sit at each level, level 1 first.
///
/// It is the column phase 13 is closed by eye with: the distribution has to
/// rise and then **stop**. An average level would hide exactly the failure the
/// phase is about — half a city flipping around a threshold averages out to a
/// flat line.
fn level_distribution(w: &World) -> String {
    let mut counts = vec![0u32; w.data().rules.house_levels.len()];
    for (_, h) in w.houses() {
        if let Some(c) = counts.get_mut(h.level.as_usize()) {
            *c += 1;
        }
    }
    counts
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("/")
}

/// The average, over the houses, of the **worst service its own level
/// requires**.
///
/// The worst and not the mean of the two: it is the quantity the mood is
/// computed from, so the column and what the renderer would draw say the same
/// thing. A house with water and no food is awful, not half happy — unless
/// its level does not ask for food, which since phase 13 is the case at level 1
/// and is why the list is read per house instead of once.
fn average_satisfaction(w: &World) -> u32 {
    let houses = w.house_count() as u32;
    if houses == 0 {
        return 0;
    }
    let total: u32 = w
        .houses()
        .map(|(_, h)| {
            w.data()
                .rules
                .required_at(h.level)
                .iter()
                .map(|k| u32::from(h.satisfaction[k.index()]))
                .min()
                .unwrap_or(0)
        })
        .sum();
    total / houses
}

/// Formats thousandths as `12.500`, the way `Milli::Display` does.
fn milli(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.unsigned_abs();
    format!("{sign}{}.{:03}", a / 1000, a % 1000)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

fn number(args: &[String], name: &str) -> Result<Option<u32>, String> {
    match flag(args, name) {
        None => Ok(None),
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| format!("{name} wants a number, found '{v}'")),
    }
}
