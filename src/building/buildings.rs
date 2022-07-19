
use crate::common::{position::Position, configuration::Configuration};

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
    pub max_residents: u8,
}

pub struct House {
    pub position: Position,
    pub resident_property: ResidentProperty,
}

pub struct WorkProperty {
    pub current_worker: u8,
    pub max_worker: u8,
}

pub struct Office {
    pub position: Position,
    pub work_property: WorkProperty,
}
pub struct Street {
    pub position: Position,
}
pub struct Garden {
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingType {
    House,
    Street,
    Garden,
    Office,
}

#[derive(Clone)]
pub struct BuildRequest {
    pub position: Position,
    pub building_type: BuildingType,
}
impl BuildRequest {
    pub fn new(position: Position, building_type: BuildingType) -> Self {
        Self {
            position,
            building_type,
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

pub trait IntoBuilding<T> {
    fn into_building(&self, configuration: &Configuration) -> Result<T, &'static str>;
}


impl IntoBuilding<House> for BuildingInConstruction {
    fn into_building(&self, configuration: &Configuration) -> Result<House, &'static str> {
        match self.request.building_type {
            BuildingType::House => Ok(House {
                position: self.request.position,
                resident_property: ResidentProperty {
                    current_residents: 0,
                    max_residents: configuration.buildings.house.max_residents,
                },
            }),
            _ => Err("NO"),
        }    }
}
impl IntoBuilding<Office> for BuildingInConstruction {
    fn into_building(&self, configuration: &Configuration) -> Result<Office, &'static str> {
        match self.request.building_type {
            BuildingType::Office => Ok(Office {
                position: self.request.position,
                work_property: WorkProperty {
                    current_worker: 0,
                    max_worker: configuration.buildings.office.max_worker,
                },
            }),
            _ => Err("NO"),
        }
    }
}
impl IntoBuilding<Street> for BuildingInConstruction {
    fn into_building(&self, _configuration: &Configuration) -> Result<Street, &'static str> {
        match self.request.building_type {
            BuildingType::Street => Ok(Street {
                position: self.request.position,
            }),
            _ => Err("NO"),
        }
    }
}
impl IntoBuilding<Garden> for BuildingInConstruction {
    fn into_building(&self, _configuration: &Configuration) -> Result<Garden, &'static str> {
        match self.request.building_type {
            BuildingType::Garden => Ok(Garden {
                position: self.request.position,
            }),
            _ => Err("NO"),
        }
    }
}
impl IntoBuilding<Building> for BuildingInConstruction {
    fn into_building(&self, configuration: &Configuration) -> Result<Building, &'static str> {
        match self.request.building_type {
            BuildingType::House => self.into_building(configuration).map(Building::House),
            BuildingType::Office => self.into_building(configuration).map(Building::Office),
            BuildingType::Garden => self.into_building(configuration).map(Building::Garden),
            BuildingType::Street => self.into_building(configuration).map(Building::Street),
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
