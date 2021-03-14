use crate::{buildings::prototype::BuildingPrototypeType, heigth::Heigth, point::Point};

pub enum Request {
    AddBuildingRequest(AddBuildingRequest),
    DeleteBuildingRequest(DeleteBuildingRequest),
    GetSnapshotRequest(GetSnapshotRequest),
    Ping,
    Close,
}

#[derive(Debug)]
pub struct AddBuildingRequest {
    pub building_prototype_type: BuildingPrototypeType,
    pub position: Point,
    pub heigth: Heigth,
}

impl AddBuildingRequest {
    pub fn new(
        building_prototype_type: BuildingPrototypeType,
        position: Point,
        heigth: Heigth,
    ) -> Self {
        Self {
            building_prototype_type,
            position,
            heigth,
        }
    }
}

#[derive(Debug)]
pub struct DeleteBuildingRequest {
    pub position: Point,
    pub heigth: Heigth,
}

impl DeleteBuildingRequest {
    pub fn new(position: Point, heigth: Heigth) -> Self {
        Self { position, heigth }
    }
}

pub struct GetSnapshotRequest {}

pub struct SpawnCitizensRequest {}
