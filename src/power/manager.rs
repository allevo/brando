use std::{collections::HashMap, sync::Arc};

use crate::{
    building::plugin::BuildingSnapshot,
    common::{configuration::Configuration, position::Position, EntityId},
};

pub struct PowerManager {
    configuration: Arc<Configuration>,
    consumers: Vec<(EntityId, EnergyPowerConsumer, Position)>,
    covered_consumers: HashMap<EntityId, (EnergyPowerConsumer, Position, EntityId)>,
    sources: HashMap<EntityId, (EntityId, EnergyPowerPlant, Position, Vec<EntityId>)>,
}

impl PowerManager {
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            consumers: Default::default(),
            covered_consumers: Default::default(),
            sources: Default::default(),
        }
    }

    pub fn register_power_consumer(
        &mut self,
        building: &BuildingSnapshot,
        position: Position,
        building_id: EntityId,
    ) {
        let energy_power_consumer = match building {
            // TODO: keep this value from configuration
            BuildingSnapshot::Office(_) => EnergyPowerConsumer {
                consumed_wh: self
                    .configuration
                    .buildings
                    .office
                    .power_consumer_configuration
                    .consume_wh,
            },
            // TODO: keep this value from configuration
            BuildingSnapshot::House(h) => EnergyPowerConsumer {
                consumed_wh: self
                    .configuration
                    .buildings
                    .house
                    .power_consumer_configuration
                    .consume_wh
                    * h.resident_property.current_residents,
            },
            // TODO: check better the following statement...
            // The following ones are not considered as consumer of electric power
            BuildingSnapshot::Garden(_)
            | BuildingSnapshot::Street(_)
            | BuildingSnapshot::BiomassPowerPlant(_) => return,
        };

        self.consumers
            .push((building_id, energy_power_consumer, position));
    }

    pub fn register_power_source(
        &mut self,
        building: &BuildingSnapshot,
        position: Position,
        building_id: EntityId,
    ) {
        let created_energy_power = match building {
            BuildingSnapshot::Office(_) | BuildingSnapshot::House(_) => return,
            BuildingSnapshot::Garden(_) | BuildingSnapshot::Street(_) => return,
            // TODO: keep this value from configuration
            BuildingSnapshot::BiomassPowerPlant(_) => EnergyPowerPlant {
                capacity_wh: 7_000_000_000,
            },
        };

        self.sources.entry(building_id).or_insert((
            building_id,
            created_energy_power,
            position,
            vec![],
        ));
    }

    // TODO: we need to simulate blackouts.
    // For the time being, we skip that complexity avoiding to assign
    // power if the plant completely use its capacity
    pub fn dedicate_power_to_consumers(&mut self) -> Vec<EntityId> {
        let mut sources: Vec<&mut (EntityId, EnergyPowerPlant, Position, Vec<_>)> =
            self.sources.values_mut().collect();

        let not_yet_covered: Vec<_> = self.consumers.drain(0..self.consumers.len()).collect();

        let mut resolved = Vec::new();

        for (building_id, consumer_property, building_position) in not_yet_covered {
            sources.sort_by_key(|(_, _, plant_position, _)| {
                (plant_position.x - building_position.x).abs()
                    + (plant_position.y - building_position.y).abs()
            });

            let source = sources
                .iter_mut()
                .find(|(_, epp, _, _)| epp.capacity_wh > consumer_property.consumed_wh);
            match source {
                None => {
                    self.consumers
                        .push((building_id, consumer_property, building_position));
                    continue;
                }
                Some((source_id, epp, _, v)) => {
                    epp.capacity_wh -= consumer_property.consumed_wh;
                    v.push(building_id);
                    self.covered_consumers.entry(building_id).or_insert((
                        consumer_property,
                        building_position,
                        *source_id,
                    ));

                    resolved.push(building_id);
                }
            }
        }

        resolved
    }

    #[allow(dead_code)]
    pub fn is_covered(&self, building_id: &EntityId) -> bool {
        self.covered_consumers.contains_key(building_id) || self.sources.contains_key(building_id)
    }
}

#[derive(Debug)]
struct EnergyPowerPlant {
    pub capacity_wh: usize,
}

#[derive(Debug)]
struct EnergyPowerConsumer {
    consumed_wh: usize,
}

#[cfg(test)]
mod tests {

    use crate::{
        building::{
            plugin::{BiomassPowerPlantSnapshot, HouseSnapshot, OfficeSnapshot},
            ResidentProperty, WorkProperty,
        },
        common::configuration::CONFIGURATION,
    };

    use super::*;

    #[test]
    fn test_dedicate_power_to_consumer() {
        let mut manager = PowerManager::new(Arc::new(CONFIGURATION));

        let entity_id = 0_u64;
        let position = Position { x: 0, y: 0 };
        let building = &BuildingSnapshot::House(HouseSnapshot {
            position: position.clone(),
            resident_property: ResidentProperty {
                current_residents: 16,
                max_residents: 16,
            },
        });
        manager.register_power_consumer(building, position, entity_id);

        let entity_id = 1_u64;
        let position = Position { x: 1, y: 0 };
        let building = &BuildingSnapshot::House(HouseSnapshot {
            position: position.clone(),
            resident_property: ResidentProperty {
                current_residents: 8,
                max_residents: 16,
            },
        });
        manager.register_power_consumer(building, position, entity_id);

        let entity_id = 2_u64;
        let position = Position { x: 2, y: 0 };
        let building = &BuildingSnapshot::Office(OfficeSnapshot {
            position: position.clone(),
            work_property: WorkProperty {
                current_worker: 10,
                max_worker: 16,
            },
        });
        manager.register_power_consumer(building, position, entity_id);

        let resolved = manager.dedicate_power_to_consumers();
        assert!(resolved.is_empty());

        let entity_id = 3_u64;
        let position = Position { x: 3, y: 0 };
        let building = &BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
            position: position.clone(),
        });
        manager.register_power_source(building, position, entity_id);

        let resolved = manager.dedicate_power_to_consumers();

        assert_eq!(vec![0_u64, 1_u64, 2_u64], resolved);

        let resolved = manager.dedicate_power_to_consumers();
        assert!(resolved.is_empty());

        assert!(manager.is_covered(&0_u64));
        assert!(manager.is_covered(&1_u64));
        assert!(manager.is_covered(&2_u64));
        assert!(manager.is_covered(&3_u64));
    }

    #[test]
    fn test_dedicate_power_to_consumer_insufficient_power() {
        let mut manager = PowerManager::new(Arc::new(CONFIGURATION));

        let entity_id = 0_u64;
        let position = Position { x: 3, y: 0 };
        let building = &BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
            position: position.clone(),
        });
        manager.register_power_source(building, position, entity_id);

        let total_capacity_wh = CONFIGURATION
            .buildings
            .biomass_power_plant
            .power_source
            .capacity_wh;
        let consume_wh_per_inhabitants = CONFIGURATION
            .buildings
            .house
            .power_consumer_configuration
            .consume_wh;
        let max_residents = CONFIGURATION.buildings.house.max_residents;

        let total_house =
            total_capacity_wh as f32 / consume_wh_per_inhabitants as f32 / max_residents as f32;
        let total_house = total_house.ceil() as u64 + 1;

        for i in 1..(total_house + 1) {
            let entity_id = i;
            let position = Position { x: 0, y: 0 };
            let building = &BuildingSnapshot::House(HouseSnapshot {
                position: position.clone(),
                resident_property: ResidentProperty {
                    current_residents: max_residents,
                    max_residents: max_residents,
                },
            });

            manager.register_power_consumer(building, position, entity_id);
        }

        manager.dedicate_power_to_consumers();

        assert!(!manager.is_covered(&total_house));
    }
}
