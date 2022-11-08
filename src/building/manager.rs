use std::sync::Arc;

use bevy::utils::{HashMap, HashSet};

use crate::common::{configuration::Configuration, position::Position, EntityId};

use super::{BiomassPowerPlant, Garden, House, Office, Street};

pub struct BuildingManager {
    configuration: Arc<Configuration>,
    position_already_used: HashSet<Position>,
    buildings: HashMap<EntityId, Building>,
}
impl BuildingManager {
    pub(super) fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            position_already_used: Default::default(),
            buildings: Default::default(),
        }
    }

    pub(super) fn start_building_creation(
        &mut self,
        building: Building,
    ) -> Result<BuildingUnderConstruction, &'static str> {
        if !self.position_already_used.insert(*building.get_position()) {
            return Err("Position already used");
        }

        let step_to_reach = building.get_step_to_reach(&self.configuration);

        Ok(BuildingUnderConstruction {
            building,
            progress_status: ProgressStatus {
                current_step: 0,
                step_to_reach,
            },
        })
    }

    pub(super) fn make_progress(
        &mut self,
        building_under_construction: &mut BuildingUnderConstruction,
    ) -> bool {
        building_under_construction.progress_status.make_progress();

        building_under_construction.progress_status.step_to_reach
            >= building_under_construction.progress_status.current_step
    }

    pub(super) fn finalize_building_creation(
        &mut self,
        building_under_construction: &mut BuildingUnderConstruction,
    ) {
        let entry = self
            .buildings
            .entry(building_under_construction.building.get_id());
        entry.insert(building_under_construction.building.clone());
    }

    pub(super) fn inhabitants_arrived_at_home(&mut self, house_id: EntityId, count: u32) {
        let house = self
            .buildings
            .get_mut(&house_id)
            .expect("house should exists");
        let house = house.to_mut_house();

        house.inhabitants_arrived(count);
    }

    pub(super) fn workers_found_job(&mut self, office_id: EntityId, count: u32) {
        let office = self
            .buildings
            .get_mut(&office_id)
            .expect("office should exists");
        let office = office.to_mut_office();

        office.workers_arrived(count);
    }

    pub(super) fn house(&self, id: EntityId, position: Position) -> House {
        House::new(
            id,
            position,
            self.configuration.buildings.house.max_residents,
        )
    }

    pub(super) fn office(&self, id: EntityId, position: Position) -> Office {
        Office::new(id, position, self.configuration.buildings.office.max_worker)
    }

    pub(super) fn garden(&self, id: EntityId, position: Position) -> Garden {
        Garden::new(id, position)
    }

    pub(super) fn street(&self, id: EntityId, position: Position) -> Street {
        Street::new(id, position)
    }

    pub(super) fn biomass_power_plant(
        &self,
        id: EntityId,
        position: Position,
    ) -> BiomassPowerPlant {
        BiomassPowerPlant::new(id, position)
    }

    #[cfg(test)]
    pub fn get_building(&self, id: &EntityId) -> &Building {
        &self.buildings[id]
    }
}

#[derive(Debug, Clone)]
pub enum Building {
    Office(Office),
    House(House),
    Garden(Garden),
    Street(Street),
    BiomassPowerPlant(BiomassPowerPlant),
}

impl Building {
    fn to_mut_house(&mut self) -> &mut House {
        match self {
            Building::House(h) => h,
            _ => unreachable!("cannot call to_mut_house for not houses"),
        }
    }
    fn to_mut_office(&mut self) -> &mut Office {
        match self {
            Building::Office(o) => o,
            _ => unreachable!("cannot call to_mut_office for not offices"),
        }
    }

    fn get_step_to_reach(&self, configuration: &Arc<Configuration>) -> u8 {
        match self {
            Building::House(_) => configuration.buildings.house.common.time_for_building,
            Building::Office(_) => configuration.buildings.office.common.time_for_building,
            Building::Garden(_) => configuration.buildings.garden.common.time_for_building,
            Building::Street(_) => configuration.buildings.street.common.time_for_building,
            Building::BiomassPowerPlant(_) => {
                configuration
                    .buildings
                    .biomass_power_plant
                    .common
                    .time_for_building
            }
        }
    }

    pub fn get_id(&self) -> EntityId {
        match self {
            Building::House(b) => *b.get_id(),
            Building::Office(b) => *b.get_id(),
            Building::Garden(b) => *b.get_id(),
            Building::Street(b) => *b.get_id(),
            Building::BiomassPowerPlant(b) => *b.get_id(),
        }
    }

    pub fn get_position(&self) -> &Position {
        match self {
            Building::House(b) => b.get_position(),
            Building::Office(b) => b.get_position(),
            Building::Garden(b) => b.get_position(),
            Building::Street(b) => b.get_position(),
            Building::BiomassPowerPlant(b) => b.get_position(),
        }
    }
}

#[derive(Debug)]
pub struct BuildingUnderConstruction {
    building: Building,
    progress_status: ProgressStatus,
}

impl BuildingUnderConstruction {
    pub fn get_building(&self) -> &Building {
        &self.building
    }
}

#[derive(Debug)]
struct ProgressStatus {
    current_step: u8,
    step_to_reach: u8,
}

impl ProgressStatus {
    pub fn make_progress(&mut self) {
        // Make progress never be called if the building is already ready to be built.
        // But we probably don't care about this in production release: it works fine anyway
        debug_assert!(self.step_to_reach > self.current_step);
        self.current_step += 1;
    }
}
