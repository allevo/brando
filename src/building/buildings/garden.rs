use lombok::Getter;

use crate::common::{position::Position, EntityId};

#[derive(Getter, Debug, Clone)]
pub struct Garden {
    id: EntityId,
    position: Position,
}

impl Garden {
    pub fn new(id: EntityId, position: Position) -> Self {
        Self { id, position }
    }
}
