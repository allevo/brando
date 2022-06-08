pub mod builder;
pub mod plugin;

use std::fmt::Display;

use crate::{common::position::Position, navigation::navigator::Reachable};

pub enum Building {
    House(House),
    Street(Street),
    Garden(Garden),
    Office(Office),
}
impl Building {
    pub fn position(&self) -> Option<Position> {
        match self {
            Building::Garden(g) => Some(g.position),
            Building::Street(s) => Some(s.position),
            Building::House(h) => Some(h.position),
            Building::Office(o) => Some(o.position),
        }
    }
}

pub struct ResidentProperty {
    pub current_residents: u8,
    pub incoming_residents: u8,
    pub max_residents: u8,
}

pub struct House {
    pub position: Position,
    pub resident_property: ResidentProperty,
}
impl From<&mut House> for Reachable {
    fn from(house: &mut House) -> Self {
        let desired_inhabitants = house.resident_property.max_residents
            - house.resident_property.current_residents
            - house.resident_property.incoming_residents;
        let actual_inhabitants = (desired_inhabitants).min(6);

        let terminates = desired_inhabitants == actual_inhabitants;

        Self {
            position: house.position,
            count: actual_inhabitants,
            terminates,
        }
    }
}

pub struct Office {
    pub position: Position,
}
pub struct Street {
    pub position: Position,
}
pub struct Garden {
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuildingType {
    House,
    Street,
    Garden,
    Office,
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
pub static OFFICE_PROTOTYPE: BuildingPrototype = BuildingPrototype {
    name: "office",
    time_for_building: 5,
    building_type: BuildingType::Office,
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
                    incoming_residents: 0,
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

impl TryInto<Office> for &mut BuildingInConstruction {
    type Error = &'static str;

    fn try_into(self) -> Result<Office, Self::Error> {
        match self.request.prototype.building_type {
            BuildingType::Office => Ok(Office {
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
            BuildingType::Office => self.try_into().map(Building::Office),
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
