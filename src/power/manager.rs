use std::{collections::HashMap, sync::Arc};

use bevy::utils::HashSet;

use crate::{
    building::BuildingSnapshot,
    common::{configuration::Configuration, position::Position, EntityId},
};

pub struct PowerManager {
    configuration: Arc<Configuration>,
    consumers: HashMap<EntityId, EnergyPowerConsumer>,
    not_yet_covered_consumers: HashSet<EntityId>,
    producers: HashMap<EntityId, EnergyPowerProducer>,
    // customer -> producer(s)[]
    assignments: HashMap<EntityId, Vec<EntityId>>,
}

impl PowerManager {
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            consumers: Default::default(),
            not_yet_covered_consumers: Default::default(),
            producers: Default::default(),
            assignments: Default::default(),
        }
    }

    pub fn register_power_consumer(&mut self, building: &BuildingSnapshot) {
        let energy_power_consumer = match building {
            BuildingSnapshot::Office(o) => EnergyPowerConsumer {
                position: *o.get_position(),
                base_expenditure: 0,
                single_expenditure: self
                    .configuration
                    .buildings
                    .office
                    .power_consumer_configuration
                    .consume_wh,
                multiplier: *o.get_current_workers(),
                covered: 0,
            },
            BuildingSnapshot::House(h) => EnergyPowerConsumer {
                position: *h.get_position(),
                base_expenditure: 0,
                single_expenditure: self
                    .configuration
                    .buildings
                    .house
                    .power_consumer_configuration
                    .consume_wh,
                multiplier: *h.get_current_residents(),
                covered: 0,
            },
            // TODO: check better the following statement...
            // The following ones are not considered as consumer of electric power
            BuildingSnapshot::Garden(_)
            | BuildingSnapshot::Street(_)
            | BuildingSnapshot::BiomassPowerPlant(_) => return,
        };

        debug_assert!(
            !self.consumers.contains_key(building.get_id()),
            "consumer {} already registered",
            building.get_id()
        );

        self.consumers
            .insert(*building.get_id(), energy_power_consumer);
        self.not_yet_covered_consumers.insert(*building.get_id());
    }

    pub fn register_power_source(&mut self, building: &BuildingSnapshot) {
        let energy_power_producer = match building {
            BuildingSnapshot::Office(_) | BuildingSnapshot::House(_) => return,
            BuildingSnapshot::Garden(_) | BuildingSnapshot::Street(_) => return,
            BuildingSnapshot::BiomassPowerPlant(_) => {
                let total_capacity_wh = self
                    .configuration
                    .buildings
                    .biomass_power_plant
                    .power_source
                    .capacity_wh;
                EnergyPowerProducer {
                    position: *building.get_position(),
                    total_capacity_wh,
                    remain_capacity_wh: total_capacity_wh,
                }
            }
        };

        debug_assert!(
            !self.producers.contains_key(building.get_id()),
            "producer {} already registered",
            building.get_id()
        );

        self.producers
            .insert(*building.get_id(), energy_power_producer);
    }

    // TODO: we need to simulate blackouts.
    // For the time being, we skip that complexity avoiding to assign
    // power if the plant completely use its capacity
    pub fn dedicate_power_to_consumers(&mut self) -> ChangePowerAssignment {
        // We want to assign `not_yet_covered_consumers` to some producers
        // For doing that, firstly we want to re-use the already assigned producers
        // if it is not enough, try to a new producers

        // TODO: avoid to recalculate too many time the same uncovered consumers
        // TODO: apply a strategy on the assignment: the the time being the "first" producer is used

        let mut changed_consumers: HashMap<EntityId, u32> = HashMap::new();
        let mut changed_producers: HashMap<EntityId, u32> = HashMap::new();

        for not_yet_covered_consumer in self.not_yet_covered_consumers.iter() {
            let consumer = self.consumers.get_mut(not_yet_covered_consumer).unwrap();

            let mut remain = consumer.requested() - consumer.covered;
            if remain == 0 {
                continue;
            }

            // try to put the remains into already assigned producers
            if let Some(assignments) = self.assignments.get(not_yet_covered_consumer) {
                for assignment in assignments {

                    let producer = self.producers.get_mut(&*assignment).unwrap();
                    let energy_to_reduce = producer.remain_capacity_wh.min(remain);
                    producer.remain_capacity_wh -= energy_to_reduce;
                    remain -= energy_to_reduce;
                    consumer.covered += energy_to_reduce;

                    let c: &mut u32 = changed_consumers
                        .entry(*not_yet_covered_consumer)
                        .or_default();
                    *c += energy_to_reduce;
                    let c: &mut u32 = changed_producers.entry(*assignment).or_default();
                    *c += energy_to_reduce;

                    if remain == 0 {
                        break;
                    }
                }

                if remain == 0 {
                    continue;
                }
            }

            // If:
            // - the assignments are not sufficient to cover the remain
            // - no assignments
            // try to find a new assignment
            // TODO: instead to choose randomly, we probably would like to choose the "best" one
            let available_producer = self.producers.iter_mut().find(|p| {
                let already_assigned = self
                    .assignments
                    .get(not_yet_covered_consumer)
                    .map(|assignments| assignments.contains(p.0))
                    .unwrap_or(false);

                !already_assigned && p.1.remain_capacity_wh >= remain
            });
            let available_producer = match available_producer {
                // No available producers:
                // - all producer are already assigned and they are not able to handle the load
                // - not already assigned producers have insufficient capacity
                // - no producers at all
                None => continue,
                Some(available_producer) => available_producer,
            };


            let energy_to_reduce = available_producer.1.remain_capacity_wh.min(remain);
            available_producer.1.remain_capacity_wh -= energy_to_reduce;
            consumer.covered += energy_to_reduce;

            let c: &mut u32 = changed_consumers
                .entry(*not_yet_covered_consumer)
                .or_default();
            *c += energy_to_reduce;
            let c: &mut u32 = changed_producers.entry(*available_producer.0).or_default();
            *c += energy_to_reduce;

            let assignments = self
                .assignments
                .entry(*not_yet_covered_consumer)
                .or_default();

            debug_assert!(!assignments.contains(available_producer.0), "producer {} already present for {}", available_producer.0, not_yet_covered_consumer);

            assignments.push(*available_producer.0);
        }

        // Remove all completely covered consumers
        self.not_yet_covered_consumers = self
            .not_yet_covered_consumers
            .iter()
            .filter(|id| !changed_consumers.contains_key(&id))
            .cloned()
            .collect();

        ChangePowerAssignment {
            consumers: changed_consumers
                .into_iter()
                .map(|e| {
                    (
                        e.0,
                        (
                            e.1,
                            self.consumers[&e.0].requested() - self.consumers[&e.0].covered,
                        ),
                    )
                })
                .collect(),
            producers: changed_producers,
        }
    }

    #[allow(dead_code)]
    pub fn calculate_missing_power_energy(&self) -> u32 {
        self.not_yet_covered_consumers
            .iter()
            .map(|c| self.consumers[c].requested() - self.consumers[c].covered)
            .sum()
    }

    #[allow(dead_code)]
    pub fn is_completely_covered(&self, building_id: &EntityId) -> (u32, bool) {
        if self.producers.contains_key(building_id) {
            return (0, true);
        }

        match self.consumers.get(building_id) {
            None => (0, false),
            Some(c) => (c.requested() - c.covered, c.requested() <= c.covered),
        }
    }
}

#[derive(Debug)]
pub struct ChangePowerAssignment {
    consumers: HashMap<EntityId, (u32, u32)>,
    producers: HashMap<EntityId, u32>,
}

#[derive(Debug)]
struct EnergyPowerProducer {
    position: Position,
    total_capacity_wh: u32,
    remain_capacity_wh: u32,
}

#[derive(Debug)]
struct EnergyPowerConsumer {
    position: Position,
    base_expenditure: u32,
    single_expenditure: u32,
    multiplier: u32,
    covered: u32,
}
impl EnergyPowerConsumer {
    fn requested(&self) -> u32 {
        self.base_expenditure + self.single_expenditure * self.multiplier
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct ConsumerAssignmentPair {
    consumer: EntityId,
    producer: EntityId,
}

#[cfg(test)]
mod tests {

    use crate::{building::snapshot::*, common::configuration::CONFIGURATION};

    use super::*;

    #[test]
    fn test_dedicate_power_to_consumer() {
        let mut manager = PowerManager::new(Arc::new(CONFIGURATION));

        let house1 = 0_u64;
        let position = Position { x: 0, y: 0 };
        let building = &BuildingSnapshot::House(HouseSnapshot {
            id: house1,
            position,
            current_residents: 16,
            max_residents: 16,
        });
        manager.register_power_consumer(building);

        let house2 = 1_u64;
        let position = Position { x: 1, y: 0 };
        let building = &BuildingSnapshot::House(HouseSnapshot {
            id: house2,
            position,
            current_residents: 8,
            max_residents: 16,
        });
        manager.register_power_consumer(building);

        let office1 = 2_u64;
        let position = Position { x: 2, y: 0 };
        let building = &BuildingSnapshot::Office(OfficeSnapshot {
            id: office1,
            position,
            current_workers: 10,
            max_workers: 16,
        });
        manager.register_power_consumer(building);

        let change_assignment = manager.dedicate_power_to_consumers();

        assert_eq!(HashMap::new(), change_assignment.consumers);
        assert_eq!(HashMap::new(), change_assignment.producers);

        let biomass_power1 = 3_u64;
        let position = Position { x: 3, y: 0 };
        let building = &BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
            id: biomass_power1,
            position,
        });
        manager.register_power_source(building);

        let change_assignment = manager.dedicate_power_to_consumers();

        assert_eq!(
            HashMap::from([
                (house1, (manager.consumers[&house1].covered, 0)),
                (house2, (manager.consumers[&house2].covered, 0)),
                (office1, (manager.consumers[&office1].covered, 0))
            ]),
            change_assignment.consumers
        );
        assert_eq!(
            HashMap::from([(
                biomass_power1,
                manager.consumers[&house1].covered
                    + manager.consumers[&house2].covered
                    + manager.consumers[&office1].covered
            )]),
            change_assignment.producers
        );

        let change_assignment = manager.dedicate_power_to_consumers();
        assert_eq!(HashMap::from([]), change_assignment.consumers);
        assert_eq!(HashMap::from([]), change_assignment.producers);

        assert_eq!((0, true), manager.is_completely_covered(&house1));
        assert_eq!((0, true), manager.is_completely_covered(&house2));
        assert_eq!((0, true), manager.is_completely_covered(&office1));
        assert_eq!((0, true), manager.is_completely_covered(&biomass_power1));
    }

    #[test]
    fn test_dedicate_power_to_consumer_insufficient_power() {
        let mut manager = PowerManager::new(Arc::new(CONFIGURATION));

        let entity_id = 0_u64;
        let position = Position { x: 3, y: 0 };
        let building = &BuildingSnapshot::BiomassPowerPlant(BiomassPowerPlantSnapshot {
            id: entity_id,
            position,
        });
        manager.register_power_source(building);

        let max_residents = CONFIGURATION.buildings.house.max_residents;

        let mut i = 0;
        let mut changes;
        loop {
            i += 1;

            let house = i;
            let position = Position { x: 0, y: 0 };
            let building = &BuildingSnapshot::House(HouseSnapshot {
                id: house,
                position,
                current_residents: max_residents,
                max_residents,
            });
            manager.register_power_consumer(building);

            changes = manager.dedicate_power_to_consumers();

            if changes.consumers.is_empty() {
                break;
            }
        }

        let missing_power = manager.calculate_missing_power_energy();
        assert_eq!(missing_power, 2400);

        let house = i + 1;
        let position = Position { x: 0, y: 0 };
        let building = &BuildingSnapshot::House(HouseSnapshot {
            id: house,
            position,
            current_residents: max_residents,
            max_residents,
        });
        manager.register_power_consumer(building);
        
        changes = manager.dedicate_power_to_consumers();
        assert_eq!(true, changes.consumers.is_empty());
        assert_eq!(true, changes.producers.is_empty());

        let missing_power = manager.calculate_missing_power_energy();
        assert_eq!(missing_power, 2400 + manager.consumers[&house].requested());

    }
}
