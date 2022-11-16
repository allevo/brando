use lombok::Getter;

use crate::common::{position::Position, EntityId};

#[derive(Getter, Debug, Clone)]
pub struct House {
    id: EntityId,
    position: Position,
    current_residents: u32,
    max_residents: u32,
}

impl House {
    pub fn new(id: EntityId, position: Position, max_residents: u32) -> Self {
        Self {
            id,
            position,
            current_residents: 0,
            max_residents,
        }
    }

    pub fn inhabitants_arrived(&mut self, count: u32) {
        self.current_residents += count;
        debug_assert!(self.max_residents >= self.current_residents);
    }

    #[allow(dead_code)]
    pub fn inhabitant_left(&mut self) {
        debug_assert!(self.current_residents >= 1);
        self.current_residents -= 1;
    }
}
