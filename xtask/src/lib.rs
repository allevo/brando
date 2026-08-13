//! The parts of `xtask` that are worth calling from a test as well as from the
//! command line.
//!
//! [`doc_check`] lives here rather than in the binary for exactly that reason:
//! the project has no CI and `jj` does not run git hooks, so the only place a
//! check reliably runs is `cargo test`.

pub mod doc_check;
