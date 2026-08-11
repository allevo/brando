//! The simulation tick.
//!
//! The order of the ten steps is **game semantics**, not an implementation
//! detail (CLAUDE.md, tick order). All ten functions exist from the start, even
//! the empty ones: if they came into being one at a time, the order would end
//! up an accident of the development timeline.
//!
//! Unit of time: 1 tick = 1 game day.

use crate::command::{Command, CommandError, OccupantKind};
use crate::data::BuildingDef;
use crate::event::Event;
use crate::ids::{BuildingId, BuildingKindId, HouseId, Level, TileIdx, TilePos};
use crate::satisfaction::Mood;
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
    // A snapshot of the houses at the start of the tick: it is the reference
    // step 10 emits its deltas against.
    let before = house_snapshot(world);
    apply_commands(world, cmds, &mut r); // 1
    rebuild_roads(world); // 2
    propagate_coverage(world); // 3
    production(world); // 4
    step_walkers(world); // 5
    houses_and_migration(world, &mut r); // 6
    finance(world); // 7
    random_events(world); // 8
    check_objectives(world, &mut r); // 9
    emit_events(world, &before, &mut r); // 10
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
            level: Level::FIRST,
            residents,
            served: crate::service::ServiceFlags::empty(),
            // At zero, not at the maximum: step 3 covers it in this same tick,
            // but its first levelling up costs the full time all the same.
            satisfaction: [0; ServiceKind::COUNT],
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
            level: Level::FIRST,
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
    let Some(b) = world.buildings.get(id) else {
        return;
    };
    let (origin, stock) = (b.origin, b.stock);
    // Read before the removal, and with the same fallback as `remove_house`.
    // Looking the kind up afterwards meant a failed lookup returned with the
    // building already out of the `SlotMap` and everything else left standing:
    // the tiles still occupied, `buildings_by_origin` still holding a dead id —
    // so `World::occupant` resolved to a building that was not there, which is
    // what `no_overlap` calls "points at a dead building" — the coverage never
    // forgotten and never invalidated. Unreachable through play, the kind
    // having been validated at placement and the dataset being immutable behind
    // an `Arc`, but it was the one place in this file that did not hold the
    // discipline `place_road` and `place_building` state out loud.
    let def = world.data.def(b.kind);
    let size = def.map_or((1, 1), |d| d.size);
    let produces = def.is_some_and(BuildingDef::is_producer);

    // From here on nothing can fail: no partial mutation.
    world.buildings.remove(id);
    // The stock disappears with the producer: it has to be recorded, otherwise
    // food conservation stops being an equality (phase 07).
    if produces {
        world.food.lost_to_demolition += i64::from(stock.to_millis());
    }
    clear_tiles(world, origin, size);
    if let Some(idx) = world.grid.idx(origin) {
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
    let size = world.data.house_def().map_or((1, 1), |d| d.size);
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

/// Step 6 — levelling up, decay and migration.
///
/// The internal order is **game semantics** as much as the order of the ten
/// steps, and the same rule applies: do not reorder without regenerating the
/// recordings and writing down why. Phases 14 and 15 add their sub-steps
/// **below** these, never above.
///
/// 6.1 comes first because it reads only what steps 3 and 4 have written *this*
/// tick, and all the rest of step 6 reads it. If it came after levelling up, a
/// house would level up on the previous tick's data: correct on average,
/// unreadable in a recording you are trying to follow by hand.
///
/// 6.2 runs only on a month boundary. The cadence is what makes the absence of
/// oscillation structural rather than a consequence of the thresholds, and it
/// makes the recordings readable: a level that can only change at multiples of
/// `ticks_per_month` can be followed by eye.
fn houses_and_migration(world: &mut World, r: &mut StepReport) {
    crate::satisfaction::update(world); // 6.1
    if world.data.rules.is_month_boundary(world.tick) {
        crate::levels::review(world, r); // 6.2
    }
}

/// Step 7 — treasury and taxes, M1.
fn finance(_world: &mut World) {}

/// Step 8 — random events, M1. First use of `RngDomain::Events`.
fn random_events(_world: &mut World) {}

/// Step 9 — scenario objectives, M1 (needs `sim-scenario`).
fn check_objectives(_world: &mut World, _r: &mut StepReport) {}

/// Step 10 — emits the coverage and mood deltas.
///
/// Only the **changes** relative to the start of the tick: a house that has
/// been hungry for ten ticks generates one event on the first, not ten. This is
/// the boundary with the renderer, and the wrong choice here would cost 40,000
/// events per tick.
fn emit_events(world: &mut World, before: &[HouseState], r: &mut StepReport) {
    let rules = &world.data.rules;

    for (house, h) in world.houses() {
        // A house born this tick has no "before": it starts uncovered and at
        // zero satisfaction, so if it is served the event is there, and its
        // mood is the `Desperate` the renderer already assumes.
        let previous = before
            .binary_search_by_key(&house, |s| s.id)
            .map_or(HouseState::newborn(house), |i| before[i]);

        if h.served != previous.served {
            for service in ServiceKind::ALL {
                if h.served.get(service) != previous.served.get(service) {
                    r.events.push(Event::ServiceCoverageChanged {
                        house,
                        service,
                        served: h.served.get(service),
                    });
                }
            }
        }

        // Against the house's level **as it is now**: a house promoted by step
        // 6.2 into a stricter requirement can lose mood in the same tick, and
        // the renderer has to hear about it.
        let mood = crate::satisfaction::mood_of(h, rules);
        if mood != previous.mood {
            r.events.push(Event::HouseMoodChanged { house, mood });
        }
    }
}

/// What step 10 compares against: the part of a house the renderer is told
/// about, as it was at the start of the tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HouseState {
    id: HouseId,
    served: ServiceFlags,
    mood: Mood,
}

impl HouseState {
    /// The state a house that did not exist at the start of the tick is
    /// compared against.
    const fn newborn(id: HouseId) -> Self {
        Self {
            id,
            served: ServiceFlags::empty(),
            mood: Mood::Desperate,
        }
    }
}

/// Every house's renderer-visible state, sorted by `HouseId` so step 10's
/// comparison is a binary search and not a scan.
///
/// The mood is computed here and not stored on the `House`: it is derived from
/// the satisfaction, and a second copy of it in the state would be one more
/// field to hash, to keep in step and to get wrong.
fn house_snapshot(world: &World) -> Vec<HouseState> {
    let rules = &world.data.rules;

    let mut v: Vec<HouseState> = world
        .houses()
        .map(|(id, h)| HouseState {
            id,
            served: h.served,
            mood: crate::satisfaction::mood_of(h, rules),
        })
        .collect();
    v.sort_unstable_by_key(|s| s.id);
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
