use crate::buildings::{
    concrete::{Building, ConcreteBuilding, House1x1, Street},
    prototype::{BuildingPrototype, BuildingPrototypeType, HOUSE_1x1, STREET},
};

pub fn concretize_building(prototype_type: &BuildingPrototypeType) -> Building {
    let prototype = match prototype_type {
        BuildingPrototypeType::Street => &STREET,
        BuildingPrototypeType::House1x1 => &HOUSE_1x1,
    };

    let id = 44;
    Building {
        id,
        prototype,
        building: prototype.into(),
    }
}

impl From<&BuildingPrototype> for ConcreteBuilding {
    fn from(proto: &BuildingPrototype) -> ConcreteBuilding {
        match proto.code {
            BuildingPrototypeType::Street => ConcreteBuilding::Street(Street {}),
            BuildingPrototypeType::House1x1 => ConcreteBuilding::House1x1(House1x1::new(1, 0)),
        }
    }
}
