#![feature(const_fn_floating_point_arithmetic)]

mod building;
mod navigator;
mod palatability;
mod plugin;
mod position;

use std::collections::HashSet;

use bevy::{input::keyboard::KeyboardInput, prelude::*};
use bevy_mod_picking::*;

use plugin::{
    building::BuildingPlugin, navigator::NavigatorPlugin, palatability::PalatabilityPlugin,
    CONFIGURATION,
};

#[derive(Component, Deref, DerefMut)]
struct GameTimer(Timer);
#[derive(Component)]
struct GameTick(u32);

struct PbrBundles {
    house: PbrBundle,
    street: PbrBundle,
    garden: PbrBundle,
    in_progress: PbrBundle,
}
impl PbrBundles {
    pub fn house(&self) -> PbrBundle {
        self.house.clone()
    }
    pub fn street(&self) -> PbrBundle {
        self.street.clone()
    }
    pub fn garden(&self) -> PbrBundle {
        self.garden.clone()
    }
    pub fn in_progress(&self) -> PbrBundle {
        self.in_progress.clone()
    }
}

impl FromWorld for PbrBundles {
    fn from_world(world: &mut World) -> Self {
        let configuration = &CONFIGURATION;

        let (house_mesh, street_mesh, garden_mesh, in_progress_mesh) = {
            let mut meshes = world
                .get_resource_mut::<Assets<Mesh>>()
                .expect("Mesh resource should be already created");

            (
                meshes.add(Mesh::from(shape::Cube {
                    size: configuration.cube_size,
                })),
                meshes.add(Mesh::from(shape::Plane {
                    size: configuration.cube_size,
                })),
                meshes.add(Mesh::from(shape::Plane {
                    size: configuration.cube_size,
                })),
                meshes.add(Mesh::from(shape::Plane {
                    size: configuration.cube_size,
                })),
            )
        };
        let (house_material, street_material, garden_material, in_progress_material) = {
            let mut materials = world
                .get_resource_mut::<Assets<StandardMaterial>>()
                .expect("StandardMaterial should be already created");

            (
                materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
                materials.add(Color::rgb(0.7, 0.7, 0.7).into()),
                materials.add(Color::rgb(0.2, 0.7, 0.2).into()),
                materials.add(Color::rgb(0.8, 0.8, 0.8).into()),
            )
        };

        let house = PbrBundle {
            mesh: house_mesh,
            material: house_material,
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        };

        let street = PbrBundle {
            mesh: street_mesh,
            material: street_material,
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        };

        let garden = PbrBundle {
            mesh: garden_mesh,
            material: garden_material,
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        };

        let in_progress = PbrBundle {
            mesh: in_progress_mesh,
            material: in_progress_material,
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        };

        PbrBundles {
            house,
            street,
            garden,
            in_progress,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugCursorPickingPlugin) // <- Adds the green debug cursor.
        .add_startup_system(setup)
        .add_event::<GameTick>()
        .add_system_to_stage(CoreStage::Update, tick)
        .add_system_to_stage(CoreStage::PostUpdate, move_camera_on_keyboard_input)
        .init_resource::<PbrBundles>()
        .add_plugin(BuildingPlugin)
        .add_plugin(NavigatorPlugin)
        .add_plugin(PalatabilityPlugin)
        .run();
}

fn tick(
    time: Res<Time>,
    mut game_timers: Query<&mut GameTimer>,
    mut my_events: EventWriter<GameTick>,
) {
    let mut game_timers = game_timers.single_mut();
    if !game_timers.tick(time.delta()).finished() {
        return;
    }

    debug!("tick!");

    my_events.send(GameTick(game_timers.0.times_finished()));
}

fn move_camera_on_keyboard_input(
    mut keyboard_input_events: EventReader<KeyboardInput>,
    mut cameras: Query<&mut Transform, With<CameraComponent>>,
    timer: Res<Time>,
) {
    let directional_events: HashSet<_> = keyboard_input_events
        .iter()
        .filter_map(|e| match e.key_code {
            Some(code)
                if code == KeyCode::Right
                    || code == KeyCode::Up
                    || code == KeyCode::Down
                    || code == KeyCode::Left =>
            {
                Some(code)
            }
            _ => None,
        })
        .collect();
    let mut delta = (0., 0.);
    if directional_events.contains(&KeyCode::Right) {
        delta.0 += CONFIGURATION.camera_velocity;
        delta.1 -= CONFIGURATION.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Left) {
        delta.0 -= CONFIGURATION.camera_velocity;
        delta.1 += CONFIGURATION.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Up) {
        delta.0 -= CONFIGURATION.camera_velocity;
        delta.1 -= CONFIGURATION.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Down) {
        delta.0 += CONFIGURATION.camera_velocity;
        delta.1 += CONFIGURATION.camera_velocity;
    };
    if delta != (0., 0.) {
        let mut camera = cameras.single_mut();
        (*camera).translation += Vec3::new(delta.0, 0., delta.1) * timer.delta_seconds();
    }
}

#[derive(Component, Debug)]
struct CameraComponent;

fn setup(mut commands: Commands) {
    // set up the camera
    let mut camera = OrthographicCameraBundle::new_3d();
    camera.orthographic_projection.scale = 3.0;
    camera.transform = Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y);

    // camera
    commands
        .spawn_bundle(camera)
        .insert(CameraComponent)
        .insert_bundle(PickingCameraBundle::default());

    // light
    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 5000.,
            color: Color::WHITE,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4),
            ..default()
        },
        ..default()
    });

    commands
        .spawn()
        .insert(GameTimer(Timer::from_seconds(1.0, true)));
}

#[cfg(test)]
mod tests {
    use crate::{
        plugin::{convert_bevy_coords_into_position, convert_position_into_bevy_coords},
        position::Position,
    };

    use super::*;

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
