use bevy::prelude::*;

use crate::{
    building::{events::BuildingCreatedEvent, BuildingSnapshot},
    common::{position::Position, EntityId},
    navigation::NavigatorResource,
    palatability::{MoreInhabitantsNeeded, MoreWorkersNeeded, PalatabilityManagerResource},
};

use super::{
    entity_storage::{AssignmentResult, BuildingNeedToBeFulfilled, EntityStorage},
    inhabitant_entity::Inhabitant,
    manager::InhabitantManager,
};

use self::{components::*, resources::*};
use events::*;

pub struct InhabitantPlugin;

impl Plugin for InhabitantPlugin {
    fn build(&self, app: &mut App) {
        let manager = InhabitantManagerResource(InhabitantManager::new());

        app.insert_resource(manager)
            .add_event::<HomeAssignedToInhabitantEvent>()
            .add_event::<JobAssignedToInhabitantEvent>()
            // Probably we would like to create Vecs with already-preallocated capacity
            .insert_resource(EntityStorageResource(EntityStorage::default()))
            .add_system(register_target)
            .add_system(create_inhabitants)
            .add_system(find_houses_for_inhabitants)
            .add_system(find_job_for_inhabitants)
            .add_system(inhabitant_want_to_work);
    }
}

fn register_target(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorageResource>,
) {
    for created_building in building_created_reader.iter() {
        let building_position: &Position = created_building.building_snapshot.get_position();
        let building_entity_id: &EntityId = created_building.building_snapshot.get_id();
        let entity = Entity::from_bits(*building_entity_id);

        match &created_building.building_snapshot {
            BuildingSnapshot::House(house) => {
                commands
                    .entity(entity)
                    .insert(TargetComponent {
                        target_position: *building_position,
                        target_type: TargetType::House,
                    })
                    .insert(TargetTypeHouse);

                info!("Register house");
                entity_storage.register_house(BuildingNeedToBeFulfilled::new(
                    entity.to_bits(),
                    *building_position,
                    house.max_residents,
                ));
            }
            BuildingSnapshot::Office(office) => {
                commands
                    .entity(entity)
                    .insert(TargetComponent {
                        target_position: *building_position,
                        target_type: TargetType::Office,
                    })
                    .insert(TargetTypeOffice);

                info!("Register office");
                entity_storage.register_office(BuildingNeedToBeFulfilled::new(
                    entity.to_bits(),
                    *building_position,
                    office.max_workers,
                ));
            }
            BuildingSnapshot::Street(_) => {}
            BuildingSnapshot::Garden(_) => {}
            BuildingSnapshot::BiomassPowerPlant(_) => {}
        }
    }
}

/// Create inhabitants
fn create_inhabitants(
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorageResource>,
    mut more_inhabitants_needed_reader: EventReader<MoreInhabitantsNeeded>,
    _palatability_manager: Res<PalatabilityManagerResource>,
) {
    // TODO: for the time being we consider the origin as the:
    // - origin
    // - unique point that the inhabitants came from
    let _position = Position { x: 0, y: 0 };

    // let palatability_manager: &PalatabilityManager = &*palatability_manager;

    let total = more_inhabitants_needed_reader
        .iter()
        .flat_map(|e| e.inhabitants_to_spawn.iter());

    for inhabitant_to_spawn in total {
        let entity = commands.spawn_empty().insert(InhabitantComponent).id();

        let inhabitant = Inhabitant::new(entity.to_bits(), inhabitant_to_spawn.education_level);

        entity_storage.introduce_inhabitant(inhabitant);
    }
}

fn find_houses_for_inhabitants(
    mut entity_storage: ResMut<EntityStorageResource>,
    navigator: Res<NavigatorResource>,
    mut inhabitant_arrived_writer: EventWriter<HomeAssignedToInhabitantEvent>,
) {
    let couples: Vec<AssignmentResult> = entity_storage.get_inhabitant_house_assignment();

    if couples.is_empty() {
        return;
    }

    info!("inhabitants-houses assignments {}", couples.len());

    for couple in couples {
        let _navigation_descriptor =
            match navigator.get_navigation_descriptor(&couple.from_position, couple.to_position) {
                // TODO consider to have a try not immediately
                // Avoiding removing HouseWaitingForInhabitantsComponent we are processing again
                // every frame. So probably the best thing todo is to remove the component,
                // adding a dedicated new one that allow us to "wait" for a while before retrying
                None => {
                    entity_storage.resign_assign_result(couple);
                    continue;
                }
                Some(nd) => nd,
            };

        inhabitant_arrived_writer.send(HomeAssignedToInhabitantEvent {
            inhabitants_entity_ids: vec![couple.from],
            building_entity_id: couple.to,
            house_position: couple.to_position,
        });

        entity_storage.found_home_for_inhabitant(&couple.from, couple.to, couple.to_position);
    }
}

fn inhabitant_want_to_work(
    mut more_workers_needed_reader: EventReader<MoreWorkersNeeded>,
    mut entity_storage: ResMut<EntityStorageResource>,
) {
    let entity_ids = more_workers_needed_reader
        .iter()
        .flat_map(|e| e.workers.iter());

    for entity_id in entity_ids {
        entity_storage.register_unemployee(*entity_id);
    }
}

fn find_job_for_inhabitants(
    mut entity_storage: ResMut<EntityStorageResource>,
    navigator: Res<NavigatorResource>,
    mut inhabitant_found_job_writer: EventWriter<JobAssignedToInhabitantEvent>,
) {
    let couples: Vec<AssignmentResult> = entity_storage.get_inhabitant_job_assignment();

    if couples.is_empty() {
        return;
    }

    info!("inhabitants-office assignments {}", couples.len());

    for couple in couples {
        let _navigation_descriptor =
            match navigator.get_navigation_descriptor(&couple.from_position, couple.to_position) {
                // TODO consider to have a try not immediately
                // "resign_assign_result" re-insert the couple inside an internal queue
                // "resign_assign_result" can track this failure and avoid to propose again
                // the same queue again and again
                None => {
                    entity_storage.resign_assign_result(couple);
                    continue;
                }
                Some(nd) => nd,
            };

        inhabitant_found_job_writer.send(JobAssignedToInhabitantEvent {
            workers_entity_ids: vec![couple.from],
            building_entity_id: couple.to,
        });

        entity_storage.found_job_for_unemployee(&couple.from, couple.to, couple.to_position);
    }
}

mod resources {
    use std::ops::{Deref, DerefMut};

    use bevy::prelude::Resource;

    use crate::inhabitant::{entity_storage::EntityStorage, manager::InhabitantManager};

    #[derive(Resource)]
    pub struct EntityStorageResource(pub EntityStorage);

    impl Deref for EntityStorageResource {
        type Target = EntityStorage;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for EntityStorageResource {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    #[derive(Resource)]
    pub struct InhabitantManagerResource(pub InhabitantManager);

    impl Deref for InhabitantManagerResource {
        type Target = InhabitantManager;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for InhabitantManagerResource {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

pub mod events {

    use crate::common::{position::Position, EntityId};

    pub struct HomeAssignedToInhabitantEvent {
        pub inhabitants_entity_ids: Vec<EntityId>,
        pub building_entity_id: EntityId,
        pub house_position: Position,
    }

    pub struct JobAssignedToInhabitantEvent {
        pub workers_entity_ids: Vec<EntityId>,
        pub building_entity_id: EntityId,
    }
}

mod components {
    use bevy::prelude::Component;

    use crate::common::position::Position;

    #[derive(Component)]
    pub struct InhabitantComponent;

    #[derive(Copy, Clone, Debug)]
    pub enum TargetType {
        Office,
        House,
    }

    #[derive(Component, Copy, Clone, Debug)]
    pub struct TargetTypeOffice;

    #[derive(Component, Copy, Clone, Debug)]
    pub struct TargetTypeHouse;

    #[derive(Component, Debug)]
    pub struct TargetComponent {
        // pub needed_count: usize,
        // pub origin_position: Position,
        pub target_position: Position,
        pub target_type: TargetType,
    }
}
