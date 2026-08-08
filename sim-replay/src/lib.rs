#![forbid(unsafe_code)]

//! Salvataggio come `seed + Vec<Command>` e hashing canonico dello stato (D4).
//!
//! Un salvataggio non e' un dump dello stato: e' il seed piu' il log dei
//! comandi, rigiocato dal core.
