use lombok::Getter;

use crate::common::{enums::EducationLevel, position::Position, EntityId};

#[derive(Getter, Debug)]
pub struct Inhabitant {
    id: EntityId,
    home: Option<Home>,
    work_place_id: Option<WorkPlace>,
    education_level: EducationLevel,
}

impl Inhabitant {
    pub fn new(id: EntityId, education_level: EducationLevel) -> Self {
        Self {
            id,
            home: None,
            work_place_id: None,
            education_level,
        }
    }

    pub fn home_found(&mut self, house_id: EntityId, house_position: Position) {
        // TODO: this in the future could be wrong.
        debug_assert!(self.home.is_none(), "Unable to change home!");

        self.home = Some(Home {
            house_id,
            house_position,
        })
    }

    pub fn work_place_found(&mut self, work_place_id: EntityId, work_place_position: Position) {
        // TODO: this in the future could be wrong.
        debug_assert!(self.work_place_id.is_none(), "Unable to change work!");

        self.work_place_id = Some(WorkPlace {
            work_place_id,
            work_place_position,
        })
    }
}

#[derive(Debug, Getter)]
pub struct Home {
    house_id: EntityId,
    house_position: Position,
}

#[derive(Debug, Getter)]
pub struct WorkPlace {
    work_place_id: EntityId,
    work_place_position: Position,
}
