#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct GameConfiguration {
    pub width_table: usize,
    pub depth_table: usize,
}

#[derive(Debug, Clone)]
pub struct BuildingsConfiguration {
    pub house: HouseConfiguration,
    pub office: OfficeConfiguration,
    pub garden: GardenConfiguration,
    pub street: StreetConfiguration,
    pub biomass_power_plant: BiomassPowerPlantConfiguration,
}

#[derive(Debug, Clone)]
pub struct HouseConfiguration {
    pub max_residents: usize,
    pub max_inhabitant_per_travel: usize,
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
    pub power_consumer_configuration: PowerConsumerConfiguration,
}
#[derive(Debug, Clone)]
pub struct OfficeConfiguration {
    pub max_worker: usize,
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
    pub power_consumer_configuration: PowerConsumerConfiguration,
}
#[derive(Debug, Clone)]
pub struct GardenConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}
#[derive(Debug, Clone)]
pub struct StreetConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
}
#[derive(Debug, Clone)]
pub struct BiomassPowerPlantConfiguration {
    pub common: CommonBuildingConfiguration,
    pub palatability_configuration: PalatabilityConfiguration,
    pub power_source: PowerSourceConfiguration,
}
#[derive(Debug, Clone)]
pub struct CommonBuildingConfiguration {
    pub building_name: &'static str,
    pub time_for_building: u8,
}
#[derive(Debug, Clone)]
pub struct PalatabilityConfiguration {
    pub source_for_house: Option<HouseSourcePalatabilityConfiguration>,
    pub source_for_office: Option<OfficeSourcePalatabilityConfiguration>,
}
#[derive(Debug, Clone)]
pub struct HouseSourcePalatabilityConfiguration {
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

#[derive(Debug, Clone)]
pub struct OfficeSourcePalatabilityConfiguration {
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

#[derive(Debug, Clone)]
pub struct PowerConsumerConfiguration {
    pub consume_wh: usize,
}

#[derive(Debug, Clone)]
pub struct PowerSourceConfiguration {
    pub capacity_wh: usize,
}

// #[cfg(test)]
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
                source_for_house: Some(HouseSourcePalatabilityConfiguration {
                    value: -1,
                    max_horizontal_distribution_distance: 2,
                    max_linear_distribution_distance: 1,
                    linear_factor: 0,
                }),
                source_for_office: None,
            },
            power_consumer_configuration: PowerConsumerConfiguration { consume_wh: 300 },
        },
        office: OfficeConfiguration {
            max_worker: 6,
            common: CommonBuildingConfiguration {
                building_name: "office",
                time_for_building: 5,
            },
            palatability_configuration: PalatabilityConfiguration {
                source_for_house: None,
                source_for_office: Some(OfficeSourcePalatabilityConfiguration {
                    value: 1,
                    max_horizontal_distribution_distance: 3,
                    max_linear_distribution_distance: 0,
                    linear_factor: 0,
                }),
            },
            power_consumer_configuration: PowerConsumerConfiguration { consume_wh: 2000 },
        },
        garden: GardenConfiguration {
            common: CommonBuildingConfiguration {
                building_name: "garden",
                time_for_building: 2,
            },
            palatability_configuration: PalatabilityConfiguration {
                source_for_house: Some(HouseSourcePalatabilityConfiguration {
                    value: 10,
                    max_horizontal_distribution_distance: 3,
                    max_linear_distribution_distance: 10,
                    linear_factor: 2,
                }),
                source_for_office: Some(OfficeSourcePalatabilityConfiguration {
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
                source_for_house: None,
                source_for_office: None,
            },
        },
        biomass_power_plant: BiomassPowerPlantConfiguration {
            common: CommonBuildingConfiguration {
                building_name: "biomassPowerPlant",
                time_for_building: 10,
            },
            palatability_configuration: PalatabilityConfiguration {
                source_for_house: None,
                source_for_office: None,
            },
            power_source: PowerSourceConfiguration {
                capacity_wh: 7_000_000_000,
            },
        },
    },
};
