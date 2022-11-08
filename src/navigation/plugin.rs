use bevy::prelude::*;

use crate::building::BuildingSnapshot;
use crate::common::position::Position;

use crate::building::plugin::BuildingCreatedEvent;

use super::navigator::Navigator;

#[cfg(test)]
pub use components::*;

pub struct NavigatorPlugin;

impl Plugin for NavigatorPlugin {
    fn build(&self, app: &mut App) {
        let navigator = Navigator::new();

        app.insert_resource(navigator)
            // .add_system(new_building_created)
            .add_system(expand_navigator_graph);
        // .add_system(tag_inhabitants_for_waiting_for_work)
        // .add_system(assign_waiting_for)
        // .add_system_to_stage(CoreStage::Last, add_node)
        // .add_system_to_stage(CoreStage::PreUpdate, handle_waiting_for_inhabitants)
        // .add_system_to_stage(CoreStage::PreUpdate, move_inhabitants_to_target);
    }
}

fn expand_navigator_graph(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut navigator: ResMut<Navigator>,
) {
    let mut need_to_rebuild = false;
    for created_building in building_created_reader.iter() {
        let building_position: &Position = created_building.building_snapshot.get_position();

        match &created_building.building_snapshot {
            BuildingSnapshot::Street(_) => {
                info!("adding node at {:?}", building_position);
                navigator.add_node(*building_position);

                need_to_rebuild = true;
            }
            BuildingSnapshot::House(_) => {}
            BuildingSnapshot::Office(_) => {}
            BuildingSnapshot::Garden(_) => {}
            BuildingSnapshot::BiomassPowerPlant(_) => {}
        }
    }

    if need_to_rebuild {
        // TODO not here
        // probably this place is not so convenient and also not so convenient rebuild
        // every time the graph.
        navigator.rebuild();
    }
}

mod components {
    use bevy::prelude::{Component, Entity};

    use crate::{common::position::Position, navigation::navigator::NavigationDescriptor};

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
}
