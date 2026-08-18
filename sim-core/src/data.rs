//! The balancing dataset: the validated shape the core consumes.
//!
//! The **definitions** live here rather than in `sim-data` because of the
//! direction of the dependencies: the `World` holds an `Arc<DataSet>`, and
//! sim-core cannot depend on sim-data. What stays in `sim-data` is the RON
//! parsing, the validation and the I/O — that is, everything the core must not
//! do (D4).

use crate::data_hash::dataset_hash;
use crate::grid::Terrain;
use crate::ids::{BuildingKindId, Level};
use crate::satisfaction::Mood;
use crate::service::ServiceKind;
use crate::tick::Tick;
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
    pub demographics: DemographicsRules,
}

/// The rates that drive births and deaths (phase 14).
///
/// **Per month and per thousand residents**, because that is the form you read
/// and reason in. The conversion to ticks divides by `ticks_per_month` and
/// loses nothing: what does not mature this tick stays in
/// [`Demographics::remainder`].
///
/// [`Demographics::remainder`]: crate::demographics::Demographics
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemographicsRules {
    /// Births per thousand **eligible** residents per month, at the city's
    /// maximum satisfaction. Counted against the eligible and not against the
    /// population: that is what makes the plateau structural.
    pub births_per_thousand_per_month: u16,
    /// Deaths per thousand residents per month, in a house whose own level has
    /// every service it asks for.
    pub deaths_per_thousand_per_month: u16,
    /// The rate for the residents of a house going without — below
    /// `unserved_threshold` on any service **its own level** requires. Not
    /// "hungry": a hut that asks for water only is not going without when there
    /// is no farm.
    pub deaths_per_thousand_per_month_when_unserved: u16,
    /// The satisfaction below which a house counts as going without.
    pub unserved_threshold: u8,
    /// The satisfaction a house needs before it can have children.
    pub birth_threshold: u8,
    /// The jitter on a rate, in thousandths, symmetric: `200` is ±20%.
    ///
    /// It can move into the difficulty profile the day a harder game should
    /// also be a more volatile one; it is here because there is no reason yet.
    pub jitter_per_thousand: u16,
}

impl DemographicsRules {
    /// Whether the demographics are **switched off**: every rate at zero.
    ///
    /// A legal configuration and not a degenerate one, which is why it is a
    /// method and not an accident. Two things need it. `coverage_equivalence`
    /// has to run on a city whose population cannot move, or it compares the
    /// stored coverage against a from-scratch one computed after step 6 has
    /// already moved somebody and diverges for a reason that is not a bug.
    /// And `bench --zero-demographics` measures `H`, the tick with the
    /// demographics off, which is the term slot 18.5 needs in order to tell the
    /// cost of the recomputation apart from the cost of step 6 itself.
    ///
    /// All three rates or none: a table with births at zero and deaths at six
    /// is a city that dies out, and that is the mistake
    /// [`Inconsistency::UnsustainableDemographics`] exists to catch. Only the
    /// unanimous case is "off".
    pub const fn is_off(&self) -> bool {
        self.births_per_thousand_per_month == 0
            && self.deaths_per_thousand_per_month == 0
            && self.deaths_per_thousand_per_month_when_unserved == 0
    }
}

/// One entry of the house levels table (phase 13): what it holds, what it
/// demands, and what it is worth to the treasury.
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
    /// the residents who actually live there. A house that can hold 8
    /// with two residents weighs two, not eight.
    pub max_residents: u16,
    /// The services needed to **rise to** this level and to **stay at** it.
    ///
    /// This is where `required_services` stops being declarative data read only
    /// by [`BuildingDef::is_house`]: the coverage never reads it, which is the
    /// hole this field closes. Three systems read it: the mood and the decay
    /// check use the house's **own** level, the level-up check the
    /// **destination's**.
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
    /// Together with the field above it forms the gap
    /// `decay_threshold..level_up_threshold`, inside which a house stays where it
    /// is whichever side it came from. A gap that is not strictly positive is
    /// [`Inconsistency::NoGap`] — the gap is a **check**, not a comment.
    ///
    /// Unread at level 1: there is no level 0 to fall to.
    pub decay_threshold: u8,
    /// The taxable base per resident (phase 16). Zero until then, and read by
    /// nobody: it is declared now because the levels are the table phase 16 will
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
    /// The lower bound, inclusive, of each band above [`Mood::Awful`], in
    /// ascending order.
    ///
    /// Validated as strictly ascending, above zero and no greater than `max`:
    /// that is what makes a satisfaction of zero always `Awful`, and
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

    /// Every entry of `house_levels`, paired with the level it stands for.
    ///
    /// The only place that pairs a position in the table with the level it
    /// stands for, which is why the cross-table checks and the tests that walk
    /// the levels all come through here instead of adding one to an index
    /// apiece.
    pub fn all_levels(&self) -> impl Iterator<Item = (Level, &HouseLevelDef)> {
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
    pub const fn is_month_boundary(&self, tick: Tick) -> bool {
        self.ticks_per_month != 0 && tick.is_multiple_of(self.ticks_per_month)
    }
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
    ///
    /// The range is service radius in walking distance unit.
    pub fn range(&self, level: Level) -> Option<u16> {
        self.range_per_level.get(level.as_usize()).copied()
    }

    /// The capacity in residents at the given level, `None` out of range.
    pub fn capacity(&self, level: Level) -> Option<u16> {
        self.capacity_per_level.get(level.as_usize()).copied()
    }

    /// Every capacity, paired with the level it stands for. The provider's
    /// counterpart of [`Rules::all_levels`].
    pub fn capacities(&self) -> impl Iterator<Item = (Level, u16)> {
        self.capacity_per_level
            .iter()
            .enumerate()
            .map(|(i, &capacity)| (Level::from_index(i), capacity))
    }
}

/// What part a building plays in the city: a **house**, which needs services,
/// or a **provider**, which supplies one.
///
/// **It is declared in the table, not worked out from what is missing.** Until
/// phase 14.4 a building was a house because it had no service *and* asked for
/// some, and keeping that inference true cost three validation checks: the
/// building's `required_services` had to stay the exact union of its levels', or
/// the house quietly stopped being read as a house at all. The field said two
/// unrelated things at once — *which satisfaction accumulators move* and *this
/// is a house* — and only the second one made it un-simplifiable.
///
/// **A sum type and not a tag beside the old fields**, because the flat struct
/// was a union in disguise: a house carried three `None`s and a well carried an
/// empty list, so every illegal combination was representable and held off by a
/// check. Here they cannot be built.
///
/// **The variants are roles, not buildings.** House and provider are two, and
/// stay two; a variant per building — house, well, farm — would put the roster
/// in the code, which D6 forbids, would duplicate `size`, `cost` and `levels`
/// into every arm, and would grow a match at every shared read for each building
/// the game gains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingRole {
    /// A house. The list is the **union** of what its levels ask for, and it
    /// says which satisfaction accumulators move — nothing else. See
    /// `satisfaction::update`, and [`HouseLevelDef::required_services`] for the
    /// per-level list the mood and the level review read instead.
    House { required_services: Vec<ServiceKind> },
    /// A provider: it supplies one service to the houses within its range.
    Provider { service: ServiceDef },
}

/// What a building grows, and the granary it grows into.
///
/// **Orthogonal to the role, because the farm is both.** It provides food *and*
/// produces it, so a role variant per kind of thing could not hold it; and
/// putting production inside [`BuildingRole::Provider`] would rule out a
/// producer that provides no service — M3's warehouse — which is the widening
/// D6 warns against inventing before something needs it.
///
/// **Both fields, always.** An output with no granary to hold it, or a granary
/// with nothing to put in it, are the two mistakes `ProducerWithoutStock` and
/// `StockWithoutOutput` name in the raw table. They stay as validation errors —
/// building this struct is what validation does when they do *not* fire — and
/// past that point the broken state has no shape to be in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub output_per_tick: Milli,
    /// The granary's lid. What would go in beyond it is lost, not queued.
    pub max_stock: Milli,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// (width, height) in tiles.
    pub size: (u8, u8),
    pub cost: Coins,
    pub levels: u8,
    pub role: BuildingRole,
    pub production: Option<Production>,
}

impl BuildingDef {
    /// Whether this building is a house. It **reads what the table declared**
    /// rather than inferring it from what the row is missing.
    ///
    /// It keeps its name and its signature from when it was a heuristic, which
    /// is why `world.rs`, `tick.rs` and `grid.rs` go on asking the same question
    /// unchanged — the point of it having been a function all along.
    pub const fn is_house(&self) -> bool {
        matches!(self.role, BuildingRole::House { .. })
    }

    /// Whether this building grows something. Independent of the role: the farm
    /// is a provider that also produces.
    pub const fn is_producer(&self) -> bool {
        self.production.is_some()
    }

    /// The service this building supplies, `None` for a house.
    pub const fn service(&self) -> Option<&ServiceDef> {
        match &self.role {
            BuildingRole::Provider { service } => Some(service),
            BuildingRole::House { .. } => None,
        }
    }

    /// The services this building **requires** — the union across its levels,
    /// and empty for anything that is not a house.
    ///
    /// It says which satisfaction accumulators move, and that is all it says
    /// since the role took over classifying.
    pub fn required_services(&self) -> &[ServiceKind] {
        match &self.role {
            BuildingRole::House { required_services } => required_services,
            BuildingRole::Provider { .. } => &[],
        }
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
/// thing that would make writing the index into a save file look natural.
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
/// It lives behind an `Arc` inside the `World`: loading is I/O and stays
/// outside the core, but the systems need the tables every tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSet {
    pub rules: Rules,
    /// What laying a road costs on each terrain, indexed by the terrain's own
    /// position in its declaration — one numbering serves both this array and
    /// the state hash, so there is no second one to drift. Whether a road may
    /// be laid there at all is not here: that is a fact about the ground and
    /// [`Terrain::is_walkable`] answers it. Every terrain has an entry, and the
    /// loader refuses a table that leaves one out.
    pub road_cost_per_terrain: [Coins; Terrain::COUNT],
    /// Indexed by [`BuildingKindId`].
    pub buildings: Vec<BuildingDef>,
    /// Indexed by [`DifficultyId`]. At most 256 of them, which is what makes
    /// the index a `u8`.
    pub difficulties: Vec<DifficultyDef>,
    /// blake3 of the **validated** content, not of the files' bytes:
    /// reformatting a RON file or adding a comment does not change the hash,
    /// changing a number does. It feeds into the state hash.
    /// [`DataSet::new`] computes it.
    pub hash: [u8; 32],
}

impl DataSet {
    /// Builds the dataset and computes its hash. The only way to obtain a
    /// `DataSet`, so the hash cannot fall out of sync with the content.
    pub fn new(
        rules: Rules,
        road_cost_per_terrain: [Coins; Terrain::COUNT],
        buildings: Vec<BuildingDef>,
        difficulties: Vec<DifficultyDef>,
    ) -> Self {
        let hash = dataset_hash(&rules, &road_cost_per_terrain, &buildings, &difficulties);
        Self {
            rules,
            road_cost_per_terrain,
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
    /// Found by the declared role and not by the textual id `"house"`: it asks
    /// the same question [`BuildingDef::is_house`] answers when a building is
    /// placed, so a building placed as a house is also read as one afterwards.
    /// A civilisation whose houses are called something else keeps working; one
    /// with two kinds of house does not, and that is a limit `House` will
    /// dissolve when it starts carrying its own [`BuildingKindId`] (M3) —
    /// declaring the role did not lift it, because a house still does not
    /// record which row it came from.
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

    /// What laying a road on this terrain takes out of the treasury.
    ///
    /// It cannot fail: the array has one entry per terrain and the loader
    /// refuses a table that leaves one out, so there is no missing row for a
    /// caller to report. Whether a road may be laid there at all is a separate
    /// question, and [`Terrain::is_walkable`] answers it without the tables.
    pub const fn road_cost(&self, t: Terrain) -> Coins {
        self.road_cost_per_terrain[t.index()]
    }

    /// The hash in hexadecimal, for error messages and replay headers.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Every inconsistency *between* tables, in one place.
    ///
    /// It lives in the core because that is where the definitions live and
    /// because it is also needed by the fixture in
    /// `sim-core/tests/common/mod.rs`, which does not go through `sim-data`.
    /// Adding a check here automatically makes it active on the fixture too: a
    /// number that has to stand in a relation with another one is a **check**,
    /// not a comment.
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
        self.check_the_house_agrees_with_the_levels(&mut out);
        self.check_the_levels(&mut out);
        self.check_the_levels_can_be_served(&mut out);
        self.check_food_capacity(&mut out);
        self.check_difficulty(&mut out);
        self.check_demographics(&mut out);
        out
    }

    /// The demographic rates have to describe a game that can be won.
    ///
    /// Two relations, and both are checks rather than comments for the same
    /// reason — a number that has to stand in a relation with another one is a
    /// check.
    ///
    /// **Births above deaths at the top.** If a city at maximum satisfaction,
    /// fully served, still shrinks, then no growth scenario is winnable and
    /// nothing in the game would say so: the population would simply drift
    /// down and the player would read it as their own fault. It is the kind of
    /// balancing mistake that surfaces only when M2's bot fails to complete
    /// scenario 1, months later.
    ///
    /// **The two thresholds have to be reachable.** A birth threshold above
    /// `satisfaction.max` means no city ever has a child, in silence — the same
    /// fault as [`Inconsistency::UnreachableThreshold`] and caught for the same
    /// reason.
    fn check_demographics(&self, out: &mut Vec<Inconsistency>) {
        let d = &self.rules.demographics;
        // Switched off is a configuration, not a city that shrinks: with every
        // rate at zero nothing is born and nobody dies, so "a perfect city
        // still empties out" has nothing to say about it.
        if !d.is_off() && d.births_per_thousand_per_month <= d.deaths_per_thousand_per_month {
            out.push(Inconsistency::UnsustainableDemographics {
                births: d.births_per_thousand_per_month,
                deaths: d.deaths_per_thousand_per_month,
            });
        }
        let max = self.rules.satisfaction.max;
        for (what, threshold) in [
            ("birth_threshold", d.birth_threshold),
            ("unserved_threshold", d.unserved_threshold),
        ] {
            if threshold > max {
                out.push(Inconsistency::UnreachableDemographicThreshold {
                    what,
                    threshold,
                    max,
                });
            }
        }
    }

    /// The house building and `rules.house_levels` describe the same house.
    ///
    /// Two of these three checks once existed to keep [`BuildingDef::is_house`]
    /// true while it was a heuristic. It is a declared role since phase 14.4, so
    /// they are re-read here for what they are still worth: one says a house
    /// exists at all — without one, `satisfaction::update` and `remove_house`
    /// silently do nothing — one that the levels are as many as declared, and
    /// one that the building's union covers every level's list, **or a level
    /// that introduces a new service is unreachable by construction**: nothing
    /// would ever move that accumulator off zero, so its threshold could never
    /// be met. That last reason is the real one, and it outlived the heuristic
    /// it was written for.
    fn check_the_house_agrees_with_the_levels(&self, out: &mut Vec<Inconsistency>) {
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
        let declared = sorted(house.required_services().iter().copied());
        if declared != union {
            out.push(Inconsistency::InconsistentRequirements {
                declared,
                in_levels: union,
            });
        }
    }

    /// The levels go up, and each one has a gap you can stand on.
    fn check_the_levels(&self, out: &mut Vec<Inconsistency>) {
        let max = self.rules.satisfaction.max;
        let mut previous: Option<u16> = None;
        for (level, def) in self.rules.all_levels() {
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
                out.push(Inconsistency::NoGap {
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
        for (level, def) in self.rules.all_levels() {
            for &service in &def.required_services {
                let best = self
                    .buildings
                    .iter()
                    .filter_map(BuildingDef::service)
                    .filter(|s| s.kind == service)
                    .filter_map(|s| s.capacity_per_level.iter().copied().max())
                    .max();
                match best {
                    None => out.push(Inconsistency::ServiceWithoutProvider { level, service }),
                    // Capacity is counted on the residents actually present, so
                    // this is not a blocker — the house is servable as
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
    /// **It lives exactly as long as the farm's own stock does.** It is the
    /// right rule while the farm
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
            let Some(service) = def.service() else {
                continue;
            };
            if service.kind != ServiceKind::Food {
                continue;
            }
            // Skipping the provider that grows nothing is what let a phantom
            // farm through: the coverage never consults the production, so such
            // a farm wins its houses on distance alone and then feeds none of
            // them, for ever — the first provider keeps a contested house, so
            // not even a real farm built afterwards can take them over. It
            // sustains nobody, and the loop below says so with
            // `sustainable: 0`.
            let available = def.production.as_ref().map_or(0, |p| {
                p.output_per_tick.to_millis().min(p.max_stock.to_millis())
            });
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
        "no building declares the house role, and with no house at all the \
         houses stop levelling up, stop eating and stop being taxed, in silence"
    )]
    NoHouse,

    #[error("the house declares {declared} levels, rules.house_levels has {in_table}")]
    LevelCountMismatch { declared: u8, in_table: usize },

    #[error(
        "the house requires {} but its levels require {}: the building's list \
         has to stay the union, or a level asking for a service missing from it \
         is unreachable — nothing ever moves that accumulator off zero, so its \
         threshold can never be met",
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
        // Unreachable: with no level below there is no `previous` capacity to be
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
         no gap between the two thresholds the city flickers at every review"
    )]
    NoGap {
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
        /// level, not a house level: the two share a type, not a table.
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

    #[error(
        "{births} births against {deaths} deaths per thousand per month: a city \
         at maximum satisfaction shrinks, so no growth scenario is winnable and \
         nothing in the game would say so"
    )]
    UnsustainableDemographics { births: u16, deaths: u16 },

    #[error(
        "demographics.{what} is {threshold}, beyond the satisfaction ceiling of \
         {max}: it is a condition no city can ever meet"
    )]
    UnreachableDemographicThreshold {
        /// The field's name in the table, so the report points at it.
        what: &'static str,
        threshold: u8,
        max: u8,
    },
}

/// The services, in a fixed order and without repeats, so two lists can be
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
