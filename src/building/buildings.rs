use crate::common::position::Position;

use super::plugin::{
    BiomassPowerPlantSnapshot, BuildingSnapshot, GardenSnapshot, HouseSnapshot, OfficeSnapshot,
    StreetSnapshot,
};

pub type BuildingId = u64;

pub enum Building {
    House(House),
    Street(Street),
    Garden(Garden),
    Office(Office),
    BiomassPowerPlant(BiomassPowerPlant),
}

#[allow(dead_code)]
impl Building {
    pub fn id(&self) -> BuildingId {
        match self {
            Building::Garden(g) => g.id,
            Building::Office(o) => o.id,
            Building::Street(s) => s.id,
            Building::House(h) => h.id,
            Building::BiomassPowerPlant(bpp) => bpp.id,
        }
    }

    pub fn as_mut_house(&mut self) -> &mut House {
        match self {
            Building::House(h) => h,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }

    pub fn into_house(self) -> House {
        match self {
            Building::House(h) => h,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }

    pub fn as_mut_office(&mut self) -> &mut Office {
        match self {
            Building::Office(o) => o,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }

    pub fn into_office(self) -> Office {
        match self {
            Building::Office(o) => o,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }
    pub fn into_street(self) -> Street {
        match self {
            Building::Street(s) => s,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }
    pub fn into_garden(self) -> Garden {
        match self {
            Building::Garden(g) => g,
            _ => unreachable!("cannot call as_house for not houses"),
        }
    }

    pub fn snapshot(&self) -> BuildingSnapshot {
        match self {
            Building::Office(o) => BuildingSnapshot::Office(OfficeSnapshot {
                position: o.position,
                work_property: o.work_property.clone(),
            }),
            Building::House(h) => BuildingSnapshot::House(HouseSnapshot {
                position: h.position,
                resident_property: h.resident_property.clone(),
            }),
            Building::Street(s) => BuildingSnapshot::Street(StreetSnapshot {
                position: s.position,
            }),
            Building::Garden(g) => BuildingSnapshot::Garden(GardenSnapshot {
                position: g.position,
            }),
            Building::BiomassPowerPlant(bpp) => {
                BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
                    position: bpp.position,
                })
            }
        }
    }
}

#[derive(Clone)]
pub struct ResidentProperty {
    pub current_residents: usize,
    pub max_residents: usize,
}

pub struct House {
    pub id: BuildingId,
    pub position: Position,
    pub resident_property: ResidentProperty,
}

#[derive(Clone)]
pub struct WorkProperty {
    pub current_worker: usize,
    pub max_worker: usize,
}

pub struct Office {
    pub id: BuildingId,
    pub position: Position,
    pub work_property: WorkProperty,
}
pub struct Street {
    pub id: BuildingId,
    pub position: Position,
}
pub struct Garden {
    pub id: BuildingId,
    pub position: Position,
}

pub struct BiomassPowerPlant {
    pub id: BuildingId,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingType {
    House,
    Street,
    Garden,
    Office,
    BiomassPowerPlant,
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
pub struct BuildingUnderConstruction {
    pub request: BuildRequest,
    pub progress_status: ProgressStatus,
}

impl BuildingUnderConstruction {
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.progress_status.is_completed()
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
