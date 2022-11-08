use std::sync::Arc;

use bevy::utils::HashMap;

use crate::{
    building::BuildingSnapshot,
    common::{
        configuration::{Configuration, SourcePalatabilityConfiguration},
        position::Position,
        EntityId,
    },
    palatability::manager::macros::apply_source,
};

use self::macros::palatability_range;

pub struct PalatabilityManager {
    configuration: Arc<Configuration>,
    total_populations: u64,
    unemployed_inhabitants: Vec<EntityId>,
    vacant_inhabitants: u64,
    vacant_work: u64,
    // This is the "concrete view" of all palatability sources
    // This mean we can have some problem on:
    // - building deletion
    // - temporary unavailability
    // - change value on upgrading / downgrading building values
    // - other (?)
    palatability_descriptors: HashMap<Position, PalatabilityDescriptor>,
}
impl PalatabilityManager {
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            total_populations: 0,
            unemployed_inhabitants: vec![],
            vacant_inhabitants: 0,
            vacant_work: 0,
            palatability_descriptors: Default::default(),
        }
    }

    pub(super) fn add_palatability_source(&mut self, source: &BuildingSnapshot) {
        let palatabilities_range = get_palatabilities_range(&self.configuration, source);

        if let Some(house_source) = palatabilities_range.house {
            apply_source!(self, house_source, house_value);
        }

        if let Some(office_source) = palatabilities_range.office {
            apply_source!(self, office_source, office_value);
        }
    }

    pub fn get_palatability(&self, building: &BuildingSnapshot) -> BuildingPalatability {
        let position = building.get_position();

        let value = self
            .palatability_descriptors
            .get(position)
            .map_or(0, |p| match building {
                BuildingSnapshot::House(_) => p.house_value,
                BuildingSnapshot::Office(_) => p.office_value,
                BuildingSnapshot::Street(_) => 0,
                BuildingSnapshot::Garden(_) => 0,
                BuildingSnapshot::BiomassPowerPlant(_) => 0,
            });

        BuildingPalatability { value }
    }

    /*
    pub fn get_house_palatability(&self, position: &Position) -> HousePalatability {
        let value: i32 = self
            .houses_sources
            .iter()
            .map(|hsp| hsp.calculate(position))
            .sum();
        HousePalatability { value }
    }

    pub fn get_office_palatability(&self, position: &Position) -> OfficePalatability {
        let value: i32 = self
            .office_sources
            .iter()
            .map(|hsp| hsp.calculate(position))
            .sum();
        OfficePalatability { value }
    }
    */

    pub(super) fn add_unemployed_inhabitants(&mut self, inhabitants: Vec<EntityId>) {
        self.unemployed_inhabitants.extend(inhabitants);
    }

    pub(super) fn increment_vacant_work(&mut self, delta: i32) {
        self.vacant_work = (self.vacant_work as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn increment_vacant_inhabitants(&mut self, delta: i32) {
        self.vacant_inhabitants = (self.vacant_inhabitants as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn consume_inhabitants_to_spawn_and_increment_populations(
        &mut self,
    ) -> Vec<InhabitantToSpawn> {
        // info!("vacant_inhabitants {}", self.vacant_inhabitants);

        let c: u8 = self.vacant_inhabitants.min(u64::from(u8::MAX)) as u8;
        self.vacant_inhabitants -= u64::from(c);
        self.total_populations += u64::from(c);

        let education_level = self.current_eduction_level();

        (0..c)
            .map(|_| InhabitantToSpawn { education_level })
            .collect()
    }

    pub(super) fn consume_workers_to_spawn(&mut self) -> Vec<EntityId> {
        if self.vacant_work == 0 {
            return vec![];
        }

        // TODO: put 1 into configuration
        let unemployed_to_consume = 1.min(self.unemployed_inhabitants.len());
        let range = 0..unemployed_to_consume;
        if range.is_empty() {
            return vec![];
        }

        self.unemployed_inhabitants.drain(range).collect()
    }

    pub fn total_populations(&self) -> u64 {
        self.total_populations
    }

    #[allow(dead_code)]
    pub fn unemployed_inhabitants(&self) -> Vec<EntityId> {
        self.unemployed_inhabitants.clone()
    }

    #[allow(dead_code)]
    pub fn vacant_work(&self) -> u64 {
        self.vacant_work
    }

    #[allow(dead_code)]
    pub fn vacant_inhabitants(&self) -> u64 {
        self.vacant_inhabitants
    }

    fn current_eduction_level(&self) -> EducationLevel {
        EducationLevel::None
    }
}

#[derive(Debug)]
struct PalatabilityDescriptor {
    house_value: i32,
    office_value: i32,
}

fn calculate_palatability_value(range: &PalatabilityRange, position: &Position) -> i32 {
    let distance = range.origin.distance(position);

    if distance < range.max_horizontal_distribution_distance {
        return range.origin_value;
    }

    if distance < range.max_linear_distribution_distance {
        let d = (distance - range.max_horizontal_distribution_distance) as i32;

        let value = range.origin_value - range.linear_factor * d;
        if range.origin_value > 0 {
            return value.max(0);
        } else {
            return value.min(0);
        }
    }

    0
}

#[derive(Debug)]
struct PalatabilitiesRange {
    house: Option<PalatabilityRange>,
    office: Option<PalatabilityRange>,
}

#[derive(Debug)]
struct PalatabilityRange {
    origin: Position,
    origin_value: i32,
    max_horizontal_distribution_distance: u32,
    max_linear_distribution_distance: u32,
    linear_factor: i32,
}

fn get_palatabilities_range(
    configuration: &Arc<Configuration>,
    building: &BuildingSnapshot,
) -> PalatabilitiesRange {
    match building {
        BuildingSnapshot::House(_) => palatability_range!(configuration, house, building),
        BuildingSnapshot::Office(_) => palatability_range!(configuration, office, building),
        BuildingSnapshot::Street(_) => palatability_range!(configuration, street, building),
        BuildingSnapshot::Garden(_) => palatability_range!(configuration, garden, building),
        BuildingSnapshot::BiomassPowerPlant(_) => {
            palatability_range!(configuration, biomass_power_plant, building)
        }
    }
}

fn get_palatability_wrapper(
    origin: Position,
) -> impl FnOnce(&SourcePalatabilityConfiguration) -> PalatabilityRange {
    move |for_house: &SourcePalatabilityConfiguration| PalatabilityRange {
        origin,
        origin_value: for_house.value,
        max_horizontal_distribution_distance: for_house.max_horizontal_distribution_distance,
        max_linear_distribution_distance: for_house.max_linear_distribution_distance,
        linear_factor: for_house.linear_factor,
    }
}

#[derive(Debug)]
pub struct BuildingPalatability {
    value: i32,
}
impl BuildingPalatability {
    #[inline]
    pub fn is_positive(&self) -> bool {
        // we consider 0 as positive PalatabilityManager
        !self.value.is_negative()
    }
}

pub struct InhabitantToSpawn {
    pub education_level: EducationLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EducationLevel {
    None,
    Low,
}

impl PartialOrd for EducationLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (EducationLevel::None, EducationLevel::None)
            | (EducationLevel::Low, EducationLevel::Low) => Some(std::cmp::Ordering::Equal),
            (_, EducationLevel::Low) => Some(std::cmp::Ordering::Less),
            (EducationLevel::Low, _) => Some(std::cmp::Ordering::Greater),
        }
    }
}

mod macros {

    macro_rules! palatability_range {
        ($configuration: tt, $name: ident, $building: ident) => {{
            let for_house = $configuration
                .buildings
                .$name
                .palatability_configuration
                .source_for_house
                .as_ref();
            let for_office = $configuration
                .buildings
                .$name
                .palatability_configuration
                .source_for_office
                .as_ref();

            PalatabilitiesRange {
                house: for_house.map(get_palatability_wrapper(*$building.get_position())),
                office: for_office.map(get_palatability_wrapper(*$building.get_position())),
            }
        }};
    }

    macro_rules! apply_source {
        ($self: ident, $palatability_range: ident, $name: ident) => {
            let max: i64 = $palatability_range
                .max_linear_distribution_distance
                .max($palatability_range.max_horizontal_distribution_distance)
                .into();
            (($palatability_range.origin.x - max)..($palatability_range.origin.x + max))
                .flat_map(|x| {
                    (($palatability_range.origin.y - max)..($palatability_range.origin.y + max))
                        .map(move |y| Position { x, y })
                })
                .for_each(|position| {
                    let delta = calculate_palatability_value(&$palatability_range, &position);

                    let entry = $self.palatability_descriptors.entry(position).or_insert(
                        PalatabilityDescriptor {
                            house_value: 0,
                            office_value: 0,
                        },
                    );

                    entry.$name += delta;
                });
        };
    }

    pub(super) use apply_source;
    pub(super) use palatability_range;
}
