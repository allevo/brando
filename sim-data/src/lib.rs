#![forbid(unsafe_code)]

//! Tabelle di bilanciamento in RON, caricate e validate all'avvio.
//!
//! Tutto cio' che e' numerico nel gioco vive qui e non nel codice
//! (CLAUDE.md, convenzioni). Il caricamento e' I/O e per questo sta qui,
//! mai dentro `sim-core` (D4).
