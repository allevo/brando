use bevy::utils::HashSet;

use crate::building::{BuildRequest, BuildingInConstruction, House, ProgressStatus};

use crate::common::position::Position;
use crate::navigation::plugin::InhabitantArrivedAtHome;

pub struct BuildingBuilder {
    position_already_used: HashSet<Position>,
}

impl BuildingBuilder {
    pub fn new() -> Self {
        Self {
            position_already_used: Default::default(),
        }
    }

    pub fn create_building(
        &mut self,
        request: BuildRequest,
    ) -> Result<BuildingInConstruction, &'static str> {
        let position = request.position;
        if !self.position_already_used.insert(position) {
            return Err("Position already used");
        }

        let progress_status = ProgressStatus {
            current_step: 0,
            step_to_reach: request.prototype.time_for_building,
        };
        Ok(BuildingInConstruction {
            request,
            progress_status,
        })
    }

    pub fn make_progress(
        &self,
        in_progress: &mut BuildingInConstruction,
    ) -> Result<bool, &'static str> {
        let current_progress = in_progress.progress_status.clone().progress();
        in_progress.progress_status = current_progress;

        Ok(in_progress.progress_status.is_completed())
    }

    pub fn go_to_live_home(
        &self,
        house: &mut House,
        arrived: &InhabitantArrivedAtHome,
    ) -> Result<(), &'static str> {
        house.resident_property.current_residents += arrived.count;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Entity;

    use crate::{
        building::{Garden, Street, GARDEN_PROTOTYPE, HOUSE_PROTOTYPE, STREET_PROTOTYPE},
        common::position::Position,
        navigation::navigator::Navigator,
        palatability::manager::PalatabilityManager,
    };

    use super::*;

    #[test]
    fn flow() {
        let mut brando = BuildingBuilder::new();
        let mut navigator = Navigator::new(Position { x: 0, y: 0 });
        let mut palatability = PalatabilityManager::new();

        // this should be created on when the user clicks on the map
        let request = BuildRequest {
            position: Position { x: 1, y: 2 },
            prototype: &HOUSE_PROTOTYPE,
        };
        let mut house_in_construction = brando.create_building(request).unwrap();

        // This simulate the pass of the time
        while !house_in_construction.is_completed() {
            brando.make_progress(&mut house_in_construction).unwrap();
        }

        // if the house.is_completed returns true, we can convert it to house
        let mut house: House = (&mut house_in_construction).try_into().unwrap();

        // Now the user create some streets...
        let street_requests = vec![
            BuildRequest {
                position: Position { x: 0, y: 0 },
                prototype: &STREET_PROTOTYPE,
            },
            BuildRequest {
                position: Position { x: 1, y: 0 },
                prototype: &STREET_PROTOTYPE,
            },
            BuildRequest {
                position: Position { x: 1, y: 1 },
                prototype: &STREET_PROTOTYPE,
            },
        ];
        let mut streets_in_construction: Vec<_> = street_requests
            .into_iter()
            .map(|s| brando.create_building(s).unwrap())
            .collect();

        // Wait for a while...
        while streets_in_construction.iter().any(|r| !r.is_completed()) {
            streets_in_construction.iter_mut().for_each(|s| {
                brando.make_progress(s).unwrap();
            });
        }
        // And convert into streets
        let street: Vec<Street> = streets_in_construction
            .into_iter()
            .map(|mut s| (&mut s).try_into().unwrap())
            .collect();

        assert_eq!(house.resident_property.current_residents, 0);

        // if the building is a street we need also to update navigator
        street
            .into_iter()
            .for_each(|s| navigator.add_node(s.position));

        // In the meantime, under the hood, we need to build the graph behind
        while navigator.rebuild() > 0 {}

        // In order to attract people, we build a garden
        let garden = BuildRequest {
            position: Position { x: 2, y: 2 },
            prototype: &GARDEN_PROTOTYPE,
        };
        let mut garden_in_construction = brando.create_building(garden).unwrap();
        while !garden_in_construction.is_completed() {
            brando.make_progress(&mut garden_in_construction).unwrap();
        }
        let garden: Garden = (&mut garden_in_construction).try_into().unwrap();
        // If the building is a garden we need to invoke palatability
        palatability.add_house_source(&garden);

        let house_palatability = palatability.get_house_palatability(&house.position);
        assert_eq!(house_palatability.is_positive(), true);

        // Now, we are ready for asking to the navigator the descriptor
        let mut navigation_descriptor = navigator.get_navigation_descriptor(&house).unwrap();
        assert_eq!(
            vec![
                Position { x: 1, y: 2 },
                Position { x: 1, y: 1 },
                Position { x: 1, y: 0 },
                Position { x: 0, y: 0 },
            ],
            navigation_descriptor.path
        );

        // waiting for a while...
        while !navigation_descriptor.is_completed() {
            navigator.make_progress(&mut navigation_descriptor).unwrap();
        }

        let arrived = InhabitantArrivedAtHome {
            count: navigation_descriptor.count,
            entity: Entity::from_raw(0),
        };

        // and set the building is fully inhabited
        brando.go_to_live_home(&mut house, &arrived).unwrap();
        palatability.add_house_source(&house);
        palatability.increment_populations(arrived.count as i32);

        assert_eq!(palatability.total_populations(), 8);
    }
}
