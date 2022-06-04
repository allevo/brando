use std::ops::{Deref, DerefMut};

use bevy::prelude::*;

use crate::{
    building::{Building, House},
    common::position::Position,
    GameTick,
};

use crate::building::plugin::{
    BuildingCreated, HouseComponent, HouseWaitingForInhabitantsComponent,
};

use super::navigator::{NavigationDescriptor, Navigator};

pub struct InhabitantArrivedAtHome {
    pub count: u8,
    pub entity: Entity,
}

pub struct NavigatorPlugin;

impl Plugin for NavigatorPlugin {
    fn build(&self, app: &mut App) {
        let navigator = Navigator::new(Position { x: 0, y: 0 });
        app.insert_resource(navigator)
            .add_event::<InhabitantArrivedAtHome>()
            .add_system_to_stage(CoreStage::Last, add_node)
            .add_system_to_stage(CoreStage::PreUpdate, handle_waiting_for_inhabitants)
            .add_system_to_stage(CoreStage::PreUpdate, move_inhabitants_to_house);
    }
}

#[derive(Component)]
struct NavigationDescriptorComponent(NavigationDescriptor);
impl Deref for NavigationDescriptorComponent {
    type Target = NavigationDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for NavigationDescriptorComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn handle_waiting_for_inhabitants(
    mut commands: Commands,
    mut navigator: ResMut<Navigator>,
    waiting_for_inhabitants_query: Query<
        (Entity, &HouseComponent),
        With<HouseWaitingForInhabitantsComponent>,
    >,
) {
    for (entity, house_component) in waiting_for_inhabitants_query.iter() {
        let house: &House = &*house_component;
        let navigation_descriptor = match navigator.get_navigation_descriptor(house) {
            // TODO consider to have a try not immediately
            // Avoiding removing HouseWaitingForInhabitantsComponent we are processing again
            // every frame. So probably the best thing todo is to remove the component,
            // adding a dedicated new one that allow us to "wait" for a while before retrying
            None => continue,
            Some(nd) => nd,
        };

        info!(
            "path ({}) found for entity id {entity:?}",
            navigation_descriptor.path.len()
        );

        commands
            .entity(entity)
            .remove::<HouseWaitingForInhabitantsComponent>()
            .insert(NavigationDescriptorComponent(navigation_descriptor));
    }
}

fn move_inhabitants_to_house(
    mut events: EventReader<GameTick>,
    mut commands: Commands,
    navigator: Res<Navigator>,
    mut waiting_for_inhabitants_query: Query<(Entity, &mut NavigationDescriptorComponent)>,
    mut inhabitant_arrived_writer: EventWriter<InhabitantArrivedAtHome>,
) {
    if events.iter().count() == 0 {
        return;
    }

    for (entity, mut navigation_descriptor_component) in waiting_for_inhabitants_query.iter_mut() {
        let navigation_descriptor: &mut NavigationDescriptor =
            &mut *navigation_descriptor_component;
        navigator.make_progress(navigation_descriptor).unwrap();

        if !navigation_descriptor.is_completed() {
            continue;
        }

        info!("navigation_descriptor ends!");

        commands
            .entity(entity)
            .remove::<NavigationDescriptorComponent>();

        inhabitant_arrived_writer.send(InhabitantArrivedAtHome {
            count: navigation_descriptor.count,
            entity,
        });
    }
}

fn add_node(
    mut navigator: ResMut<Navigator>,
    mut building_created_reader: EventReader<BuildingCreated>,
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
