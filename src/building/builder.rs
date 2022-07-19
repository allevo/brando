use std::sync::Arc;

use bevy::utils::HashSet;

use crate::building::{BuildRequest, BuildingInConstruction, House, ProgressStatus};
use crate::common::configuration::Configuration;
use crate::common::position::Position;

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
    ) -> Result<BuildingInConstruction, &'static str> {
        let position = request.position;
        if !self.position_already_used.insert(position) {
            return Err("Position already used");
        }

        let time_for_building = match request.building_type {
            super::BuildingType::House => self.configuration.buildings.house.common.time_for_building,
            super::BuildingType::Street => self.configuration.buildings.street.common.time_for_building,
            super::BuildingType::Garden => self.configuration.buildings.garden.common.time_for_building,
            super::BuildingType::Office => self.configuration.buildings.office.common.time_for_building,
        };

        let progress_status = ProgressStatus {
            current_step: 0,
            step_to_reach: time_for_building,
        };
        Ok(BuildingInConstruction {
            request,
            progress_status,
        })
    }

    pub(super) fn make_progress(
        &self,
        in_progress: &mut BuildingInConstruction,
    ) -> Result<bool, &'static str> {
        let current_progress = in_progress.progress_status.clone().progress();
        in_progress.progress_status = current_progress;

        Ok(in_progress.progress_status.is_completed())
    }

    pub(super) fn go_to_live_home(
        &self,
        house: &mut House,
        arrived: u8,
    ) -> Result<(), &'static str> {
        house.resident_property.current_residents += arrived;

        Ok(())
    }
}
