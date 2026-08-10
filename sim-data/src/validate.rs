//! Validation of the raw tables.
//!
//! The report gathers **every** error, not just the first: whoever is fixing a
//! table wants to see all the problems in one go, not recompile six times. A
//! `?` on the first check would make the validation a sham.

use std::collections::BTreeMap;
use std::fmt;

use sim_core::{Coins, Milli, ServiceKind, Terrain};

use crate::raw::{RawBuildingDef, RawDataSet, RawSatisfaction};
use sim_core::data::{
    BuildingDef, DataSet, DifficultyDef, Rules, SatisfactionRules, ServiceDef, TerrainDef,
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

    #[error(
        "capacity of {capacity} residents, but the output sustains {sustainable}: \
         the houses in excess would stay assigned to a provider that does not feed them"
    )]
    CapacityBeyondOutput { capacity: u16, sustainable: u16 },

    #[error(
        "a house born with {starting} residents, but at level 1 it holds {max}: \
         the rest of the game cannot represent a house beyond its own capacity"
    )]
    StartingResidentsBeyondCapacity { starting: u16, max: u16 },

    #[error("terrain missing from the table: {terrain:?}")]
    MissingTerrain { terrain: Terrain },

    #[error("terrain already declared")]
    DuplicateTerrain,

    #[error("too many buildings in the table: the maximum is {max}")]
    TooManyBuildings { max: usize },

    #[error("too many difficulty profiles in the table: the maximum is {max}")]
    TooManyProfiles { max: usize },
}

/// The set of problems found in one validation pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

impl ValidationReport {
    fn push(&mut self, path: impl Into<String>, kind: ValidationErrorKind) {
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
    // tables are sound: the consistency between a food provider's capacity and
    // what its output sustains crosses `rules` and `buildings`, and on an
    // already broken table it would produce noise instead of information. The
    // same goes for a house born beyond its own capacity, which crosses `rules`
    // and `difficulty`.
    let data = DataSet::new(rules, terrain, buildings, difficulties);
    for v in data.unsustainable_food_capacity() {
        rep.push(
            format!("buildings[{}].service.capacity_per_level", v.building),
            ValidationErrorKind::CapacityBeyondOutput {
                capacity: v.capacity,
                sustainable: v.sustainable,
            },
        );
    }
    for v in data.difficulty_beyond_house_capacity() {
        rep.push(
            format!("profiles[{}].starting_residents_per_house", v.profile),
            ValidationErrorKind::StartingResidentsBeyondCapacity {
                starting: v.starting,
                max: v.max,
            },
        );
    }
    if !rep.is_empty() {
        return Err(rep);
    }

    Ok(data)
}

fn validate_rules(raw: &RawDataSet, rep: &mut ValidationReport) -> Rules {
    let r = &raw.rules;

    if r.ticks_per_month < 1 {
        rep.push(
            "rules.ticks_per_month",
            ValidationErrorKind::TooSmall {
                min: 1,
                found: i64::from(r.ticks_per_month),
            },
        );
    }
    if r.months_per_year < 1 {
        rep.push(
            "rules.months_per_year",
            ValidationErrorKind::TooSmall {
                min: 1,
                found: i64::from(r.months_per_year),
            },
        );
    }
    if r.ticks_per_month.checked_mul(r.months_per_year).is_none() {
        rep.push(
            "rules.months_per_year",
            ValidationErrorKind::TooSmall { min: 1, found: 0 },
        );
    }
    if r.residents_per_house_level.is_empty() {
        rep.push(
            "rules.residents_per_house_level",
            ValidationErrorKind::Empty,
        );
    }
    if r.food_per_resident < 0 {
        rep.push(
            "rules.food_per_resident",
            ValidationErrorKind::Negative {
                found: i64::from(r.food_per_resident),
            },
        );
    }

    Rules {
        ticks_per_month: r.ticks_per_month,
        months_per_year: r.months_per_year,
        starting_treasury: Coins::new(r.starting_treasury),
        residents_per_house_level: r.residents_per_house_level.clone(),
        food_per_resident: Milli::from_millis(r.food_per_resident),
        satisfaction: validate_satisfaction(&r.satisfaction, rep),
    }
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
    // always `Mood::Desperate`, which the renderer's contract for a newly-built
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

fn validate_terrain(raw: &RawDataSet, rep: &mut ValidationReport) -> BTreeMap<Terrain, TerrainDef> {
    let mut out = BTreeMap::new();

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
        let def = TerrainDef {
            buildable: t.buildable,
            walkable: t.walkable,
            road_cost: Coins::new(t.road_cost),
        };
        if out.insert(t.terrain, def).is_some() {
            rep.push(
                format!("{path}.terrain"),
                ValidationErrorKind::DuplicateTerrain,
            );
        }
    }

    // The table has to cover every variant: a missing terrain would become an
    // `unwrap` in a hot path.
    for t in Terrain::ALL {
        if !out.contains_key(&t) {
            rep.push(
                "terrains",
                ValidationErrorKind::MissingTerrain { terrain: t },
            );
        }
    }

    out
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

        let service = validate_service(b, &path, rep);
        let required_services = validate_required_services(b, &path, rep);
        validate_output(b, &path, rep);

        out.push(BuildingDef {
            id: b.id.clone(),
            size: b.size,
            cost: Coins::new(b.cost),
            levels: b.levels,
            service,
            required_services,
            output_per_tick: b.output_per_tick.map(Milli::from_millis),
            max_stock: b.max_stock.map(Milli::from_millis),
        });
    }

    out
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

fn validate_required_services(
    b: &RawBuildingDef,
    path: &str,
    rep: &mut ValidationReport,
) -> Vec<ServiceKind> {
    let mut out = Vec::with_capacity(b.required_services.len());
    for (j, name) in b.required_services.iter().enumerate() {
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

fn validate_output(b: &RawBuildingDef, path: &str, rep: &mut ValidationReport) {
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
            if stock.is_none_or(|g| g <= 0) {
                rep.push(
                    format!("{path}.max_stock"),
                    ValidationErrorKind::ProducerWithoutStock,
                );
            }
        }
        (None, Some(_)) => rep.push(
            format!("{path}.max_stock"),
            ValidationErrorKind::StockWithoutOutput,
        ),
        (None, None) => {}
    }
}

fn known_services() -> String {
    ServiceKind::ALL
        .iter()
        .map(|k| k.as_id())
        .collect::<Vec<_>>()
        .join(", ")
}
