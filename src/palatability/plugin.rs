use bevy::prelude::*;

use crate::building::BuildingSnapshot;

use crate::GameTick;

use crate::building::events::BuildingCreatedEvent;
use crate::inhabitant::events::HomeAssignedToInhabitantEvent;
use crate::resources::ConfigurationResource;

pub use self::events::*;
pub use self::resources::*;

use super::manager::PalatabilityManager;

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &ConfigurationResource = app.world.resource();
        let palatability = PalatabilityManager::new((*configuration).clone());

        app.insert_resource(PalatabilityManagerResource(palatability))
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
    mut palatability: ResMut<PalatabilityManagerResource>,
) {
    for building_created in building_created_reader.iter() {
        palatability.add_palatability_source(&building_created.building_snapshot);
    }
}

fn habit_house(
    mut inhabitant_arrived_writer: EventReader<HomeAssignedToInhabitantEvent>,
    mut palatability: ResMut<PalatabilityManagerResource>,
) {
    let inhabitants: Vec<_> = inhabitant_arrived_writer
        .iter()
        .flat_map(|a| a.inhabitants_entity_ids.iter())
        .copied()
        .collect();
    if inhabitants.is_empty() {
        return;
    }

    palatability.increment_vacant_inhabitants(-(inhabitants.len() as i32));
    palatability.add_unemployed_inhabitants(inhabitants);
}

fn try_spawn_inhabitants(
    mut game_tick: EventReader<GameTick>,
    mut palatability: ResMut<PalatabilityManagerResource>,
    mut more_inhabitants_needed_writer: EventWriter<MoreInhabitantsNeeded>,
) {
    if game_tick.iter().count() == 0 {
        return;
    }

    let inhabitants_to_spawn =
        palatability.consume_inhabitants_to_spawn_and_increment_populations();
    if !inhabitants_to_spawn.is_empty() {
        more_inhabitants_needed_writer.send(MoreInhabitantsNeeded {
            inhabitants_to_spawn,
        });

        let population = palatability.total_populations();
        info!("population count: {population:?}");
    }
}

fn try_spawn_workers(
    mut game_tick: EventReader<GameTick>,
    mut palatability: ResMut<PalatabilityManagerResource>,
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
    mut palatability: ResMut<PalatabilityManagerResource>,
) {
    for building_created in building_created_reader.iter() {
        match &building_created.building_snapshot {
            BuildingSnapshot::House(house) => {
                // NB: `current_residents` is always 0 here
                let delta = house.max_residents - house.current_residents;
                palatability.increment_vacant_inhabitants(delta as i32);
            }
            BuildingSnapshot::Office(office) => {
                // NB: `current_worker` is always 0 here
                let delta = office.max_workers - office.current_workers;
                palatability.increment_vacant_work(delta as i32);
            }
            BuildingSnapshot::Garden(_)
            | BuildingSnapshot::Street(_)
            | BuildingSnapshot::BiomassPowerPlant(_) => {}
        }
    }
}

mod resources {
    use std::ops::{Deref, DerefMut};

    use bevy::prelude::Resource;

    use crate::palatability::manager::PalatabilityManager;

    #[derive(Resource)]
    pub struct PalatabilityManagerResource(pub PalatabilityManager);

    impl Deref for PalatabilityManagerResource {
        type Target = PalatabilityManager;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for PalatabilityManagerResource {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

mod events {
    use crate::{common::EntityId, palatability::manager::InhabitantToSpawn};

    pub struct MoreInhabitantsNeeded {
        pub inhabitants_to_spawn: Vec<InhabitantToSpawn>,
    }

    pub struct MoreWorkersNeeded {
        pub workers: Vec<EntityId>,
    }
}
