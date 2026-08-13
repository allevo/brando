//! The documents, checked from the test suite.
//!
//! This exists because `cargo test` is the only thing that reliably runs here:
//! the project has no CI, and `jj` does not run git hooks. A check that has to
//! be remembered is a check that stops happening — which is the whole reason
//! `clippy.toml` carries D4's prohibitions instead of a paragraph asking for
//! them.

use xtask::doc_check;

#[test]
fn the_documents_agree_with_the_code() {
    let root = match doc_check::repo_root() {
        Ok(r) => r,
        Err(e) => panic!("cannot find the repository root: {e}"),
    };
    let findings = match doc_check::run(&root) {
        Ok(f) => f,
        Err(e) => panic!("doc-check could not run: {e}"),
    };
    if findings.is_empty() {
        return;
    }
    let mut report = format!("\n{} problem(s):\n\n", findings.len());
    for f in &findings {
        report.push_str(&format!("  {f}\n"));
    }
    report.push_str("\nRun `cargo xtask doc-check` for the same list.\n");
    panic!("{report}");
}
