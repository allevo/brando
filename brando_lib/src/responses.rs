use crate::{
    builder,
    buildings::concrete::{Building, BuildingId, ConcreteBuilding},
    map::MapSnapshot,
};
use std::rc::Rc;

pub enum Response {
    AddBuildingResponse(AddBuildingResponse),
    DeleteBuildingResponse(DeleteBuildingResponse),
    GetSnapshotResponse(MapSnapshot), // TODO: fix me
    Pong,
    Close,
}

#[cfg_attr(test, derive(PartialEq, Debug, Clone))]
pub struct AddBuildingResponse {
    pub building: Rc<Building>,
}
impl AddBuildingResponse {
    pub fn new(building: Rc<Building>) -> Self {
        Self { building }
    }
}

#[cfg_attr(test, derive(PartialEq, Debug, Clone))]
pub struct DeleteBuildingResponse {}
impl DeleteBuildingResponse {
    pub fn new() -> Self {
        Self {}
    }
}
