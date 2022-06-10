use bevy::prelude::*;

use crate::{
    building::Building,
    common::{configuration::CONFIGURATION, position::Position},
    GameTick,
};

use crate::building::plugin::BuildingCreatedEvent;

use super::navigator::{NavigationDescriptor, Navigator};

#[cfg(test)]
pub use components::*;

#[cfg(not(test))]
use components::*;

use events::*;

pub struct NavigatorPlugin;

impl Plugin for NavigatorPlugin {
    fn build(&self, app: &mut App) {
        let navigator = Navigator::new(Position { x: 0, y: 0 });
        app.insert_resource(navigator)
            .add_event::<InhabitantArrivedAtHomeEvent>()
            .add_system(new_building_created)
            .add_system_to_stage(CoreStage::Last, add_node)
            .add_system_to_stage(CoreStage::PreUpdate, handle_waiting_for_inhabitants)
            .add_system_to_stage(CoreStage::PreUpdate, move_inhabitants_to_house);
    }
}

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
            Building::Office(_o) => {}
            Building::Garden(_g) => {}
            Building::Street(_s) => {}
        }
    }
}

fn handle_waiting_for_inhabitants(
    mut game_events: EventReader<GameTick>,
    mut commands: Commands,
    navigator: Res<Navigator>,
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

        let delta = navigator.calculate_delta(waiting_for_inhabitants.count, &CONFIGURATION);

        info!("path ({navigation_descriptor}) found for house (id={entity:?}) at {position:?} for {delta} people");

        waiting_for_inhabitants.count -= delta;

        let mut command = commands.entity(entity);
        command.insert(NavigationDescriptorComponent(navigation_descriptor, delta));
    }
}

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
    }
}

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

pub mod events {
    use bevy::prelude::Entity;

    pub struct InhabitantArrivedAtHomeEvent {
        pub count: u8,
        pub entity: Entity,
    }
}

mod components {
    use bevy::prelude::Component;

    use crate::{common::position::Position, navigation::navigator::NavigationDescriptor};

    #[derive(Component)]
    pub struct HouseWaitingForInhabitantsComponent {
        pub count: u8,
        pub position: Position,
    }

    #[derive(Component)]
    pub struct OfficeWaitingForWorkersComponent;

    #[derive(Component)]
    pub struct NavigationDescriptorComponent(pub NavigationDescriptor, pub u8);
}
