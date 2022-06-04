use bevy::math::Vec3;

use super::{configuration::Configuration, position::Position};

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

#[cfg(test)]
mod tests {
    use crate::common::{
        configuration::CONFIGURATION,
        position::Position,
        position_utils::{convert_bevy_coords_into_position, convert_position_into_bevy_coords},
    };

    #[test]
    fn calculate_plate_positions() {
        let half_width = CONFIGURATION.width_table as f32 / 2. * CONFIGURATION.cube_size;
        let half_depth = CONFIGURATION.depth_table as f32 / 2. * CONFIGURATION.cube_size;
        let x_positions = (0..CONFIGURATION.width_table)
            .map(|i| i as f32 * CONFIGURATION.cube_size - half_width)
            .collect::<Vec<f32>>();
        let z_positions = (0..CONFIGURATION.depth_table)
            .map(|i| i as f32 * CONFIGURATION.cube_size - half_depth)
            .collect::<Vec<f32>>();

        assert_eq!(
            x_positions,
            vec![
                -4.8, -4.5, -4.2000003, -3.9, -3.6000001, -3.3000002, -3.0, -2.7, -2.4, -2.1000001,
                -1.8000002, -1.5, -1.2, -0.9000001, -0.5999999, -0.3000002, 0.0, 0.3000002,
                0.5999999, 0.9000001, 1.1999998, 1.5, 1.8000002, 2.1, 2.4, 2.7000003, 3.0,
                3.3000002, 3.6000004, 3.9000006, 4.2, 4.5
            ]
        );
        assert_eq!(
            z_positions,
            vec![
                -4.8, -4.5, -4.2000003, -3.9, -3.6000001, -3.3000002, -3.0, -2.7, -2.4, -2.1000001,
                -1.8000002, -1.5, -1.2, -0.9000001, -0.5999999, -0.3000002, 0.0, 0.3000002,
                0.5999999, 0.9000001, 1.1999998, 1.5, 1.8000002, 2.1, 2.4, 2.7000003, 3.0,
                3.3000002, 3.6000004, 3.9000006, 4.2, 4.5
            ]
        );
    }

    #[test]
    fn test_position_converts() {
        let positions = vec![
            Position { x: 0, y: 0 },
            Position { x: 1, y: 0 },
            Position { x: 1, y: 1 },
        ];
        for position in positions {
            let t = convert_position_into_bevy_coords(&CONFIGURATION, &position);
            let p = convert_bevy_coords_into_position(&CONFIGURATION, &t);
            assert_eq!(position, p);
        }
    }
}
