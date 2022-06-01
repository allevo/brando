pub mod builder;

use std::fmt::Display;

use crate::{
    navigator::Reachable, palatability::HouseSourcePalatabilityDescriptor, position::Position,
};

pub enum Building {
    House(House),
    Street(Street),
    Garden(Garden),
}
impl Building {
    pub fn position(&self) -> Option<Position> {
        match self {
            Building::Garden(g) => Some(g.position),
            Building::Street(s) => Some(s.position),
            Building::House(h) => Some(h.position),
        }
    }
}

pub struct ResidentProperty {
    pub current_residents: u8,
    pub max_residents: u8,
}

pub struct House {
    pub position: Position,
    pub resident_property: ResidentProperty,
}
impl From<&House> for Reachable {
    fn from(house: &House) -> Self {
        let count =
            house.resident_property.max_residents - house.resident_property.current_residents;
        Self {
            position: house.position,
            count,
        }
    }
}

impl From<&House> for HouseSourcePalatabilityDescriptor {
    fn from(house: &House) -> Self {
        Self {
            origin: house.position,
            value: -1,
            max_horizontal_distribution_distance: 2,
            max_linear_distribution_distance: 1,
            linear_factor: 0,
        }
    }
}
pub struct Street {
    pub position: Position,
}
pub struct Garden {
    pub position: Position,
}
impl From<&Garden> for HouseSourcePalatabilityDescriptor {
    fn from(garden: &Garden) -> Self {
        Self {
            origin: garden.position,
            value: 10,
            max_horizontal_distribution_distance: 3,
            max_linear_distribution_distance: 10,
            linear_factor: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuildingType {
    House,
    Street,
    Garden,
}

pub struct BuildingPrototype {
    name: &'static str,
    pub time_for_building: u8,
    pub building_type: BuildingType,
}
impl Display for BuildingPrototype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub static HOUSE_PROTOTYPE: BuildingPrototype = BuildingPrototype {
    name: "house",
    time_for_building: 10,
    building_type: BuildingType::House,
};
pub static STREET_PROTOTYPE: BuildingPrototype = BuildingPrototype {
    name: "street",
    time_for_building: 2,
    building_type: BuildingType::Street,
};
pub static GARDEN_PROTOTYPE: BuildingPrototype = BuildingPrototype {
    name: "garden",
    time_for_building: 2,
    building_type: BuildingType::Garden,
};

#[derive(Clone)]
pub struct BuildRequest {
    pub position: Position,
    pub prototype: &'static BuildingPrototype,
}
impl BuildRequest {
    pub fn new(position: Position, prototype: &'static BuildingPrototype) -> Self {
        Self {
            position,
            prototype,
        }
    }
}

#[derive(Clone)]
pub struct BuildingInConstruction {
    pub request: BuildRequest,
    pub progress_status: ProgressStatus,
}

impl BuildingInConstruction {
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.progress_status.is_completed()
    }
}

impl TryInto<House> for &mut BuildingInConstruction {
    type Error = &'static str;

    fn try_into(self) -> Result<House, Self::Error> {
        match self.request.prototype.building_type {
            BuildingType::House => Ok(House {
                position: self.request.position,
                resident_property: ResidentProperty {
                    current_residents: 0,
                    max_residents: 8,
                },
            }),
            _ => Err("NO"),
        }
    }
}
impl TryInto<Street> for &mut BuildingInConstruction {
    type Error = &'static str;

    fn try_into(self) -> Result<Street, Self::Error> {
        match self.request.prototype.building_type {
            BuildingType::Street => Ok(Street {
                position: self.request.position,
            }),
            _ => Err("NO"),
        }
    }
}

impl TryInto<Garden> for &mut BuildingInConstruction {
    type Error = &'static str;

    fn try_into(self) -> Result<Garden, Self::Error> {
        match self.request.prototype.building_type {
            BuildingType::Garden => Ok(Garden {
                position: self.request.position,
            }),
            _ => Err("NO"),
        }
    }
}

impl TryInto<Building> for &mut BuildingInConstruction {
    type Error = &'static str;

    fn try_into(self) -> Result<Building, Self::Error> {
        match self.request.prototype.building_type {
            BuildingType::House => self.try_into().map(Building::House),
            BuildingType::Street => self.try_into().map(Building::Street),
            BuildingType::Garden => self.try_into().map(Building::Garden),
        }
    }
}

#[derive(Clone)]
pub struct ProgressStatus {
    pub current_step: u8,
    pub step_to_reach: u8,
}
impl ProgressStatus {
    pub fn progress(self) -> Self {
        Self {
            current_step: self.current_step + 1,
            ..self
        }
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        self.current_step >= self.step_to_reach
    }
}
