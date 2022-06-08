use bevy::prelude::*;

use crate::palatability::manager::{HouseSourcePalatabilityDescriptor, OfficeSourcePalatabilityDescriptor, ToHouseSourcePalatabilityDescriptor, ToOfficeSourcePalatabilityDescriptor};
use crate::{navigation::plugin::InhabitantArrivedAtHome};

use crate::building::plugin::BuildingCreated;

use super::manager::PalatabilityManager;

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let palatability = PalatabilityManager::new();

        app.insert_resource(palatability)
            .add_system_to_stage(CoreStage::Last, increment_house_palatability)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house);
    }
}

fn increment_house_palatability(
    mut building_created_reader: EventReader<BuildingCreated>,
    mut palatability: ResMut<PalatabilityManager>,
) {
    for building_created in building_created_reader.iter() {
        let descriptor: Option<HouseSourcePalatabilityDescriptor> = building_created.building.to_house_source_palatability();
        info!("added as house palatability source");
        palatability.add_house_source(descriptor);

        let descriptor: Option<OfficeSourcePalatabilityDescriptor> = building_created.building.to_office_source_palatability();
        info!("added as house palatability source");
        palatability.add_office_source(descriptor);
    }
}

fn habit_house(
    mut inhabitant_arrived_writer: EventReader<InhabitantArrivedAtHome>,
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
