pub mod building;
mod configuration;
pub mod navigator;
pub mod palatability;

use bevy::math::Vec3;
pub use configuration::CONFIGURATION;

use crate::position::Position;

use self::configuration::Configuration;

pub fn convert_bevy_coords_into_position(configuration: &Configuration, coords: &Vec3) -> Position {
    let (half_width, half_depth) = configuration.half();
    let x = ((coords.x as f32 + half_width) / configuration.cube_size).round() as i64;
    let y = ((coords.z as f32 + half_depth) / configuration.cube_size).round() as i64;
    Position { x, y }
}

pub fn convert_position_into_bevy_coords(
    configuration: &Configuration,
    position: &Position,
) -> Vec3 {
    let (half_width, half_depth) = configuration.half();
    let x = position.x as f32 * configuration.cube_size - half_width;
    let z = position.y as f32 * configuration.cube_size - half_depth;
    Vec3::new(x, 0.5, z)
}
