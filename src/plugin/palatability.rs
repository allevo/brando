use bevy::prelude::*;

use crate::{
    building::Building,
    palatability::{HouseSourcePalatabilityDescriptor, Palatability},
};

use super::{building::BuildingCreated, navigator::InhabitantArrivedAtHome};

pub struct PalatabilityPlugin;

impl Plugin for PalatabilityPlugin {
    fn build(&self, app: &mut App) {
        let palatability = Palatability::new();

        app.insert_resource(palatability)
            .add_system_to_stage(CoreStage::Last, handle_new_house)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house);
    }
}

fn handle_new_house(
    mut building_created_reader: EventReader<BuildingCreated>,
    mut palatability: ResMut<Palatability>,
) {
    for building_created in building_created_reader.iter() {
        let descriptor: HouseSourcePalatabilityDescriptor = match &building_created.building {
            Building::Garden(g) => g.into(),
            Building::House(h) => h.into(),
            Building::Street(_) => continue,
        };

        info!("added as house palatability source");
        palatability.add_house_source(descriptor);
    }
}

fn habit_house(
    mut inhabitant_arrived_writer: EventReader<InhabitantArrivedAtHome>,
    mut palatability: ResMut<Palatability>,
) {
    let count: u8 = inhabitant_arrived_writer.iter().map(|a| a.count).sum();
    if count == 0 {
        return;
    }

    palatability.increment_populations(count as i32);

    let population = palatability.total_populations();
    info!("population count: {population:?}");
}
