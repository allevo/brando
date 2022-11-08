use lombok::Getter;

use crate::common::{position::Position, EntityId};

#[derive(Getter, Debug, Clone)]
pub struct Street {
    id: EntityId,
    position: Position,
}

impl Street {
    pub fn new(id: EntityId, position: Position) -> Self {
        Self { id, position }
    }
}
