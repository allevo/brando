use crate::{
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
        &mut self,
        request: AddBuildingRequest,
    ) -> Result<AddBuildingResponse, AddBuildingError> {
        let cost = self.calculate_cost(&request);

        let has_budget = self.mayor.has_budget(cost);

        if !has_budget {
            return Err(AddBuildingError::InsufficientBudget);
        }
        let result = self.map.add_building(request);
        if let Err(err) = result {
            return Err(err);
        }

        self.mayor.decrement_budget(cost).unwrap();

        result
    }

    pub fn delete_building(
        &mut self,
        request: DeleteBuildingRequest,
    ) -> Result<DeleteBuildingResponse, DeleteBuildingError> {
        self.map.delete_building(request)
    }

    pub fn get_map_snapshot(&self, _request: GetSnapshotRequest) -> MapSnapshot {
        self.map.get_snapshot()
    }

    pub fn spawn_citizens(&self, _request: SpawnCitizensRequest) {}

    fn calculate_cost(&self, _request: &AddBuildingRequest) -> Cost {
        0
    }
}

#[cfg(test)]
mod test_orchestrator {
    use crate::{
        buildings::{
            concrete::{Building, ConcreteBuilding, House1x1},
            prototype::{BuildingPrototypeType, HOUSE_1x1},
        },
        heigth::Heigth,
        point::Point,
    };
    use std::rc::Rc;

    use super::*;

    use mockers::{matchers::any, Scenario};

    #[test]
    fn add_building() {
        let building_id = 42;
        let building = Rc::new(Building {
            id: building_id,
            building: ConcreteBuilding::House1x1(House1x1::new(0, 0)),
            prototype: &HOUSE_1x1,
        });
        let expected_add_building_response = AddBuildingResponse::new(building);

        let scenario = Scenario::new();
        let (mock_mayor, mayor_handle) = scenario.create_mock_for::<dyn Mayor>();
        let (mock_map, map_handle) = scenario.create_mock_for::<dyn Map>();

        scenario.expect(mayor_handle.has_budget(0).and_return(true));
        scenario.expect(
            map_handle
                .add_building(any())
                .and_return(Ok(expected_add_building_response.clone())),
        );
        scenario.expect(mayor_handle.decrement_budget(0).and_return(Ok(())));

        let mut orchestrator = Orchestrator::new(mock_map, mock_mayor);

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(0, 0),
            Heigth::Ground,
        );
        let add_building_response = orchestrator.add_building(request);

        assert_eq!(add_building_response, Ok(expected_add_building_response));
    }

    #[test]
    fn add_building_no_budget() {
        let scenario = Scenario::new();
        let (mock_mayor, mayor_handle) = scenario.create_mock_for::<dyn Mayor>();
        let (mock_map, map_handle) = scenario.create_mock_for::<dyn Map>();

        scenario.expect(mayor_handle.has_budget(0).and_return(false));

        let mut orchestrator = Orchestrator::new(mock_map, mock_mayor);

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(0, 0),
            Heigth::Ground,
        );
        let add_building_response = orchestrator.add_building(request);

        assert_eq!(
            add_building_response,
            Err(AddBuildingError::InsufficientBudget)
        );
    }

    #[test]
    fn add_building_already_taken() {
        let scenario = Scenario::new();
        let (mock_mayor, mayor_handle) = scenario.create_mock_for::<dyn Mayor>();
        let (mock_map, map_handle) = scenario.create_mock_for::<dyn Map>();

        scenario.expect(mayor_handle.has_budget(0).and_return(true));
        scenario.expect(
            map_handle
                .add_building(any())
                .and_return(Err(AddBuildingError::AlreadyTaken)),
        );

        let mut orchestrator = Orchestrator::new(mock_map, mock_mayor);

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(0, 0),
            Heigth::Ground,
        );
        let add_building_response = orchestrator.add_building(request);

        assert_eq!(add_building_response, Err(AddBuildingError::AlreadyTaken));
    }

    #[test]
    fn delete_building() {
        let expected_delete_building_response = DeleteBuildingResponse::new();

        let scenario = Scenario::new();
        let (mock_mayor, _) = scenario.create_mock_for::<dyn Mayor>();
        let (mock_map, map_handle) = scenario.create_mock_for::<dyn Map>();

        scenario.expect(
            map_handle
                .delete_building(any())
                .and_return(Ok(expected_delete_building_response.clone())),
        );

        let mut orchestrator = Orchestrator::new(mock_map, mock_mayor);

        let request = DeleteBuildingRequest::new(Point::new(0, 0), Heigth::Ground);
        let delete_building_response = orchestrator.delete_building(request);

        assert_eq!(
            delete_building_response,
            Ok(expected_delete_building_response)
        );
    }

    #[test]
    fn delete_building_no_building() {
        let scenario = Scenario::new();
        let (mock_mayor, _) = scenario.create_mock_for::<dyn Mayor>();
        let (mock_map, map_handle) = scenario.create_mock_for::<dyn Map>();

        scenario.expect(
            map_handle
                .delete_building(any())
                .and_return(Err(DeleteBuildingError::NoBuildingFound)),
        );

        let mut orchestrator = Orchestrator::new(mock_map, mock_mayor);

        let request = DeleteBuildingRequest::new(Point::new(0, 0), Heigth::Ground);
        let delete_building_response = orchestrator.delete_building(request);

        assert_eq!(
            delete_building_response,
            Err(DeleteBuildingError::NoBuildingFound)
        );
    }
}
