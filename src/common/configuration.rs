pub struct Configuration {
    pub cube_size: f32,
    pub width_table: usize,
    pub depth_table: usize,
    pub camera_velocity: f32,

    pub buildings: BuildingsConfiguration,
}

impl Configuration {
    pub const fn half(&self) -> (f32, f32) {
        (
            self.width_table as f32 / 2. * self.cube_size,
            self.depth_table as f32 / 2. * self.cube_size,
        )
    }
}

pub struct BuildingsConfiguration {
    pub house: HouseConfiguration,
    pub office: OfficeConfiguration,
    pub garden: GardenConfiguration,
    pub street: StreetConfiguration,
}

pub struct HouseConfiguration {
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct OfficeConfiguration {
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct GardenConfiguration {
    pub palatability_configuration: PalatabilityConfiguration,
}
pub struct StreetConfiguration {
    pub palatability_configuration: PalatabilityConfiguration,
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
    width_table: 32,
    depth_table: 32,
    camera_velocity: 0.75,

    buildings: BuildingsConfiguration {
        house: HouseConfiguration {
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
            palatability_configuration: PalatabilityConfiguration {
                house_source: None,
                office_source: None,
            },
        },
    },
};
