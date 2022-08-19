use std::sync::Arc;

use bevy::utils::HashSet;

use crate::building::{BuildRequest, BuildingUnderConstruction, House, ProgressStatus};
use crate::common::configuration::Configuration;
use crate::common::position::Position;

use super::Office;

pub struct BuildingBuilder {
    configuration: Arc<Configuration>,
    position_already_used: HashSet<Position>,
}

impl BuildingBuilder {
    pub(super) fn new(configuration: Arc<Configuration>) -> Self {
        Self {
            configuration,
            position_already_used: Default::default(),
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

    pub(super) fn go_to_live_home(
        &self,
        house: &mut House,
        arrived: usize,
    ) -> Result<(), &'static str> {
        house.resident_property.current_residents += arrived;

        Ok(())
    }

    pub(super) fn job_found(
        &self,
        office: &mut Office,
        arrived: usize,
    ) -> Result<(), &'static str> {
        office.work_property.current_worker += arrived;

        Ok(())
    }
}
