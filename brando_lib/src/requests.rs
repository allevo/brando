use crate::{buildings::prototype::{BuildingPrototype, BuildingPrototypeType}, heigth::{self, Heigth}, point::Point};


pub enum Request {
    AddBuildingRequest(AddBuildingRequest),
    DeleteBuildingRequest(DeleteBuildingRequest),
    GetSnapshotRequest(GetSnapshotRequest),
    Ping,
    Close,
}

pub struct AddBuildingRequest {
    pub building_prototype_type: BuildingPrototypeType,
    pub position: Point,
    pub heigth: Heigth,
}

impl AddBuildingRequest {
    pub fn new(building_prototype_type: BuildingPrototypeType, position: Point, heigth: Heigth) -> Self {
        Self {
            building_prototype_type,
            position,
            heigth,
        }
    }
}

pub struct DeleteBuildingRequest {
    pub position: Point,
    pub heigth: Heigth,
}

impl DeleteBuildingRequest {
    pub fn new(position: Point, heigth: Heigth) -> Self {
        Self {
            position,
            heigth,
        }
    }
}

pub struct GetSnapshotRequest {}

pub struct SpawnCitizensRequest {}
