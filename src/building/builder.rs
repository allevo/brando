use bevy::utils::HashSet;

use crate::building::{BuildRequest, BuildingInConstruction, House, ProgressStatus};

use crate::common::position::Position;
use crate::navigation::plugin::InhabitantArrivedAtHome;

pub struct BuildingBuilder {
    position_already_used: HashSet<Position>,
}

impl BuildingBuilder {
    pub fn new() -> Self {
        Self {
            position_already_used: Default::default(),
        }
    }

    pub fn create_building(
        &mut self,
        request: BuildRequest,
    ) -> Result<BuildingInConstruction, &'static str> {
        let position = request.position;
        if !self.position_already_used.insert(position) {
            return Err("Position already used");
        }

        let progress_status = ProgressStatus {
            current_step: 0,
            step_to_reach: request.prototype.time_for_building,
        };
        Ok(BuildingInConstruction {
            request,
            progress_status,
        })
    }

    pub fn make_progress(
        &self,
        in_progress: &mut BuildingInConstruction,
    ) -> Result<bool, &'static str> {
        let current_progress = in_progress.progress_status.clone().progress();
        in_progress.progress_status = current_progress;

        Ok(in_progress.progress_status.is_completed())
    }

    pub fn go_to_live_home(
        &self,
        house: &mut House,
        arrived: &InhabitantArrivedAtHome,
    ) -> Result<(), &'static str> {
        house.resident_property.current_residents += arrived.count;

        Ok(())
    }
}
