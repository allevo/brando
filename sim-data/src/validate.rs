//! Validation of the raw tables.
//!
//! The report gathers **every** error, not just the first: whoever is fixing a
//! table wants to see all the problems in one go, not recompile six times. A
//! `?` on the first check would make the validation a sham.

use std::fmt;

use sim_core::{Coins, Level, Milli, ServiceKind, Terrain};

use crate::raw::{RawBuildingDef, RawDataSet, RawDemographics, RawMigration, RawSatisfaction};
use sim_core::data::{
    BuildingDef, BuildingRole, DataSet, DemographicsRules, DifficultyDef, HouseLevelDef,
    Inconsistency, MigrationRules, Production, Rules, SatisfactionRules, ServiceDef,
};

/// A single problem, with the logical path of the field that causes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// E.g. `buildings[1].service.range_per_level`.
    pub path: String,
    pub kind: ValidationErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationErrorKind {
    #[error("empty value")]
    Empty,

    #[error("duplicate id, already used by {previous}")]
    DuplicateId { previous: String },

    #[error("expected at least {min}, found {found}")]
    TooSmall { min: i64, found: i64 },

    #[error("negative value: {found}")]
    Negative { found: i64 },

    #[error("expected one value per level: levels = {levels}, values = {found}")]
    WrongLengthPerLevel { levels: u8, found: usize },

    #[error("unknown service: {name:?} (known: {known})")]
    UnknownService { name: String, known: String },

    #[error("{found} is beyond the maximum of {max}")]
    BeyondMax { found: u16, max: u16 },

    #[error(
        "the mood bands have to be strictly ascending and above zero: \
         {found} does not come after {previous}"
    )]
    MoodBandsOutOfOrder { previous: u16, found: u16 },

    #[error("a producer must have max_stock > 0")]
    ProducerWithoutStock,

    #[error("max_stock on a building that produces nothing")]
    StockWithoutOutput,

    #[error("unknown role: {name:?} (known: {known})")]
    UnknownRole { name: String, known: String },

    #[error("a building with the provider role supplies no service")]
    ProviderWithoutService,

    #[error("a house declares a service: a house needs services, it does not supply them")]
    ServiceOnAHouse,

    #[error(
        "required_services on a provider: only a house requires services, so \
         this list would be read by nobody"
    )]
    RequirementsOnAProvider,

    /// A relation between two tables that does not hold.
    ///
    /// One variant for all of them, and the message comes from `sim-core`:
    /// [`Inconsistency`] is where the checks live, because the fixture in
    /// `sim-core/tests/common/mod.rs` does not come through here and has to be
    /// protected by the same rules. Adding a check there is then one place, not
    /// two, and what `sim-data` adds is only the path of the RON field to
    /// blame.
    #[error(transparent)]
    Inconsistent(Inconsistency),

    #[error("terrain missing from the table: {terrain:?}")]
    MissingTerrain { terrain: Terrain },

    #[error("terrain already declared")]
    DuplicateTerrain,

    #[error("no road can be laid on {terrain:?}, so its road_cost has to be 0, found {found}")]
    UnreachableRoadCost { terrain: Terrain, found: i32 },

    #[error("too many buildings in the table: the maximum is {max}")]
    TooManyBuildings { max: usize },

    #[error("too many difficulty profiles in the table: the maximum is {max}")]
    TooManyProfiles { max: usize },

    #[error(
        "satisfaction_weight + free_places_weight is {found}, not 1000: attractiveness would stop being a share expressed in thousandths"
    )]
    MigrationWeightsNotAThousand { found: u32 },

    // --- map faults (phase 16) ---
    #[error("invalid map size {width}x{height}: each side must be 1..={max}")]
    InvalidMapSize { width: u16, height: u16, max: u16 },

    #[error("expected {expected} rows of {block}, found {found}")]
    WrongMapRowCount {
        block: &'static str,
        expected: u16,
        found: usize,
    },

    #[error("row {row} of {block} has {found} characters, expected {expected}")]
    WrongMapRowLength {
        block: &'static str,
        row: usize,
        expected: u16,
        found: usize,
    },

    #[error(
        "unclaimed character {found:?} in {block} at row {row}, column {col}: known are {known}"
    )]
    UnclaimedMapCharacter {
        block: &'static str,
        row: usize,
        col: usize,
        found: char,
        known: &'static str,
    },

    #[error("ground height {found} at row {row}, column {col} is beyond the maximum of {max}")]
    GroundHeightBeyondMax {
        row: usize,
        col: usize,
        found: u8,
        max: u8,
    },

    #[error(
        "the walkable ground does not form one connected region: {sizes:?} — until bridges \
         exist, every walkable tile has to be able to reach every other"
    )]
    SeveredMap { sizes: Vec<usize> },
}

/// The set of problems found in one validation pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

impl ValidationReport {
    pub(crate) fn push(&mut self, path: impl Into<String>, kind: ValidationErrorKind) {
        self.errors.push(ValidationError {
            path: path.into(),
            kind,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} validation error{} in the tables:",
            self.errors.len(),
            if self.errors.len() == 1 { "" } else { "s" }
        )?;
        for e in &self.errors {
            writeln!(f, "  - {}: {}", e.path, e.kind)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationReport {}

/// Validates and builds in a single pass.
///
/// The plan called for `validate(&RawDataSet) -> Result<(), ValidationReport>`
/// with the conversion done separately: doing it in two passes would force the
/// same invariants to be rechecked during the conversion (or trusted with a few
/// `expect`s). A single pass, returning the dataset it built, is the same
/// guarantee without the duplication.
pub fn validate(raw: &RawDataSet) -> Result<DataSet, ValidationReport> {
    let mut rep = ValidationReport::default();

    let rules = validate_rules(raw, &mut rep);
    let terrain = validate_terrain(raw, &mut rep);
    let buildings = validate_buildings(raw, &mut rep);
    let difficulties = validate_difficulty(raw, &mut rep);

    if !rep.is_empty() {
        return Err(rep);
    }

    // The checks **across** tables come afterwards, and only if the individual
    // tables are sound: they cross `rules`, `buildings` and `difficulty`, and
    // on an already broken table they would produce noise instead of
    // information. They live in `sim-core` so that `sim-core`'s own fixture,
    // which never comes through here, is protected by the same rules.
    let data = DataSet::new(rules, terrain, buildings, difficulties);
    for i in data.inconsistencies() {
        rep.push(path_of(&i), ValidationErrorKind::Inconsistent(i));
    }
    if !rep.is_empty() {
        return Err(rep);
    }

    Ok(data)
}

/// The RON field to blame for an inconsistency.
///
/// It is the one thing `sim-data` adds to a check that lives in `sim-core`: the
/// core knows the relation, this crate knows what the file it came from is
/// shaped like.
fn path_of(i: &Inconsistency) -> String {
    /// The path of a level. It is the **index** the path names, not the level:
    /// what the reader has to go and edit is an entry of a RON list.
    fn level_path(level: Level, field: &str) -> String {
        format!("rules.house_levels[{}].{field}", level.as_usize())
    }

    match *i {
        Inconsistency::NoHouse => "buildings".to_string(),
        Inconsistency::LevelCountMismatch { .. }
        | Inconsistency::InconsistentRequirements { .. } => "rules.house_levels".to_string(),
        Inconsistency::CapacityNotIncreasing { level, .. }
        | Inconsistency::CapacityBeyondEveryProvider { level, .. } => {
            level_path(level, "max_residents")
        }
        Inconsistency::NoGap { level, .. } => level_path(level, "decay_threshold"),
        Inconsistency::UnreachableThreshold { level, .. } => {
            level_path(level, "level_up_threshold")
        }
        Inconsistency::ServiceWithoutProvider { level, .. } => {
            level_path(level, "required_services")
        }
        Inconsistency::CapacityBeyondOutput { building, .. } => {
            format!("buildings[{building}].service.capacity_per_level")
        }
        Inconsistency::StartingResidentsBeyondCapacity { difficulty, .. } => {
            format!("profiles[{difficulty}].starting_residents_per_house")
        }
        Inconsistency::UnsustainableDemographics { .. } => {
            "rules.demographics.births_per_thousand_per_month".to_string()
        }
        Inconsistency::UnreachableDemographicThreshold { what, .. } => {
            format!("rules.demographics.{what}")
        }
        Inconsistency::UnreachableMigrationThreshold { what, .. } => {
            format!("rules.migration.{what}")
        }
        Inconsistency::NoGapBetweenEmigrationAndBirths { .. } => {
            "rules.migration.emigration_threshold".to_string()
        }
    }
}

fn validate_rules(raw: &RawDataSet, rep: &mut ValidationReport) -> Rules {
    let r = &raw.rules;

    if r.food_per_resident < 0 {
        rep.push(
            "rules.food_per_resident",
            ValidationErrorKind::Negative {
                found: i64::from(r.food_per_resident),
            },
        );
    }
    if r.flatten_cost_per_step < 0 {
        rep.push(
            "rules.flatten_cost_per_step",
            ValidationErrorKind::Negative {
                found: i64::from(r.flatten_cost_per_step),
            },
        );
    }

    Rules {
        starting_treasury: Coins::new(r.starting_treasury),
        house_levels: validate_house_levels(raw, rep),
        max_build_slope: r.max_build_slope,
        flatten_cost_per_step: Coins::new(r.flatten_cost_per_step),
        food_per_resident: Milli::from_millis(r.food_per_resident),
        satisfaction: validate_satisfaction(&r.satisfaction, rep),
        demographics: validate_demographics(&r.demographics, rep),
        migration: validate_migration(&r.migration, rep),
    }
}

/// Field checks for the migration rates and weights (phase 15).
///
/// Only what can be judged from **one field of one table**: the relations that
/// cross `rules.migration` with `rules.demographics` and `rules.satisfaction`
/// live in `DataSet::inconsistencies`, so `sim-core`'s hand-built fixture is
/// protected by them too.
fn validate_migration(m: &RawMigration, rep: &mut ValidationReport) -> MigrationRules {
    const PATH: &str = "rules.migration";

    let rules = MigrationRules {
        satisfaction_weight: m.satisfaction_weight,
        free_places_weight: m.free_places_weight,
        founding_immigration_per_thousand_per_month: m.founding_immigration_per_thousand_per_month,
        founding_population_threshold: m.founding_population_threshold,
        immigration_per_thousand_per_month: m.immigration_per_thousand_per_month,
        emigration_per_thousand_per_month_unhappy: m.emigration_per_thousand_per_month_unhappy,
        emigration_threshold: m.emigration_threshold,
        jitter_per_thousand: m.jitter_per_thousand,
    };

    // Attractiveness divides its weighted sum by 1000 exactly once: if the two
    // weights do not add up to it, the result stops being a share expressed in
    // thousandths and every number derived from it silently means something
    // else than what it says.
    let sum = u32::from(m.satisfaction_weight) + u32::from(m.free_places_weight);
    if sum != 1_000 {
        rep.push(
            PATH,
            ValidationErrorKind::MigrationWeightsNotAThousand { found: sum },
        );
    }

    if m.jitter_per_thousand >= 1_000 {
        rep.push(
            format!("{PATH}.jitter_per_thousand"),
            ValidationErrorKind::BeyondMax {
                found: m.jitter_per_thousand,
                max: 999,
            },
        );
    }

    // Switched off — every rate at zero — is a configuration
    // (`MigrationRules::is_off`), for the same reason as the demographics: a
    // table describing no migration is not describing one badly.
    if rules.is_off() {
        return rules;
    }

    // A rate of zero on its own is a table somebody half filled in, the same
    // fault `validate_demographics` refuses for its own three rates.
    for (field, value) in [
        (
            "founding_immigration_per_thousand_per_month",
            m.founding_immigration_per_thousand_per_month,
        ),
        (
            "immigration_per_thousand_per_month",
            m.immigration_per_thousand_per_month,
        ),
        (
            "emigration_per_thousand_per_month_unhappy",
            m.emigration_per_thousand_per_month_unhappy,
        ),
    ] {
        if value < 1 {
            rep.push(
                format!("{PATH}.{field}"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(value),
                },
            );
        }
    }

    // At zero the founding regime would never apply — a population is never
    // below zero — so a city founded with every house empty would be rated on a
    // population of nobody, times any rate, for ever. That is the bootstrap trap
    // the two regimes exist to avoid, and one written as a table value rather
    // than as a bug in the code, which is why it is refused here.
    if m.founding_population_threshold < 1 {
        rep.push(
            format!("{PATH}.founding_population_threshold"),
            ValidationErrorKind::TooSmall {
                min: 1,
                found: i64::from(m.founding_population_threshold),
            },
        );
    }

    rules
}

/// Field checks for the demographic rates.
///
/// Only what can be judged from **one field of one table**: the two relations
/// that cross tables — births above deaths, and the thresholds against
/// `satisfaction.max` — live in `DataSet::inconsistencies`, so `sim-core`'s
/// hand-built fixture is protected by them too.
fn validate_demographics(d: &RawDemographics, rep: &mut ValidationReport) -> DemographicsRules {
    const PATH: &str = "rules.demographics";

    let rules = DemographicsRules {
        births_per_thousand_per_month: d.births_per_thousand_per_month,
        deaths_per_thousand_per_month: d.deaths_per_thousand_per_month,
        deaths_per_thousand_per_month_when_unserved: d.deaths_per_thousand_per_month_when_unserved,
        unserved_threshold: d.unserved_threshold,
        birth_threshold: d.birth_threshold,
        jitter_per_thousand: d.jitter_per_thousand,
    };

    // A jitter of a thousand or more lets `1000 + jitter` reach zero and flip
    // the sign of a rate: a tick with negative births. Checked even when the
    // demographics are off, because it is a property of the field and not of
    // the game the table describes.
    if d.jitter_per_thousand >= 1_000 {
        rep.push(
            format!("{PATH}.jitter_per_thousand"),
            ValidationErrorKind::BeyondMax {
                found: d.jitter_per_thousand,
                max: 999,
            },
        );
    }

    // Every rate at zero means the demographics are **switched off**, which is
    // a configuration and not a mistake: `coverage_equivalence` needs a city
    // whose population cannot move, and `bench --zero-demographics` needs the
    // same to measure `H`. The rest of these checks are about a game, and a
    // table that describes no demographics at all is not describing one badly.
    if rules.is_off() {
        return rules;
    }

    // A rate of zero on its own is a table somebody half filled in. Zero for
    // the unserved in particular would make demolishing the farm cost nothing,
    // which is the whole mechanic of the `hunger` scenario.
    for (field, value) in [
        (
            "births_per_thousand_per_month",
            d.births_per_thousand_per_month,
        ),
        (
            "deaths_per_thousand_per_month",
            d.deaths_per_thousand_per_month,
        ),
        (
            "deaths_per_thousand_per_month_when_unserved",
            d.deaths_per_thousand_per_month_when_unserved,
        ),
    ] {
        if value < 1 {
            rep.push(
                format!("{PATH}.{field}"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(value),
                },
            );
        }
    }

    // Going without has to be worse than being served, or the raised rate is a
    // number that decides nothing — and a number in a table that decides
    // nothing is the sort of thing somebody later balances.
    if d.deaths_per_thousand_per_month_when_unserved <= d.deaths_per_thousand_per_month {
        rep.push(
            format!("{PATH}.deaths_per_thousand_per_month_when_unserved"),
            ValidationErrorKind::TooSmall {
                min: i64::from(d.deaths_per_thousand_per_month) + 1,
                found: i64::from(d.deaths_per_thousand_per_month_when_unserved),
            },
        );
    }

    rules
}

/// The house levels (phase 13), checked field by field.
///
/// **Shape only.** Everything relational — the capacity that has to grow, the
/// gap, a threshold beyond the ceiling, a service nobody provides —
/// is [`Inconsistency`], because those are the checks that also have to protect
/// `sim-core`'s fixture.
///
/// The one relation that stays here is a level requiring **nothing**: `all()`
/// over an empty list is true, so such a level would be reached for free, and
/// the reason it is not an `Inconsistency` is that it is a property of one
/// field of one table.
fn validate_house_levels(raw: &RawDataSet, rep: &mut ValidationReport) -> Vec<HouseLevelDef> {
    let levels = &raw.rules.house_levels;
    if levels.is_empty() {
        rep.push("rules.house_levels", ValidationErrorKind::Empty);
    }

    let mut out = Vec::with_capacity(levels.len());
    for (i, l) in levels.iter().enumerate() {
        let path = format!("rules.house_levels[{i}]");

        if l.required_services.is_empty() {
            rep.push(
                format!("{path}.required_services"),
                ValidationErrorKind::Empty,
            );
        }
        if l.taxable_per_resident < 0 {
            rep.push(
                format!("{path}.taxable_per_resident"),
                ValidationErrorKind::Negative {
                    found: i64::from(l.taxable_per_resident),
                },
            );
        }

        out.push(HouseLevelDef {
            max_residents: l.max_residents,
            required_services: services(&l.required_services, &path, rep),
            level_up_threshold: l.level_up_threshold,
            decay_threshold: l.decay_threshold,
            taxable_per_resident: Milli::from_millis(l.taxable_per_resident),
        });
    }
    out
}

/// The satisfaction curve (phase 12).
///
/// The cross-table check against the level thresholds arrives with phase 13,
/// which is what introduces them. What is checked here is the shape: the curve
/// has to be able to move, and the bands have to partition `0..=max` in order.
fn validate_satisfaction(s: &RawSatisfaction, rep: &mut ValidationReport) -> SatisfactionRules {
    const PATH: &str = "rules.satisfaction";

    for (field, value) in [
        ("max", s.max),
        ("step_up", s.step_up),
        ("step_down", s.step_down),
    ] {
        if value < 1 {
            rep.push(
                format!("{PATH}.{field}"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(value),
                },
            );
        }
    }

    // A step larger than the maximum is not wrong arithmetically — it
    // saturates — but it says the table means something it does not: a house
    // that reaches the maximum in a single tick has no curve at all.
    for (field, value) in [("step_up", s.step_up), ("step_down", s.step_down)] {
        if value > s.max {
            rep.push(
                format!("{PATH}.{field}"),
                ValidationErrorKind::BeyondMax {
                    found: u16::from(value),
                    max: u16::from(s.max),
                },
            );
        }
    }

    // Strictly ascending and starting above zero. The first half keeps the
    // bands from overlapping; the second is what makes a satisfaction of zero
    // always `Mood::Awful`, which the renderer's contract for a newly-built
    // house depends on.
    let mut previous = 0u8;
    for (i, &t) in s.mood_thresholds.iter().enumerate() {
        if t <= previous {
            rep.push(
                format!("{PATH}.mood_thresholds[{i}]"),
                ValidationErrorKind::MoodBandsOutOfOrder {
                    previous: u16::from(previous),
                    found: u16::from(t),
                },
            );
        }
        if t > s.max {
            rep.push(
                format!("{PATH}.mood_thresholds[{i}]"),
                ValidationErrorKind::BeyondMax {
                    found: u16::from(t),
                    max: u16::from(s.max),
                },
            );
        }
        previous = t;
    }

    SatisfactionRules {
        max: s.max,
        step_up: s.step_up,
        step_down: s.step_down,
        mood_thresholds: s.mood_thresholds,
    }
}

fn validate_terrain(raw: &RawDataSet, rep: &mut ValidationReport) -> [Coins; Terrain::COUNT] {
    let mut out: [Option<Coins>; Terrain::COUNT] = [None; Terrain::COUNT];

    for (i, t) in raw.terrain.terrains.iter().enumerate() {
        let path = format!("terrains[{i}]");
        if t.road_cost < 0 {
            rep.push(
                format!("{path}.road_cost"),
                ValidationErrorKind::Negative {
                    found: i64::from(t.road_cost),
                },
            );
        }
        // A road cannot be laid on ground that is not walkable, so nothing ever
        // reads that ground's cost. Left unchecked, a designer could raise it to
        // make water expensive to cross and get no feedback at all, because the
        // number is unreachable by construction. Refusing it is the difference
        // between a rule and a comment explaining why a number is ignored.
        if !t.terrain.is_walkable() && t.road_cost != 0 {
            rep.push(
                format!("{path}.road_cost"),
                ValidationErrorKind::UnreachableRoadCost {
                    terrain: t.terrain,
                    found: t.road_cost,
                },
            );
        }
        let slot = &mut out[t.terrain.index()];
        if slot.is_some() {
            rep.push(
                format!("{path}.terrain"),
                ValidationErrorKind::DuplicateTerrain,
            );
        } else {
            *slot = Some(Coins::new(t.road_cost));
        }
    }

    // The table has to cover every variant: a missing terrain would become a
    // cost nobody declared on a road somebody can lay.
    for t in Terrain::ALL {
        if out[t.index()].is_none() {
            rep.push(
                "terrains",
                ValidationErrorKind::MissingTerrain { terrain: t },
            );
        }
    }

    // The zero filling a slot the table left out never reaches the simulation:
    // the report is not empty in that case, and `validate` returns it as an
    // error before a `DataSet` is ever built.
    out.map(|c| c.unwrap_or(Coins::ZERO))
}

fn validate_buildings(raw: &RawDataSet, rep: &mut ValidationReport) -> Vec<BuildingDef> {
    let defs = &raw.buildings.buildings;

    if defs.len() > usize::from(u16::MAX) {
        rep.push(
            "buildings",
            ValidationErrorKind::TooManyBuildings {
                max: usize::from(u16::MAX),
            },
        );
    }

    let mut out = Vec::with_capacity(defs.len());
    for (i, b) in defs.iter().enumerate() {
        let path = format!("buildings[{i}]");

        if b.id.trim().is_empty() {
            rep.push(format!("{path}.id"), ValidationErrorKind::Empty);
        } else if let Some(prev) = defs.iter().take(i).position(|o| o.id == b.id) {
            rep.push(
                format!("{path}.id"),
                ValidationErrorKind::DuplicateId {
                    previous: format!("buildings[{prev}].id"),
                },
            );
        }

        if b.levels < 1 {
            rep.push(
                format!("{path}.levels"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(b.levels),
                },
            );
        }
        if b.cost < 0 {
            rep.push(
                format!("{path}.cost"),
                ValidationErrorKind::Negative {
                    found: i64::from(b.cost),
                },
            );
        }
        if b.size.0 < 1 {
            rep.push(
                format!("{path}.size.0"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(b.size.0),
                },
            );
        }
        if b.size.1 < 1 {
            rep.push(
                format!("{path}.size.1"),
                ValidationErrorKind::TooSmall {
                    min: 1,
                    found: i64::from(b.size.1),
                },
            );
        }

        out.push(BuildingDef {
            id: b.id.clone(),
            size: b.size,
            cost: Coins::new(b.cost),
            levels: b.levels,
            role: validate_role(b, &path, rep),
            production: validate_production(b, &path, rep),
        });
    }

    out
}

/// The role names the RON tables use.
///
/// Part of the contract with the data files, like [`ServiceKind::as_id`]:
/// renaming one invalidates every table. The literals in [`validate_role`]'s
/// match are these same two, and `every_known_role_is_accepted` is what keeps
/// the two lists from drifting apart.
const ROLES: [&str; 2] = ["house", "provider"];

/// Turns the flat row into the sum, reporting whatever contradicts it.
///
/// **The fallback role is never observed.** [`validate`] returns the report
/// before the dataset is built whenever anything was pushed, and every path
/// that reaches the fallback pushes. It exists so that one unreadable row does
/// not remove an entry from `buildings` and shift what `buildings[i]` means for
/// every error reported after it.
fn validate_role(b: &RawBuildingDef, path: &str, rep: &mut ValidationReport) -> BuildingRole {
    match b.role.as_str() {
        "house" => {
            if b.service.is_some() {
                rep.push(
                    format!("{path}.service"),
                    ValidationErrorKind::ServiceOnAHouse,
                );
            }
            BuildingRole::House {
                required_services: services(&b.required_services, path, rep),
            }
        }
        "provider" => {
            if !b.required_services.is_empty() {
                rep.push(
                    format!("{path}.required_services"),
                    ValidationErrorKind::RequirementsOnAProvider,
                );
            }
            match validate_service(b, path, rep) {
                Some(service) => BuildingRole::Provider { service },
                None => {
                    // An unknown service kind has already been reported by
                    // `validate_service`; saying so twice would be noise. What
                    // it cannot report is the row that declares no service at
                    // all, because it has no service block to blame.
                    if b.service.is_none() {
                        rep.push(
                            format!("{path}.service"),
                            ValidationErrorKind::ProviderWithoutService,
                        );
                    }
                    fallback_role()
                }
            }
        }
        other => {
            rep.push(
                format!("{path}.role"),
                ValidationErrorKind::UnknownRole {
                    name: other.to_string(),
                    known: ROLES.join(", "),
                },
            );
            fallback_role()
        }
    }
}

fn fallback_role() -> BuildingRole {
    BuildingRole::House {
        required_services: Vec::new(),
    }
}

fn validate_difficulty(raw: &RawDataSet, rep: &mut ValidationReport) -> Vec<DifficultyDef> {
    let profiles = &raw.difficulty.profiles;

    if profiles.is_empty() {
        // With no profile there is no game to start: `World::new` wants an id
        // and there would be none to give it.
        rep.push("profiles", ValidationErrorKind::Empty);
    }
    // `DifficultyId` is a `u8`: profile 256 would be unreachable, and silently.
    if profiles.len() > usize::from(u8::MAX) + 1 {
        rep.push(
            "profiles",
            ValidationErrorKind::TooManyProfiles {
                max: usize::from(u8::MAX) + 1,
            },
        );
    }

    let mut out = Vec::with_capacity(profiles.len());
    for (i, p) in profiles.iter().enumerate() {
        let path = format!("profiles[{i}]");

        if p.id.trim().is_empty() {
            rep.push(format!("{path}.id"), ValidationErrorKind::Empty);
        } else if let Some(prev) = profiles.iter().take(i).position(|o| o.id == p.id) {
            rep.push(
                format!("{path}.id"),
                ValidationErrorKind::DuplicateId {
                    previous: format!("profiles[{prev}].id"),
                },
            );
        }

        out.push(DifficultyDef {
            id: p.id.clone(),
            starting_residents_per_house: p.starting_residents_per_house,
        });
    }

    out
}

fn validate_service(
    b: &RawBuildingDef,
    path: &str,
    rep: &mut ValidationReport,
) -> Option<ServiceDef> {
    let s = b.service.as_ref()?;

    let kind = match ServiceKind::from_id(&s.kind) {
        Some(k) => Some(k),
        None => {
            rep.push(
                format!("{path}.service.kind"),
                ValidationErrorKind::UnknownService {
                    name: s.kind.clone(),
                    known: known_services(),
                },
            );
            None
        }
    };

    // One value per level: it is the likeliest mistake once M1 makes the levels
    // more than one.
    for (field, values) in [
        ("range_per_level", &s.range_per_level),
        ("capacity_per_level", &s.capacity_per_level),
    ] {
        if values.len() != usize::from(b.levels) {
            rep.push(
                format!("{path}.service.{field}"),
                ValidationErrorKind::WrongLengthPerLevel {
                    levels: b.levels,
                    found: values.len(),
                },
            );
        }
    }

    kind.map(|kind| ServiceDef {
        kind,
        range_per_level: s.range_per_level.clone(),
        capacity_per_level: s.capacity_per_level.clone(),
    })
}

/// Resolves a list of service ids, reporting every unknown one.
///
/// Shared by the buildings' `required_services` and the house levels': the path
/// is passed in, so both report `<owner>.required_services[j]`.
fn services(names: &[String], path: &str, rep: &mut ValidationReport) -> Vec<ServiceKind> {
    let mut out = Vec::with_capacity(names.len());
    for (j, name) in names.iter().enumerate() {
        match ServiceKind::from_id(name) {
            Some(k) => out.push(k),
            None => rep.push(
                format!("{path}.required_services[{j}]"),
                ValidationErrorKind::UnknownService {
                    name: name.clone(),
                    known: known_services(),
                },
            ),
        }
    }
    out
}

/// Builds the production block, or reports why it cannot.
///
/// The two errors it can report are the two halves of a producer that does not
/// hold together: an output with no granary to put it in, and a granary with
/// nothing to fill it. They stay validation errors — they are properties of one
/// row of one table — and building [`Production`] is what happens when neither
/// fires. Past this point the broken combination has no shape to be in, which is
/// what it means for the core to make the state unrepresentable rather than
/// merely checked.
fn validate_production(
    b: &RawBuildingDef,
    path: &str,
    rep: &mut ValidationReport,
) -> Option<Production> {
    match (b.output_per_tick, b.max_stock) {
        (Some(p), stock) => {
            if p < 0 {
                rep.push(
                    format!("{path}.output_per_tick"),
                    ValidationErrorKind::Negative {
                        found: i64::from(p),
                    },
                );
            }
            match stock {
                Some(g) if g > 0 => Some(Production {
                    output_per_tick: Milli::from_millis(p),
                    max_stock: Milli::from_millis(g),
                }),
                _ => {
                    rep.push(
                        format!("{path}.max_stock"),
                        ValidationErrorKind::ProducerWithoutStock,
                    );
                    None
                }
            }
        }
        (None, Some(_)) => {
            rep.push(
                format!("{path}.max_stock"),
                ValidationErrorKind::StockWithoutOutput,
            );
            None
        }
        (None, None) => None,
    }
}

fn known_services() -> String {
    ServiceKind::ALL
        .iter()
        .map(|k| k.as_id())
        .collect::<Vec<_>>()
        .join(", ")
}
