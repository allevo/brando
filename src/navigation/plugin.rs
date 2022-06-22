use bevy::prelude::*;

use crate::{
    building::Building,
    common::{configuration::CONFIGURATION, position::Position},
    palatability::plugin::MoreInhabitantsNeeded,
    GameTick,
};

use crate::building::plugin::BuildingCreatedEvent;

use super::navigator::{NavigationDescriptor, Navigator};

#[cfg(test)]
pub use components::*;

#[cfg(not(test))]
use components::*;

use events::*;

#[derive(Default)]
struct WaitingForStorage {
    house: Vec<Entity>,
}

pub struct NavigatorPlugin;

impl Plugin for NavigatorPlugin {
    fn build(&self, app: &mut App) {
        let navigator = Navigator::new(Position { x: 0, y: 0 });
        app.insert_resource(navigator)
            .add_event::<InhabitantArrivedAtHomeEvent>()
            // Probably we would like to create Vecs with already-preallocated capacity
            .insert_resource(WaitingForStorage::default())
            .add_system(new_building_created)
            .add_system(spawn_inhabitants)
            .add_system_to_stage(CoreStage::Last, add_node)
            .add_system_to_stage(CoreStage::PreUpdate, handle_waiting_for_inhabitants)
            .add_system_to_stage(CoreStage::PreUpdate, move_inhabitants_to_house);
    }
}

/// Flags buildings are receivers or donors of something
fn new_building_created(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut commands: Commands,
) {
    for created_building in building_created_reader.iter() {
        match &created_building.building {
            Building::House(house) => {
                let desired_residents = house.resident_property.max_residents;
                let position = house.position;

                let mut command = commands.entity(created_building.entity);
                command.insert(HouseWaitingForInhabitantsComponent {
                    count: desired_residents,
                    position,
                });
            }
            Building::Office(office) => {
                let position = office.position;
                let mut command = commands.entity(created_building.entity);
                command.insert(OfficeWaitingForWorkersComponent { position });
            }
            Building::Garden(_g) => {}
            Building::Street(_s) => {}
        }
    }
}

/// Try to find a good path for not-full houses
fn handle_waiting_for_inhabitants(
    mut game_events: EventReader<GameTick>,
    mut commands: Commands,
    navigator: Res<Navigator>,
    mut waiting_for_storage: ResMut<WaitingForStorage>,
    mut waiting_for_inhabitants_query: Query<
        (Entity, &mut HouseWaitingForInhabitantsComponent),
        (Without<NavigationDescriptorComponent>,),
    >,
) {
    if game_events.iter().count() == 0 {
        return;
    }

    for (entity, mut waiting_for_inhabitants) in waiting_for_inhabitants_query.iter_mut() {
        if waiting_for_inhabitants.count == 0 {
            let mut command = commands.entity(entity);
            command.remove::<HouseWaitingForInhabitantsComponent>();
            continue;
        }

        let position = waiting_for_inhabitants.position;
        let navigation_descriptor = match navigator.get_navigation_descriptor(position) {
            // TODO consider to have a try not immediately
            // Avoiding removing HouseWaitingForInhabitantsComponent we are processing again
            // every frame. So probably the best thing todo is to remove the component,
            // adding a dedicated new one that allow us to "wait" for a while before retrying
            None => continue,
            Some(nd) => nd,
        };

        // Delta cannot be negative (and probably neither zero)
        // We would like to force it to be `u8` forcing the type here
        // We need also be carefull about how much inhabitant we have available here!!
        let delta: u8 = navigator
            .calculate_delta(waiting_for_inhabitants.count, &CONFIGURATION)
            .min(waiting_for_storage.house.len() as u8);
        if delta == 0 {
            warn!("The calculated delta is 0: skipped");
            continue;
        }

        info!("path ({navigation_descriptor}) found for house (id={entity:?}) at {position:?} for {delta} people");

        waiting_for_inhabitants.count -= delta;

        // Consume inhabitants
        let inhabitants_to_move: Vec<_> =
            waiting_for_storage.house.drain(0..delta as usize).collect();
        for inhabitant_to_move in &inhabitants_to_move {
            commands
                .entity(*inhabitant_to_move)
                .remove::<WaitingForHomeComponent>();
        }

        let mut command = commands.entity(entity);
        command.insert(NavigationDescriptorComponent(
            navigation_descriptor,
            delta,
            inhabitants_to_move,
        ));
    }
}

/// Track progress for inhabitants that have a house as target
fn move_inhabitants_to_house(
    mut game_tick: EventReader<GameTick>,
    mut commands: Commands,
    navigator: Res<Navigator>,
    mut waiting_for_inhabitants_query: Query<(Entity, &mut NavigationDescriptorComponent)>,
    mut inhabitant_arrived_writer: EventWriter<InhabitantArrivedAtHomeEvent>,
) {
    if game_tick.iter().count() == 0 {
        return;
    }

    for (entity, mut navigation_descriptor_component) in waiting_for_inhabitants_query.iter_mut() {
        let navigation_descriptor: &mut NavigationDescriptor =
            &mut navigation_descriptor_component.0;

        // TODO Move inhabitants also
        navigator.make_progress(navigation_descriptor);

        if !navigation_descriptor.is_completed() {
            continue;
        }

        info!("navigation_descriptor ends!");

        commands
            .entity(entity)
            .remove::<NavigationDescriptorComponent>();

        inhabitant_arrived_writer.send(InhabitantArrivedAtHomeEvent {
            count: navigation_descriptor_component.1,
            entity,
        });

        for inhabitant in &navigation_descriptor_component.2 {
            commands.entity(*inhabitant).insert(WaitingForWorkComponent);
        }
    }
}

/// Add a node to the street graph
fn add_node(
    mut navigator: ResMut<Navigator>,
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
) {
    let streets_created = building_created_reader
        .iter()
        .filter(|bc| matches!(bc.building, Building::Street(_)));

    for street_created in streets_created {
        let position = street_created
            .building
            .position()
            .expect("street has always position");
        info!("adding node at {:?}", position);
        navigator.add_node(position);
    }

    // TODO not here
    // probably this place is not so convenient and also not so convenient rebuild
    // every time the graph.
    navigator.rebuild();
}

/// Spawn inhabitants that needs house
fn spawn_inhabitants(
    mut commands: Commands,
    mut waiting_for_storage: ResMut<WaitingForStorage>,
    mut more_inhabitants_needed_reader: EventReader<MoreInhabitantsNeeded>,
) {
    let total: u32 = more_inhabitants_needed_reader
        .iter()
        .map(|e| e.count as u32)
        .sum();
    if total == 0 {
        return;
    }

    for _ in 0..total {
        let entity = commands
            .spawn()
            .insert(InhabitantComponent {
                position: Position { x: 0, y: 0 },
            })
            .insert(WaitingForHomeComponent)
            .id();
        waiting_for_storage.house.push(entity);
    }

    info!(
        "waiting for house {} (newer: {total})",
        waiting_for_storage.house.len()
    );
}

pub mod events {
    use bevy::prelude::Entity;

    pub struct InhabitantArrivedAtHomeEvent {
        pub count: u8,
        pub entity: Entity,
    }
}

mod components {
    use bevy::prelude::{Component, Entity};

    use crate::{common::position::Position, navigation::navigator::NavigationDescriptor};

    #[derive(Component)]
    pub struct HouseWaitingForInhabitantsComponent {
        pub count: u8,
        pub position: Position,
    }

    #[derive(Component)]
    pub struct OfficeWaitingForWorkersComponent {
        pub position: Position,
    }

    #[derive(Component)]
    pub struct NavigationDescriptorComponent(pub NavigationDescriptor, pub u8, pub Vec<Entity>);

    #[derive(Component)]
    pub struct InhabitantComponent {
        pub position: Position,
    }

    #[derive(Component)]
    pub struct WaitingForHomeComponent;
    #[derive(Component)]
    pub struct WaitingForWorkComponent;
}
