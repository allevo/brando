//! Recording and regenerating the expected replays.

use std::path::Path;
use std::sync::Arc;

use sim_core::{DataSet, Tick};
use sim_replay::{CHECKPOINT_EVERY, GridSpec, Header, Recording, checkpoints};

use crate::scenario::{self, Scenario};

/// How many ticks each recording covers: one game year.
pub fn expected_ticks(data: &DataSet) -> u32 {
    data.rules.ticks_per_year()
}

pub fn record(sc: &Scenario, data: &DataSet) -> Recording {
    // The textual id and not the index: reordering the table must not silently
    // change the meaning of a recording already written.
    let difficulty = data
        .difficulty(sc.difficulty)
        .map(|d| d.id.clone())
        .unwrap_or_else(|| panic!("the scenario's profile is not in the dataset"));

    Recording {
        header: Header {
            format_version: sim_replay::FORMAT_VERSION,
            seed: sc.seed,
            difficulty,
            grid: GridSpec {
                width: sc.side,
                height: sc.side,
                terrain: sim_core::Terrain::Plain,
            },
            dataset_hash: data.hash_hex(),
        },
        // `Recording::commands` is the wire format and stays a bare `u32` (see
        // its doc comment); `Scenario::commands` is `Tick`-typed, so this is
        // the one explicit crossing of that boundary.
        commands: sc.commands.iter().map(|(t, c)| (t.get(), *c)).collect(),
    }
}

/// Regenerates the `.ron` and `.hashes` of every scenario.
///
/// With `check` it writes nothing and reports the files that would have
/// changed: that is the difference between recordings that mean something and
/// recordings that get regenerated out of habit every time they go red.
pub fn regen(data: &Arc<DataSet>, check: bool) -> Result<Vec<String>, String> {
    let dir = sim_replay::expected_dir();
    if !check {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let mut changed = Vec::new();
    for name in scenario::NAMES {
        let sc =
            scenario::by_name(name, data).ok_or_else(|| format!("unknown scenario: {name}"))?;
        let rec = record(&sc, data);

        let ron = rec
            .to_ron()
            .map_err(|e| format!("serialising {name}: {e}"))?;
        compare_or_write(&dir.join(format!("{name}.ron")), &ron, check, &mut changed)?;

        let until = expected_ticks(data);
        let cps = checkpoints(&rec, Arc::clone(data), Tick::new(until), CHECKPOINT_EVERY)
            .map_err(|e| format!("replaying {name}: {e}"))?;
        let hashes = sim_replay::expected::render(&cps, name, CHECKPOINT_EVERY);
        compare_or_write(
            &dir.join(format!("{name}.hashes")),
            &hashes,
            check,
            &mut changed,
        )?;
    }
    Ok(changed)
}

fn compare_or_write(
    path: &Path,
    content: &str,
    check: bool,
    changed: &mut Vec<String>,
) -> Result<(), String> {
    let current = std::fs::read_to_string(path).ok();
    if current.as_deref() == Some(content) {
        return Ok(());
    }
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    if check {
        changed.push(match current {
            None => format!("{name} (missing)"),
            Some(_) => format!("{name} (different)"),
        });
        return Ok(());
    }
    std::fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    changed.push(name);
    Ok(())
}
