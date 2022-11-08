use lombok::Getter;

use crate::common::{position::Position, EntityId};

#[derive(Getter, Debug, Clone)]
pub struct BiomassPowerPlant {
    id: EntityId,
    position: Position,
}

impl BiomassPowerPlant {
    pub fn new(id: EntityId, position: Position) -> Self {
        Self { id, position }
    }
}
