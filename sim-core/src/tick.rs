//! The simulation tick.
//!
//! The order of the ten steps is **game semantics**, not an implementation
//! detail (CLAUDE.md, tick order). All ten functions exist from the start, even
//! the empty ones: if they came into being one at a time, the order would end
//! up an accident of the development timeline.
//!
//! Unit of time: 1 tick = 1 game day.

use crate::command::{Command, CommandError, OccupantKind};
use crate::event::Event;
use crate::ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
use crate::service::{ServiceFlags, ServiceKind};
use crate::units::Coins;
use crate::world::{Building, House, Occupant, World};

/// What happened during a tick.
///
/// An invalid command does **not** interrupt the tick and is not an `Err` of
/// the tick: it is discarded and recorded with the index it had in `cmds`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepReport {
    pub rejected: Vec<(usize, CommandError)>,
    pub events: Vec<Event>,
}

impl StepReport {
    pub fn accepted(&self, total: usize) -> usize {
        total - self.rejected.len()
    }
}

/// Advances the world by one tick, applying the incoming commands.
pub fn step(world: &mut World, cmds: &[Command]) -> StepReport {
    let mut r = StepReport::default();
    // A snapshot of the services at the start of the tick: it is the reference
    // step 10 emits its deltas against.
    let services_before = service_snapshot(world);
    apply_commands(world, cmds, &mut r); // 1
    rebuild_roads(world); // 2
    propagate_coverage(world); // 3
    production(world); // 4
    step_walkers(world); // 5
    houses_and_migration(world); // 6
    finance(world); // 7
    random_events(world); // 8
    check_objectives(world, &mut r); // 9
    emit_events(world, &services_before, &mut r); // 10
    world.tick = world.tick.saturating_add(1);
    r
}

// --- 1. commands -----------------------------------------------------------

fn apply_commands(world: &mut World, cmds: &[Command], r: &mut StepReport) {
    for (i, cmd) in cmds.iter().enumerate() {
        let outcome = match *cmd {
            Command::PlaceRoad { at } => place_road(world, at, r),
            Command::PlaceBuilding { kind, origin } => place_building(world, kind, origin, r),
            Command::Demolish { at } => demolish(world, at, r),
        };
        if let Err(e) = outcome {
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
            occupant: OccupantKind::Road,
        });
    }
    if let Some(occ) = world.occupant(idx) {
        return Err(CommandError::TileOccupied {
            at,
            occupant: occ.into(),
        });
    }

    let def = terrain_def(world, tile.terrain, at)?;
    if !def.walkable {
        return Err(CommandError::UnsuitableTerrain {
            at,
            terrain: tile.terrain,
        });
    }
    let cost = def.road_cost;
    charge(world, cost)?;

    // From here on nothing can fail: no partial mutation.
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
    let size = def.size;
    let cost = def.cost;
    let is_a_house = def.is_house();
    // How full a new house is born is the difficulty's one knob (A13): at
    // `hard` it is zero and the house only fills up by migration.
    let residents = if is_a_house {
        world
            .data
            .difficulty(world.difficulty)
            .map_or(0, |d| d.starting_residents_per_house)
    } else {
        0
    };

    // The whole area is validated before any mutation: a partial overlap would
    // leave the state inconsistent.
    let tiles = tiles_covered(world, origin, size)?;
    for (idx, pos) in &tiles {
        let tile = world
            .grid
            .get(*idx)
            .ok_or(CommandError::OutOfBounds(*pos))?;
        if tile.flags.has_road() {
            return Err(CommandError::TileOccupied {
                at: *pos,
                occupant: OccupantKind::Road,
            });
        }
        if let Some(occ) = world.occupant(*idx) {
            return Err(CommandError::TileOccupied {
                at: *pos,
                occupant: occ.into(),
            });
        }
        let tdef = terrain_def(world, tile.terrain, *pos)?;
        if !tdef.buildable {
            return Err(CommandError::UnsuitableTerrain {
                at: *pos,
                terrain: tile.terrain,
            });
        }
    }

    charge(world, cost)?;

    // --- from here on no failure is possible ---
    let origin_idx = tiles
        .first()
        .map(|(i, _)| *i)
        .unwrap_or_else(|| TileIdx::new(0));

    if is_a_house {
        let id = world.houses.insert(House {
            origin,
            level: 1,
            residents,
            served: crate::service::ServiceFlags::empty(),
        });
        world.houses_by_origin.insert(origin_idx, id);
        occupy(world, &tiles, origin_idx, true);
        // A new house needs covering: without this it would stay unserved until
        // something else dirtied the coverage. That is exactly the forgotten
        // invalidation the equivalence test catches.
        mark_all_providers_dirty(world);
        r.events.push(Event::HousePlaced { id, origin });
    } else {
        let id = world.buildings.insert(Building {
            kind,
            origin,
            level: 1,
            stock: crate::units::Milli::ZERO,
        });
        world.buildings_by_origin.insert(origin_idx, id);
        occupy(world, &tiles, origin_idx, false);
        world.dirty.mark_coverage(id);
        r.events.push(Event::BuildingPlaced { id, kind, origin });
    }
    Ok(())
}

fn demolish(world: &mut World, at: TilePos, r: &mut StepReport) -> Result<(), CommandError> {
    let idx = world.grid.idx(at).ok_or(CommandError::OutOfBounds(at))?;

    if let Some(occ) = world.occupant(idx) {
        match occ {
            Occupant::Building(id) => remove_building(world, id, r),
            Occupant::House(id) => remove_house(world, id, r),
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

fn remove_building(world: &mut World, id: BuildingId, r: &mut StepReport) {
    let Some(b) = world.buildings.remove(id) else {
        return;
    };
    let Some(def) = world.data.def(b.kind) else {
        return;
    };
    let size = def.size;
    // The stock disappears with the producer: it has to be recorded, otherwise
    // food conservation stops being an equality (phase 07).
    if def.is_producer() {
        world.food.lost_to_demolition += i64::from(b.stock.to_millis());
    }
    clear_tiles(world, b.origin, size);
    if let Some(idx) = world.grid.idx(b.origin) {
        world.buildings_by_origin.remove(&idx);
    }
    world.dirty.forget_coverage(id);
    // Every other provider's coverage has to be revisited: the one just removed
    // may have been serving houses that are now free again (phase 06).
    mark_all_providers_dirty(world);
    r.events.push(Event::BuildingRemoved { id });
}

fn remove_house(world: &mut World, id: HouseId, r: &mut StepReport) {
    let Some(h) = world.houses.remove(id) else {
        return;
    };
    // A house's size is the one of its kind; in M0 houses are 1x1, but the size
    // is re-read from the table so as not to get stuck the day they no longer
    // are.
    let size = world
        .data
        .kind_by_id("house")
        .and_then(|k| world.data.def(k))
        .map_or((1, 1), |d| d.size);
    clear_tiles(world, h.origin, size);
    if let Some(idx) = world.grid.idx(h.origin) {
        world.houses_by_origin.remove(&idx);
    }
    mark_all_providers_dirty(world);
    r.events.push(Event::HouseRemoved { id });
}

// --- 2..10: the steps that fill up in the later phases ----------------------

/// Step 2 — rebuilds the road network, but only if it is dirty.
///
/// The network's topology changes the walked distances, so after a rebuild
/// every provider has to be re-evaluated (step 3).
fn rebuild_roads(world: &mut World) {
    if !world.dirty.roads {
        return;
    }
    world.roads.rebuild(&world.grid);
    world.dirty.roads = false;
    mark_all_providers_dirty(world);
}

/// Step 3 — aggregate service coverage. It is the project's hot path.
fn propagate_coverage(world: &mut World) {
    crate::coverage::propagate_coverage(world);
}

/// Step 4 — production and consumption along the chains.
fn production(world: &mut World) {
    crate::production::production(world);
}

/// Step 5 — real logistics walkers, M3 (D3).
fn step_walkers(_world: &mut World) {}

/// Step 6 — levelling up, decay and migration, M1.
fn houses_and_migration(_world: &mut World) {}

/// Step 7 — treasury and taxes, M1.
fn finance(_world: &mut World) {}

/// Step 8 — random events, M1. First use of `RngDomain::Events`.
fn random_events(_world: &mut World) {}

/// Step 9 — scenario objectives, M1 (needs `sim-scenario`).
fn check_objectives(_world: &mut World, _r: &mut StepReport) {}

/// Step 10 — emits the coverage deltas.
///
/// Only the **changes** relative to the start of the tick: a house that has
/// been hungry for ten ticks generates one event on the first, not ten. This is
/// the boundary with the renderer, and the wrong choice here would cost 40,000
/// events per tick.
fn emit_events(world: &mut World, before: &[(HouseId, ServiceFlags)], r: &mut StepReport) {
    for (house, _) in world.houses() {
        let now = world
            .house(house)
            .map_or(ServiceFlags::empty(), |h| h.served);
        // A house born this tick has no "before": it starts uncovered, so if it
        // is served the event is there.
        let previous = before
            .binary_search_by_key(&house, |(h, _)| *h)
            .map_or(ServiceFlags::empty(), |i| before[i].1);
        if now == previous {
            continue;
        }
        for service in ServiceKind::ALL {
            if now.get(service) != previous.get(service) {
                r.events.push(Event::ServiceCoverageChanged {
                    house,
                    service,
                    served: now.get(service),
                });
            }
        }
    }
}

/// Every house's service flags, sorted by `HouseId` so step 10's comparison is
/// a binary search and not a scan.
fn service_snapshot(world: &World) -> Vec<(HouseId, ServiceFlags)> {
    let mut v: Vec<(HouseId, ServiceFlags)> =
        world.houses().map(|(id, h)| (id, h.served)).collect();
    v.sort_unstable_by_key(|(id, _)| *id);
    v
}

// --- helpers ---------------------------------------------------------------

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

/// Takes the cost out of the treasury. A structured error, never a negative
/// treasury.
fn charge(world: &mut World, cost: Coins) -> Result<(), CommandError> {
    let available = world.economy.treasury;
    let left = available
        .checked_sub(cost)
        .ok_or(CommandError::InsufficientFunds {
            needed: cost,
            available,
        })?;
    if left.is_negative() {
        return Err(CommandError::InsufficientFunds {
            needed: cost,
            available,
        });
    }
    world.economy.treasury = left;
    Ok(())
}

/// The tiles an area covers, in increasing `TileIdx` order. An error if even
/// one of them leaves the map.
fn tiles_covered(
    world: &World,
    origin: TilePos,
    size: (u8, u8),
) -> Result<Vec<(TileIdx, TilePos)>, CommandError> {
    let (w, h) = size;
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

fn occupy(world: &mut World, tiles: &[(TileIdx, TilePos)], origin_idx: TileIdx, is_house: bool) {
    let occ = crate::grid::TileOccupant {
        origin: origin_idx,
        is_house,
    };
    for (idx, _) in tiles {
        if let Some(t) = world.grid.get_mut(*idx) {
            t.set_occupant(occ);
        }
    }
}

fn clear_tiles(world: &mut World, origin: TilePos, size: (u8, u8)) {
    let Ok(tiles) = tiles_covered(world, origin, size) else {
        return;
    };
    for (idx, _) in tiles {
        if let Some(t) = world.grid.get_mut(idx) {
            t.clear_occupant();
        }
    }
}

/// In M0 recomputing coverage is naive: when the topology changes, every
/// provider goes dirty again. `CLAUDE.md` licenses that, as long as the flags
/// exist — and they do.
///
/// "Everything dirty" is said with the global flag, not by listing the
/// providers one by one in `dirty.coverage`: the list serves **targeted**
/// invalidation, and filling it with the complete set would add no information
/// for whoever reads it one day. It holds even when no provider is left — the
/// existing assignments still have to be thrown away, and that is the case the
/// list alone cannot express.
fn mark_all_providers_dirty(world: &mut World) {
    world.dirty.invalidate_coverage();
}
