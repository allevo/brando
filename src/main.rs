#![feature(const_fn_floating_point_arithmetic)]

mod building;
mod common;
mod inhabitant;
mod navigation;
mod palatability;
mod power;

#[cfg(test)]
mod e2e;

use std::{collections::HashSet, sync::Arc};

use bevy::{input::keyboard::KeyboardInput, prelude::*, render::camera::ScalingMode, time::Time};
use bevy_mod_picking::*;

use building::BuildingPlugin;
use common::configuration::CONFIGURATION;
use inhabitant::InhabitantPlugin;
use navigation::NavigatorPlugin;
use palatability::PalatabilityPlugin;
use power::PowerPlugin;
use resources::ConfigurationResource;
use tracing::debug;

#[derive(Component, Deref, DerefMut)]
struct GameTimer(Timer);
#[derive(Component)]
struct GameTick(u32);

#[derive(Resource)]
struct PbrBundles {
    house: PbrBundle,
    street: PbrBundle,
    garden: PbrBundle,
    office: PbrBundle,
    biomass_power_plant: PbrBundle,
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
    pub fn office(&self) -> PbrBundle {
        self.office.clone()
    }
    pub fn biomass_power_plant(&self) -> PbrBundle {
        self.biomass_power_plant.clone()
    }
    pub fn in_progress(&self) -> PbrBundle {
        self.in_progress.clone()
    }
}

macro_rules! get_colored_plane {
    ($world: ident, $configuration: ident, $type: tt, $r: literal, $g: literal, $b: literal) => {{
        let mesh = {
            let mut meshes = $world
                .get_resource_mut::<Assets<Mesh>>()
                .expect("Mesh resource should be already created");
            meshes.add(Mesh::from(shape::$type {
                size: $configuration.cube_size,
            }))
        };
        let material = {
            let mut materials = $world
                .get_resource_mut::<Assets<StandardMaterial>>()
                .expect("StandardMaterial should be already created");
            materials.add(Color::rgb($r as f32 / 255., $g as f32 / 255., $b as f32 / 255.).into())
        };
        PbrBundle {
            mesh,
            material,
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        }
    }};
    (plane $world: ident, $configuration: ident, $r: literal, $g: literal, $b: literal) => {
        get_colored_plane!($world, $configuration, Plane, $r, $g, $b)
    };
    (cube $world: ident, $configuration: ident, $r: literal, $g: literal, $b: literal) => {
        get_colored_plane!($world, $configuration, Cube, $r, $g, $b)
    };
}

impl FromWorld for PbrBundles {
    fn from_world(world: &mut World) -> Self {
        let configuration = (*(world.resource::<ConfigurationResource>())).clone();

        let house = get_colored_plane!(cube world, configuration, 150, 150, 150);
        let street = get_colored_plane!(plane world, configuration, 81, 81, 81);
        let garden = get_colored_plane!(plane world, configuration, 81, 112, 55);
        let in_progress = get_colored_plane!(plane world, configuration, 33, 33, 33);
        let office = get_colored_plane!(plane world, configuration, 31, 125, 219);
        let biomass_power_plant = get_colored_plane!(plane world, configuration, 197, 34, 34);

        PbrBundles {
            house,
            street,
            garden,
            in_progress,
            office,
            biomass_power_plant,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugCursorPickingPlugin) // <- Adds the green debug cursor.
        .insert_resource(ConfigurationResource(Arc::new(CONFIGURATION)))
        .add_plugin(MainPlugin)
        .run();
}

pub struct MainPlugin;

impl Plugin for MainPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup)
            .add_event::<GameTick>()
            .add_system_to_stage(CoreStage::Update, tick)
            .add_system_to_stage(CoreStage::PostUpdate, move_camera_on_keyboard_input)
            .init_resource::<PbrBundles>()
            .add_plugin(BuildingPlugin)
            .add_plugin(NavigatorPlugin)
            .add_plugin(InhabitantPlugin)
            .add_plugin(PalatabilityPlugin)
            .add_plugin(PowerPlugin);
    }
}

/// Send game tick: realtime is just an interpolation of discrete time
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

    my_events.send(GameTick(game_timers.0.times_finished_this_tick()));
}

/// Allow to move the camera
fn move_camera_on_keyboard_input(
    mut keyboard_input_events: EventReader<KeyboardInput>,
    mut cameras: Query<&mut Transform, With<CameraComponent>>,
    configuration: Res<ConfigurationResource>,
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
        delta.0 += configuration.camera_velocity;
        delta.1 -= configuration.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Left) {
        delta.0 -= configuration.camera_velocity;
        delta.1 += configuration.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Up) {
        delta.0 -= configuration.camera_velocity;
        delta.1 -= configuration.camera_velocity;
    }
    if directional_events.contains(&KeyCode::Down) {
        delta.0 += configuration.camera_velocity;
        delta.1 += configuration.camera_velocity;
    };
    if delta != (0., 0.) {
        let mut camera = cameras.single_mut();
        camera.translation += Vec3::new(delta.0, 0., delta.1) * timer.delta_seconds();
    }
}

#[derive(Component, Debug)]
struct CameraComponent;

fn setup(mut commands: Commands) {
    // set up the camera
    let mut camera = Camera3dBundle {
        projection: OrthographicProjection {
            scale: 3.0,
            scaling_mode: ScalingMode::FixedVertical(2.),
            ..default()
        }
        .into(),
        ..default()
    };
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
        .spawn_empty()
        .insert(GameTimer(Timer::from_seconds(1.0, TimerMode::Repeating)));
}

pub mod resources {
    use std::{ops::Deref, sync::Arc};

    use bevy::prelude::Resource;

    use crate::common::configuration::Configuration;

    #[derive(Resource)]
    pub struct ConfigurationResource(pub Arc<Configuration>);

    impl Deref for ConfigurationResource {
        type Target = Arc<Configuration>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}
