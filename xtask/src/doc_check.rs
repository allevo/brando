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
    f.extend(no_plan_paths_in_code(&sources));
    f.extend(decision_ids_exist(root, &sources)?);
    f.extend(open_decisions_are_scheduled(root)?);
    f.extend(rules_match_the_tables(root)?);
    f.extend(rules_have_no_values(root)?);
    f.extend(architecture_matches_the_tick(root)?);
    f.extend(plan_files_have_a_status(root)?);
    f.extend(frozen_orders_match_the_code(root, &sources)?);
    Ok(f)
}

// --- 1. the code never cites a document path ------------------------------

/// Paths rot when a file moves; ids do not. A comment cites `A12` and states
/// its rule in full where it cites it, never pointing at a phase document.
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

// --- 2. every cited id exists ---------------------------------------------

/// A citation that outlives its decision is worse than no citation: it looks
/// authoritative and resolves to nothing.
fn decision_ids_exist(root: &Path, sources: &[(PathBuf, String)]) -> Result<Vec<Finding>, String> {
    let decisions = read(root, "DECISIONS.md")?;
    let claude = read(root, "CLAUDE.md")?;
    let known_a = headed_ids(&decisions, "## A");
    let known_d = headed_ids(&claude, "### D");

    let mut out = Vec::new();
    for (path, text) in sources {
        for (n, line) in text.lines().enumerate() {
            for (letter, num) in cited_ids(line) {
                let known = if letter == 'A' { &known_a } else { &known_d };
                if !known.contains(&num) {
                    out.push(Finding {
                        check: "unknown-id",
                        at: format!("{}:{}", path.display(), n + 1),
                        what: format!(
                            "cites {letter}{num}, which is not defined in {}",
                            if letter == 'A' {
                                "DECISIONS.md"
                            } else {
                                "CLAUDE.md"
                            }
                        ),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// The numbers of every heading that starts with `prefix`, e.g. `## A12 — ...`.
fn headed_ids(text: &str, prefix: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// `A12` / `D4` appearing as whole tokens. Not inside an identifier, and not
/// after `0x`, so a hex literal is never mistaken for a citation.
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

// --- 3. an open decision holds a slot in the roadmap ----------------------

/// A17 and A18 stayed invisible while four phases were planned on top of them.
/// A decision that is open has a numbered slot at the point where it has to be
/// closed, or it is not really scheduled at all.
fn open_decisions_are_scheduled(root: &Path) -> Result<Vec<Finding>, String> {
    let decisions = read(root, "DECISIONS.md")?;
    let roadmap = read(root, "ROADMAP.md")?;
    let mut out = Vec::new();

    // In DECISIONS.md: the id of the heading above each TO_BE_DECIDED status.
    let mut open_in_decisions = Vec::new();
    let mut current = None;
    for line in decisions.lines() {
        if let Some(rest) = line.strip_prefix("## A") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            current = digits.parse::<u32>().ok();
        }
        if line.contains("TO_BE_DECIDED")
            && line.starts_with("**Status:")
            && let Some(id) = current
        {
            open_in_decisions.push(id);
        }
    }

    // In ROADMAP.md: rows that are both TO_BE_DECIDED and half-numbered.
    let mut open_in_roadmap = Vec::new();
    for line in roadmap.lines() {
        if !line.starts_with('|') || !line.contains("TO_BE_DECIDED") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let slot = cells
            .get(1)
            .map(|c| c.trim().trim_matches('*'))
            .unwrap_or("");
        if !slot.contains('.') {
            out.push(Finding {
                check: "open-decision",
                at: "ROADMAP.md".to_string(),
                what: format!("row \"{slot}\" is TO_BE_DECIDED but has no half number"),
            });
        }
        for (letter, num) in cited_ids(line) {
            if letter == 'A' {
                open_in_roadmap.push(num);
            }
        }
    }

    for id in &open_in_decisions {
        if !open_in_roadmap.contains(id) {
            out.push(Finding {
                check: "open-decision",
                at: "DECISIONS.md".to_string(),
                what: format!("A{id} is TO_BE_DECIDED but holds no slot in ROADMAP.md"),
            });
        }
    }
    for id in &open_in_roadmap {
        if !open_in_decisions.contains(id) {
            out.push(Finding {
                check: "open-decision",
                at: "ROADMAP.md".to_string(),
                what: format!("A{id} has a TO_BE_DECIDED slot but is not open in DECISIONS.md"),
            });
        }
    }
    Ok(out)
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
/// targets, `A12`/`D4`/`M1`, and `phase 15` / `step 6.1` / `slot 14.5`.
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

    // A12, D4, M1 — an id letter followed by digits.
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

// --- 7. every phase document says what it is ------------------------------

/// A phase file with no status reads as current truth. It is not: it is a
/// record of a moment, and the moment has usually passed.
fn plan_files_have_a_status(root: &Path) -> Result<Vec<Finding>, String> {
    let plan = root.join("plan");
    let mut files: Vec<PathBuf> = fs::read_dir(&plan)
        .map_err(|e| format!("{}: {e}", plan.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();

    let mut out = Vec::new();
    let mut highest_implemented = 0_u32;
    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let head: Vec<&str> = text.lines().take(12).collect();
        let status = head.iter().find_map(|l| l.strip_prefix("> **Status: "));
        let Some(status) = status else {
            out.push(Finding {
                check: "plan-status",
                at: path.display().to_string(),
                what: "has no `> **Status: ...**` header in its first lines".to_string(),
            });
            continue;
        };
        let label = status
            .split("**")
            .next()
            .unwrap_or("")
            .trim_end_matches('.');
        let known = [
            "implemented",
            "not yet built",
            "superseded",
            "closed",
            "index",
        ];
        if !known.iter().any(|k| label.starts_with(k)) {
            out.push(Finding {
                check: "plan-status",
                at: path.display().to_string(),
                what: format!("status \"{label}\" is not one of {known:?}"),
            });
        }
        if label.starts_with("implemented")
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse::<u32>() {
                highest_implemented = highest_implemented.max(n);
            }
        }
    }

    // The roadmap is the only place that states how far the tree has got, so it
    // has to agree with the phase files that claim to be done.
    let roadmap = read(root, "ROADMAP.md")?;
    let claimed = roadmap
        .lines()
        .find_map(|l| l.split("Implemented through phase ").nth(1))
        .and_then(|rest| {
            let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
            d.parse::<u32>().ok()
        });
    match claimed {
        None => out.push(Finding {
            check: "plan-status",
            at: "ROADMAP.md".to_string(),
            what: "no \"Implemented through phase N\" line".to_string(),
        }),
        Some(n) if n != highest_implemented => out.push(Finding {
            check: "plan-status",
            at: "ROADMAP.md".to_string(),
            what: format!(
                "says phase {n}, but the highest phase file marked implemented is {highest_implemented}"
            ),
        }),
        Some(_) => {}
    }
    Ok(out)
}

// --- 8. the frozen declaration orders -------------------------------------

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

/// Identifiers that begin with a capital, in order of appearance.
fn capitalised(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_read_as_whole_tokens() {
        assert_eq!(cited_ids("counted on residents (A12)"), vec![('A', 12)]);
        assert_eq!(
            cited_ids("no floats (D4), see A5."),
            vec![('D', 4), ('A', 5)]
        );
        // Not a citation: part of a word, or a hex literal.
        assert!(cited_ids("SHA1 and 0xD4 and DATA1").is_empty());
    }

    #[test]
    fn references_are_not_values() {
        assert!(!strip_references("see A12 and D4, phase 15, step 6.1").contains(['1', '4', '5']));
        assert!(strip_references("the rate is 12 per thousand").contains("12"));
    }

    #[test]
    fn backticks_and_capitals() {
        assert_eq!(backticked("a `b` c `d`"), vec!["b", "d"]);
        assert_eq!(capitalised("Births, Deaths"), vec!["Births", "Deaths"]);
    }
}
