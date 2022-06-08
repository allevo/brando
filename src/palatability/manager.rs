use crate::{common::position::Position, building::{Building, House, Garden, Office}};

pub struct PalatabilityManager {
    total_populations: u64,
    // TODO: change approach to store the rendered values
    houses_sources: Vec<HouseSourcePalatabilityDescriptor>,
    // TODO: change approach to store the rendered values
    office_sources: Vec<OfficeSourcePalatabilityDescriptor>,
}
impl PalatabilityManager {
    pub fn new() -> Self {
        Self {
            total_populations: 0,
            houses_sources: vec![],
            office_sources: vec![],
        }
    }

    pub fn add_house_source(&mut self, source: impl Into<Option<HouseSourcePalatabilityDescriptor>>) {
        let source = match source.into() {
            None => return,
            Some(source) => source,
        };
        self.houses_sources.push(source);
    }
    
    pub fn add_office_source(&mut self, source: impl Into<Option<OfficeSourcePalatabilityDescriptor>>) {
        let source = match source.into() {
            None => return,
            Some(source) => source,
        };
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

    pub fn increment_populations(&mut self, delta: i32) {
        self.total_populations = (self.total_populations as i128 + delta as i128).max(0) as u64;
    }

    pub fn total_populations(&self) -> u64 {
        self.total_populations
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
            Building::Street(_) => None,
            Building::Office(_) => None,
        }
    }
}

impl ToHouseSourcePalatabilityDescriptor for House {
    fn to_house_source_palatability(&self) -> Option<HouseSourcePalatabilityDescriptor> {
        Some(HouseSourcePalatabilityDescriptor {
            origin: self.position,
            value: -1,
            max_horizontal_distribution_distance: 2,
            max_linear_distribution_distance: 1,
            linear_factor: 0,
        })
    }
}
impl ToHouseSourcePalatabilityDescriptor for Garden {
    fn to_house_source_palatability(&self) -> Option<HouseSourcePalatabilityDescriptor> {
        Some(HouseSourcePalatabilityDescriptor {
            origin: self.position,
            value: 10,
            max_horizontal_distribution_distance: 3,
            max_linear_distribution_distance: 10,
            linear_factor: 2,
        })
    }
}

impl ToOfficeSourcePalatabilityDescriptor for Building {
    fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor> {
        match self {
            Building::Garden(_) => None,
            Building::House(_) => None,
            Building::Street(_) => None,
            Building::Office(_) => None,
        }
    }
}
impl ToOfficeSourcePalatabilityDescriptor for Garden {
    fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor> {
        Some(OfficeSourcePalatabilityDescriptor {
            origin: self.position,
            value: 10,
            max_horizontal_distribution_distance: 3,
            max_linear_distribution_distance: 10,
            linear_factor: 2,
        })
    }
}
impl ToOfficeSourcePalatabilityDescriptor for Office {
    fn to_office_source_palatability(&self) -> Option<OfficeSourcePalatabilityDescriptor> {
        Some(OfficeSourcePalatabilityDescriptor {
            origin: self.position,
            value: 1,
            max_horizontal_distribution_distance: 3,
            max_linear_distribution_distance: 0,
            linear_factor: 0,
        })
    }
}
