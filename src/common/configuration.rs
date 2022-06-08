pub struct Configuration {
    pub cube_size: f32,

    pub camera_velocity: f32,

    pub game: GameConfiguration,

    pub buildings: BuildingsConfiguration,
}

impl Configuration {
    #[inline]
    pub const fn half(&self) -> (f32, f32) {
        (
            self.game.width_table as f32 / 2. * self.cube_size,
            self.game.depth_table as f32 / 2. * self.cube_size,
        )
    }
}

pub struct GameConfiguration {
    pub width_table: usize,
    pub depth_table: usize,
}

pub struct BuildingsConfiguration {
    pub house: HouseConfiguration,
    pub office: OfficeConfiguration,
    pub garden: GardenConfiguration,
    pub street: StreetConfiguration,
}

pub struct HouseConfiguration {
    pub max_residents: u8,
    pub max_inhabitant_per_travel: u8,
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct OfficeConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct GardenConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct StreetConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}

pub struct CommonBuildingConfiguration {
    pub building_name: &'static str,
    pub time_for_building: u8,
}

pub struct PalatabilityConfiguration {
    pub house_source: Option<HouseSourcePalatabilityConfiguration>,
    pub office_source: Option<OfficeSourcePalatabilityConfiguration>,
}

pub struct HouseSourcePalatabilityConfiguration {
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

pub struct OfficeSourcePalatabilityConfiguration {
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

pub const CONFIGURATION: Configuration = Configuration {
    cube_size: 0.3,
    camera_velocity: 0.75,

    game: GameConfiguration {
        width_table: 32,
        depth_table: 32,
    },

    buildings: BuildingsConfiguration {
        house: HouseConfiguration {
            max_residents: 8,
            max_inhabitant_per_travel: 6,
            common: CommonBuildingConfiguration {
                building_name: "house",
                time_for_building: 10,
            },
            palatability_configuration: PalatabilityConfiguration {
                house_source: Some(HouseSourcePalatabilityConfiguration {
                    value: -1,
                    max_horizontal_distribution_distance: 2,
                    max_linear_distribution_distance: 1,
                    linear_factor: 0,
                }),
                office_source: None,
            },
        },
        office: OfficeConfiguration {
            common: CommonBuildingConfiguration {
                building_name: "office",
                time_for_building: 5,
            },
            palatability_configuration: PalatabilityConfiguration {
                house_source: None,
                office_source: Some(OfficeSourcePalatabilityConfiguration {
                    value: 1,
                    max_horizontal_distribution_distance: 3,
                    max_linear_distribution_distance: 0,
                    linear_factor: 0,
                }),
            },
        },
        garden: GardenConfiguration {
            common: CommonBuildingConfiguration {
                building_name: "garden",
                time_for_building: 2,
            },
            palatability_configuration: PalatabilityConfiguration {
                house_source: Some(HouseSourcePalatabilityConfiguration {
                    value: 10,
                    max_horizontal_distribution_distance: 3,
                    max_linear_distribution_distance: 10,
                    linear_factor: 2,
                }),
                office_source: Some(OfficeSourcePalatabilityConfiguration {
                    value: 10,
                    max_horizontal_distribution_distance: 3,
                    max_linear_distribution_distance: 10,
                    linear_factor: 2,
                }),
            },
        },
        street: StreetConfiguration {
            common: CommonBuildingConfiguration {
                building_name: "street",
                time_for_building: 2,
            },
            palatability_configuration: PalatabilityConfiguration {
                house_source: None,
                office_source: None,
            },
        },
    },
};
