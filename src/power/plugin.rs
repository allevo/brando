use std::sync::Arc;

use bevy::prelude::*;

use crate::building::plugin::BuildingCreatedEvent;
use crate::common::configuration::Configuration;

pub use self::events::*;

use super::manager::PowerManager;

pub struct PowerPlugin;

impl Plugin for PowerPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &Arc<Configuration> = app.world.resource();
        let power_manager = PowerManager::new(configuration.clone());

        app.insert_resource(power_manager)
            // .add_event::<PowerCoveredBuildingEvent>()
            .add_system(register_power_consumers)
            .add_system(dedicate_power_to_consumers);
    }
}

fn register_power_consumers(
    mut power_manager: ResMut<PowerManager>,
    mut building_created: EventReader<BuildingCreatedEvent>,
) {
    for building_created_event in building_created.iter() {
        let building_snapshot = &building_created_event.building_snapshot;
        power_manager.register_power_consumer(building_snapshot);
        power_manager.register_power_source(building_snapshot);
    }
}

fn dedicate_power_to_consumers(mut power_manager: ResMut<PowerManager>, mut commands: Commands) {
    let covered_buildings = power_manager.dedicate_power_to_consumers();

    for building_id in covered_buildings {
        let entity = Entity::from_bits(building_id);
        commands.entity(entity).insert(PowerCoveredComponent);
    }

    /*
    // TODO: probably we can bulk this batch using one event
    let events = covered_buildings.into_iter()
        .map(|covered_building_id| PowerCoveredBuildingEvent { covered_building_id });
    power_covered_building_writer.send_batch(events);
    */
}

#[derive(Component)]
struct PowerCoveredComponent;

mod events {}
