
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum BuildingPrototypeType {
    Street,
    House1x1,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct BuildingPrototype {
    pub code: BuildingPrototypeType,
    size: SizeP,
}

pub static STREET: BuildingPrototype = BuildingPrototype {
    code: BuildingPrototypeType::Street,
    size: SizeP(1, 1),
};
pub static HOUSE_1x1: BuildingPrototype = BuildingPrototype {
    code: BuildingPrototypeType::House1x1,
    size: SizeP(1, 1),
};

#[derive(Debug, PartialEq, Serialize)]
pub struct SizeP(u32, u32);