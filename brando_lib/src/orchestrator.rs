use crate::{
    builder::concretize_building,
    errors::{AddBuildingError, DeleteBuildingError},
    map::{Map, MapSnapshot},
    mayor::Mayor,
    requests::{
        AddBuildingRequest, DeleteBuildingRequest, GetSnapshotRequest, SpawnCitizensRequest,
    },
    responses::{AddBuildingResponse, DeleteBuildingResponse},
};

type Cost = u32;

pub struct Orchestrator<Map_: Map, Mayor_: Mayor> {
    map: Map_,
    mayor: Mayor_,
}

impl<Map_: Map, Mayor_: Mayor> Orchestrator<Map_, Mayor_> {
    pub fn new(map: Map_, mayor: Mayor_) -> Self {
        Self { map, mayor }
    }

    pub fn add_building(
        self: &mut Self,
        request: AddBuildingRequest,
    ) -> Result<AddBuildingResponse, AddBuildingError> {
        let cost = self.calculate_cost(&request);

        let has_budget = self.mayor.has_budget(cost);

        if !has_budget {
            return Err(AddBuildingError::InsufficientBudget);
        }

        let result = self.map.check_for_adding_building(&request);
        if let Some(err) = result {
            return Err(err.into());
        }
        let result = self.map.add_building(request);
        if let Err(err) = result {
            return Err(err.into());
        }

        self.mayor.decrement_budget(cost).unwrap();

        result
    }

    pub fn delete_building(
        self: &mut Self,
        request: DeleteBuildingRequest,
    ) -> Result<DeleteBuildingResponse, DeleteBuildingError> {
        let result = self.map.check_for_deleting_building(&request);
        if let Some(err) = result {
            return Err(err.into());
        }
        let result = self.map.delete_building(request);
        if let Err(err) = result {
            return Err(err.into());
        }

        Ok(DeleteBuildingResponse::new())
    }

    pub fn get_map_snapshot(self: &Self, _request: GetSnapshotRequest) -> MapSnapshot {
        self.map.get_snapshot()
    }

    pub fn spawn_citizens(self: &Self, _request: SpawnCitizensRequest) {}

    fn calculate_cost(self: &Self, _request: &AddBuildingRequest) -> Cost {
        0
    }
}
