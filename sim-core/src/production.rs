//! Produzione e consumo (passo 4 del tick).
//!
//! In M0 la fattoria e' un provider di copertura come il pozzo: produce in una
//! giacenza locale e le case che copre consumano da li' (A5). Non e' la catena
//! produttiva definitiva — magazzini e walker logistici reali sono M3 (D3) —
//! ma e' la versione minima che chiude un ciclo produzione→consumo
//! osservabile, e non introduce concetti da rimuovere: la copertura resta
//! valida, in M3 cambia solo *da dove* arriva la merce.

use std::sync::Arc;

use crate::ids::{BuildingId, HouseId};
use crate::service::ServiceKind;
use crate::units::Milli;
use crate::world::World;

/// Contabilita' cumulativa del cibo.
///
/// Serve a verificare la conservazione e all'evaluator di M2. Non influenza
/// nessuna decisione di gioco e non entra nell'hash canonico dello stato.
///
/// I campi sono `i64` di millesimi e non `Milli`: sono totali cumulativi, che
/// su una partita lunga escono dal range di `Milli(i32)`. Saturare li' dentro
/// romperebbe in silenzio l'uguaglianza che questi campi esistono per
/// verificare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FoodLedger {
    pub prodotto: i64,
    pub consumato: i64,
    /// Cibo evaporato perche' il granaio era pieno.
    pub perso_per_giacenza_piena: i64,
    /// Cibo evaporato con la fattoria che lo conteneva.
    ///
    /// Esiste per lo stesso motivo del campo sopra: senza, demolire un
    /// produttore pieno romperebbe l'uguaglianza di conservazione, e il test
    /// piu' importante della fase segnalerebbe un bug che non c'e'.
    pub perso_per_demolizione: i64,
}

impl FoodLedger {
    /// Quanto dovrebbe esserci nelle giacenze delle fattorie vive.
    pub const fn atteso_in_giacenza(&self) -> i64 {
        self.prodotto - self.consumato - self.perso_per_giacenza_piena - self.perso_per_demolizione
    }
}

pub(crate) fn production(world: &mut World) {
    // L'Arc si clona per poter leggere le tabelle mentre si mutano gli
    // edifici: e' un incremento di refcount, non una copia del dataset.
    let data = Arc::clone(&world.data);

    // --- 1. le fattorie producono ---
    for (_, b) in world.buildings.iter_mut() {
        let Some(def) = data.def(b.kind) else {
            continue;
        };
        let (Some(produzione), Some(max)) = (def.produzione_per_tick, def.giacenza_max) else {
            continue;
        };

        let lordo = b.stock.saturating_add(produzione);
        // La saturazione al massimo della giacenza e' **semantica di gioco**:
        // il granaio e' pieno e il resto si perde. Il termine `perso` esiste
        // proprio perche' senza di lui la conservazione non sarebbe
        // un'uguaglianza.
        let nuovo = lordo.min(max);
        let perso = i64::from(lordo.to_millis()) - i64::from(nuovo.to_millis());

        world.food.prodotto += i64::from(produzione.to_millis());
        world.food.perso_per_giacenza_piena += perso;
        b.stock = nuovo;
    }

    // --- 2. le case mangiano ---
    // In ordine di HouseId: e' un ordine deterministico ed e' regola di gioco,
    // come l'ordinamento per (distanza, TileIdx) della fase 06.
    let case: Vec<HouseId> = world.houses.keys().collect();
    for h in case {
        let Some(casa) = world.houses.get(h) else {
            continue;
        };
        let abitanti = i32::from(casa.abitanti);
        let provider = world.coverage.provider(h, ServiceKind::Cibo);

        let ha_mangiato = match provider {
            None => false,
            Some(p) => {
                let consumo = data
                    .rules
                    .consumo_cibo_per_abitante
                    .checked_mul_int(abitanti)
                    .unwrap_or(Milli::ZERO);
                consuma(world, p, consumo)
            }
        };

        if let Some(casa) = world.houses.get_mut(h) {
            casa.servita.set(ServiceKind::Cibo, ha_mangiato);
        }
    }
}

/// Preleva `consumo` dalla giacenza del provider.
///
/// Restituisce `false` senza toccare nulla se la giacenza non basta: **niente
/// consumo parziale**. La scelta e' deliberata — rende la conservazione
/// verificabile con un'uguaglianza esatta invece che con una disuguaglianza, e
/// in M1 dara' un segnale binario pulito a "la casa ha mangiato questo mese".
fn consuma(world: &mut World, provider: BuildingId, consumo: Milli) -> bool {
    let Some(b) = world.buildings.get_mut(provider) else {
        return false;
    };
    if b.stock < consumo {
        return false;
    }
    let Some(resto) = b.stock.checked_sub(consumo) else {
        return false;
    };
    b.stock = resto;
    world.food.consumato += i64::from(consumo.to_millis());
    true
}
