//! Il tick della simulazione.
//!
//! L'ordine dei dieci passi e' **semantica di gioco**, non un dettaglio
//! implementativo (CLAUDE.md, ordine del tick). Tutte e dieci le funzioni
//! esistono da subito, anche vuote: se nascessero man mano, l'ordine
//! diventerebbe un accidente della cronologia di sviluppo.
//!
//! Unita' di tempo: 1 tick = 1 giorno di gioco.

use crate::command::{Command, CommandError, Occupato};
use crate::event::Event;
use crate::ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
use crate::units::Coins;
use crate::world::{Building, House, Occupante, World};

/// Cosa e' successo in un tick.
///
/// Un comando invalido **non** interrompe il tick e non e' un `Err` del tick:
/// viene scartato e registrato con l'indice che aveva in `cmds`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepReport {
    pub rejected: Vec<(usize, CommandError)>,
    pub events: Vec<Event>,
}

impl StepReport {
    pub fn accettati(&self, totale: usize) -> usize {
        totale - self.rejected.len()
    }
}

/// Avanza il mondo di un tick applicando i comandi in arrivo.
pub fn step(world: &mut World, cmds: &[Command]) -> StepReport {
    let mut r = StepReport::default();
    apply_commands(world, cmds, &mut r); // 1
    rebuild_roads(world); // 2
    propagate_coverage(world); // 3
    production(world); // 4
    step_walkers(world); // 5
    houses_and_migration(world); // 6
    finance(world); // 7
    random_events(world); // 8
    check_objectives(world, &mut r); // 9
    emit_events(world, &mut r); // 10
    world.tick = world.tick.saturating_add(1);
    r
}

// --- 1. comandi ------------------------------------------------------------

fn apply_commands(world: &mut World, cmds: &[Command], r: &mut StepReport) {
    for (i, cmd) in cmds.iter().enumerate() {
        let esito = match *cmd {
            Command::PlaceRoad { at } => place_road(world, at, r),
            Command::PlaceBuilding { kind, origin } => place_building(world, kind, origin, r),
            Command::Demolish { at } => demolish(world, at, r),
        };
        if let Err(e) = esito {
            r.rejected.push((i, e));
        }
    }
}

fn place_road(world: &mut World, at: TilePos, r: &mut StepReport) -> Result<(), CommandError> {
    let idx = world.grid.idx(at).ok_or(CommandError::OutOfBounds(at))?;
    let tile = world.grid.get(idx).ok_or(CommandError::OutOfBounds(at))?;

    if tile.flags.has_road() {
        return Err(CommandError::TileOccupied {
            at,
            occupant: Occupato::Strada,
        });
    }
    if let Some(occ) = world.occupante(idx) {
        return Err(CommandError::TileOccupied {
            at,
            occupant: occ.into(),
        });
    }

    let def = terrain_def(world, tile.terrain, at)?;
    if !def.attraversabile {
        return Err(CommandError::UnsuitableTerrain {
            at,
            terrain: tile.terrain,
        });
    }
    let costo = def.costo_strada;
    paga(world, costo)?;

    // Da qui in poi non si puo' piu' fallire: nessuna mutazione parziale.
    if let Some(t) = world.grid.get_mut(idx) {
        t.flags.set_road(true);
    }
    world.dirty.roads = true;
    r.events.push(Event::RoadPlaced { at });
    Ok(())
}

fn place_building(
    world: &mut World,
    kind: BuildingKindId,
    origin: TilePos,
    r: &mut StepReport,
) -> Result<(), CommandError> {
    let def = world
        .data
        .def(kind)
        .ok_or(CommandError::UnknownBuildingKind(kind))?;
    let footprint = def.footprint;
    let costo = def.costo;
    let e_una_casa = def.e_una_casa();
    let abitanti = if e_una_casa {
        world
            .data
            .rules
            .abitanti_per_livello_casa
            .first()
            .copied()
            .unwrap_or(0)
    } else {
        0
    };

    // L'intero footprint si valida prima di qualunque mutazione: una
    // sovrapposizione parziale lascerebbe lo stato incoerente.
    let tiles = tiles_del_footprint(world, origin, footprint)?;
    for (idx, pos) in &tiles {
        let tile = world
            .grid
            .get(*idx)
            .ok_or(CommandError::OutOfBounds(*pos))?;
        if tile.flags.has_road() {
            return Err(CommandError::TileOccupied {
                at: *pos,
                occupant: Occupato::Strada,
            });
        }
        if let Some(occ) = world.occupante(*idx) {
            return Err(CommandError::TileOccupied {
                at: *pos,
                occupant: occ.into(),
            });
        }
        let tdef = terrain_def(world, tile.terrain, *pos)?;
        if !tdef.costruibile {
            return Err(CommandError::UnsuitableTerrain {
                at: *pos,
                terrain: tile.terrain,
            });
        }
    }

    paga(world, costo)?;

    // --- da qui in poi nessun fallimento possibile ---
    let origin_idx = tiles
        .first()
        .map(|(i, _)| *i)
        .unwrap_or_else(|| TileIdx::new(0));

    if e_una_casa {
        let id = world.houses.insert(House {
            origin,
            level: 1,
            abitanti,
            servita: crate::service::ServiceFlags::empty(),
        });
        world.case_per_origine.insert(origin_idx, id);
        occupa(world, &tiles, origin_idx, true);
        // Una casa nuova va coperta: senza questo, resterebbe non servita
        // finche' qualcos'altro non sporca la copertura. E' esattamente
        // l'invalidazione dimenticata che il test di equivalenza coglie.
        segna_tutti_i_provider(world);
        r.events.push(Event::HousePlaced { id, origin });
    } else {
        let id = world.buildings.insert(Building {
            kind,
            origin,
            level: 1,
            stock: crate::units::Milli::ZERO,
        });
        world.edifici_per_origine.insert(origin_idx, id);
        occupa(world, &tiles, origin_idx, false);
        world.dirty.segna_coverage(id);
        r.events.push(Event::BuildingPlaced { id, kind, origin });
    }
    Ok(())
}

fn demolish(world: &mut World, at: TilePos, r: &mut StepReport) -> Result<(), CommandError> {
    let idx = world.grid.idx(at).ok_or(CommandError::OutOfBounds(at))?;

    if let Some(occ) = world.occupante(idx) {
        match occ {
            Occupante::Edificio(id) => rimuovi_edificio(world, id, r),
            Occupante::Casa(id) => rimuovi_casa(world, id, r),
        }
        return Ok(());
    }

    let tile = world.grid.get(idx).ok_or(CommandError::OutOfBounds(at))?;
    if tile.flags.has_road() {
        if let Some(t) = world.grid.get_mut(idx) {
            t.flags.set_road(false);
        }
        world.dirty.roads = true;
        r.events.push(Event::RoadRemoved { at });
        return Ok(());
    }

    Err(CommandError::NothingToDemolish(at))
}

fn rimuovi_edificio(world: &mut World, id: BuildingId, r: &mut StepReport) {
    let Some(b) = world.buildings.remove(id) else {
        return;
    };
    let Some(def) = world.data.def(b.kind) else {
        return;
    };
    let footprint = def.footprint;
    libera_footprint(world, b.origin, footprint);
    if let Some(idx) = world.grid.idx(b.origin) {
        world.edifici_per_origine.remove(&idx);
    }
    world.dirty.dimentica_coverage(id);
    // La copertura di tutti gli altri provider va rivista: quello rimosso
    // poteva servire case che ora tornano libere (fase 06).
    segna_tutti_i_provider(world);
    r.events.push(Event::BuildingRemoved { id });
}

fn rimuovi_casa(world: &mut World, id: HouseId, r: &mut StepReport) {
    let Some(h) = world.houses.remove(id) else {
        return;
    };
    // Il footprint di una casa e' quello del suo tipo; in M0 le case sono
    // 1x1, ma il footprint si rilegge dalla tabella per non incastrarsi il
    // giorno in cui non lo saranno piu'.
    let footprint = world
        .data
        .kind_by_id("casa")
        .and_then(|k| world.data.def(k))
        .map_or((1, 1), |d| d.footprint);
    libera_footprint(world, h.origin, footprint);
    if let Some(idx) = world.grid.idx(h.origin) {
        world.case_per_origine.remove(&idx);
    }
    segna_tutti_i_provider(world);
    r.events.push(Event::HouseRemoved { id });
}

// --- 2..10: i passi che si riempiono nelle fasi successive ------------------

/// Passo 2 — ricostruisce la rete stradale, ma solo se e' sporca.
///
/// La topologia della rete cambia le distanze percorse, quindi dopo una
/// ricostruzione ogni provider va rivalutato (passo 3).
fn rebuild_roads(world: &mut World) {
    if !world.dirty.roads {
        return;
    }
    world.roads.rebuild(&world.grid);
    world.dirty.roads = false;
    segna_tutti_i_provider(world);
}

/// Passo 3 — copertura aggregata dei servizi. E' l'hot path del progetto.
fn propagate_coverage(world: &mut World) {
    crate::coverage::propagate_coverage(world);
}

/// Passo 4 — si riempie nella fase 07.
fn production(_world: &mut World) {}

/// Passo 5 — walker logistici reali, M3 (D3).
fn step_walkers(_world: &mut World) {}

/// Passo 6 — evoluzione, degrado e migrazione, M1.
fn houses_and_migration(_world: &mut World) {}

/// Passo 7 — tesoro e tasse, M1.
fn finance(_world: &mut World) {}

/// Passo 8 — eventi casuali, M1. Primo uso di `RngDomain::Events`.
fn random_events(_world: &mut World) {}

/// Passo 9 — obiettivi di scenario, M1 (serve `sim-scenario`).
fn check_objectives(_world: &mut World, _r: &mut StepReport) {}

/// Passo 10 — in M0 gli eventi sono gia' stati accodati dai passi che li
/// generano; qui si aggiungeranno i delta di copertura (fase 07).
fn emit_events(_world: &mut World, _r: &mut StepReport) {}

// --- helper ----------------------------------------------------------------

fn terrain_def(
    world: &World,
    terrain: crate::grid::Terrain,
    at: TilePos,
) -> Result<&crate::data::TerrainDef, CommandError> {
    world
        .data
        .terrain(terrain)
        .ok_or(CommandError::UnsuitableTerrain { at, terrain })
}

/// Scala il costo dal tesoro. Errore strutturato, mai un tesoro negativo.
fn paga(world: &mut World, costo: Coins) -> Result<(), CommandError> {
    let disponibile = world.economy.tesoro;
    let resto = disponibile
        .checked_sub(costo)
        .ok_or(CommandError::InsufficientFunds {
            needed: costo,
            available: disponibile,
        })?;
    if resto.is_negative() {
        return Err(CommandError::InsufficientFunds {
            needed: costo,
            available: disponibile,
        });
    }
    world.economy.tesoro = resto;
    Ok(())
}

/// I tile del footprint, in ordine di `TileIdx` crescente. Errore se anche
/// uno solo esce dalla mappa.
fn tiles_del_footprint(
    world: &World,
    origin: TilePos,
    footprint: (u8, u8),
) -> Result<Vec<(TileIdx, TilePos)>, CommandError> {
    let (w, h) = footprint;
    let mut out = Vec::with_capacity(usize::from(w) * usize::from(h));
    for dy in 0..h {
        for dx in 0..w {
            let x = origin
                .x
                .checked_add(dx)
                .ok_or(CommandError::OutOfBounds(origin))?;
            let y = origin
                .y
                .checked_add(dy)
                .ok_or(CommandError::OutOfBounds(origin))?;
            let pos = TilePos::new(x, y);
            let idx = world.grid.idx(pos).ok_or(CommandError::OutOfBounds(pos))?;
            out.push((idx, pos));
        }
    }
    Ok(out)
}

fn occupa(world: &mut World, tiles: &[(TileIdx, TilePos)], origin_idx: TileIdx, e_casa: bool) {
    let occ = crate::grid::TileOccupant {
        origin: origin_idx,
        e_casa,
    };
    for (idx, _) in tiles {
        if let Some(t) = world.grid.get_mut(*idx) {
            t.set_occupante(occ);
        }
    }
}

fn libera_footprint(world: &mut World, origin: TilePos, footprint: (u8, u8)) {
    let Ok(tiles) = tiles_del_footprint(world, origin, footprint) else {
        return;
    };
    for (idx, _) in tiles {
        if let Some(t) = world.grid.get_mut(idx) {
            t.libera_occupante();
        }
    }
}

/// In M0 il ricalcolo della copertura e' ingenuo: quando la topologia cambia,
/// tutti i provider tornano dirty. `CLAUDE.md` lo autorizza, purche' i flag
/// esistano — ed esistono.
fn segna_tutti_i_provider(world: &mut World) {
    // Anche quando non resta nessun provider: le assegnazioni esistenti vanno
    // comunque buttate.
    world.dirty.invalida_coverage();
    let ids: Vec<BuildingId> = world.buildings.keys().collect();
    for id in ids {
        world.dirty.segna_coverage(id);
    }
}
