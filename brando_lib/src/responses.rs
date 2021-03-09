use crate::{buildings::concrete::BuildingId, map::MapSnapshot};

pub enum Response {
    AddBuildingResponse(AddBuildingResponse),
    DeleteBuildingResponse(DeleteBuildingResponse),
    GetSnapshotResponse(MapSnapshot), // TODO: fix me
    Pong,
    Close,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct AddBuildingResponse {
    pub building_id: BuildingId,
}
impl AddBuildingResponse {
    pub fn new(building_id: BuildingId) -> Self {
        Self { building_id }
    }
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct DeleteBuildingResponse {}
impl DeleteBuildingResponse {
    pub fn new() -> Self {
        Self {}
    }
}
