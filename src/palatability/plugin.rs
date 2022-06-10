use bevy::prelude::*;

use crate::building::Building;
use crate::navigation::plugin::events::InhabitantArrivedAtHomeEvent;
use crate::GameTick;

use crate::building::plugin::BuildingCreatedEvent;

pub use self::events::*;

use super::manager::PalatabilityManager;

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let palatability = PalatabilityManager::new();

        app.insert_resource(palatability)
            .add_event::<MoreInhabitantsNeeded>()
            .add_system_to_stage(CoreStage::Last, increment_palatabilities)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house)
            .add_system(try_spawn_inhabitants)
            .add_system(listen_building_created);
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
    let count: u8 = inhabitant_arrived_writer.iter().map(|a| a.count).sum();
    if count == 0 {
        return;
    }

    palatability.increment_unemployed_inhabitants(count as i32);
    palatability.increment_vacant_inhabitants(-(count as i32));
}

fn try_spawn_inhabitants(
    mut game_tick: EventReader<GameTick>,
    mut palatability: ResMut<PalatabilityManager>,
    mut more_inhabitants_needed_writer: EventWriter<MoreInhabitantsNeeded>,
) {
    if game_tick.iter().count() == 0 {
        return;
    }

    let inhabitants_count = palatability.get_inhabitants_to_spawn_and_increment_populations();
    if inhabitants_count == 0 {
        return;
    }

    more_inhabitants_needed_writer.send(MoreInhabitantsNeeded {
        count: inhabitants_count,
    });

    let population = palatability.total_populations();
    info!("population count: {population:?}");
}

fn listen_building_created(
    mut building_created_reader: EventReader<BuildingCreatedEvent>,
    mut palatability: ResMut<PalatabilityManager>,
) {
    for building_created in building_created_reader.iter() {
        match &building_created.building {
            Building::House(house) => {
                let delta = house.resident_property.max_residents
                    - house.resident_property.current_residents;
                palatability.increment_vacant_inhabitants(delta as i32);
            }
            Building::Office(office) => {
                let delta = office.work_property.max_worker - office.work_property.current_worker;
                palatability.increment_vacant_work(delta as i32);
            }
            Building::Garden(_) | Building::Street(_) => {}
        }
    }
}

mod events {
    pub struct MoreInhabitantsNeeded {
        pub count: u8,
    }
}
