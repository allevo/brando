use bevy::prelude::*;

use crate::{common::position::Position, palatability::{plugin::{MoreInhabitantsNeeded, MoreWorkersNeeded}, manager::PalatabilityManager}, building::plugin::{BuildingSnapshot, BuildingCreatedEvent}, navigation::navigator::Navigator};

use super::{manager::InhabitantManager, entity_storage::{EntityStorage, AssignmentResult, BuildingNeedToBeFulfilled}};


use self::components::*;
pub use events::*;

pub struct InhabitantPlugin;


impl Plugin for InhabitantPlugin {
    fn build(&self, app: &mut App) {
        let manager = InhabitantManager::new();

        app
            .insert_resource(manager)
            
            .add_event::<HomeAssignedToInhabitantEvent>()
            .add_event::<JobAssignedToInhabitantEvent>()
            // Probably we would like to create Vecs with already-preallocated capacity
            .insert_resource(EntityStorage::default())
            .add_system(register_target)
            .add_system(create_inhabitants)
            .add_system(find_houses_for_inhabitants)
            .add_system(find_job_for_inhabitants)
            .add_system(inhabitant_want_to_work)
            ;
    }
}


fn register_target(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorage>,
) {
    for created_building in building_created_reader.iter() {
        let building_position: &Position = &created_building.position;
        let building_entity: Entity = created_building.building_entity;

        match &created_building.building {
            BuildingSnapshot::House(house) => {
                commands
                    .entity(building_entity)
                    .insert(TargetComponent {
                        target_position: *building_position,
                        target_type: TargetType::House,
                    })
                    .insert(TargetTypeHouse);

                info!("Register house");
                entity_storage.register_house(BuildingNeedToBeFulfilled::new(
                    building_entity,
                    *building_position,
                    house.resident_property.max_residents,
                ));
            }
            BuildingSnapshot::Office(office) => {
                commands
                    .entity(building_entity)
                    .insert(TargetComponent {
                        target_position: *building_position,
                        target_type: TargetType::Office,
                    })
                    .insert(TargetTypeOffice);

                info!("Register office");
                entity_storage.register_office(BuildingNeedToBeFulfilled::new(
                    building_entity,
                    *building_position,
                    office.work_property.max_worker,
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
    mut entity_storage: ResMut<EntityStorage>,
    mut more_inhabitants_needed_reader: EventReader<MoreInhabitantsNeeded>,
    mut palatability_manager: Res<PalatabilityManager>,
) {    
    // TODO: for the time being we consider the origin as the:
    // - origin
    // - unique point that the inhabitants came from
    let position = Position { x: 0, y: 0 };

    // let palatability_manager: &PalatabilityManager = &*palatability_manager;

    let total = more_inhabitants_needed_reader
        .iter()
        .flat_map(|e| e.inhabitants_to_spawn.iter());
    
    for inhabitant_to_spawn in total {
        let entity = commands
            .spawn()
            .insert(InhabitantComponent)
            .id();

        entity_storage.introduce_inhabitant(entity, inhabitant_to_spawn.education_level);
    }
}

fn find_houses_for_inhabitants(
    mut entity_storage: ResMut<EntityStorage>,
    navigator: Res<Navigator>,
    mut inhabitant_arrived_writer: EventWriter<HomeAssignedToInhabitantEvent>,
) {
    let couples: Vec<AssignmentResult> = entity_storage.get_inhabitant_house_assignment();

    if couples.is_empty() {
        return;
    }

    info!("inhabitants-houses assignments {}", couples.len());

    for couple in couples {
        let navigation_descriptor =
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

        /*
        commands.entity(couple.from).insert(AssignedHouse {
            house_entity: couple.to,
            house_position: couple.to_position,
            navigation_descriptor,
        });
        */

        inhabitant_arrived_writer.send(HomeAssignedToInhabitantEvent {
            inhabitants_entities: vec![couple.from],
            building_entity: couple.to,
            house_position: couple.to_position,
        });

        // commands.entity(couple.from).insert(WaitingForWorkComponent);

        entity_storage.set_inhabitant_house_position(couple.from, couple.to_position);
    }
}

fn inhabitant_want_to_work(
    mut more_workers_needed_reader: EventReader<MoreWorkersNeeded>,
    mut entity_storage: ResMut<EntityStorage>,
) {
    let entity_ids = more_workers_needed_reader
        .iter()
        .flat_map(|e| e.workers.iter());

    for entity_id in entity_ids {
        let entity = Entity::from_bits(*entity_id);
        entity_storage.register_unemployee(entity);
    }
}

fn find_job_for_inhabitants(
    mut entity_storage: ResMut<EntityStorage>,
    navigator: Res<Navigator>,
    mut inhabitant_found_job_writer: EventWriter<JobAssignedToInhabitantEvent>,
) {
    let couples: Vec<AssignmentResult> = entity_storage.get_inhabitant_job_assignment();

    if couples.is_empty() {
        return;
    }

    info!("inhabitants-office assignments {}", couples.len());

    for couple in couples {
        let navigation_descriptor =
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

        /*
        commands.entity(couple.from).insert(AssignedOffice {
            office_entity: couple.to,
            office_position: couple.to_position,
            navigation_descriptor,
        });
        */

        inhabitant_found_job_writer.send(JobAssignedToInhabitantEvent {
            workers_entities: vec![couple.from],
            building_entity: couple.to,
        });

        /*
        commands
            .entity(couple.from)
            .remove::<WaitingForWorkComponent>();
        */
    }
}

pub mod events {
    use bevy::prelude::Entity;

    use crate::common::position::Position;

    pub struct HomeAssignedToInhabitantEvent {
        pub inhabitants_entities: Vec<Entity>,
        pub building_entity: Entity,
        pub house_position: Position,
    }

    pub struct JobAssignedToInhabitantEvent {
        pub workers_entities: Vec<Entity>,
        pub building_entity: Entity,
    }
}


mod components {
    use bevy::prelude::{Component};

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