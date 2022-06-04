use bevy::prelude::*;

use crate::palatability::manager::HouseSourcePalatabilityDescriptor;
use crate::{building::Building, navigation::plugin::InhabitantArrivedAtHome};

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
        let descriptor: HouseSourcePalatabilityDescriptor = match &building_created.building {
            Building::Garden(g) => g.into(),
            Building::House(h) => h.into(),
            Building::Street(_) => continue,
            // The offices should raise the house palatability or not?
            Building::Office(_) => continue,
        };

        info!("added as house palatability source");
        palatability.add_house_source(descriptor);
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
