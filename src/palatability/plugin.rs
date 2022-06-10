use bevy::prelude::*;

use crate::navigation::plugin::events::InhabitantArrivedAtHomeEvent;

use crate::building::plugin::BuildingCreatedEvent;

use super::manager::PalatabilityManager;

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let palatability = PalatabilityManager::new();

        app.insert_resource(palatability)
            .add_system_to_stage(CoreStage::Last, increment_palatabilities)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house);
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

    palatability.increment_populations(count as i32);

    let population = palatability.total_populations();
    info!("population count: {population:?}");
}
