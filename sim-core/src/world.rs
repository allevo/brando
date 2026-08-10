//! The game state.
//!
//! A concrete struct with `SlotMap`s and `Vec`s, not an ECS `World` (D1): the
//! iteration order of an ECS's queries is not a stable contract, and for
//! balancing runs a tight loop over dense arrays is orders of magnitude faster.

use std::collections::BTreeMap;
use std::sync::Arc;

use slotmap::SlotMap;

use crate::coverage::Coverage;
use crate::data::{DataSet, DifficultyId};
use crate::grid::Grid;
use crate::ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
use crate::levels::PopulationTotals;
use crate::network::RoadNetwork;
use crate::production::FoodTotals;
use crate::rng::RngSet;
use crate::service::{ServiceFlags, ServiceKind};
use crate::units::{Coins, Milli};

/// A building that provides a service or produces goods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Building {
    pub kind: BuildingKindId,
    pub origin: TilePos,
    /// Level, counting from 1. In M0 it always stays 1.
    pub level: u8,
    /// Local stock, only for producers (phase 07).
    pub stock: Milli,
}

/// A house: the unit of population simulation (D5).
///
/// Individuals are not simulated. In M0 the residents are a fixed value from
/// the `rules` and the house does not level up: migration and levelling are M1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct House {
    pub origin: TilePos,
    pub level: u8,
    pub residents: u16,
    /// Which services reach it this tick.
    pub served: ServiceFlags,
    /// How long the service has been there, **not how much of it arrives**
    /// (A10). One slot per service, including the ones the current level does
    /// not require: if the house levels up, the time already accumulated on a
    /// service it was receiving anyway was not a lie.
    ///
    /// Goes up by `step_up` when the service is satisfied this tick, down by
    /// `step_down` when it is missing, saturating in `0..=max` (phase 12,
    /// step 6.1). `u8` and not `i16`: it is clamped and never needs the sign.
    ///
    /// **What it reads is [`House::served`], which means "covered".** Since
    /// phase 12 that is true of the food bit too: step 4 stops rewriting it to
    /// mean "it ate", and the two coincide only because a covered house always
    /// eats (A5). The day that invariant falls over — M3, when the goods come
    /// from a warehouse — satisfaction will rise for a house that did not eat.
    /// It is written here because here is where it would be an inexplicable
    /// balancing bug, and `FoodTotals::covered_but_unfed` is what says out loud
    /// that it has happened.
    pub satisfaction: [u8; ServiceKind::COUNT],
}

/// A real logistics walker (D3): goods transport, trade caravans, immigrants.
///
/// Empty in M0 and for all of M1: the real walkers arrive with M3. The water
/// carriers the player will see wandering around are decorative, live in the
/// renderer and never show up here (D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walker {
    pub at: TilePos,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Economy {
    pub treasury: Coins,
}

/// What has to be recomputed on the next tick.
///
/// The flags exist from the start as a deliberate choice: retrofitting them
/// later is painful (CLAUDE.md, tick order). In M0 the use is naive — when the
/// roads change, every provider goes dirty — but the structure is the final one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirtyFlags {
    pub roads: bool,
    /// Providers whose coverage has to be recomputed. An ordered `Vec`, not a
    /// `HashSet` (D4).
    ///
    /// **Nobody reads its contents today**: step 3 recomputes everything and
    /// only looks at [`DirtyFlags::coverage_needs_recompute`]. The list exists
    /// for targeted invalidation, which is the real optimisation of step 3 —
    /// keeping it filled now costs little and says which information will be
    /// needed. That is why "everything dirty" is **not** expressed by listing
    /// every provider but with [`DirtyFlags::invalidate_coverage`]: the day the
    /// list does get read, "everything" is precisely the case to avoid, not the
    /// one to enumerate.
    pub coverage: Vec<BuildingId>,
    /// Coverage has to be revisited even if no provider is in the list.
    ///
    /// It covers the case the list alone cannot express: once the last provider
    /// is demolished nobody is left to mark as dirty, but the existing
    /// assignments still have to be thrown away. Without this flag, houses
    /// stayed served by a well that no longer existed — a bug found by the
    /// demolition test in phase 06.
    pub coverage_invalidated: bool,
}

impl DirtyFlags {
    /// Marks a provider for recomputation, without duplicates and in
    /// `BuildingId` order.
    ///
    /// The order here is hygiene, not semantics: it is the iteration order of
    /// `World::buildings` that decides who wins a contested house (phase 06),
    /// and nobody reads this list yet. Keeping it sorted makes it comparable
    /// and turns insertion into a push at the end, since providers almost
    /// always arrive in increasing order.
    pub fn mark_coverage(&mut self, id: BuildingId) {
        self.coverage_invalidated = true;
        if let Err(pos) = self.coverage.binary_search(&id) {
            self.coverage.insert(pos, id);
        }
    }

    /// Coverage has to be revisited, without naming a specific provider.
    pub fn invalidate_coverage(&mut self) {
        self.coverage_invalidated = true;
    }

    pub fn coverage_needs_recompute(&self) -> bool {
        self.coverage_invalidated || !self.coverage.is_empty()
    }

    pub fn forget_coverage(&mut self, id: BuildingId) {
        if let Ok(pos) = self.coverage.binary_search(&id) {
            self.coverage.remove(pos);
        }
    }
}

/// The complete state of a game.
#[derive(Debug, Clone)]
pub struct World {
    pub(crate) tick: u32,
    pub(crate) grid: Grid,
    /// Iterating a `SlotMap` goes by slot index, so it is deterministic given
    /// the same sequence of insertions and removals — which the command log
    /// guarantees (D4). It is a non-obvious invariant the state hash depends on.
    pub(crate) buildings: SlotMap<BuildingId, Building>,
    pub(crate) houses: SlotMap<HouseId, House>,
    pub(crate) walkers: Vec<Walker>,
    pub(crate) economy: Economy,
    pub(crate) rng: RngSet,
    pub(crate) dirty: DirtyFlags,
    /// A structure **derived** from the grid: it does not enter the state hash
    /// (phase 08), otherwise a rebuild bug would show up as a hash divergence
    /// instead of a failing equivalence test.
    pub(crate) roads: RoadNetwork,
    /// Derived like [`RoadNetwork`], and outside the hash for the same reason.
    pub(crate) coverage: Coverage,
    /// Diagnostic bookkeeping, outside the hash: it influences no game
    /// decision.
    pub(crate) food: FoodTotals,
    /// Diagnostic like [`FoodTotals`], and outside the hash for the same
    /// reason. It is where the flows of population accumulate: one of them in
    /// phase 13, all four in phase 14.
    pub(crate) population: PopulationTotals,
    /// Indexes from origin tile to id. They are `BTreeMap`s and not `HashMap`s
    /// (D4): the iteration order is a contract.
    pub(crate) buildings_by_origin: BTreeMap<TileIdx, BuildingId>,
    pub(crate) houses_by_origin: BTreeMap<TileIdx, HouseId>,
    /// The balancing tables (A2). They live inside the state rather than being
    /// a parameter of `step` so they need not be threaded through every
    /// internal function; their hash feeds the state hash, so a balance change
    /// makes the replay fail immediately and for the right reason.
    pub(crate) data: Arc<DataSet>,
    /// Chosen at the start of a game, never changeable afterwards: it changes
    /// the simulation, so it is **state** — it goes into the hash and it
    /// travels in the replay's header (A13).
    pub(crate) difficulty: DifficultyId,
}

impl World {
    /// The initial world: an empty grid, the treasury from the `rules`, the RNG
    /// from the seed.
    ///
    /// The difficulty is a parameter and not a default on purpose: it changes
    /// the simulation, and a default is the mechanism by which one caller out
    /// of four would silently keep playing on another profile (A13).
    pub fn new(grid: Grid, data: Arc<DataSet>, seed: u64, difficulty: DifficultyId) -> Self {
        let treasury = data.rules.starting_treasury;
        let tiles = grid.len();
        Self {
            tick: 0,
            grid,
            buildings: SlotMap::with_key(),
            houses: SlotMap::with_key(),
            walkers: Vec::new(),
            economy: Economy { treasury },
            rng: RngSet::from_seed(seed),
            dirty: DirtyFlags::default(),
            roads: RoadNetwork::new(tiles),
            coverage: Coverage::default(),
            food: FoodTotals::default(),
            population: PopulationTotals::default(),
            buildings_by_origin: BTreeMap::new(),
            houses_by_origin: BTreeMap::new(),
            data,
            difficulty,
        }
    }

    pub const fn tick(&self) -> u32 {
        self.tick
    }

    pub const fn difficulty(&self) -> DifficultyId {
        self.difficulty
    }

    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    pub const fn economy(&self) -> &Economy {
        &self.economy
    }

    pub const fn rng(&self) -> &RngSet {
        &self.rng
    }

    pub const fn dirty(&self) -> &DirtyFlags {
        &self.dirty
    }

    pub const fn roads(&self) -> &RoadNetwork {
        &self.roads
    }

    pub const fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    pub const fn food(&self) -> &FoodTotals {
        &self.food
    }

    /// The running totals of the population's flows, as opposed to
    /// [`World::population`], which is how many people there are right now.
    pub const fn population_totals(&self) -> &PopulationTotals {
        &self.population
    }

    /// The sum of every producer's stock, in thousandths.
    ///
    /// `i64` like [`FoodTotals`]: it is the term that closes the conservation
    /// equality, and it has to cope with the same range.
    pub fn total_stock(&self) -> i64 {
        self.buildings
            .values()
            .filter(|b| {
                self.data
                    .def(b.kind)
                    .is_some_and(crate::data::BuildingDef::is_producer)
            })
            .map(|b| i64::from(b.stock.to_millis()))
            .sum()
    }

    pub fn data(&self) -> &DataSet {
        &self.data
    }

    pub fn buildings(&self) -> impl Iterator<Item = (BuildingId, &Building)> {
        self.buildings.iter()
    }

    pub fn building(&self, id: BuildingId) -> Option<&Building> {
        self.buildings.get(id)
    }

    pub fn houses(&self) -> impl Iterator<Item = (HouseId, &House)> {
        self.houses.iter()
    }

    pub fn house(&self, id: HouseId) -> Option<&House> {
        self.houses.get(id)
    }

    pub fn walkers(&self) -> &[Walker] {
        &self.walkers
    }

    pub fn building_count(&self) -> usize {
        self.buildings.len()
    }

    pub fn house_count(&self) -> usize {
        self.houses.len()
    }

    /// Total population.
    pub fn population(&self) -> u32 {
        self.houses.values().map(|h| u32::from(h.residents)).sum()
    }

    /// Sets the terrain of a tile.
    ///
    /// It serves **scenario setup**, before the game begins: it is the only
    /// mutation of the state that does not go through a `Command`, because the
    /// map is not a move by the player. It is not a channel for the renderer,
    /// which only ever writes commands into the core.
    /// `false` if the position is off the map.
    pub fn set_terrain(&mut self, pos: TilePos, terrain: crate::grid::Terrain) -> bool {
        match self.grid.at_mut(pos) {
            Some(t) => {
                t.terrain = terrain;
                true
            }
            None => false,
        }
    }

    /// The road tiles orthogonally adjacent to a building's area.
    ///
    /// **Game rule**: a building is hooked up to the network if at least one
    /// tile of its area touches a road orthogonally. It is the Zeus rule, where
    /// what counts is the entrance and not the building; in M1 it might become
    /// "one designated entrance tile", and then this is the function to change.
    pub fn building_entrances(&self, id: BuildingId) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.building_entrances_into(id, &mut out);
        out
    }

    /// Like [`World::building_entrances`], writing into a reusable buffer.
    pub fn building_entrances_into(&self, id: BuildingId, out: &mut Vec<TileIdx>) {
        out.clear();
        let Some(b) = self.buildings.get(id) else {
            return;
        };
        let size = self.data.def(b.kind).map_or((1, 1), |d| d.size);
        self.entrances_into(b.origin, size, out);
    }

    /// Like [`World::building_entrances`], for a house.
    pub fn house_entrances(&self, id: HouseId) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.house_entrances_into(id, &mut out);
        out
    }

    /// Like [`World::house_entrances`], writing into a reusable buffer.
    ///
    /// It exists for step 3, which calls it once per house on every
    /// recomputation: returning a fresh `Vec` every time meant 3,750
    /// allocations per recomputation at the reference scale, all of barely two
    /// elements.
    pub fn house_entrances_into(&self, id: HouseId, out: &mut Vec<TileIdx>) {
        out.clear();
        let Some(h) = self.houses.get(id) else {
            return;
        };
        self.entrances_into(h.origin, (1, 1), out);
    }

    /// The road tiles adjacent to an area, in `TileIdx` order.
    pub fn entrances(&self, origin: TilePos, size: (u8, u8)) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.entrances_into(origin, size, &mut out);
        out
    }

    /// Like [`World::entrances`], writing into a reusable buffer.
    pub fn entrances_into(&self, origin: TilePos, size: (u8, u8), out: &mut Vec<TileIdx>) {
        out.clear();
        for dy in 0..size.1 {
            for dx in 0..size.0 {
                let (Some(x), Some(y)) = (origin.x.checked_add(dx), origin.y.checked_add(dy))
                else {
                    continue;
                };
                let Some(idx) = self.grid.idx(TilePos::new(x, y)) else {
                    continue;
                };
                for v in self.grid.neighbors4(idx) {
                    if self.grid.get(v).is_some_and(|t| t.flags.has_road()) {
                        out.push(v);
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// The distance in tiles walked along the network, from one building to
    /// another.
    ///
    /// `None` if they are not connected or if they are further apart than
    /// `max`. The distance is counted between the **entrance road tiles**: two
    /// buildings facing the same road are 0 apart, and a corridor of N tiles
    /// between the two entrances counts as N - 1. It is walked distance, not
    /// Euclidean (D2).
    pub fn road_distance(&self, from: BuildingId, to: BuildingId, max: u16) -> Option<u16> {
        let starts = self.building_entrances(from);
        let targets = self.building_entrances(to);
        if starts.is_empty() || targets.is_empty() {
            return None;
        }
        let mut best: Option<u16> = None;
        // A local scratch buffer: this is not the hot path — step 3 uses its
        // own, reused across providers.
        let mut visited = crate::network::Visited::new(self.grid.len());
        crate::network::bfs_roads(&self.grid, &starts, max, &mut visited, |t, d| {
            if targets.binary_search(&t).is_ok() && best.is_none_or(|m| d < m) {
                best = Some(d);
            }
        });
        best
    }

    /// Resolves a tile's occupant into its id.
    ///
    /// `None` if the tile is free. An occupied tile that does not resolve to a
    /// live id is a bug, and it is the invariant that catches demolition
    /// mistakes (phase 09).
    pub fn occupant(&self, idx: TileIdx) -> Option<Occupant> {
        let occ = self.grid.get(idx)?.occupant()?;
        if occ.is_house {
            self.houses_by_origin
                .get(&occ.origin)
                .copied()
                .map(Occupant::House)
        } else {
            self.buildings_by_origin
                .get(&occ.origin)
                .copied()
                .map(Occupant::Building)
        }
    }
}

/// Whoever occupies a tile, resolved into an id in the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupant {
    Building(BuildingId),
    House(HouseId),
}

/// Direct-mutation hooks, behind the `test-util` feature.
///
/// They are deliberately not part of the normal API: the only write channel
/// into the core is `Command` (CLAUDE.md, core/renderer boundary). They serve
/// the phase 08 test that checks the state hash really covers every field of
/// the state — a check that by construction has to be able to touch one field
/// at a time.
#[cfg(feature = "test-util")]
impl World {
    pub fn building_mut(&mut self, id: BuildingId) -> Option<&mut Building> {
        self.buildings.get_mut(id)
    }

    pub fn house_mut(&mut self, id: HouseId) -> Option<&mut House> {
        self.houses.get_mut(id)
    }

    pub const fn economy_mut(&mut self) -> &mut Economy {
        &mut self.economy
    }

    /// Consumes one value from a domain's stream, to check that the RNG's
    /// position enters the hash.
    pub fn consume_rng(&mut self, domain: crate::rng::RngDomain) {
        use rand::RngCore as _;
        self.rng.get(domain).next_u64();
    }

    /// Forces the difficulty of an already started game.
    ///
    /// The game itself never does this — the profile is chosen at the start and
    /// stays put (A13). It exists for two tests: the perturbation that checks
    /// the difficulty enters the state hash, and the one that compares two
    /// games at different difficulties *net of the byte itself*, which is the
    /// only way to say "the knob acted here and nowhere else" once the byte is
    /// hashed from tick 0.
    pub const fn set_difficulty(&mut self, difficulty: DifficultyId) {
        self.difficulty = difficulty;
    }

    /// A **compile-time** canary for the state hash (A3).
    ///
    /// It does nothing at runtime. It exists because the exhaustive
    /// `let World { .. }` stops compiling the moment a field is added to the
    /// state: the reminder arrives while you are writing the field, not when a
    /// test fails — and it arrives even if nobody has added the matching
    /// perturbation to `the_hash_covers_the_whole_state`, which today is the
    /// only way that test notices anything.
    ///
    /// If you are reading this because it does not compile: add the field here,
    /// then decide whether it belongs in `hash_world` (state) or not
    /// (derived/diagnostic), and either way write which of the two in the
    /// field's doc comment.
    pub const fn field_canary(&self) {
        let Self {
            tick: _,
            grid: _,
            buildings: _,
            houses: _,
            walkers: _,
            economy: _,
            rng: _,
            dirty: _,
            roads: _,
            coverage: _,
            food: _,
            population: _,
            buildings_by_origin: _,
            houses_by_origin: _,
            data: _,
            difficulty: _,
        } = self;
    }
}
