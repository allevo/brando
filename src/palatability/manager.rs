use tracing::info;

use crate::{
    building::{Building, Garden, House, Office, Street},
    common::position::Position,
};

#[derive(Default)]
pub struct PalatabilityManager {
    total_populations: u64,
    unemployed_inhabitants: u64,
    vacant_inhabitants: u64,
    vacant_work: u64,
    // TODO: change approach to store the rendered values
    // We can consider also an async update:
    // - collects all the sources into a dedicated collection
    // - process little by little the palatability adding into a concrete view
    houses_sources: Vec<HouseSourcePalatabilityDescriptor>,
    office_sources: Vec<OfficeSourcePalatabilityDescriptor>,
}
impl PalatabilityManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn add_house_source(&mut self, source: &impl ToHouseSourcePalatabilityDescriptor) {
        let source = match source.to_house_source_palatability() {
            None => return,
            Some(source) => source,
        };
        info!("added as house palatability source");
        self.houses_sources.push(source);
    }

    pub(super) fn add_office_source(&mut self, source: &impl ToOfficeSourcePalatabilityDescriptor) {
        let source = match source.to_office_source_palatability() {
            None => return,
            Some(source) => source,
        };
        info!("added as house palatability source");
        self.office_sources.push(source);
    }

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

    pub(super) fn increment_unemployed_inhabitants(&mut self, delta: i32) {
        self.unemployed_inhabitants =
            (self.unemployed_inhabitants as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn increment_vacant_work(&mut self, delta: i32) {
        self.vacant_work = (self.vacant_work as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn increment_vacant_inhabitants(&mut self, delta: i32) {
        self.vacant_inhabitants = (self.vacant_inhabitants as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn get_inhabitants_to_spawn_and_increment_populations(&mut self) -> u8 {
        let c: u8 = self.vacant_inhabitants.min(u8::MAX as u64) as u8;
        self.vacant_inhabitants = 0;
        self.total_populations += c as u64;

        c
    }

    pub fn total_populations(&self) -> u64 {
        self.total_populations
    }

    #[allow(dead_code)]
    pub fn unemployed_inhabitants(&self) -> u64 {
        self.unemployed_inhabitants
    }

    #[allow(dead_code)]
    pub fn vacant_work(&self) -> u64 {
        self.vacant_work
    }

    #[allow(dead_code)]
    pub fn vacant_inhabitants(&self) -> u64 {
        self.vacant_inhabitants
    }
}

pub struct HouseSourcePalatabilityDescriptor {
    pub origin: Position,
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

impl HouseSourcePalatabilityDescriptor {
    fn calculate(&self, position: &Position) -> i32 {
        let distance = self.origin.distance(position);

        if distance < self.max_horizontal_distribution_distance {
            return self.value;
        }

        if distance < self.max_linear_distribution_distance {
            let d = (distance - self.max_horizontal_distribution_distance) as i32;

            let value = self.value - self.linear_factor * d;
            if self.value > 0 {
                return value.max(0);
            } else {
                return value.min(0);
            }
        }

        0
    }
}

pub struct OfficeSourcePalatabilityDescriptor {
    pub origin: Position,
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

impl OfficeSourcePalatabilityDescriptor {
    fn calculate(&self, position: &Position) -> i32 {
        let distance = self.origin.distance(position);

        if distance < self.max_horizontal_distribution_distance {
            return self.value;
        }

        if distance < self.max_linear_distribution_distance {
            let d = (distance - self.max_horizontal_distribution_distance) as i32;

            let value = self.value - self.linear_factor * d;
            if self.value > 0 {
                return value.max(0);
            } else {
                return value.min(0);
            }
        }

        0
    }
}

pub struct HousePalatability {
    value: i32,
}

impl HousePalatability {
    #[inline]
    pub fn is_positive(&self) -> bool {
        // we consider 0 as positive PalatabilityManager
        !self.value.is_negative()
    }
}

pub struct OfficePalatability {
    value: i32,
}

impl OfficePalatability {
    #[inline]
    pub fn is_positive(&self) -> bool {
        // we consider 0 as positive PalatabilityManager
        !self.value.is_negative()
    }
}

pub trait ToHouseSourcePalatabilityDescriptor {
    fn to_house_source_palatability(&self) -> Option<HouseSourcePalatabilityDescriptor>;
}
pub trait ToOfficeSourcePalatabilityDescriptor {
    fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor>;
}

impl ToHouseSourcePalatabilityDescriptor for Building {
    fn to_house_source_palatability(&self) -> Option<HouseSourcePalatabilityDescriptor> {
        match self {
            Building::Garden(g) => g.to_house_source_palatability(),
            Building::House(h) => h.to_house_source_palatability(),
            Building::Street(s) => s.to_house_source_palatability(),
            Building::Office(o) => o.to_house_source_palatability(),
        }
    }
}

impl ToOfficeSourcePalatabilityDescriptor for Building {
    fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor> {
        match self {
            Building::Garden(g) => g.to_office_source_palatability(),
            Building::House(h) => h.to_office_source_palatability(),
            Building::Street(s) => s.to_office_source_palatability(),
            Building::Office(o) => o.to_office_source_palatability(),
        }
    }
}

macro_rules! impl_to_source_palatability_descriptor {
    ($cl: ty, $name: tt) => {
        impl ToHouseSourcePalatabilityDescriptor for $cl {
            fn to_house_source_palatability(&self) -> Option<HouseSourcePalatabilityDescriptor> {
                use crate::common::configuration::CONFIGURATION;

                let c = &CONFIGURATION
                    .buildings
                    .$name
                    .palatability_configuration
                    .house_source;
                let e = match c {
                    None => return None,
                    Some(e) => e,
                };
                Some(HouseSourcePalatabilityDescriptor {
                    origin: self.position,
                    value: e.value,
                    max_horizontal_distribution_distance: e.max_horizontal_distribution_distance,
                    max_linear_distribution_distance: e.max_linear_distribution_distance,
                    linear_factor: e.linear_factor,
                })
            }
        }
        impl ToOfficeSourcePalatabilityDescriptor for $cl {
            fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor> {
                use crate::common::configuration::CONFIGURATION;

                let c = &CONFIGURATION
                    .buildings
                    .$name
                    .palatability_configuration
                    .office_source;
                let e = match c {
                    None => return None,
                    Some(e) => e,
                };
                Some(OfficeSourcePalatabilityDescriptor {
                    origin: self.position,
                    value: e.value,
                    max_horizontal_distribution_distance: e.max_horizontal_distribution_distance,
                    max_linear_distribution_distance: e.max_linear_distribution_distance,
                    linear_factor: e.linear_factor,
                })
            }
        }
    };
}
impl_to_source_palatability_descriptor!(House, house);
impl_to_source_palatability_descriptor!(Garden, garden);
impl_to_source_palatability_descriptor!(Office, office);
impl_to_source_palatability_descriptor!(Street, street);
