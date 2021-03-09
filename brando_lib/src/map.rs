use std::collections::HashMap;

use crate::{
    builder::concretize_building,
    errors::{AddBuildingError, DeleteBuildingError},
    heigth::Heigth,
    point::Point,
    requests::DeleteBuildingRequest,
    responses::{AddBuildingResponse, DeleteBuildingResponse},
};

use crate::buildings::concrete::Building;
use crate::requests::AddBuildingRequest;

use serde::{Deserialize, Serialize};

pub struct CellDescriptor<'a> {
    pub point: Point,
    pub buildings_on_point: Option<&'a BuildingsOnPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildingsOnPoint {
    pub ground: Option<Building>,
}

pub trait Map {
    fn add_building(
        self: &mut Self,
        request: AddBuildingRequest,
    ) -> Result<AddBuildingResponse, AddBuildingError>;

    fn check_for_adding_building(
        self: &Self,
        request: &AddBuildingRequest,
    ) -> Option<AddBuildingError>;

    fn delete_building(
        self: &mut Self,
        request: DeleteBuildingRequest,
    ) -> Result<DeleteBuildingResponse, DeleteBuildingError>;

    fn check_for_deleting_building(
        self: &Self,
        request: &DeleteBuildingRequest,
    ) -> Option<DeleteBuildingError>;

    fn get_snapshot(self: &Self) -> MapSnapshot;
}

pub struct MatrixMap {
    dim: Point,
    buildings: HashMap<Point, BuildingsOnPoint>,
}

impl MatrixMap {
    pub fn new(dim: Point) -> Self {
        Self {
            dim,
            buildings: HashMap::new(),
        }
    }

    fn is_in_map(self: &Self, point: &Point) -> bool {
        let max_x = self.dim.x();
        let max_y = self.dim.y();
        let x = point.x();
        let y = point.y();

        x <= max_x && x >= 0 && y <= max_y && y >= 0
    }
}

impl Map for MatrixMap {
    fn add_building(
        self: &mut Self,
        request: AddBuildingRequest,
    ) -> Result<AddBuildingResponse, AddBuildingError> {
        if let Some(err) = self.check_for_adding_building(&request) {
            return Err(err);
        }

        let buildings_on_point = self
            .buildings
            .entry(request.position)
            .or_insert(BuildingsOnPoint { ground: None });

        let building = concretize_building(&request.building_prototype_type);
        let building_id = building.id.clone();

        match request.heigth {
            Heigth::Ground => {
                buildings_on_point.ground = Some(building);
            }
        };

        Ok(AddBuildingResponse::new(building_id))
    }

    fn check_for_adding_building(
        self: &Self,
        request: &AddBuildingRequest,
    ) -> Option<AddBuildingError> {
        if !self.is_in_map(&request.position) {
            return Some(AddBuildingError::OutOfMap);
        }

        match (self.buildings.get(&request.position), &request.heigth) {
            (Some(v), Heigth::Ground) => {
                if v.ground.is_some() {
                    Some(AddBuildingError::AlreadyTaken)
                } else {
                    None
                }
            }
            (None, _) => None,
        }
    }

    fn delete_building(
        self: &mut Self,
        request: DeleteBuildingRequest,
    ) -> Result<DeleteBuildingResponse, DeleteBuildingError> {
        if let Some(err) = self.check_for_deleting_building(&request) {
            return Err(err);
        }

        let entry = self.buildings.entry(request.position);
        match request.heigth {
            Heigth::Ground => {
                entry.and_modify(|a| {
                    a.ground = None;
                });
            }
        };

        Ok(DeleteBuildingResponse::new())
    }

    fn check_for_deleting_building(
        self: &Self,
        request: &DeleteBuildingRequest,
    ) -> Option<DeleteBuildingError> {
        if !self.is_in_map(&request.position) {
            return Some(DeleteBuildingError::OutOfMap);
        }

        match (self.buildings.get(&request.position), &request.heigth) {
            (Some(v), Heigth::Ground) => {
                if v.ground.is_none() {
                    Some(DeleteBuildingError::NoBuildingFound)
                } else {
                    None
                }
            }
            (None, _) => Some(DeleteBuildingError::NoBuildingFound),
        }
    }

    fn get_snapshot(self: &Self) -> MapSnapshot {
        MapSnapshot::new(self.dim.clone(), self.buildings.clone())
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct MapSnapshot {
    dim: Point,
    buildings: HashMap<Point, BuildingsOnPoint>,
}

impl MapSnapshot {
    pub fn new(dim: Point, buildings: HashMap<Point, BuildingsOnPoint>) -> Self {
        Self { dim, buildings }
    }

    pub fn get_cell_at(self: &Self, point: &Point) -> CellDescriptor {
        let buildings_on_point = self.buildings.get(point);
        CellDescriptor {
            point: point.clone(),
            buildings_on_point,
        }
    }
}

#[cfg(test)]
mod test_matrix_map {
    use super::*;
    use crate::buildings::prototype::BuildingPrototypeType;

    #[test]
    fn check_for_adding_building_should_throw_if_point_is_out_of_the_map() {
        let map = MatrixMap::new(Point::new(20, 20));

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(50, 50),
            Heigth::Ground,
        );

        let result = map.check_for_adding_building(&request);

        assert_eq!(
            Some(AddBuildingError::OutOfMap),
            result,
            "Out of map points are not admitted"
        );
    }

    #[test]
    fn check_for_adding_building_should_throw_if_point_already_taken_by_another_building() {
        let mut map = MatrixMap::new(Point::new(20, 20));

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(1, 1),
            Heigth::Ground,
        );

        let result = map.add_building(request);
        assert_eq!(
            Ok(AddBuildingResponse::new(44)),
            result,
            "in-map points are admitted"
        );

        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(1, 1),
            Heigth::Ground,
        );
        let result = map.check_for_adding_building(&request);
        assert_eq!(
            Some(AddBuildingError::AlreadyTaken),
            result,
            "overlapped buildings are not admitted"
        );
    }

    #[test]
    fn check_for_deleting_building_should_throw_if_point_is_out_of_the_map() {
        let map = MatrixMap::new(Point::new(20, 20));
        let request = DeleteBuildingRequest::new(Point::new(50, 50), Heigth::Ground);

        let result = map.check_for_deleting_building(&request);

        assert_eq!(
            Some(DeleteBuildingError::OutOfMap),
            result,
            "Out of map points are not admitted"
        );
    }

    #[test]
    fn check_for_deleting_building_should_throw_if_point_is_not_already_taken() {
        let map = MatrixMap::new(Point::new(20, 20));

        let request = DeleteBuildingRequest::new(Point::new(0, 0), Heigth::Ground);
        let result = map.check_for_deleting_building(&request);
        assert_eq!(
            Some(DeleteBuildingError::NoBuildingFound),
            result,
            "cannot delete if building is not found"
        );
    }

    #[test]
    fn flow() {
        let mut map = MatrixMap::new(Point::new(20, 20));
        let request = AddBuildingRequest::new(
            BuildingPrototypeType::Street,
            Point::new(0, 0),
            Heigth::Ground,
        );

        map.add_building(request).unwrap();
        let snapshot = map.get_snapshot();
        let cell = snapshot.get_cell_at(&Point::new(0, 0));

        assert_eq!(
            cell.buildings_on_point.unwrap().ground.is_some(),
            true,
            "After add the cell should not be empty"
        );

        let request = DeleteBuildingRequest::new(Point::new(0, 0), Heigth::Ground);
        map.delete_building(request).unwrap();

        let snapshot = map.get_snapshot();
        let cell = snapshot.get_cell_at(&Point::new(0, 0));
        assert_eq!(
            cell.buildings_on_point.unwrap().ground.is_none(),
            true,
            "After deletion the cell should be empty"
        );
    }
}
