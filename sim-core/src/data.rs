//! The balancing dataset: the validated shape the core consumes.
//!
//! The **definitions** live here rather than in `sim-data` because of the
//! direction of the dependencies: the `World` holds an `Arc<DataSet>` (A2), and
//! sim-core cannot depend on sim-data. What stays in `sim-data` is the RON
//! parsing, the validation and the I/O — that is, everything the core must not
//! do (D4).

use std::collections::BTreeMap;

use crate::data_hash::dataset_hash;
use crate::grid::Terrain;
use crate::ids::{BuildingKindId, Level};
use crate::satisfaction::Mood;
use crate::service::ServiceKind;
use crate::units::{Coins, Milli};

/// Global simulation constants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub ticks_per_month: u32,
    pub months_per_year: u32,
    pub starting_treasury: Coins,
    /// Indexed by house level (level 1 = index 0).
    pub house_levels: Vec<HouseLevelDef>,
    pub food_per_resident: Milli,
    pub satisfaction: SatisfactionRules,
}

/// One rung of the house ladder (phase 13): what it holds, what it demands,
/// and what it is worth to the treasury.
///
/// **Why in `Rules` and not in `BuildingDef`.** `House` does not carry a
/// [`BuildingKindId`], and in M1 there is only one kind of house. Adding one so
/// that a per-kind table could be indexed would be the invented abstraction D6
/// forbids before the second civilisation. When it really is needed,
/// [`Rules::max_residents`] is once again the only place to change — that is
/// why it is a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseLevelDef {
    /// The most residents it can hold. It is the house's ceiling, **not** what
    /// the coverage counts a provider's capacity against: that is counted on
    /// the residents who actually live there (A12). A house that can hold 8
    /// with two residents weighs two, not eight.
    pub max_residents: u16,
    /// The services needed to **rise to** this level and to **stay at** it.
    ///
    /// This is where `required_services` stops being declarative data read only
    /// by [`BuildingDef::is_house`] — the gap noted in A9. Three systems read
    /// it: the mood and the decay check use the house's **own** level, the
    /// level-up check the **destination's**.
    ///
    /// What it does **not** decide is which satisfaction accumulators move:
    /// those follow the union declared by [`BuildingDef::required_services`],
    /// and they have to, or a level introducing a service the level below does
    /// not require would be unreachable — its accumulator would sit frozen at
    /// zero for ever. See `satisfaction::update`.
    pub required_services: Vec<ServiceKind>,
    /// The minimum satisfaction on **each** of [`HouseLevelDef::required_services`]
    /// to rise here.
    ///
    /// Unread at level 1: a house is born there and nobody rises into it.
    pub level_up_threshold: u8,
    /// Below this, on any one of [`HouseLevelDef::required_services`], a house
    /// **at this level** decays.
    ///
    /// Together with the field above it forms the hysteresis band
    /// `decay_threshold..level_up_threshold`, inside which a house stays where
    /// it is whichever side it came from. A band that is not strictly positive
    /// is [`Inconsistency::NoHysteresis`] — hysteresis is a **check**, not a
    /// comment (A10).
    ///
    /// Unread at level 1: there is no level 0 to fall to.
    pub decay_threshold: u8,
    /// The taxable base per resident (phase 16). Zero until then, and read by
    /// nobody: it is declared now because the ladder is the table phase 16 will
    /// want it in, and adding it later would regenerate the recordings a second
    /// time for one number.
    pub taxable_per_resident: Milli,
}

/// How fast a house's satisfaction rises and falls, and where the bands the
/// renderer draws begin (phase 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatisfactionRules {
    /// The ceiling of the accumulator. `max / step_up` ticks take a served
    /// house from zero to here.
    pub max: u8,
    /// Added when the service is there this tick.
    pub step_up: u8,
    /// Subtracted when it is missing. Larger than `step_up` in the production
    /// tables: losing the water is an event, getting it back is an investment.
    pub step_down: u8,
    /// The lower bound, inclusive, of each band above [`Mood::Desperate`], in
    /// ascending order.
    ///
    /// Validated as strictly ascending, above zero and no greater than `max`:
    /// that is what makes a satisfaction of zero always `Desperate`, and
    /// [`Mood::of`] total.
    pub mood_thresholds: [u8; Mood::COUNT - 1],
}

impl Rules {
    /// Ticks in a game year. Scenario objectives are expressed in months and
    /// years, never in ticks (M1).
    ///
    /// Saturating for [`is_month_boundary`]'s reason: both operands come from a
    /// table, and the core does not panic on data. Tables loaded through
    /// `sim-data` cannot overflow it — `validate_rules` refuses them — but the
    /// hand-built fixtures (`sim-core/tests/common/mod.rs`, `xtask`'s bench)
    /// never go through that door.
    ///
    /// [`is_month_boundary`]: Self::is_month_boundary
    pub const fn ticks_per_year(&self) -> u32 {
        self.ticks_per_month.saturating_mul(self.months_per_year)
    }

    /// The definition of a house level, `None` out of range.
    pub fn house_level(&self, level: Level) -> Option<&HouseLevelDef> {
        self.house_levels.get(level.as_usize())
    }

    /// The ladder, rung by rung, each with its level.
    ///
    /// The only place that pairs a position in the table with the level it
    /// stands for, which is why the cross-table checks and the tests that walk
    /// the ladder all come through here instead of adding one to an index
    /// apiece.
    pub fn house_ladder(&self) -> impl Iterator<Item = (Level, &HouseLevelDef)> {
        self.house_levels
            .iter()
            .enumerate()
            .map(|(i, def)| (Level::from_index(i), def))
    }

    /// The most residents a house can hold at the given level.
    ///
    /// `None` out of range, like [`ServiceDef::range`] and
    /// [`ServiceDef::capacity`]: a level that does not exist in the table is a
    /// data error, not a panic.
    ///
    /// It is a function and not a direct access to the `Vec` because phase 13
    /// restructured the flat `residents_per_house_level` into the per-level
    /// table: the **body** changed, not the callers. It is still the one place
    /// to change the day houses come in kinds.
    pub fn max_residents(&self, level: Level) -> Option<u16> {
        Some(self.house_level(level)?.max_residents)
    }

    /// The services a house at this level requires.
    ///
    /// Empty out of range, and that is deliberate: the callers are the mood and
    /// the decay check, and a level that does not exist has to demand nothing
    /// rather than panic. A level that really demands nothing is refused by
    /// validation.
    pub fn required_at(&self, level: Level) -> &[ServiceKind] {
        self.house_level(level)
            .map_or(&[], |l| l.required_services.as_slice())
    }

    /// The highest level a house can reach. `None` if the table is empty, which
    /// validation refuses.
    pub fn top_house_level(&self) -> Option<Level> {
        self.house_levels
            .len()
            .checked_sub(1)
            .map(Level::from_index)
    }

    /// Whether this tick is a month boundary — when the level review happens
    /// (step 6.2).
    ///
    /// The cadence is what makes the absence of oscillation **structural**
    /// rather than a consequence of the thresholds: thirty ticks pass between
    /// two decisions, so a house cannot change level more than twelve times a
    /// year whatever the balancing does. Tick 0 is a boundary and the review
    /// there is a no-op: every accumulator is still at zero.
    ///
    /// The guard on zero is not defensive noise: `ticks_per_month` comes from a
    /// table, a modulo by zero is a panic, and the core does not panic on data.
    pub const fn is_month_boundary(&self, tick: u32) -> bool {
        self.ticks_per_month != 0 && tick.is_multiple_of(self.ticks_per_month)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainDef {
    pub buildable: bool,
    pub walkable: bool,
    pub road_cost: Coins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDef {
    pub kind: ServiceKind,
    /// Range in tiles walked along the road network (D2), one value per level.
    pub range_per_level: Vec<u16>,
    /// **Residents** served at the same time, one value per level.
    ///
    /// Not houses: once houses have levels (M1) the population varies from
    /// house to house, and a capacity counted in houses would no longer say
    /// how many people the provider can really serve.
    pub capacity_per_level: Vec<u16>,
}

impl ServiceDef {
    /// The range at the given level, `None` out of range.
    pub fn range(&self, level: Level) -> Option<u16> {
        self.range_per_level.get(level.as_usize()).copied()
    }

    /// The capacity in residents at the given level, `None` out of range.
    pub fn capacity(&self, level: Level) -> Option<u16> {
        self.capacity_per_level.get(level.as_usize()).copied()
    }

    /// The capacities, rung by rung, each with its level. The provider's
    /// counterpart of [`Rules::house_ladder`].
    pub fn capacities(&self) -> impl Iterator<Item = (Level, u16)> {
        self.capacity_per_level
            .iter()
            .enumerate()
            .map(|(i, &capacity)| (Level::from_index(i), capacity))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// (width, height) in tiles.
    pub size: (u8, u8),
    pub cost: Coins,
    pub levels: u8,
    pub service: Option<ServiceDef>,
    pub required_services: Vec<ServiceKind>,
    pub output_per_tick: Option<Milli>,
    pub max_stock: Option<Milli>,
}

impl BuildingDef {
    /// A building is a house if it requires services instead of providing them.
    /// A structural rule, not a number: it belongs in the code on purpose.
    pub fn is_house(&self) -> bool {
        self.service.is_none() && !self.required_services.is_empty()
    }

    pub fn is_producer(&self) -> bool {
        self.output_per_tick.is_some()
    }

    /// How many tiles it takes up.
    pub const fn tile_count(&self) -> u16 {
        self.size.0 as u16 * self.size.1 as u16
    }
}

/// An index into the profiles table.
///
/// Not an enum: difficulty is data (D6), and a civilisation or a scenario will
/// be able to declare its own without touching the code.
///
/// No `Default`. It looks like an inconvenience and it is not: [`World::new`]
/// has four callers, and a default is exactly the mechanism by which one of the
/// four would be left behind with nothing to flag it.
///
/// It is not `Serialize` either, on purpose: what travels in a replay's header
/// is the **textual** id, never this index. A `Serialize` impl here is the one
/// thing that would make writing the index into a save file look natural (A13).
///
/// [`World::new`]: crate::world::World::new
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DifficultyId(u8);

impl DifficultyId {
    /// Crate-private: outside `sim-core` an id can only be obtained from
    /// [`DataSet::difficulty_by_id`], so one that resolves to no profile cannot
    /// be built by mistake.
    pub(crate) const fn new(v: u8) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// A difficulty profile: the knobs chosen at the start of a game.
///
/// Born with one field. Phases 13, 14 and 16 add theirs here without touching
/// the replay's header or [`World::new`]'s signature again.
///
/// [`World::new`]: crate::world::World::new
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifficultyDef {
    pub id: String,
    /// The residents of a newly-built house. Zero is legitimate: the house
    /// fills up by migration (phase 15).
    pub starting_residents_per_house: u16,
}

/// The validated tables, ready for the core.
///
/// It lives behind an `Arc` inside the `World` (A2): loading is I/O and stays
/// outside the core, but the systems need the tables every tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSet {
    pub rules: Rules,
    /// A `BTreeMap` and not a `HashMap`: the iteration order is a contract (D4).
    pub terrain: BTreeMap<Terrain, TerrainDef>,
    /// Indexed by [`BuildingKindId`].
    pub buildings: Vec<BuildingDef>,
    /// Indexed by [`DifficultyId`]. At most 256 of them, which is what makes
    /// the index a `u8`.
    pub difficulties: Vec<DifficultyDef>,
    /// blake3 of the **validated** content, not of the files' bytes:
    /// reformatting a RON file or adding a comment does not change the hash,
    /// changing a number does. It feeds into the state hash (A2).
    /// [`DataSet::new`] computes it.
    pub hash: [u8; 32],
}

impl DataSet {
    /// Builds the dataset and computes its hash. The only way to obtain a
    /// `DataSet`, so the hash cannot fall out of sync with the content.
    pub fn new(
        rules: Rules,
        terrain: BTreeMap<Terrain, TerrainDef>,
        buildings: Vec<BuildingDef>,
        difficulties: Vec<DifficultyDef>,
    ) -> Self {
        let hash = dataset_hash(&rules, &terrain, &buildings, &difficulties);
        Self {
            rules,
            terrain,
            buildings,
            difficulties,
            hash,
        }
    }

    pub fn def(&self, kind: BuildingKindId) -> Option<&BuildingDef> {
        self.buildings.get(kind.as_usize())
    }

    /// The definition of the house.
    ///
    /// Found by [`BuildingDef::is_house`] and not by the textual id `"house"`:
    /// it is the same classifier the placing of a building uses to decide
    /// whether what is being built is a house, so a building placed as a house
    /// is also read as one afterwards. A civilisation whose houses are called
    /// something else keeps working; one with two kinds of house does not, and
    /// that is a limit `House` will dissolve when it starts carrying its own
    /// kind (M3).
    pub fn house_def(&self) -> Option<&BuildingDef> {
        self.buildings.iter().find(|b| b.is_house())
    }

    /// Resolves the textual id used in the tables and in the scenarios.
    pub fn kind_by_id(&self, id: &str) -> Option<BuildingKindId> {
        let pos = self.buildings.iter().position(|b| b.id == id)?;
        u16::try_from(pos).ok().map(BuildingKindId::new)
    }

    pub fn difficulty(&self, d: DifficultyId) -> Option<&DifficultyDef> {
        self.difficulties.get(d.as_usize())
    }

    /// Resolves the textual id used in the tables and in the replay headers.
    pub fn difficulty_by_id(&self, id: &str) -> Option<DifficultyId> {
        let pos = self.difficulties.iter().position(|d| d.id == id)?;
        u8::try_from(pos).ok().map(DifficultyId::new)
    }

    /// The known profile ids, for error messages.
    pub fn difficulty_ids(&self) -> String {
        self.difficulties
            .iter()
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn terrain(&self, t: Terrain) -> Option<&TerrainDef> {
        self.terrain.get(&t)
    }

    /// The hash in hexadecimal, for error messages and replay headers.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Every inconsistency *between* tables, in one place.
    ///
    /// It lives in the core because that is where the definitions live (A2) and
    /// because it is also needed by the fixture in
    /// `sim-core/tests/common/mod.rs`, which does not go through `sim-data`.
    /// Adding a check here automatically makes it active on the fixture too: it
    /// is the generalisation of A5's lesson — a number that has to stand in a
    /// relation with another one is a **check**, not a comment.
    ///
    /// The split with `sim-data` is by kind, not by convenience: what can be
    /// checked on one field of one table (a negative cost, an unknown service
    /// name, a list of the wrong length) stays in `sim-data`'s validation, and
    /// everything **relational** comes here, where the fixture is protected by
    /// it too.
    ///
    /// The order of the checks is fixed and is part of the contract: it is the
    /// order the validation report comes out in, and tests assert on whole
    /// vectors.
    pub fn inconsistencies(&self) -> Vec<Inconsistency> {
        let mut out = Vec::new();
        self.check_the_house_agrees_with_the_ladder(&mut out);
        self.check_the_ladder(&mut out);
        self.check_the_levels_can_be_served(&mut out);
        self.check_food_capacity(&mut out);
        self.check_difficulty(&mut out);
        out
    }

    /// The house building and `rules.house_levels` describe the same house.
    ///
    /// [`BuildingDef::is_house`] is a heuristic — no service, and it requires
    /// some. If the per-level lists were to become the only place requirements
    /// live and the building's line disappeared, **the house would stop being a
    /// house** and half the game would change behaviour in silence. These three
    /// checks are what makes that impossible: one says a house exists at all,
    /// one that the levels are as many as declared, one that the building's
    /// list is exactly the union of the levels'.
    fn check_the_house_agrees_with_the_ladder(&self, out: &mut Vec<Inconsistency>) {
        let Some(house) = self.house_def() else {
            out.push(Inconsistency::NoHouse);
            return;
        };
        if usize::from(house.levels) != self.rules.house_levels.len() {
            out.push(Inconsistency::LevelCountMismatch {
                declared: house.levels,
                in_table: self.rules.house_levels.len(),
            });
        }

        let union = sorted(
            self.rules
                .house_levels
                .iter()
                .flat_map(|l| l.required_services.iter().copied()),
        );
        let declared = sorted(house.required_services.iter().copied());
        if declared != union {
            out.push(Inconsistency::InconsistentRequirements {
                declared,
                in_levels: union,
            });
        }
    }

    /// The ladder goes up, and each rung has a hysteresis band you can stand
    /// on.
    fn check_the_ladder(&self, out: &mut Vec<Inconsistency>) {
        let max = self.rules.satisfaction.max;
        let mut previous: Option<u16> = None;
        for (level, def) in self.rules.house_ladder() {
            if let Some(previous) = previous
                && def.max_residents <= previous
            {
                out.push(Inconsistency::CapacityNotIncreasing {
                    level,
                    max_residents: def.max_residents,
                    previous,
                });
            }
            previous = Some(def.max_residents);

            // Level 1's thresholds are unread — nobody rises into it and there
            // is no level 0 to fall to — so checking them would report on
            // numbers that decide nothing.
            if level == Level::FIRST {
                continue;
            }
            if def.decay_threshold >= def.level_up_threshold {
                out.push(Inconsistency::NoHysteresis {
                    level,
                    decay: def.decay_threshold,
                    level_up: def.level_up_threshold,
                });
            }
            // Only the upper threshold is compared against the ceiling: with
            // the band above green, `decay < level_up <= max` follows.
            if def.level_up_threshold > max {
                out.push(Inconsistency::UnreachableThreshold {
                    level,
                    threshold: def.level_up_threshold,
                    max,
                });
            }
        }
    }

    /// Somebody provides what the levels ask for, and to a house that is full.
    fn check_the_levels_can_be_served(&self, out: &mut Vec<Inconsistency>) {
        for (level, def) in self.rules.house_ladder() {
            for &service in &def.required_services {
                let best = self
                    .buildings
                    .iter()
                    .filter_map(|b| b.service.as_ref())
                    .filter(|s| s.kind == service)
                    .filter_map(|s| s.capacity_per_level.iter().copied().max())
                    .max();
                match best {
                    None => out.push(Inconsistency::ServiceWithoutProvider { level, service }),
                    // With A12 this is not a blocker — the house is servable as
                    // long as it stays half empty — but it is a dataset in
                    // which a level can never be served in full, and the city
                    // plugs up without saying why.
                    Some(best) if best < def.max_residents => {
                        out.push(Inconsistency::CapacityBeyondEveryProvider {
                            level,
                            service,
                            max_residents: def.max_residents,
                            best,
                        });
                    }
                    Some(_) => {}
                }
            }
        }
    }

    /// The food providers that claim more capacity than their output can
    /// sustain.
    ///
    /// **Why this is a check and not a convention in the comments.** If a food
    /// provider can take on more residents than it feeds, the houses in excess
    /// stay assigned to it — the first provider wins a contested house — and
    /// stop eating: hunger becomes an **absorbing** state, one that not even
    /// building a second farm dissolves. With this check green the reverse
    /// implication holds instead, and it is a rule of the game: *a house
    /// covered by the food service always eats*.
    ///
    /// The arithmetic is for the worst case, an empty stock at the start of the
    /// tick: what the provider can hand out is the smaller of one tick's output
    /// and what the granary can hold. A provider that declares **neither** is
    /// part of the same rule and not an exception to it: it hands out nothing,
    /// so every positive capacity it claims is beyond what it sustains.
    ///
    /// **It lives exactly as long as A5.** It is the right rule while the farm
    /// produces into its own stock; in M3 the goods will come from a warehouse
    /// via real logistics walkers, capacity will stop depending on local
    /// output, and this check has to go along with the simplification it
    /// guards.
    fn check_food_capacity(&self, out: &mut Vec<Inconsistency>) {
        let per_resident = self.rules.food_per_resident.to_millis();
        if per_resident <= 0 {
            // Free food: any capacity is sustainable. It is not this check's
            // job to say the dataset makes no sense.
            return;
        }

        for (building, def) in self.buildings.iter().enumerate() {
            let Some(service) = def.service.as_ref() else {
                continue;
            };
            if service.kind != ServiceKind::Food {
                continue;
            }
            // Skipping the provider that declares no output, or no granary to
            // hold it in, is what let a phantom farm through: the coverage
            // never consults `output_per_tick`, so it wins its houses on
            // distance alone and then feeds none of them, for ever — the first
            // provider keeps a contested house, so not even a real farm built
            // afterwards can take them over. It sustains nobody, and the loop
            // below says so with `sustainable: 0`.
            let available = match (def.output_per_tick, def.max_stock) {
                (Some(output), Some(max_stock)) => output.to_millis().min(max_stock.to_millis()),
                _ => 0,
            };
            let sustainable = u16::try_from(available / per_resident).unwrap_or(u16::MAX);
            for (level, capacity) in service.capacities() {
                if capacity > sustainable {
                    out.push(Inconsistency::CapacityBeyondOutput {
                        building,
                        level,
                        capacity,
                        sustainable,
                    });
                }
            }
        }
    }

    /// No difficulty profile builds a house beyond its own capacity.
    ///
    /// A house born beyond `max_residents(1)` is a state the rest of the game
    /// cannot represent: since phase 13, `residents <= max_residents(level)` is
    /// assumed everywhere.
    fn check_difficulty(&self, out: &mut Vec<Inconsistency>) {
        // No level 1 in the table is `rules.house_levels` being empty, which
        // validation reports on its own: saying it twice would be noise.
        let Some(max_residents) = self.rules.max_residents(Level::FIRST) else {
            return;
        };
        for (difficulty, def) in self.difficulties.iter().enumerate() {
            if def.starting_residents_per_house > max_residents {
                out.push(Inconsistency::StartingResidentsBeyondCapacity {
                    difficulty,
                    starting: def.starting_residents_per_house,
                    max_residents,
                });
            }
        }
    }
}

/// A relation between two tables that does not hold.
///
/// [`DataSet::inconsistencies`] finds them; `sim-data` turns each one into a
/// validation error with the path of the field that causes it. The message
/// lives here, next to the check, because the *why* is what makes it useful.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Inconsistency {
    #[error(
        "no building is a house: `is_house()` classifies as a house whatever \
         requires services without providing any, and with none of them the \
         houses stop levelling up, stop eating and stop being taxed, in silence"
    )]
    NoHouse,

    #[error("the house declares {declared} levels, rules.house_levels has {in_table}")]
    LevelCountMismatch { declared: u8, in_table: usize },

    #[error(
        "the house requires {} but its levels require {}: the building's list \
         has to stay the union, or `is_house()` stops recognising it",
        ids(declared),
        ids(in_levels)
    )]
    InconsistentRequirements {
        declared: Vec<ServiceKind>,
        in_levels: Vec<ServiceKind>,
    },

    #[error(
        "level {level} holds {max_residents} residents, no more than level {} \
         with {previous}: levelling up would shrink the house",
        // Unreachable: with no rung below there is no `previous` capacity to be
        // no more than, so the check cannot fire at the first level.
        level.previous().unwrap_or(Level::FIRST)
    )]
    CapacityNotIncreasing {
        level: Level,
        max_residents: u16,
        previous: u16,
    },

    #[error(
        "level {level} decays below {decay} and is reached at {level_up}: with \
         no hysteresis band the city oscillates at every review"
    )]
    NoHysteresis {
        level: Level,
        decay: u8,
        level_up: u8,
    },

    #[error(
        "level {level} is reached at a satisfaction of {threshold}, beyond the \
         maximum of {max}: it is unreachable by construction"
    )]
    UnreachableThreshold {
        level: Level,
        threshold: u8,
        max: u8,
    },

    #[error("level {level} requires {}, which no building provides", service.as_id())]
    ServiceWithoutProvider { level: Level, service: ServiceKind },

    #[error(
        "level {level} holds {max_residents} residents and the largest provider \
         of {} takes {best}: the level can never be served in full",
        service.as_id()
    )]
    CapacityBeyondEveryProvider {
        level: Level,
        service: ServiceKind,
        max_residents: u16,
        best: u16,
    },

    #[error(
        "capacity of {capacity} residents, but the output sustains {sustainable}: \
         the houses in excess would stay assigned to a provider that does not feed them"
    )]
    CapacityBeyondOutput {
        /// Index into [`DataSet::buildings`].
        building: usize,
        /// The level at which the capacity overshoots. The **provider's**
        /// level, not a house rung: the two ladders share a type, not a table.
        level: Level,
        /// Residents claimed in the table.
        capacity: u16,
        /// Residents the output really sustains.
        sustainable: u16,
    },

    #[error(
        "a house born with {starting} residents, but at level 1 it holds \
         {max_residents}: the rest of the game cannot represent a house beyond \
         its own capacity"
    )]
    StartingResidentsBeyondCapacity {
        /// Index into [`DataSet::difficulties`].
        difficulty: usize,
        /// Residents the profile puts in a newly-built house.
        starting: u16,
        /// Residents a house can hold at level 1.
        max_residents: u16,
    },
}

/// The services, in a canonical order and without repeats, so two lists can be
/// compared as sets.
fn sorted(services: impl IntoIterator<Item = ServiceKind>) -> Vec<ServiceKind> {
    let mut v: Vec<ServiceKind> = services.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The service ids of a list, for an error message.
fn ids(services: &[ServiceKind]) -> String {
    if services.is_empty() {
        return "nothing".to_string();
    }
    services
        .iter()
        .map(|s| s.as_id())
        .collect::<Vec<_>>()
        .join(", ")
}
