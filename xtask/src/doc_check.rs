//! The documents checked against the code.
//!
//! The project's own lesson, generalised: *a balancing number that has to stand
//! in a particular relation with another one is a validation check, not a
//! comment*. The same is true of prose. Every rule below is one that used to be
//! a promise to remember and is now something that fails.
//!
//! There is no CI here and `jj` does not run git hooks, so this runs from
//! `cargo test` as well as from `cargo xtask doc-check`.

use std::fs;
use std::path::{Path, PathBuf};

use sim_core::Terrain;

/// One thing that is wrong. `where_` is a path, or a path and a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub at: String,
    pub what: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.check, self.at, self.what)
    }
}

/// The repository root, derived from this crate's manifest directory.
///
/// It works from the binary and from the integration test alike, because both
/// are compiled inside the `xtask` package.
pub fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// The entry point for `cargo xtask doc-check`.
pub fn main(_args: &[String]) -> Result<(), String> {
    let root = repo_root()?;
    let findings = run(&root)?;
    if findings.is_empty() {
        println!("doc-check: all checks pass");
        return Ok(());
    }
    for f in &findings {
        eprintln!("{f}");
    }
    Err(format!(
        "doc-check: {} problem(s). The documents and the code disagree.",
        findings.len()
    ))
}

/// Every check, in one pass. An empty result means the documents agree with the
/// code.
pub fn run(root: &Path) -> Result<Vec<Finding>, String> {
    let sources = source_files(root)?;
    let mut f = Vec::new();
    // The tasks are read once and then handed to every check that needs them,
    // so a malformed header is reported as itself rather than a second time as
    // each disagreement it caused.
    let (tasks, malformed) = read_tasks(root)?;
    f.extend(malformed);
    f.extend(no_plan_paths_in_code(&sources));
    f.extend(decision_ids_exist(root, &sources, &tasks)?);
    f.extend(cited_slots_exist(&sources, &tasks));
    f.extend(rules_match_the_tables(root)?);
    f.extend(rules_have_no_values(root)?);
    f.extend(architecture_matches_the_tick(root)?);
    f.extend(tasks_agree_with_their_names(&tasks));
    f.extend(task_names_sort_the_way_the_numbers_run(&tasks));
    f.extend(open_questions_are_scheduled(root, &tasks)?);
    f.extend(roadmap_agrees_with_the_tasks(root, &tasks)?);
    f.extend(frozen_orders_match_the_code(root, &sources)?);
    f.extend(skill_matches_the_task_header(root)?);
    f.extend(rules_match_what_a_terrain_allows(root)?);
    Ok(f)
}

// --- the number a phase carries -------------------------------------------

/// A phase number: `14`, or `14.4` for work that falls between two whole
/// phases and claims nothing about the state before it.
///
/// The components are compared one at a time as whole numbers, so a whole phase
/// always comes before its own half numbers and `14.5.5` sits between `14.5`
/// and `14.6`. Read as a decimal fraction instead, a half number could only ever
/// be one digit deep and `14.5.5` would not be a number at all. The number is an
/// ordering device and never a quantity: nothing is added to it or averaged with
/// it.
///
/// Comparing the components as numbers is only half of it, because nothing
/// outside this checker does: a listing compares bytes. The names are written so
/// that the two orders agree — see
/// [`task_names_sort_the_way_the_numbers_run`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PhaseNumber(Vec<u32>);

impl PhaseNumber {
    /// The number `s` opens with, so one piece of code reads a file name
    /// (`14.4-building-kind.md`) and a run of prose (`14.4, in which ...`).
    ///
    /// `None` when `s` does not open with a digit, and `None` when a component
    /// is missing, as in `14..4`. Nothing in the plan directory is unnumbered
    /// any more, so the first case is now only reached by prose.
    fn leading(s: &str) -> Option<Self> {
        let parts = leading_components(s)
            .iter()
            .map(|p| p.parse::<u32>())
            .collect::<Result<Vec<u32>, _>>()
            .ok()?;
        Some(Self(parts))
    }

    /// True for a half number: work that falls between two whole phases.
    fn is_fractional(&self) -> bool {
        self.0.len() > 1
    }
}

impl std::fmt::Display for PhaseNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, part) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

/// The number `s` opens with, split into its components and still as text, so
/// that a check can see how many digits each one was written with — which is
/// what decides where a file name lands in a listing, and which parsing throws
/// away.
///
/// Empty components survive rather than being skipped: `14..4` comes back with
/// an empty one in the middle, and every caller treats that as "not a number".
fn leading_components(s: &str) -> Vec<String> {
    let head: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    // A trailing dot ends a sentence; it does not open an empty component.
    head.trim_end_matches('.')
        .split('.')
        .map(str::to_string)
        .collect()
}

// --- 1. the code never cites a document path ------------------------------

/// Paths rot when a file moves; ids do not. A comment cites `D4` and states
/// its rule in full where it cites it, never pointing at a task document.
fn no_plan_paths_in_code(sources: &[(PathBuf, String)]) -> Vec<Finding> {
    let needle = concat!("plan", "/");
    let mut out = Vec::new();
    for (path, text) in sources {
        for (n, line) in text.lines().enumerate() {
            if line.contains(needle) {
                out.push(Finding {
                    check: "no-plan-paths",
                    at: format!("{}:{}", path.display(), n + 1),
                    what: "cites a phase document by path. Cite the decision id instead, \
                           and state the rule where you cite it."
                        .to_string(),
                });
            }
        }
    }
    out
}

// --- 2. every cited id exists, and only `D` ids are cited at all -----------

/// A citation that outlives its decision is worse than no citation: it looks
/// authoritative and resolves to nothing.
///
/// Two rules, because there are no longer two id namespaces. A `D` is the
/// constitution, it binds code that does not exist yet, and it lives in the one
/// task that declares itself the constitution — cited by id and never by path,
/// so moving that file breaks nothing. An `A` was an implementation decision in
/// a register that no longer exists: every rule it held is now stated in full at
/// the place it binds, and a leftover tag would point at nothing. There is
/// deliberately no way to reintroduce one quietly.
fn decision_ids_exist(
    root: &Path,
    sources: &[(PathBuf, String)],
    tasks: &[Task],
) -> Result<Vec<Finding>, String> {
    let Some(constitution) = tasks.iter().find(|t| t.kind == "constitution") else {
        return Ok(vec![Finding {
            check: "unknown-id",
            at: "the plan directory".to_string(),
            what: "no task declares `kind: constitution`, so no `D<n>` can be resolved".to_string(),
        }]);
    };
    let path = root.join("plan").join(&constitution.file);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let known_d = headed_ids(&text, "## D");

    let mut out = Vec::new();
    for (path, text) in sources {
        for (n, line) in text.lines().enumerate() {
            for (letter, num) in cited_ids(line) {
                let at = format!("{}:{}", path.display(), n + 1);
                if letter == 'A' {
                    out.push(Finding {
                        check: "unknown-id",
                        at,
                        what: format!(
                            "cites A{num}. There is no decision register to resolve it against: \
                             state the rule in full where it binds, with no tag."
                        ),
                    });
                } else if !known_d.contains(&num) {
                    out.push(Finding {
                        check: "unknown-id",
                        at,
                        what: format!("cites D{num}, which the constitution does not define"),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// A slot cited from a comment names a task that exists.
///
/// This is the gap the decisions review wrote down as unwritable: fourteen
/// comments named a slot, nothing checked them, and matching bare decimals in
/// Rust source would have reported every version number and every measured
/// millisecond. What makes it writable is that a number is only read as a
/// citation when a word introduces it, and `phase` and `slot` are the two words
/// the sources already use. A bare `14.5` is still invisible here, and that is
/// the deliberate limit.
fn cited_slots_exist(sources: &[(PathBuf, String)], tasks: &[Task]) -> Vec<Finding> {
    let mut out = Vec::new();
    for (path, text) in sources {
        for (n, line) in text.lines().enumerate() {
            for (word, number) in cited_slots(line) {
                if !tasks.iter().any(|t| t.id == number) {
                    out.push(Finding {
                        check: "unknown-slot",
                        at: format!("{}:{}", path.display(), n + 1),
                        what: format!("cites {word} {number}, which is not a task"),
                    });
                }
            }
        }
    }
    out
}

/// The numbers `phase` and `slot` introduce, in either case and either number:
/// `phase 14`, `Phase 14.4`, `slot 18.5`, `phases 15`.
fn cited_slots(line: &str) -> Vec<(&'static str, PhaseNumber)> {
    // Lowercasing only maps ASCII letters, so every byte offset still lines up
    // with the original and the digits are untouched.
    let lower = line.to_ascii_lowercase();
    let mut out = Vec::new();
    for word in ["phase", "slot"] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(word) {
            let start = from + i;
            from = start + word.len();
            // The word has to stand on its own: `rephrase 14` is not a citation.
            let before = start.checked_sub(1).map(|i| lower.as_bytes()[i]);
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
                continue;
            }
            let after = &lower[from..];
            let Some(rest) = after.strip_prefix(' ').or_else(|| after.strip_prefix("s ")) else {
                continue;
            };
            if let Some(number) = PhaseNumber::leading(rest) {
                out.push((word, number));
            }
        }
    }
    out
}

/// The numbers of every heading that starts with `prefix`, e.g. `## D4 — ...`.
fn headed_ids(text: &str, prefix: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// An id letter followed by digits — `D4`, or a leftover from the register
/// that no longer exists — appearing as a whole token. Not inside an
/// identifier, and not after `0x`, so a hex literal is never mistaken for a
/// citation.
fn cited_ids(line: &str) -> Vec<(char, u32)> {
    let b: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for i in 0..b.len() {
        if b[i] != 'A' && b[i] != 'D' {
            continue;
        }
        if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == '_' || b[i - 1] == 'x') {
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == i + 1 || j - i > 3 {
            continue;
        }
        if j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
            continue;
        }
        let num: String = b[i + 1..j].iter().collect();
        if let Ok(n) = num.parse() {
            out.push((b[i], n));
        }
    }
    out
}

// --- 3. an open question is scheduled where it has to be answered ---------

/// An open question holds a numbered slot at the point where it has to be
/// answered, or it is not really scheduled at all.
///
/// That is what happened to two decisions that stayed invisible while four
/// phases were planned on top of them. The slot is a row in `ROADMAP.md`; its
/// number is a half number because the work falls between two whole phases; and
/// the file it names holds the argument, declares itself an open question, and
/// is still a plan. A row claiming to be open while the phase answering it has
/// been built is the same failure in the other direction.
fn open_questions_are_scheduled(root: &Path, tasks: &[Task]) -> Result<Vec<Finding>, String> {
    let roadmap = read(root, "ROADMAP.md")?;
    let mut out = Vec::new();
    for line in roadmap.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let what = cells.get(2).map(|c| c.trim()).unwrap_or("");
        if !what.starts_with("**Open question") {
            continue;
        }
        let slot = cells
            .get(1)
            .map(|c| c.trim().trim_matches('*'))
            .unwrap_or("");
        if !PhaseNumber::leading(slot).is_some_and(|n| n.is_fractional()) {
            out.push(Finding {
                check: "open-question",
                at: "ROADMAP.md".to_string(),
                what: format!("row \"{slot}\" is an open question but has no half number"),
            });
        }
        let Some(file) = plan_link(what) else {
            out.push(Finding {
                check: "open-question",
                at: "ROADMAP.md".to_string(),
                what: format!("row \"{slot}\" names no task document to hold the argument"),
            });
            continue;
        };
        // Matched by the end of the path, so the directory is never spelled out
        // here — check 1 scans this file too.
        let Some(task) = tasks.iter().find(|t| file.ends_with(&t.file)) else {
            out.push(Finding {
                check: "open-question",
                at: "ROADMAP.md".to_string(),
                what: format!(
                    "row \"{slot}\" names {file}, which is not a task: it does not exist, or \
                     its own header does not read"
                ),
            });
            continue;
        };
        if PhaseNumber::leading(slot).is_some_and(|n| n != task.id) {
            out.push(Finding {
                check: "open-question",
                at: task.file.clone(),
                what: format!(
                    "is named by roadmap row \"{slot}\", but says `id: {}`",
                    task.id
                ),
            });
        }
        if task.kind != "open-question" {
            out.push(Finding {
                check: "open-question",
                at: task.file.clone(),
                what: format!(
                    "row \"{slot}\" calls this an open question, but it says `kind: {}`",
                    task.kind
                ),
            });
        }
        if task.status != "not-yet-built" {
            out.push(Finding {
                check: "open-question",
                at: task.file.clone(),
                what: format!(
                    "row \"{slot}\" says the question is open, but this task says \
                     `status: {}`",
                    task.status
                ),
            });
        }
    }
    Ok(out)
}

/// The target of the first markdown link in `s` that points at a phase
/// document, e.g. `14.5-an-empty-house.md` out of `[title](...)`.
fn plan_link(s: &str) -> Option<String> {
    // Spelled this way for the same reason the needle in check 1 is: this file
    // is scanned by that check, and a phase path written out here would trip it.
    let needle = concat!("(", "plan", "/");
    let start = s.find(needle)? + 1;
    let end = s[start..].find(')')? + start;
    Some(s[start..end].to_string())
}

// --- 4. RULES.md and the tables name the same parameters ------------------

/// Container keys and pure identifiers: they carry no rule, so `RULES.md` has
/// no reason to name them.
const STRUCTURAL_KEYS: [&str; 4] = ["buildings", "terrains", "profiles", "id"];

/// Both directions. A parameter added to a table and never described is a rule
/// nobody wrote down; a parameter described under a name the tables no longer
/// use is a rule nobody can find.
fn rules_match_the_tables(root: &Path) -> Result<Vec<Finding>, String> {
    let rules_doc = read(root, "RULES.md")?;
    let mut keys: Vec<String> = Vec::new();
    let data_dir = root.join("sim-data/data");
    let mut ron: Vec<PathBuf> = fs::read_dir(&data_dir)
        .map_err(|e| format!("{}: {e}", data_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    ron.sort();
    for path in &ron {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("//") {
                continue;
            }
            for key in ron_keys(line) {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }

    let doc_tokens = tokens(&rules_doc);
    let mut out = Vec::new();
    for key in &keys {
        if STRUCTURAL_KEYS.contains(&key.as_str()) {
            continue;
        }
        if !doc_tokens.contains(key) {
            out.push(Finding {
                check: "rules-parameters",
                at: "RULES.md".to_string(),
                what: format!(
                    "`{key}` is a parameter in sim-data/data but RULES.md never names it"
                ),
            });
        }
    }

    // The reverse: a snake_case name in backticks that no table defines.
    for (n, line) in rules_doc.lines().enumerate() {
        for span in backticked(line) {
            if span.contains('/') || span.contains("::") || span.contains(".rs") {
                continue;
            }
            for t in tokens(&span) {
                let snake = t.contains('_')
                    && t.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                if snake && !keys.contains(&t) {
                    out.push(Finding {
                        check: "rules-parameters",
                        at: format!("RULES.md:{}", n + 1),
                        what: format!("names `{t}`, which is not a parameter in sim-data/data"),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Every `key:` on the line, not just the first: `terrain.ron` and
/// `difficulty.ron` put a whole record on one line.
fn ron_keys(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if *c != ':' {
            continue;
        }
        // RON's `Some(...)` and paths use `::`; a key never does.
        if chars.get(i + 1) == Some(&':') || (i > 0 && chars[i - 1] == ':') {
            continue;
        }
        let mut start = i;
        while start > 0 {
            let p = chars[start - 1];
            if p.is_ascii_lowercase() || p.is_ascii_digit() || p == '_' {
                start -= 1;
            } else {
                break;
            }
        }
        let key: String = chars[start..i].iter().collect();
        if key.is_empty() || !key.starts_with(|c: char| c.is_ascii_lowercase()) {
            continue;
        }
        out.push(key);
    }
    out
}

// --- 5. RULES.md carries no values ----------------------------------------

/// Numbers change at every rebalance; the rules they are plugged into do not.
/// A value written here is a second home for a number that already has one, and
/// the two will disagree.
fn rules_have_no_values(root: &Path) -> Result<Vec<Finding>, String> {
    let text = read(root, "RULES.md")?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let stripped = strip_references(line);
        // Only a *bare* number is a value. `u16`, `blake3` and `2.5D` are names
        // that happen to contain a digit, and they carry no balancing.
        let value = stripped
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_'))
            .find(|t| {
                t.chars().any(|c| c.is_ascii_digit())
                    && t.chars().all(|c| c.is_ascii_digit() || c == '.')
            });
        if let Some(value) = value {
            out.push(Finding {
                check: "rules-no-values",
                at: format!("RULES.md:{}", n + 1),
                what: format!(
                    "contains the value {value}. RULES.md names parameters, never their values: \"{}\"",
                    line.trim()
                ),
            });
        }
    }
    Ok(out)
}

/// Removes the forms in which a digit is a reference and not a value: link
/// targets, an id letter followed by digits, and `phase 15` / `step 6.1` /
/// `slot 14.5`.
fn strip_references(line: &str) -> String {
    let mut s = String::new();
    let mut rest = line;
    // Markdown link targets, which carry anchors full of digits.
    while let Some(open) = rest.find("](") {
        s.push_str(&rest[..open]);
        match rest[open..].find(')') {
            Some(close) => rest = &rest[open + close + 1..],
            None => {
                rest = "";
                break;
            }
        }
    }
    s.push_str(rest);

    for word in [
        "phases", "phase", "steps", "step", "slots", "slot", "levels", "level",
    ] {
        while let Some(pos) = s.to_ascii_lowercase().find(word) {
            let after = &s[pos + word.len()..];
            let trimmed = after.trim_start();
            let digits: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if digits.is_empty() {
                // Not a reference: neutralise the word so the loop advances.
                s.replace_range(pos..pos + word.len(), &"#".repeat(word.len()));
                continue;
            }
            let eaten = after.len() - trimmed.len() + digits.len();
            s.replace_range(pos..pos + word.len() + eaten, "");
        }
    }

    // `D4`, `M1` — an id letter followed by digits.
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let is_id_letter = matches!(c, 'A' | 'D' | 'M' | 'a' | 'd' | 'm');
        let boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if is_id_letter && boundary {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && (j >= chars.len() || !chars[j].is_ascii_alphanumeric()) {
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

// --- 6. ARCHITECTURE.md and the real tick ---------------------------------

/// Reordering a step silently invalidates every recorded replay, and a step
/// quietly filled in makes the document describe a game that no longer exists.
fn architecture_matches_the_tick(root: &Path) -> Result<Vec<Finding>, String> {
    let tick = read(root, "sim-core/src/tick.rs")?;
    let arch = read(root, "ARCHITECTURE.md")?;
    let mut out = Vec::new();

    // The calls inside `step`, in order, each tagged with its step number.
    let mut steps: Vec<(u32, String)> = Vec::new();
    let mut inside = false;
    for line in tick.lines() {
        if line.starts_with("pub fn step(") {
            inside = true;
            continue;
        }
        if inside && line == "}" {
            break;
        }
        if !inside {
            continue;
        }
        let Some((code, comment)) = line.split_once("//") else {
            continue;
        };
        let Ok(n) = comment.trim().parse::<u32>() else {
            continue;
        };
        let name = code.trim().split('(').next().unwrap_or("").trim();
        if !name.is_empty() {
            steps.push((n, name.to_string()));
        }
    }

    if steps.len() != 10 {
        out.push(Finding {
            check: "tick-order",
            at: "sim-core/src/tick.rs".to_string(),
            what: format!(
                "found {} numbered steps in step(), expected 10",
                steps.len()
            ),
        });
        return Ok(out);
    }
    for (i, (n, _)) in steps.iter().enumerate() {
        let expected = u32::try_from(i + 1).unwrap_or(0);
        if *n != expected {
            out.push(Finding {
                check: "tick-order",
                at: "sim-core/src/tick.rs".to_string(),
                what: format!("step {n} is in position {expected} of step()"),
            });
        }
    }

    // A step is empty when its definition is a one-line `{}`.
    let implemented: Vec<bool> = steps
        .iter()
        .map(|(_, name)| {
            let needle = format!("fn {name}(");
            !tick
                .lines()
                .filter(|l| l.contains(&needle))
                .any(|l| l.trim_end().ends_with("{}"))
        })
        .collect();

    // The table in ARCHITECTURE.md.
    let mut documented: Vec<(u32, bool)> = Vec::new();
    for line in arch.lines() {
        if !line.starts_with("| ") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let (Some(num), Some(state)) = (cells.get(1), cells.get(3)) else {
            continue;
        };
        let Ok(n) = num.trim().parse::<u32>() else {
            continue;
        };
        if (1..=10).contains(&n) && documented.len() < 10 {
            documented.push((n, state.contains("yes")));
        }
    }

    if documented.len() != 10 {
        out.push(Finding {
            check: "tick-order",
            at: "ARCHITECTURE.md".to_string(),
            what: format!(
                "the tick table has {} rows numbered 1-10, expected 10",
                documented.len()
            ),
        });
        return Ok(out);
    }
    for (i, (n, doc_impl)) in documented.iter().enumerate() {
        let expected = u32::try_from(i + 1).unwrap_or(0);
        if *n != expected {
            out.push(Finding {
                check: "tick-order",
                at: "ARCHITECTURE.md".to_string(),
                what: format!("step {n} is in row {expected} of the tick table"),
            });
            continue;
        }
        if implemented.get(i) != Some(doc_impl) {
            out.push(Finding {
                check: "tick-order",
                at: "ARCHITECTURE.md".to_string(),
                what: format!(
                    "step {n} is {} in tick.rs but the table says {}",
                    yes_no(implemented.get(i).copied().unwrap_or(false)),
                    yes_no(*doc_impl)
                ),
            });
        }
    }
    Ok(out)
}

fn yes_no(b: bool) -> &'static str {
    if b { "implemented" } else { "empty" }
}

// --- 7. every task says what it is, in fields -----------------------------

/// What a task file states about itself, in fields rather than in prose.
///
/// The old status line answered two questions with one word: `closed` and
/// `index` said what the *document* was, `implemented` and `not yet built` what
/// the *work* was. One field answering two questions is how a document ends up
/// disagreeing with itself, so `kind` answers the first and `status` the
/// second.
#[derive(Debug)]
struct Task {
    /// The file's own name, e.g. `14.4-building-kind.md`. Used for reporting
    /// and for the check that the id and the name are the same number.
    file: String,
    id: PhaseNumber,
    kind: String,
    status: String,
    /// `None` while the work is still to do. Its presence is the same statement
    /// as the status, checked against it, and against the roadmap's row.
    closed: Option<String>,
}

/// What a task can be. `open-question` is a question a later task has to
/// answer, and it is what a roadmap slot has to point at. `constitution` is the
/// one task that holds rules rather than a record of work, and there is exactly
/// one: it is where every `D<n>` cited from the sources resolves.
const TASK_KINDS: [&str; 3] = ["phase", "open-question", "constitution"];

/// How far a task has got. There is deliberately no `closed` and no `index`:
/// those two labels described documents that were not tasks at all, and an
/// unnumbered document is no longer allowed in the plan directory.
const TASK_STATUSES: [&str; 3] = ["implemented", "not-yet-built", "superseded"];

/// The fields every task carries, in the order they are written.
const TASK_FIELDS: [&str; 4] = ["id", "kind", "status", "opened"];

/// Work that is finished says when it finished. Work that is not cannot, and a
/// date on it would be a guess presented as a record.
fn expects_closed(status: &str) -> bool {
    status == "implemented" || status == "superseded"
}

/// The `key: value` block a task file opens with, between two `---` lines.
///
/// Deliberately not a YAML parser. A task header is four or five flat string
/// fields, and everything a real parser would additionally accept — nesting,
/// anchors, four spellings of `true` — is surface with nothing behind it.
fn front_matter(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err("does not open with a `---` front matter block".to_string());
    }
    let mut out = Vec::new();
    for line in lines {
        if line == "---" {
            return Ok(out);
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("front matter line \"{line}\" is not `key: value`"));
        };
        out.push((key.trim().to_string(), value.trim().to_string()));
    }
    Err("front matter is never closed by a second `---`".to_string())
}

/// `2026-08-14`, and nothing else. A date that sorts as text is a date two
/// checks can compare without either of them owning a calendar.
fn is_iso_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// One task file's header, or the first thing wrong with it.
fn read_task(file: &str, text: &str) -> Result<Task, String> {
    let fields = front_matter(text)?;
    for (i, (key, _)) in fields.iter().enumerate() {
        if fields[..i].iter().any(|(seen, _)| seen == key) {
            return Err(format!("front matter names `{key}` twice"));
        }
    }
    let value = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim());

    let mut expected: Vec<&str> = TASK_FIELDS.to_vec();
    let status = value("status").unwrap_or("").to_string();
    if expects_closed(&status) {
        expected.push("closed");
    }
    for key in &expected {
        if value(key).is_none() {
            return Err(format!("front matter has no `{key}`"));
        }
    }
    for (key, _) in &fields {
        if !expected.contains(&key.as_str()) {
            return Err(match key.as_str() {
                "closed" => format!("is \"{status}\", so it cannot carry a `closed` date"),
                _ => format!("front matter carries `{key}`, which is not one of {expected:?}"),
            });
        }
    }

    let id = value("id").unwrap_or("");
    let id = PhaseNumber::leading(id).ok_or_else(|| format!("`id: {id}` is not a number"))?;
    let kind = value("kind").unwrap_or("").to_string();
    if !TASK_KINDS.contains(&kind.as_str()) {
        return Err(format!("`kind: {kind}` is not one of {TASK_KINDS:?}"));
    }
    if !TASK_STATUSES.contains(&status.as_str()) {
        return Err(format!(
            "`status: {status}` is not one of {TASK_STATUSES:?}"
        ));
    }
    let opened = value("opened").unwrap_or("").to_string();
    if !is_iso_date(&opened) {
        return Err(format!("`opened: {opened}` is not a YYYY-MM-DD date"));
    }
    let closed = value("closed").map(str::to_string);
    if let Some(closed) = &closed {
        if !is_iso_date(closed) {
            return Err(format!("`closed: {closed}` is not a YYYY-MM-DD date"));
        }
        // Both are ISO, so text order is date order.
        if closed < &opened {
            return Err(format!(
                "closed on {closed}, before it was opened on {opened}"
            ));
        }
    }
    Ok(Task {
        file: file.to_string(),
        id,
        kind,
        status,
        closed,
    })
}

/// Every markdown file in the plan directory, read as a task. The ones that
/// parse come back as tasks; the ones that do not come back as findings, so a
/// broken header is reported once as itself rather than again as everything it
/// made disagree.
fn read_tasks(root: &Path) -> Result<(Vec<Task>, Vec<Finding>), String> {
    let plan = root.join("plan");
    let mut files: Vec<PathBuf> = fs::read_dir(&plan)
        .map_err(|e| format!("{}: {e}", plan.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    // In the order the numbers really run. The names are written so that byte
    // order agrees with that, but this sort deliberately does not lean on it:
    // it compares the numbers, so a name that breaks the rule is still read in
    // its real place and reported for what it is rather than for what it made
    // the ordering do.
    files.sort_by_key(|p| {
        let name = file_name(p);
        (PhaseNumber::leading(&name), name)
    });

    let mut tasks = Vec::new();
    let mut findings = Vec::new();
    for path in &files {
        let name = file_name(path);
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        match read_task(&name, &text) {
            Ok(task) => tasks.push(task),
            Err(what) => findings.push(Finding {
                check: "task-header",
                at: name,
                what,
            }),
        }
    }
    Ok((tasks, findings))
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// A task's id is the number its own file name opens with.
///
/// The id is stated twice on purpose — once where a reader finds it and once
/// where a link finds it — and this is what stops the two drifting apart.
fn tasks_agree_with_their_names(tasks: &[Task]) -> Vec<Finding> {
    let mut out = Vec::new();
    for task in tasks {
        match PhaseNumber::leading(&task.file) {
            Some(from_name) if from_name == task.id => {}
            Some(from_name) => out.push(Finding {
                check: "task-header",
                at: task.file.clone(),
                what: format!(
                    "says `id: {}`, but its name opens with {from_name}",
                    task.id
                ),
            }),
            None => out.push(Finding {
                check: "task-header",
                at: task.file.clone(),
                what: format!(
                    "says `id: {}`, but its name does not open with a number. Every document \
                     under plan is a task and carries its id in its name.",
                    task.id
                ),
            }),
        }
    }
    out
}

/// A task's name is written so that byte order and the order the numbers really
/// run are the same order: the first number with two digits, every number after
/// it with one.
///
/// This checker compares the components as numbers and would order `14.10`
/// after `14.9` quite happily. Nothing else would. A shell listing, an editor's
/// sidebar and a file list on the web all compare bytes, and under byte
/// comparison `14.10-...` lands before `14.4-...` — so a rule kept privately
/// here would be contradicted publicly by every listing a reader ever opens, and
/// the tree grew exactly one such file before this check existed. Held to, the
/// two orders agree: `-` sorts before `.`, so a whole phase precedes its own
/// half numbers, and the slot above `.9` descends a level (`14.9.5`) instead of
/// reaching a second digit. The slots between two phases never run out; they run
/// deeper rather than wider.
///
/// It is the **name** that a listing sorts, so it is the name that is checked.
/// The id in the front matter arrives here parsed and no longer knows how many
/// digits it was written with, and the check that the two agree is a separate
/// one.
fn task_names_sort_the_way_the_numbers_run(tasks: &[Task]) -> Vec<Finding> {
    let mut out = Vec::new();
    for task in tasks {
        let parts = leading_components(&task.file);
        let head = parts.join(".");
        // A name that opens with no number at all is already reported as itself
        // by the check that a task agrees with its name.
        if parts.iter().any(|p| p.is_empty()) {
            continue;
        }
        if parts[0].len() != 2 {
            out.push(Finding {
                check: "id-shape",
                at: task.file.clone(),
                what: format!(
                    "`{head}` does not open with two digits. The first number is written with \
                     two — `09`, not `9` — so that a listing, which compares bytes, runs in the \
                     same order as the numbers do."
                ),
            });
        }
        if parts[1..].iter().any(|p| p.len() != 1) {
            out.push(Finding {
                check: "id-shape",
                at: task.file.clone(),
                what: format!(
                    "`{head}` writes a number after the first with more than one digit, and a \
                     listing compares bytes: `14.10` lands before `14.4`. A number after the \
                     first is a single digit — descend a level instead, the way `14.9.5` sits \
                     above `14.9`."
                ),
            });
        }
    }
    out
}

// --- 8. ROADMAP.md and the tasks agree ------------------------------------

/// The roadmap's "Done" rows and the tasks they name state the same dates, and
/// the line saying how far the tree has got is the highest task that claims to
/// be built.
///
/// A date used to be written in three places — the row, the status line, and
/// sometimes the file name — and no two of them agreed. Two are left, because
/// the roadmap is read as a table and a task is read on its own, and this is
/// what stops those two drifting.
///
/// **What this cannot check.** A row whose `#` is neither a number nor a range
/// of them names work that ran without a task file, and there is nothing to
/// compare it against. Such a row is skipped, and it is written down here
/// rather than left to be discovered: a check that quietly covers less than it
/// appears to is the failure this file exists to prevent.
fn roadmap_agrees_with_the_tasks(root: &Path, tasks: &[Task]) -> Result<Vec<Finding>, String> {
    let roadmap = read(root, "ROADMAP.md")?;
    let mut out = Vec::new();

    let claimed = roadmap
        .lines()
        .find_map(|l| l.split("Implemented through phase ").nth(1))
        .and_then(PhaseNumber::leading);
    let highest = tasks
        .iter()
        .filter(|t| t.status == "implemented")
        .map(|t| &t.id)
        .max();
    match (claimed, highest) {
        (None, _) => out.push(Finding {
            check: "roadmap-tasks",
            at: "ROADMAP.md".to_string(),
            what: "no \"Implemented through phase N\" line".to_string(),
        }),
        (Some(n), None) => out.push(Finding {
            check: "roadmap-tasks",
            at: "ROADMAP.md".to_string(),
            what: format!("says phase {n}, but no task is marked implemented"),
        }),
        (Some(n), Some(highest)) if &n != highest => out.push(Finding {
            check: "roadmap-tasks",
            at: "ROADMAP.md".to_string(),
            what: format!("says phase {n}, but the highest implemented task is {highest}"),
        }),
        (Some(_), Some(_)) => {}
    }

    let mut covered: Vec<&PhaseNumber> = Vec::new();
    for line in done_rows(&roadmap) {
        let cells: Vec<&str> = line.split('|').collect();
        let (Some(number), Some(when)) = (cells.get(1), cells.get(3)) else {
            continue;
        };
        let when = when.trim();
        let Some((first, last)) = row_ids(number) else {
            continue;
        };
        let named: Vec<&Task> = tasks
            .iter()
            .filter(|t| t.id >= first && t.id <= last)
            .collect();
        if named.is_empty() {
            out.push(Finding {
                check: "roadmap-tasks",
                at: "ROADMAP.md".to_string(),
                what: format!("row \"{}\" names no task that exists", number.trim()),
            });
            continue;
        }
        for task in named {
            covered.push(&task.id);
            match &task.closed {
                Some(closed) if closed == when => {}
                Some(closed) => out.push(Finding {
                    check: "roadmap-tasks",
                    at: task.file.clone(),
                    what: format!("closed on {closed}, but the roadmap row says {when}"),
                }),
                None => out.push(Finding {
                    check: "roadmap-tasks",
                    at: task.file.clone(),
                    what: format!(
                        "is in the roadmap's \"Done\" table, but its status is \"{}\"",
                        task.status
                    ),
                }),
            }
        }
    }

    for task in tasks.iter().filter(|t| t.status == "implemented") {
        if !covered.contains(&&task.id) {
            out.push(Finding {
                check: "roadmap-tasks",
                at: task.file.clone(),
                what: "says it is implemented, but no row of the roadmap's \"Done\" table \
                       names it. The roadmap is the only file that states how far the tree \
                       has got, so work missing from it did not happen."
                    .to_string(),
            });
        }
    }
    Ok(out)
}

/// The body rows of the roadmap's "Done" table: everything between that heading
/// and the next one, minus the header row and the dashes under it.
fn done_rows(roadmap: &str) -> impl Iterator<Item = &str> {
    roadmap
        .lines()
        .skip_while(|l| !l.starts_with("## Done"))
        .skip(1)
        .take_while(|l| !l.starts_with("## "))
        .filter(|l| l.starts_with('|'))
        .filter(|l| {
            let first = l.split('|').nth(1).map(str::trim).unwrap_or("");
            first != "#" && !first.starts_with("---")
        })
}

/// The first and last task a `#` cell names: `14.4` on its own, or both ends of
/// a range like `00–09`. `None` for a cell that names no number at all, which
/// is a batch that ran without a task file.
fn row_ids(cell: &str) -> Option<(PhaseNumber, PhaseNumber)> {
    let cell = cell.trim().trim_matches('*').trim();
    let (first, last) = match cell.split_once(['–', '—', '-']) {
        Some((first, last)) => (first, last),
        None => (cell, cell),
    };
    Some((
        PhaseNumber::leading(first.trim())?,
        PhaseNumber::leading(last.trim())?,
    ))
}

// --- 9. the frozen declaration orders -------------------------------------

/// The highest-value check here, and the only one that guards the simulation
/// rather than the prose: reordering one of these enums silently changes every
/// state hash, and nothing about it looks like an error.
fn frozen_orders_match_the_code(
    root: &Path,
    sources: &[(PathBuf, String)],
) -> Result<Vec<Finding>, String> {
    let glossary = read(root, "GLOSSARY.md")?;
    let mut out = Vec::new();
    let mut checked = 0;

    for line in glossary.lines() {
        if !line.starts_with("| Declaration order of") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let (Some(what), Some(value)) = (cells.get(1), cells.get(2)) else {
            continue;
        };
        let Some(ty) = backticked(what).into_iter().next() else {
            continue;
        };
        let ty = ty.split("::").next().unwrap_or(&ty).to_string();
        let documented: Vec<String> = capitalised(value);
        let Some(actual) = declared_order(sources, &ty) else {
            out.push(Finding {
                check: "frozen-order",
                at: "GLOSSARY.md".to_string(),
                what: format!("`{ty}` has a frozen order but no `const ALL` was found for it"),
            });
            continue;
        };
        checked += 1;
        if documented != actual {
            out.push(Finding {
                check: "frozen-order",
                at: "GLOSSARY.md".to_string(),
                what: format!(
                    "`{ty}` is declared {actual:?} but GLOSSARY.md freezes it as {documented:?}"
                ),
            });
        }
    }

    if checked == 0 {
        out.push(Finding {
            check: "frozen-order",
            at: "GLOSSARY.md".to_string(),
            what: "no \"Declaration order of\" row was found to check".to_string(),
        });
    }
    Ok(out)
}

/// The variants of `ty` in the order its `const ALL` lists them.
fn declared_order(sources: &[(PathBuf, String)], ty: &str) -> Option<Vec<String>> {
    let head = format!("impl {ty} {{");
    for (_, text) in sources {
        let Some(start) = text.find(&head) else {
            continue;
        };
        let body = &text[start..];
        let all = body.find("const ALL")?;
        let open = body[all..].find('[')?;
        // The array type, e.g. `[RngKind; 4]`, comes first; the values follow.
        let after_ty = body[all + open + 1..].find('[')? + all + open + 2;
        let close = body[after_ty..].find(']')? + after_ty;
        return Some(
            body[after_ty..close]
                .split(',')
                .filter_map(|v| v.trim().rsplit("::").next())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect(),
        );
    }
    None
}

// --- 10. the skill teaches the header the code accepts --------------------

/// Where the procedure for opening and closing a task is written down.
const TASK_SKILL: &str = ".claude/skills/task/SKILL.md";

/// The skill carries a front matter template, and a template is a second copy
/// of what [`read_task`] accepts.
///
/// A second copy of a rule that nothing keeps in agreement with the first is a
/// fork waiting to be noticed, which is why the decision register was removed
/// rather than kept up to date. The same argument applies here, so the
/// agreement is a check: the template names the fields the code requires, and
/// the two lines listing the kinds and the statuses name exactly the values the
/// code allows. Drift one of the three and this fails, naming both sides.
///
/// It is a check and not a comment for the reason the balancing numbers taught:
/// if you find yourself writing a comment explaining why two things must agree,
/// write a check instead.
fn skill_matches_the_task_header(root: &Path) -> Result<Vec<Finding>, String> {
    let Ok(text) = fs::read_to_string(root.join(TASK_SKILL)) else {
        return Ok(vec![Finding {
            check: "skill-template",
            at: TASK_SKILL.to_string(),
            what: "is missing, so nothing states the header a task has to carry".to_string(),
        }]);
    };
    Ok(skill_states_the_header(&text))
}

/// The three comparisons, on the text alone so that each of them can be shown
/// failing without a file on disk to fail with.
fn skill_states_the_header(text: &str) -> Vec<Finding> {
    let mut out = Vec::new();

    // The template is the first fenced block tagged `yaml`. It shows finished
    // work, which is the only shape carrying every field at once.
    let mut expected: Vec<&str> = TASK_FIELDS.to_vec();
    expected.push("closed");
    match fenced(text, "yaml").as_deref().map(front_matter) {
        Some(Ok(fields)) => {
            let named: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
            if named != expected {
                out.push(Finding {
                    check: "skill-template",
                    at: TASK_SKILL.to_string(),
                    what: format!("shows a header of {named:?}, but a task carries {expected:?}"),
                });
            }
        }
        Some(Err(what)) => out.push(Finding {
            check: "skill-template",
            at: TASK_SKILL.to_string(),
            what: format!("shows a header that does not read: {what}"),
        }),
        None => out.push(Finding {
            check: "skill-template",
            at: TASK_SKILL.to_string(),
            what: "has no fenced `yaml` block, so it shows no header at all".to_string(),
        }),
    }

    // Each of the two enums is listed on one line, in backticks, introduced by
    // the field it belongs to. The prose after the list carries no backticks,
    // so everything but the first span is a value.
    for (field, allowed) in [
        ("kind", TASK_KINDS.as_slice()),
        ("status", TASK_STATUSES.as_slice()),
    ] {
        let opening = format!("- `{field}` is one of ");
        let Some(line) = text.lines().find(|l| l.starts_with(&opening)) else {
            out.push(Finding {
                check: "skill-template",
                at: TASK_SKILL.to_string(),
                what: format!(
                    "has no line opening \"{opening}\", so {allowed:?} is written nowhere"
                ),
            });
            continue;
        };
        let listed: Vec<String> = backticked(line).into_iter().skip(1).collect();
        if listed != allowed {
            out.push(Finding {
                check: "skill-template",
                at: TASK_SKILL.to_string(),
                what: format!(
                    "says `{field}` is one of {listed:?}, but the code allows {allowed:?}"
                ),
            });
        }
    }
    out
}

// --- shared helpers -------------------------------------------------------

fn read(root: &Path, rel: &str) -> Result<String, String> {
    let path = root.join(rel);
    fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every `.rs`, `.ron` and `.toml` that is source rather than recording.
fn source_files(root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out = Vec::new();
    for crate_dir in ["sim-core", "sim-data", "sim-replay", "xtask"] {
        walk(&root.join(crate_dir), root, &mut out)?;
    }
    for file in ["Cargo.toml", "clippy.toml"] {
        let path = root.join(file);
        if let Ok(text) = fs::read_to_string(&path) {
            out.push((PathBuf::from(file), text));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(PathBuf, String)>) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            // `expected/` holds the recordings: data, not source.
            if name == "target" || name == "expected" {
                continue;
            }
            walk(&path, root, out)?;
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "ron" | "toml") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        out.push((rel, text));
    }
    Ok(())
}

/// Words, split on anything that cannot appear in an identifier.
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// The contents of every `` `...` `` span on the line.
fn backticked(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        out.push(rest[..close].to_string());
        rest = &rest[close + 1..];
    }
    out
}

/// The body of the first fenced block tagged `tag`, without its two fences.
///
/// `None` when the block is never opened, and `None` when it is opened and left
/// empty — a block with nothing in it states nothing, and reporting it as
/// missing says the same thing more usefully than an empty comparison would.
fn fenced(text: &str, tag: &str) -> Option<String> {
    let opening = format!("```{tag}");
    let mut lines = text.lines().skip_while(|l| l.trim_end() != opening);
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("```")).collect();
    (!body.is_empty()).then(|| body.join("\n"))
}

/// Identifiers that begin with a capital, in order of appearance.
fn capitalised(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .collect()
}

// --- 11. what a terrain allows ---------------------------------------------

/// `RULES.md` states, terrain by terrain, whether a building may stand on it
/// and whether a road may be laid on it. Those answers used to be rows in
/// `terrain.ron`, and the parameter check above kept the page and the table
/// honest about each other. They are facts about the `Terrain` enum now, and
/// nothing was left watching them: the table could say rock is buildable and
/// every other check would pass.
///
/// So ask the code. This check does not parse Rust the way the frozen-order
/// check has to — `xtask` depends on `sim-core`, so it calls the two methods
/// and compares their answers against the page.
fn rules_match_what_a_terrain_allows(root: &Path) -> Result<Vec<Finding>, String> {
    let rules = read(root, "RULES.md")?;
    let mut out = Vec::new();

    for t in Terrain::ALL {
        let name = format!("{t:?}");
        let row = rules.lines().find(|l| {
            l.starts_with("| `") && backticked(l).first().is_some_and(|first| *first == name)
        });
        let Some(row) = row else {
            out.push(Finding {
                check: "terrain-facts",
                at: "RULES.md".to_string(),
                what: format!("`{name}` exists but RULES.md has no row for it"),
            });
            continue;
        };
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        let (Some(buildable), Some(walkable)) = (cells.get(2), cells.get(3)) else {
            out.push(Finding {
                check: "terrain-facts",
                at: "RULES.md".to_string(),
                what: format!("`{name}`'s row does not have the two answer cells"),
            });
            continue;
        };
        for (cell, actual, what) in [
            (buildable, t.is_buildable(), "a building may stand on it"),
            (walkable, t.is_walkable(), "a road may be laid on it"),
        ] {
            let said = match *cell {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            };
            if said != Some(actual) {
                out.push(Finding {
                    check: "terrain-facts",
                    at: "RULES.md".to_string(),
                    what: format!(
                        "`{name}`: RULES.md says {cell:?} where the code says \
                         {actual} to \"{what}\""
                    ),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_read_as_whole_tokens() {
        // The two literals below are spelled in pieces for the reason the plan
        // path is on line 138: this file is scanned by the check that refuses a
        // leftover `A` id, and a whole one written here would be reported as a
        // real citation in the sources.
        assert_eq!(
            cited_ids(concat!("counted on residents (", "A", "12)")),
            vec![('A', 12)]
        );
        assert_eq!(
            cited_ids(concat!("no floats (D4), see ", "A", "5.")),
            vec![('D', 4), ('A', 5)]
        );
        // Not a citation: part of a word, or a hex literal.
        assert!(cited_ids("SHA1 and 0xD4 and DATA1").is_empty());
    }

    #[test]
    fn half_numbers_are_read_as_numbers() {
        let p = |s: &str| PhaseNumber::leading(s).expect("parses");
        assert!(p("14-births-deaths.md") < p("14.4-building-kind.md"));
        assert!(p("14.4") < p("15-migration.md"));
        // Component-wise and not decimal, which is the whole reason the numbers
        // are parsed rather than compared as text: a whole phase carries no
        // padding beyond its two digits, and a half number can run deeper than
        // one component.
        assert!(p("09-invariants-closeout.md") < p("10-beyond-m0.md"));
        assert!(p("14.9") < p("14.9.5"));
        assert!(p("14.9.5") < p("15"));
        assert!(p("14.5") < p("14.5.5") && p("14.5.5") < p("14.6"));
        assert_eq!(p("14.4-building-kind.md").to_string(), "14.4");
        // A sentence's full stop is not part of the number.
        assert_eq!(p("14. Everything up to"), p("14"));
        assert!(p("14.5").is_fractional() && !p("15").is_fractional());
        // Not numbered at all, and so never the highest.
        assert!(PhaseNumber::leading("bug-hunt-2026-08-11.md").is_none());
        assert!(PhaseNumber::leading("README.md").is_none());
        assert!(PhaseNumber::leading("14..4").is_none());
    }

    #[test]
    fn a_name_sorts_the_way_its_number_runs() {
        let task = |file: &str| Task {
            file: file.to_string(),
            id: PhaseNumber::leading(file).expect("parses"),
            kind: "phase".to_string(),
            status: "implemented".to_string(),
            closed: Some("2026-08-15".to_string()),
        };
        let complaints = |name: &str| task_names_sort_the_way_the_numbers_run(&[task(name)]).len();

        // Two digits at the front, one in every number after it.
        assert_eq!(complaints("09-invariants-closeout.md"), 0);
        assert_eq!(complaints("14.4-building-kind.md"), 0);
        assert_eq!(complaints("14.5.5-the-constitution.md"), 0);
        assert_eq!(complaints("14.9.5-writing-a-task-is-a-procedure.md"), 0);
        // A second digit after the first: byte order puts it before `14.4`.
        assert_eq!(complaints("14.10-writing-a-task-is-a-procedure.md"), 1);
        // One digit at the front: byte order puts it after `10`.
        assert_eq!(complaints("9-invariants-closeout.md"), 1);
    }

    #[test]
    fn a_name_that_keeps_the_shape_sorts_the_same_by_bytes_and_by_number() {
        // This is what the shape rule buys, and the reason it is worth a check:
        // `ls` and an editor's sidebar never parse a number, and they are what a
        // reader actually looks at.
        let in_the_order_the_numbers_run = [
            "09-invariants-closeout.md",
            "10-beyond-m0.md",
            "14-births-deaths.md",
            "14.4-building-kind.md",
            "14.5-an-empty-house-consumes-no-capacity.md",
            "14.5.5-the-constitution.md",
            "14.6-a-house-is-covered-by-services.md",
            "14.9-decisions-live-where-they-are-cited.md",
            "14.9.5-writing-a-task-is-a-procedure.md",
            "14.9.6-an-id-sorts-the-same-way-everywhere.md",
            "15-migration.md",
        ];

        let mut by_number = in_the_order_the_numbers_run.to_vec();
        by_number.sort_by_key(|n| PhaseNumber::leading(n).expect("parses"));
        assert_eq!(by_number, in_the_order_the_numbers_run);

        let mut by_bytes = in_the_order_the_numbers_run.to_vec();
        by_bytes.sort_unstable();
        assert_eq!(by_bytes, in_the_order_the_numbers_run);
    }

    #[test]
    fn a_slot_is_cited_only_when_a_word_introduces_it() {
        let n = |s: &str| PhaseNumber::leading(s).expect("parses");
        assert_eq!(
            cited_slots("what slot 14.5 is about"),
            [("slot", n("14.5"))]
        );
        assert_eq!(
            cited_slots("since phase 14.4: a provider"),
            [("phase", n("14.4"))]
        );
        assert_eq!(
            cited_slots("when phase 15 answers it."),
            [("phase", n("15"))]
        );
        assert_eq!(cited_slots("Phases 14 and 15"), [("phase", n("14"))]);
        // The deliberate limit: a bare number is invisible, because matching one
        // would report every version number and every measured millisecond.
        assert!(cited_slots("when 14.5 is answered").is_empty());
        assert!(cited_slots("step 6.1 costs 14.5 ms with rand_pcg 0.3").is_empty());
        // Not the word, only part of one.
        assert!(cited_slots("rephrase 14").is_empty());
    }

    /// A well-formed header, used as the starting point for the cases below so
    /// that each of them differs from a passing file in exactly one way.
    fn header(extra: &str) -> String {
        format!(
            "---\nid: 14.4\nkind: phase\nstatus: implemented\nopened: 2026-08-13\nclosed: 2026-08-14\n{extra}---\n\n# Phase 14.4\n"
        )
    }

    #[test]
    fn a_task_states_its_id_its_kind_and_its_dates() {
        let task = read_task("14.4-building-kind.md", &header("")).expect("a task");
        assert_eq!(task.id, PhaseNumber::leading("14.4").expect("parses"));
        assert_eq!(task.kind, "phase");
        assert_eq!(task.closed.as_deref(), Some("2026-08-14"));

        // A document that opens with prose has no header at all, which is the
        // shape every file had before the front matter arrived.
        let err = read_task("14.4-x.md", "# Phase 14.4\n").expect_err("no header");
        assert!(err.contains("front matter"), "{err}");
    }

    #[test]
    fn a_date_is_a_statement_the_status_has_to_agree_with() {
        // Unfinished work cannot say when it finished.
        let open = "---\nid: 15\nkind: phase\nstatus: not-yet-built\nopened: 2026-08-09\nclosed: 2026-08-14\n---\n";
        let err = read_task("15-migration.md", open).expect_err("closed while open");
        assert!(err.contains("cannot carry a `closed` date"), "{err}");

        // Finished work has to.
        let done = "---\nid: 15\nkind: phase\nstatus: implemented\nopened: 2026-08-09\n---\n";
        let err = read_task("15-migration.md", done).expect_err("no closed date");
        assert!(err.contains("no `closed`"), "{err}");

        // And it cannot have finished before it started.
        let backwards = header("").replace("closed: 2026-08-14", "closed: 2026-08-12");
        let err = read_task("14.4-x.md", &backwards).expect_err("closed before opened");
        assert!(err.contains("before it was opened"), "{err}");

        for bad in ["2026-8-14", "14-08-2026", "yesterday"] {
            let text = header("").replace("closed: 2026-08-14", &format!("closed: {bad}"));
            assert!(read_task("14.4-x.md", &text).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn a_task_is_read_only_as_the_fields_it_may_carry() {
        let err = read_task("14.4-x.md", &header("size: M\n")).expect_err("unknown field");
        assert!(err.contains("size"), "{err}");
        let err = read_task("14.4-x.md", &header("kind: phase\n")).expect_err("twice");
        assert!(err.contains("twice"), "{err}");
        let bad = header("").replace("kind: phase", "kind: review");
        assert!(read_task("14.4-x.md", &bad).is_err());
        let bad = header("").replace("status: implemented", "status: closed");
        assert!(read_task("14.4-x.md", &bad).is_err());
    }

    #[test]
    fn a_roadmap_row_names_every_task_inside_a_range() {
        let ends = |cell| {
            let (first, last) = row_ids(cell).expect("a range");
            (first.to_string(), last.to_string())
        };
        // An en dash in the roadmap, a hyphen if anyone types one.
        assert_eq!(ends("00–09"), ("0".to_string(), "9".to_string()));
        assert_eq!(ends("00-09"), ("0".to_string(), "9".to_string()));
        // A lone number is a range of one, so both ends read the same.
        assert_eq!(ends("14.4"), ("14.4".to_string(), "14.4".to_string()));
        assert_eq!(ends("**18.5**"), ("18.5".to_string(), "18.5".to_string()));
        // A batch that ran without a task file names nothing to compare against.
        assert!(row_ids("—").is_none());

        let inside = |id: &str| {
            let (first, last) = row_ids("00–09").expect("a range");
            let id = PhaseNumber::leading(id).expect("parses");
            id >= first && id <= last
        };
        assert!(inside("00") && inside("09"));
        assert!(!inside("10") && !inside("14.4"));
    }

    #[test]
    fn a_roadmap_row_names_the_document_holding_the_argument() {
        let target = concat!("plan", "/").to_string() + "14.5-empty.md";
        let row = format!("**Open question — [An empty house]({target}):** does it?");
        assert_eq!(plan_link(&row).as_deref(), Some(target.as_str()));
        assert!(plan_link("**Open question — nothing linked**").is_none());
    }

    #[test]
    fn references_are_not_values() {
        let line = concat!("see ", "A", "12 and D4, phase 15, step 6.1");
        assert!(!strip_references(line).contains(['1', '4', '5']));
        assert!(strip_references("the rate is 12 per thousand").contains("12"));
    }

    #[test]
    fn backticks_and_capitals() {
        assert_eq!(backticked("a `b` c `d`"), vec!["b", "d"]);
        assert_eq!(capitalised("Births, Deaths"), vec!["Births", "Deaths"]);
    }

    #[test]
    fn a_fenced_block_is_read_without_its_fences() {
        let text = "before\n```yaml\nid: 1\n```\nafter\n```yaml\nsecond\n```\n";
        assert_eq!(fenced(text, "yaml").as_deref(), Some("id: 1"));
        // A tag that is never opened, and one opened over nothing.
        assert!(fenced(text, "sh").is_none());
        assert!(fenced("```yaml\n```\n", "yaml").is_none());
    }

    /// A skill that states the header correctly, built from its three moving
    /// parts so that each case below differs from a passing file in exactly one
    /// way.
    fn skill(fields: &[&str], kinds: &[&str], statuses: &[&str]) -> String {
        let list = |values: &[&str]| {
            values
                .iter()
                .map(|v| format!("`{v}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let header = fields
            .iter()
            .map(|f| format!("{f}: x\n"))
            .collect::<String>();
        format!(
            "# Task\n\n```yaml\n---\n{header}---\n```\n\n\
             - `kind` is one of {}. It says what the document is.\n\
             - `status` is one of {}. It says how far the work got.\n",
            list(kinds),
            list(statuses),
        )
    }

    /// The header the skill teaches and the header the code accepts are the
    /// same header, or one of them is a fork nobody is keeping in agreement.
    #[test]
    fn the_skill_and_the_code_state_the_same_header() {
        let every_field = [TASK_FIELDS.as_slice(), &["closed"]].concat();
        let good = skill(&every_field, &TASK_KINDS, &TASK_STATUSES);
        assert!(skill_states_the_header(&good).is_empty());

        // One drift per case, and each names both sides in its message.
        let missing = skill(&TASK_FIELDS, &TASK_KINDS, &TASK_STATUSES);
        let found = skill_states_the_header(&missing);
        assert!(found[0].what.contains("closed"), "{}", found[0]);

        let stale = skill(&every_field, &["phase", "open-question"], &TASK_STATUSES);
        let found = skill_states_the_header(&stale);
        assert!(found[0].what.contains("constitution"), "{}", found[0]);

        let stale = skill(&every_field, &TASK_KINDS, &["implemented", "closed"]);
        let found = skill_states_the_header(&stale);
        assert!(found[0].what.contains("not-yet-built"), "{}", found[0]);

        // A skill that shows no header at all, and one whose lists are gone.
        assert_eq!(skill_states_the_header("# Task\n").len(), 3);
    }
}
