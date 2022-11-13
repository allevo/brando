mod buildings;
mod manager;
mod plugin;

// #[cfg(test)]
// pub use buildings::*;

pub use buildings::snapshot::*;

pub use plugin::events;
pub use plugin::{BuildingManagerResource, BuildingPlugin};

#[cfg(test)]
pub use plugin::{
    BiomassPowerPlantComponent, BuildingUnderConstructionComponent, GardenComponent,
    HouseComponent, OfficeComponent, PlaneComponent, StreetComponent,
};
