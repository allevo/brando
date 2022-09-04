use std::sync::Arc;

use bevy::prelude::*;

use crate::common::configuration::Configuration;

use crate::GameTick;

use crate::building::plugin::{BuildingCreatedEvent, BuildingSnapshot};
use crate::navigation::plugin::events::InhabitantArrivedAtHomeEvent;

pub use self::events::*;

use super::manager::PalatabilityManager;

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &Arc<Configuration> = app.world.resource();
        let palatability = PalatabilityManager::new(configuration.clone());

        app.insert_resource(palatability)
            .add_event::<MoreInhabitantsNeeded>()
            .add_event::<MoreWorkersNeeded>()
            .add_system_to_stage(CoreStage::Last, increment_palatabilities)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house)
            .add_system(try_spawn_inhabitants)
            .add_system(try_spawn_workers)
            .add_system(increment_vacant_spot);
    }
}

fn increment_palatabilities(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut palatability: ResMut<PalatabilityManager>,
) {
    for building_created in building_created_reader.iter() {
        palatability.add_house_source(&building_created.building);
        palatability.add_office_source(&building_created.building);
    }
}

fn habit_house(
    mut inhabitant_arrived_writer: EventReader<InhabitantArrivedAtHomeEvent>,
    mut palatability: ResMut<PalatabilityManager>,
) {
    let inhabitants: Vec<_> = inhabitant_arrived_writer
        .iter()
        .flat_map(|a| a.inhabitants_entities.iter())
        .map(|e| (*e).to_bits())
        .collect();
    if inhabitants.is_empty() {
        return;
    }

    palatability.increment_vacant_inhabitants(-(inhabitants.len() as i32));
    palatability.add_unemployed_inhabitants(inhabitants);
}

fn try_spawn_inhabitants(
    mut game_tick: EventReader<GameTick>,
    mut palatability: ResMut<PalatabilityManager>,
    mut more_inhabitants_needed_writer: EventWriter<MoreInhabitantsNeeded>,
) {
    if game_tick.iter().count() == 0 {
        return;
    }

    let inhabitants_count = palatability.consume_inhabitants_to_spawn_and_increment_populations();
    if inhabitants_count != 0 {
        more_inhabitants_needed_writer.send(MoreInhabitantsNeeded {
            count: inhabitants_count,
        });

        let population = palatability.total_populations();
        info!("population count: {population:?}");
    }
}

fn try_spawn_workers(
    mut game_tick: EventReader<GameTick>,
    mut palatability: ResMut<PalatabilityManager>,
    mut more_workers_needed_writer: EventWriter<MoreWorkersNeeded>,
) {
    if game_tick.iter().count() == 0 {
        return;
    }

    let workers = palatability.consume_workers_to_spawn();
    if workers.is_empty() {
        return;
    }

    info!("worker {} to spawn", workers.len());
    let event = MoreWorkersNeeded { workers };

    more_workers_needed_writer.send(event);
}

fn increment_vacant_spot(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut palatability: ResMut<PalatabilityManager>,
) {
    for building_created in building_created_reader.iter() {
        match &building_created.building {
            BuildingSnapshot::House(house) => {
                // NB: `current_residents` is always 0 here
                let delta = house.resident_property.max_residents
                    - house.resident_property.current_residents;
                palatability.increment_vacant_inhabitants(delta as i32);
            }
            BuildingSnapshot::Office(office) => {
                // NB: `current_worker` is always 0 here
                let delta = office.work_property.max_worker - office.work_property.current_worker;
                palatability.increment_vacant_work(delta as i32);
            }
            BuildingSnapshot::Garden(_)
            | BuildingSnapshot::Street(_)
            | BuildingSnapshot::BiomassPowerPlant(_) => {}
        }
    }
}

mod events {
    use crate::common::EntityId;

    pub struct MoreInhabitantsNeeded {
        pub count: u8,
    }

    pub struct MoreWorkersNeeded {
        pub workers: Vec<EntityId>,
    }
}
