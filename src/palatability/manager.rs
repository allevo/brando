use std::sync::Arc;

use tracing::info;

use crate::{
    building::plugin::{
        BiomassPowerPlantSnapshot, BuildingSnapshot, GardenSnapshot, HouseSnapshot, OfficeSnapshot,
        StreetSnapshot,
    },
    common::{configuration::Configuration, position::Position},
};

pub struct PalatabilityManager {
    configuration: Arc<Configuration>,
    total_populations: u64,
    unemployed_inhabitants: Vec<u64>,
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
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            total_populations: 0,
            unemployed_inhabitants: vec![],
            vacant_inhabitants: 0,
            vacant_work: 0,
            houses_sources: vec![],
            office_sources: vec![],
        }
    }

    pub(super) fn add_house_source(&mut self, source: &impl ToHouseSourcePalatabilityDescriptor) {
        let source = match source.to_house_source_palatability(&self.configuration) {
            None => return,
            Some(source) => source,
        };
        info!("added as house palatability source");
        self.houses_sources.push(source);
    }

    pub(super) fn add_office_source(&mut self, source: &impl ToOfficeSourcePalatabilityDescriptor) {
        let source = match source.to_office_source_palatability(&self.configuration) {
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

    pub(super) fn add_unemployed_inhabitants(&mut self, inhabitants: Vec<u64>) {
        self.unemployed_inhabitants.extend(inhabitants);
    }

    pub(super) fn increment_vacant_work(&mut self, delta: i32) {
        self.vacant_work = (self.vacant_work as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn increment_vacant_inhabitants(&mut self, delta: i32) {
        self.vacant_inhabitants = (self.vacant_inhabitants as i128 + delta as i128).max(0) as u64;
    }

    pub(super) fn consume_inhabitants_to_spawn_and_increment_populations(&mut self) -> u8 {
        let c: u8 = self.vacant_inhabitants.min(u8::MAX as u64) as u8;
        self.vacant_inhabitants = 0;
        self.total_populations += c as u64;

        c
    }

    pub(super) fn consume_workers_to_spawn(&mut self) -> Vec<u64> {
        if self.unemployed_inhabitants.is_empty() {
            return vec![];
        }
        if self.vacant_work == 0 {
            return vec![];
        }

        self.unemployed_inhabitants.drain(0..1).collect()
    }

    pub fn total_populations(&self) -> u64 {
        self.total_populations
    }

    #[allow(dead_code)]
    pub fn unemployed_inhabitants(&self) -> Vec<u64> {
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
    fn to_house_source_palatability(
        &self,
        configuration: &Configuration,
    ) -> Option<HouseSourcePalatabilityDescriptor>;
}
pub trait ToOfficeSourcePalatabilityDescriptor {
    fn to_office_source_palatability(
        &self,
        configuration: &Configuration,
    ) -> Option<OfficeSourcePalatabilityDescriptor>;
}

impl ToHouseSourcePalatabilityDescriptor for BuildingSnapshot {
    fn to_house_source_palatability(
        &self,
        configuration: &Configuration,
    ) -> Option<HouseSourcePalatabilityDescriptor> {
        match self {
            BuildingSnapshot::Garden(g) => g.to_house_source_palatability(configuration),
            BuildingSnapshot::House(h) => h.to_house_source_palatability(configuration),
            BuildingSnapshot::Street(s) => s.to_house_source_palatability(configuration),
            BuildingSnapshot::Office(o) => o.to_house_source_palatability(configuration),
            BuildingSnapshot::BiomassPowerPlant(bpp) => {
                bpp.to_house_source_palatability(configuration)
            }
        }
    }
}

impl ToOfficeSourcePalatabilityDescriptor for BuildingSnapshot {
    fn to_office_source_palatability(
        &self,
        configuration: &Configuration,
    ) -> Option<OfficeSourcePalatabilityDescriptor> {
        match self {
            BuildingSnapshot::Garden(g) => g.to_office_source_palatability(configuration),
            BuildingSnapshot::House(h) => h.to_office_source_palatability(configuration),
            BuildingSnapshot::Street(s) => s.to_office_source_palatability(configuration),
            BuildingSnapshot::Office(o) => o.to_office_source_palatability(configuration),
            BuildingSnapshot::BiomassPowerPlant(bpp) => {
                bpp.to_office_source_palatability(configuration)
            }
        }
    }
}

macro_rules! impl_to_source_palatability_descriptor {
    ($cl: ty, $name: tt) => {
        impl ToHouseSourcePalatabilityDescriptor for $cl {
            fn to_house_source_palatability(
                &self,
                configuration: &Configuration,
            ) -> Option<HouseSourcePalatabilityDescriptor> {
                let e = configuration
                    .buildings
                    .$name
                    .palatability_configuration
                    .source_for_house
                    .as_ref()?;
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
            fn to_office_source_palatability(
                &self,
                configuration: &Configuration,
            ) -> Option<OfficeSourcePalatabilityDescriptor> {
                let e = configuration
                    .buildings
                    .$name
                    .palatability_configuration
                    .source_for_office
                    .as_ref()?;
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
impl_to_source_palatability_descriptor!(HouseSnapshot, house);
impl_to_source_palatability_descriptor!(GardenSnapshot, garden);
impl_to_source_palatability_descriptor!(OfficeSnapshot, office);
impl_to_source_palatability_descriptor!(StreetSnapshot, street);
impl_to_source_palatability_descriptor!(BiomassPowerPlantSnapshot, biomass_power_plant);
