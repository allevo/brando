//! City services, which work by **aggregate coverage** (D2).
//!
//! A provider building serves the houses within a range measured along the
//! road network, not as the crow flies. The walkers the player will see
//! wandering around for these services are decorative, live in the renderer
//! and cannot influence the core's state.

use serde::{Deserialize, Serialize};

/// The service kinds the core knows about.
///
/// Unlike building kinds, which are data (D6), services are an enum: the core
/// has to index them into fixed-size arrays (`ServiceFlags`, `Coverage`) and
/// their effects are rules, not numbers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ServiceKind {
    Water,
    Food,
}

impl ServiceKind {
    pub const ALL: [ServiceKind; 2] = [ServiceKind::Water, ServiceKind::Food];

    pub const COUNT: usize = Self::ALL.len();

    /// The name used in the RON tables. It is part of the contract with
    /// `sim-data`: renaming it invalidates the data files.
    pub const fn as_id(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Food => "food",
        }
    }

    /// `None` if the name matches no known service: that is a table validation
    /// error, not a panic.
    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_id() == s)
    }

    /// Position in the arrays indexed by service.
    pub const fn index(self) -> usize {
        match self {
            Self::Water => 0,
            Self::Food => 1,
        }
    }
}

/// For each service, whether the house is served this tick.
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
        for k in ServiceKind::ALL {
            d.field(k.as_id(), &self.get(k));
        }
        d.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ids_round_trip() {
        for k in ServiceKind::ALL {
            assert_eq!(ServiceKind::from_id(k.as_id()), Some(k));
            assert!(k.index() < ServiceKind::COUNT);
        }
        assert_eq!(ServiceKind::from_id("astral_transport"), None);
    }

    #[test]
    fn the_indices_are_distinct() {
        let mut seen = Vec::new();
        for k in ServiceKind::ALL {
            assert!(!seen.contains(&k.index()), "duplicate index for {k:?}");
            seen.push(k.index());
        }
    }

    #[test]
    fn the_flags_are_independent() {
        let mut f = ServiceFlags::empty();
        assert!(!f.get(ServiceKind::Water));
        f.set(ServiceKind::Water, true);
        assert!(f.get(ServiceKind::Water));
        assert!(!f.get(ServiceKind::Food));
        f.set(ServiceKind::Water, false);
        assert_eq!(f, ServiceFlags::empty());
    }
}
