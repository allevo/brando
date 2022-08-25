use bevy::prelude::*;

use crate::palatability::plugin::MoreWorkersNeeded;
use crate::{common::position::Position, palatability::plugin::MoreInhabitantsNeeded};

use crate::building::plugin::{BuildingCreatedEvent, BuildingSnapshot};

use super::{
    entity_storage::{AssignmentResult, BuildingNeedToBeFulfilled, EntityStorage},
    navigator::Navigator,
};

#[cfg(test)]
pub use components::*;

#[cfg(not(test))]
use components::*;

use events::*;

pub struct NavigatorPlugin;

impl Plugin for NavigatorPlugin {
    fn build(&self, app: &mut App) {
        let navigator = Navigator::new();

        app.insert_resource(navigator)
            .add_event::<InhabitantArrivedAtHomeEvent>()
            .add_event::<InhabitantFoundJobEvent>()
            // Probably we would like to create Vecs with already-preallocated capacity
            .insert_resource(EntityStorage::default())
            // .add_system(new_building_created)
            .add_system(expand_navigator_graph)
            .add_system(create_inhabitants)
            .add_system(find_houses_for_inhabitants)
            .add_system(find_job_for_inhabitants)
            .add_system(inhabitant_want_to_work);
        // .add_system(tag_inhabitants_for_waiting_for_work)
        // .add_system(assign_waiting_for)
        // .add_system_to_stage(CoreStage::Last, add_node)
        // .add_system_to_stage(CoreStage::PreUpdate, handle_waiting_for_inhabitants)
        // .add_system_to_stage(CoreStage::PreUpdate, move_inhabitants_to_target);
    }
}

fn expand_navigator_graph(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut commands: Commands,
    mut navigator: ResMut<Navigator>,
    mut entity_storage: ResMut<EntityStorage>,
) {
    let mut need_to_rebuild = false;
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
            BuildingSnapshot::Street(_) => {
                info!("adding node at {:?}", building_position);
                navigator.add_node(*building_position);

                need_to_rebuild = true;
            }
            BuildingSnapshot::Garden(_) => {}
        }
    }

    if need_to_rebuild {
        // TODO not here
        // probably this place is not so convenient and also not so convenient rebuild
        // every time the graph.
        navigator.rebuild();
    }
}

/// Create inhabitants
fn create_inhabitants(
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorage>,
    mut more_inhabitants_needed_reader: EventReader<MoreInhabitantsNeeded>,
) {
    let total: u32 = more_inhabitants_needed_reader
        .iter()
        .map(|e| e.count as u32)
        .sum();
    if total == 0 {
        return;
    }

    let position = Position { x: 0, y: 0 };

    for _ in 0..total {
        let entity = commands
            .spawn()
            .insert(InhabitantComponent { position })
            .id();

        entity_storage.introduce_inhabitants(entity);
    }
}

fn find_houses_for_inhabitants(
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorage>,
    navigator: Res<Navigator>,
    mut inhabitant_arrived_writer: EventWriter<InhabitantArrivedAtHomeEvent>,
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

        commands.entity(couple.from).insert(AssignedHouse {
            house_entity: couple.to,
            house_position: couple.to_position,
            navigation_descriptor,
        });

        inhabitant_arrived_writer.send(InhabitantArrivedAtHomeEvent {
            inhabitants_entities: vec![couple.from],
            building_entity: couple.to,
            house_position: couple.to_position,
        });

        commands.entity(couple.from).insert(WaitingForWorkComponent);

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
    mut commands: Commands,
    mut entity_storage: ResMut<EntityStorage>,
    navigator: Res<Navigator>,
    mut inhabitant_found_job_writer: EventWriter<InhabitantFoundJobEvent>,
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
                // Avoiding removing HouseWaitingForInhabitantsComponent we are processing again
                // every frame. So probably the best thing todo is to remove the component,
                // adding a dedicated new one that allow us to "wait" for a while before retrying
                None => {
                    entity_storage.resign_assign_result(couple);
                    continue;
                }
                Some(nd) => nd,
            };

        commands.entity(couple.from).insert(AssignedOffice {
            office_entity: couple.to,
            office_position: couple.to_position,
            navigation_descriptor,
        });

        inhabitant_found_job_writer.send(InhabitantFoundJobEvent {
            workers_entities: vec![couple.from],
            building_entity: couple.to,
        });

        commands
            .entity(couple.from)
            .remove::<WaitingForWorkComponent>();
    }
}

pub mod events {
    use bevy::prelude::Entity;

    use crate::common::position::Position;

    pub struct InhabitantArrivedAtHomeEvent {
        pub inhabitants_entities: Vec<Entity>,
        pub building_entity: Entity,
        pub house_position: Position,
    }

    pub struct InhabitantFoundJobEvent {
        pub workers_entities: Vec<Entity>,
        pub building_entity: Entity,
    }
}

mod components {
    use bevy::prelude::{Component, Entity};

    use crate::{common::position::Position, navigation::navigator::NavigationDescriptor};

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
    pub struct InhabitantsAssignedComponent {
        pub from: Vec<InhabitantAssigned>,
    }

    #[derive(Debug)]
    pub struct InhabitantAssigned {
        pub inhabitant: Entity,
        pub origin_position: Position,
    }

    #[derive(Component, Debug)]
    pub struct TargetComponent {
        // pub needed_count: usize,
        // pub origin_position: Position,
        pub target_position: Position,
        pub target_type: TargetType,
    }

    #[derive(Component)]
    pub struct NavigationDescriptorComponent {
        pub descriptor: NavigationDescriptor,
        pub entities_to_move: Vec<Entity>,
        pub target_type: TargetType,
    }

    #[derive(Component)]
    pub struct InhabitantComponent {
        pub position: Position,
    }

    #[derive(Component)]
    pub struct AssignedHouse {
        pub house_entity: Entity,
        pub house_position: Position,
        pub navigation_descriptor: NavigationDescriptor,
    }

    #[derive(Component)]
    pub struct AssignedOffice {
        pub office_entity: Entity,
        pub office_position: Position,
        pub navigation_descriptor: NavigationDescriptor,
    }

    #[derive(Component)]
    pub struct WaitingForWorkComponent;

    #[derive(Component)]
    pub struct MovableComponent;
}
