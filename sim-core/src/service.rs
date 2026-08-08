//! I servizi cittadini, che funzionano per **copertura aggregata** (D2).
//!
//! Un edificio provider serve le case entro un raggio misurato sulla rete
//! stradale, non in linea d'aria. I walker che il giocatore vedra' camminare
//! per questi servizi sono decorativi, vivono nel renderer e non possono
//! influenzare lo stato del core.

use serde::{Deserialize, Serialize};

/// Tipi di servizio noti al core.
///
/// A differenza dei tipi di edificio, che sono dati (D6), i servizi sono un
/// enum: il core deve poterli indicizzare in array a dimensione fissa
/// (`ServiceFlags`, `Coverage`) e i loro effetti sono regole, non numeri.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ServiceKind {
    Acqua,
    Cibo,
}

impl ServiceKind {
    pub const TUTTI: [ServiceKind; 2] = [ServiceKind::Acqua, ServiceKind::Cibo];

    pub const COUNT: usize = Self::TUTTI.len();

    /// Nome usato nelle tabelle RON. E' parte del contratto con `sim-data`:
    /// rinominarlo invalida i file di dati.
    pub const fn as_id(self) -> &'static str {
        match self {
            Self::Acqua => "acqua",
            Self::Cibo => "cibo",
        }
    }

    /// `None` se il nome non corrisponde a nessun servizio noto: e' un errore
    /// di validazione della tabella, non un panic.
    pub fn from_id(s: &str) -> Option<Self> {
        Self::TUTTI.into_iter().find(|k| k.as_id() == s)
    }

    /// Posizione negli array indicizzati per servizio.
    pub const fn index(self) -> usize {
        match self {
            Self::Acqua => 0,
            Self::Cibo => 1,
        }
    }
}

/// Per ogni servizio, se la casa e' servita in questo tick.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct ServiceFlags(u8);

impl ServiceFlags {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn get(self, kind: ServiceKind) -> bool {
        self.0 & (1 << kind.index()) != 0
    }

    pub const fn set(&mut self, kind: ServiceKind, on: bool) {
        let bit = 1 << kind.index();
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

impl std::fmt::Debug for ServiceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("ServiceFlags");
        for k in ServiceKind::TUTTI {
            d.field(k.as_id(), &self.get(k));
        }
        d.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gli_id_fanno_round_trip() {
        for k in ServiceKind::TUTTI {
            assert_eq!(ServiceKind::from_id(k.as_id()), Some(k));
            assert!(k.index() < ServiceKind::COUNT);
        }
        assert_eq!(ServiceKind::from_id("trasporto_astrale"), None);
    }

    #[test]
    fn gli_indici_sono_distinti() {
        let mut visti = Vec::new();
        for k in ServiceKind::TUTTI {
            assert!(!visti.contains(&k.index()), "indice duplicato per {k:?}");
            visti.push(k.index());
        }
    }

    #[test]
    fn i_flag_sono_indipendenti() {
        let mut f = ServiceFlags::empty();
        assert!(!f.get(ServiceKind::Acqua));
        f.set(ServiceKind::Acqua, true);
        assert!(f.get(ServiceKind::Acqua));
        assert!(!f.get(ServiceKind::Cibo));
        f.set(ServiceKind::Acqua, false);
        assert_eq!(f, ServiceFlags::empty());
    }
}
