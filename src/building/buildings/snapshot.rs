use lombok::Getter;

use crate::{
    building::manager::Building,
    common::{position::Position, EntityId},
};

#[derive(Debug)]
pub enum BuildingSnapshot {
    House(HouseSnapshot),
    Office(OfficeSnapshot),
    Street(StreetSnapshot),
    Garden(GardenSnapshot),
    BiomassPowerPlant(BiomassPowerPlantSnapshot),
}

#[allow(dead_code)]
impl BuildingSnapshot {
    pub fn into_house(self) -> HouseSnapshot {
        match self {
            BuildingSnapshot::House(h) => h,
            _ => unreachable!("BuildingSnapshot is not an HouseSnapshot"),
        }
    }

    pub fn into_office(self) -> OfficeSnapshot {
        match self {
            BuildingSnapshot::Office(o) => o,
            _ => unreachable!("BuildingSnapshot is not an OfficeSnapshot"),
        }
    }

    pub fn get_position(&self) -> &Position {
        match self {
            BuildingSnapshot::House(b) => b.get_position(),
            BuildingSnapshot::Office(b) => b.get_position(),
            BuildingSnapshot::Garden(b) => b.get_position(),
            BuildingSnapshot::Street(b) => b.get_position(),
            BuildingSnapshot::BiomassPowerPlant(b) => b.get_position(),
        }
    }

    pub fn get_id(&self) -> &EntityId {
        match self {
            BuildingSnapshot::House(b) => b.get_id(),
            BuildingSnapshot::Office(b) => b.get_id(),
            BuildingSnapshot::Garden(b) => b.get_id(),
            BuildingSnapshot::Street(b) => b.get_id(),
            BuildingSnapshot::BiomassPowerPlant(b) => b.get_id(),
        }
    }
}

impl From<&Building> for BuildingSnapshot {
    fn from(building: &Building) -> Self {
        match building {
            Building::House(h) => BuildingSnapshot::House(HouseSnapshot {
                id: *h.get_id(),
                position: *h.get_position(),
                current_residents: *h.get_current_residents(),
                max_residents: *h.get_max_residents(),
            }),
            Building::Office(o) => BuildingSnapshot::Office(OfficeSnapshot {
                id: *o.get_id(),
                position: *o.get_position(),
                current_workers: *o.get_current_workers(),
                max_workers: *o.get_max_workers(),
            }),
            Building::Garden(g) => BuildingSnapshot::Garden(GardenSnapshot {
                id: *g.get_id(),
                position: *g.get_position(),
            }),
            Building::Street(s) => BuildingSnapshot::Street(StreetSnapshot {
                id: *s.get_id(),
                position: *s.get_position(),
            }),
            Building::BiomassPowerPlant(b) => {
                BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
                    id: *b.get_id(),
                    position: *b.get_position(),
                })
            }
        }
    }
}

#[derive(Getter, Debug)]
pub struct HouseSnapshot {
    pub id: EntityId,
    pub position: Position,
    pub current_residents: u32,
    pub max_residents: u32,
}
#[derive(Getter, Debug)]
pub struct OfficeSnapshot {
    pub id: EntityId,
    pub position: Position,
    pub current_workers: u32,
    pub max_workers: u32,
}
#[derive(Getter, Debug)]
pub struct StreetSnapshot {
    pub id: EntityId,
    pub position: Position,
}
#[derive(Getter, Debug)]
pub struct GardenSnapshot {
    pub id: EntityId,
    pub position: Position,
}
#[derive(Getter, Debug)]
pub struct BiomassPowerPlantSnapshot {
    pub id: EntityId,
    pub position: Position,
}
