use lombok::Getter;

use crate::common::{position::Position, EntityId};

#[derive(Getter, Debug, Clone)]
pub struct Office {
    id: EntityId,
    position: Position,
    current_workers: u32,
    max_workers: u32,
}

impl Office {
    pub fn new(id: EntityId, position: Position, max_workers: u32) -> Self {
        Self {
            id,
            position,
            current_workers: 0,
            max_workers,
        }
    }

    pub fn workers_arrived(&mut self, count: u32) {
        self.current_workers += count;
        debug_assert!(self.max_workers >= self.current_workers);
    }

    pub fn worker_left(&mut self) {
        debug_assert!(self.current_workers >= 1);
        self.current_workers -= 1;
    }
}
