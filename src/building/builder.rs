use std::sync::Arc;

use bevy::utils::{HashMap, HashSet};

use crate::building::{BuildRequest, BuildingUnderConstruction, House, ProgressStatus};
use crate::common::EntityId;
use crate::common::configuration::Configuration;
use crate::common::position::Position;

use super::{
    BiomassPowerPlant, Building, BuildingType, Garden, Office, ResidentProperty,
    Street, WorkProperty,
};

pub struct BuildingBuilder {
    configuration: Arc<Configuration>,
    position_already_used: HashSet<Position>,

    buildings: HashMap<EntityId, Building>,
}

impl BuildingBuilder {
    pub(super) fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            position_already_used: Default::default(),
            buildings: Default::default(),
        }
    }

    pub(super) fn create_building(
        &mut self,
        request: BuildRequest,
    ) -> Result<BuildingUnderConstruction, &'static str> {
        let position = request.position;
        if !self.position_already_used.insert(position) {
            return Err("Position already used");
        }

        let time_for_building = match request.building_type {
            super::BuildingType::House => {
                self.configuration.buildings.house.common.time_for_building
            }
            super::BuildingType::Street => {
                self.configuration.buildings.street.common.time_for_building
            }
            super::BuildingType::Garden => {
                self.configuration.buildings.garden.common.time_for_building
            }
            super::BuildingType::Office => {
                self.configuration.buildings.office.common.time_for_building
            }
            super::BuildingType::BiomassPowerPlant => {
                self.configuration
                    .buildings
                    .biomass_power_plant
                    .common
                    .time_for_building
            }
        };

        let progress_status = ProgressStatus {
            current_step: 0,
            step_to_reach: time_for_building,
        };
        Ok(BuildingUnderConstruction {
            request,
            progress_status,
        })
    }

    pub(super) fn make_progress(
        &self,
        under_construction: &mut BuildingUnderConstruction,
    ) -> Result<bool, &'static str> {
        let current_progress = under_construction.progress_status.clone().progress();
        under_construction.progress_status = current_progress;

        Ok(under_construction.progress_status.is_completed())
    }

    pub(super) fn build<'a, 's>(
        &'s mut self,
        id: EntityId,
        under_construction: BuildingUnderConstruction,
        configuration: &Configuration,
    ) -> &'a Building
    where
        's: 'a,
    {
        let building = match under_construction.request.building_type {
            BuildingType::House => Building::House(House {
                id,
                position: under_construction.request.position,
                resident_property: ResidentProperty {
                    current_residents: 0,
                    max_residents: configuration.buildings.house.max_residents,
                },
            }),
            BuildingType::Office => Building::Office(Office {
                id,
                position: under_construction.request.position,
                work_property: WorkProperty {
                    current_worker: 0,
                    max_worker: configuration.buildings.office.max_worker,
                },
            }),
            BuildingType::Garden => Building::Garden(Garden {
                id,
                position: under_construction.request.position,
            }),
            BuildingType::Street => Building::Street(Street {
                id,
                position: under_construction.request.position,
            }),
            BuildingType::BiomassPowerPlant => Building::BiomassPowerPlant(BiomassPowerPlant {
                id,
                position: under_construction.request.position,
            }),
        };

        self.buildings.entry(id).or_insert(building);

        &self.buildings[&id]
    }

    pub(super) fn go_to_live_home(
        &mut self,
        house_id: EntityId,
        arrived: usize,
    ) -> Result<(), &'static str> {
        let house = self.buildings.get_mut(&house_id).unwrap();
        let house = house.as_mut_house();

        house.resident_property.current_residents += arrived;

        Ok(())
    }

    pub(super) fn job_found(
        &mut self,
        office_id: EntityId,
        arrived: usize,
    ) -> Result<(), &'static str> {
        let office = self.buildings.get_mut(&office_id).unwrap();
        let office = office.as_mut_office();

        office.work_property.current_worker += arrived;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_building(&self, id: &EntityId) -> &Building {
        &self.buildings[id]
    }
}
