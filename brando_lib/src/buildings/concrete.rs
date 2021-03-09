
use serde::{Serialize};
use super::prototype::BuildingPrototype;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Street {}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct House1x1 {
    max_citizens: u8,
    current_citizens: u8,
}
impl House1x1 {
    pub fn new(max_citizens: u8, current_citizens: u8) -> Self {
        Self {
            max_citizens,
            current_citizens,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ConcreteBuilding {
    Street(Street),
    House1x1(House1x1),
}

pub type BuildingId = usize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Building {
    pub id: BuildingId,
    pub prototype: &'static BuildingPrototype,
    pub building: ConcreteBuilding,
}