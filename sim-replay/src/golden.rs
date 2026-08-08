//! Il file `.hashes` che accompagna ogni golden.
//!
//! Formato: una riga `tick,hash_esadecimale` per checkpoint, piu' commenti
//! che iniziano con `#`. Testuale e diffabile di proposito: quando un golden
//! cambia, si deve vedere *a quale tick* ha cominciato a divergere.

use std::fmt::Write as _;

use crate::replay::Checkpoint;

#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    #[error("riga {riga} malformata: {contenuto:?}")]
    RigaMalformata { riga: usize, contenuto: String },
}

pub fn rendi(checkpoints: &[Checkpoint], scenario: &str, ogni: u32) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# golden di '{scenario}': hash canonico dello stato ogni {ogni} tick."
    );
    let _ = writeln!(
        s,
        "# Se cambia senza che sia cambiato il bilanciamento, e' stata introdotta"
    );
    let _ = writeln!(
        s,
        "# una fonte di non-determinismo: fermarsi e trovarla, non rigenerare."
    );
    for c in checkpoints {
        let _ = writeln!(s, "{},{}", c.tick, crate::hash::hash_hex(&c.hash));
    }
    s
}

pub fn leggi(testo: &str) -> Result<Vec<(u32, String)>, GoldenError> {
    let mut out = Vec::new();
    for (i, riga) in testo.lines().enumerate() {
        let riga = riga.trim();
        if riga.is_empty() || riga.starts_with('#') {
            continue;
        }
        let (t, h) = riga
            .split_once(',')
            .ok_or_else(|| GoldenError::RigaMalformata {
                riga: i + 1,
                contenuto: riga.to_string(),
            })?;
        let tick = t.trim().parse().map_err(|_| GoldenError::RigaMalformata {
            riga: i + 1,
            contenuto: riga.to_string(),
        })?;
        out.push((tick, h.trim().to_string()));
    }
    Ok(out)
}
