//! The `.hashes` file that goes with every recorded replay.
//!
//! Format: one `tick,hex_hash` line per checkpoint, plus comments starting with
//! `#`. Textual and diffable on purpose: when a recording changes, you have to
//! be able to see *at which tick* it started to diverge.

use std::fmt::Write as _;

use sim_core::Tick;

use crate::replay::Checkpoint;

#[derive(Debug, thiserror::Error)]
pub enum ExpectedError {
    #[error("line {line} is malformed: {content:?}")]
    MalformedLine { line: usize, content: String },
}

pub fn render(checkpoints: &[Checkpoint], scenario: &str, every: u32) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# expected hashes for '{scenario}': the state hash every {every} ticks."
    );
    let _ = writeln!(
        s,
        "# If this changes without the balancing having changed, a source of"
    );
    let _ = writeln!(
        s,
        "# non-determinism has been introduced: stop and find it, do not regenerate."
    );
    for c in checkpoints {
        let _ = writeln!(s, "{},{}", c.tick, crate::hash::hash_hex(&c.hash));
    }
    s
}

pub fn parse(text: &str) -> Result<Vec<(Tick, String)>, ExpectedError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (t, h) = line
            .split_once(',')
            .ok_or_else(|| ExpectedError::MalformedLine {
                line: i + 1,
                content: line.to_string(),
            })?;
        let tick: u32 = t.trim().parse().map_err(|_| ExpectedError::MalformedLine {
            line: i + 1,
            content: line.to_string(),
        })?;
        out.push((Tick::new(tick), h.trim().to_string()));
    }
    Ok(out)
}
