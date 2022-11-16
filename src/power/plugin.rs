use bevy::prelude::*;

use crate::building::events::BuildingCreatedEvent;
use crate::inhabitant::events::{HomeAssignedToInhabitantEvent, JobAssignedToInhabitantEvent};
use crate::resources::ConfigurationResource;

use self::components::PowerCoveredComponent;
pub use self::events::*;
use self::resources::PowerManagerResource;

use super::manager::PowerManager;

pub struct PowerPlugin;

impl Plugin for PowerPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &ConfigurationResource = app.world.resource();
        let power_manager = PowerManager::new((*configuration).clone());

        app.insert_resource(PowerManagerResource(power_manager))
            // .add_event::<PowerCoveredBuildingEvent>()
            .add_system(register_power_consumers)
            .add_system(dedicate_power_to_consumers)
            .add_system(increment_power_consumption);
    }
}

fn register_power_consumers(
    mut power_manager: ResMut<PowerManagerResource>,
    mut building_created: EventReader<BuildingCreatedEvent>,
) {
    for building_created_event in building_created.iter() {
        let building_snapshot = &building_created_event.building_snapshot;
        power_manager.register_power_consumer(building_snapshot);
        power_manager.register_power_source(building_snapshot);
    }
}

fn increment_power_consumption(
    mut power_manager: ResMut<PowerManagerResource>,
    mut home_assigned_to_inhabitant_event_reader: EventReader<HomeAssignedToInhabitantEvent>,
    mut job_assigned_to_inhabitant_event_reader: EventReader<JobAssignedToInhabitantEvent>,
) {
    for event in home_assigned_to_inhabitant_event_reader.iter() {
        let house_id = event.building_entity_id;
        let delta_count = u32::try_from(event.inhabitants_entity_ids.len()).unwrap();
        power_manager.register_new_inhabitants_at_home(house_id, delta_count);
    }

    for event in job_assigned_to_inhabitant_event_reader.iter() {
        let house_id = event.building_entity_id;
        let delta_count = u32::try_from(event.workers_entity_ids.len()).unwrap();
        power_manager.register_new_inhabitants_at_home(house_id, delta_count);
    }
}

fn dedicate_power_to_consumers(
    mut power_manager: ResMut<PowerManagerResource>,
    mut commands: Commands,
) {
    let covered_buildings = power_manager.dedicate_power_to_consumers();

    for building_id in covered_buildings.consumers.keys() {
        let entity = Entity::from_bits(*building_id);
        commands.entity(entity).insert(PowerCoveredComponent);
    }

    /*
    // TODO: probably we can bulk this batch using one event
    let events = covered_buildings.consumers.iter()
        .map(|covered_building_id| PowerCoveredBuildingEvent { covered_building_id });
    power_covered_building_writer.send_batch(events);
    */
}

mod resources {
    use std::ops::{Deref, DerefMut};

    use bevy::prelude::Resource;

    use crate::power::manager::PowerManager;

    #[derive(Resource)]
    pub struct PowerManagerResource(pub PowerManager);

    impl Deref for PowerManagerResource {
        type Target = PowerManager;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for PowerManagerResource {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

mod components {
    use bevy::prelude::Component;

    #[derive(Component)]
    pub struct PowerCoveredComponent;
}

mod events {}
