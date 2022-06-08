#![feature(const_fn_floating_point_arithmetic)]

mod building;
mod common;
mod navigation;
mod palatability;

use std::collections::HashSet;

use bevy::{input::keyboard::KeyboardInput, prelude::*};
use bevy_mod_picking::*;

use building::plugin::BuildingPlugin;
use common::configuration::CONFIGURATION;
use navigation::plugin::NavigatorPlugin;
use palatability::plugin::PalatabilityPlugin;

#[derive(Component, Deref, DerefMut)]
struct GameTimer(Timer);
#[derive(Component)]
struct GameTick(u32);

struct PbrBundles {
    house: PbrBundle,
    street: PbrBundle,
    garden: PbrBundle,
    office: PbrBundle,
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
        let configuration = &CONFIGURATION;

        let house = get_colored_plane!(cube world, configuration, 150, 150, 150);
        let street = get_colored_plane!(plane world, configuration, 81, 81, 81);
        let garden = get_colored_plane!(plane world, configuration, 81, 112, 55);
        let in_progress = get_colored_plane!(plane world, configuration, 33, 33, 33);
        let office = get_colored_plane!(plane world, configuration, 31, 125, 219);

        PbrBundles {
            house,
            street,
            garden,
            in_progress,
            office,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugCursorPickingPlugin) // <- Adds the green debug cursor.
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
            .add_plugin(PalatabilityPlugin);
    }
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
        common::configuration::CONFIGURATION, palatability::manager::PalatabilityManager, GameTick,
        MainPlugin,
    };
    use helpers::*;

    #[test]
    fn test_main() {
        let mut app = create_app!();

        let entities = get_entities!(planes app);
        let house_entity = entities.get(position_to_index(1, 2)).unwrap();
        let street_entity_1 = entities.get(position_to_index(0, 0)).unwrap();
        let street_entity_2 = entities.get(position_to_index(0, 1)).unwrap();
        let street_entity_3 = entities.get(position_to_index(0, 2)).unwrap();

        let house_entity_2 = entities.get(position_to_index(1, 1)).unwrap();
        let garden_entity = entities.get(position_to_index(2, 1)).unwrap();

        release_keyboard_key!(app, KeyCode::H);
        run!(app, 1);
        select_plane!(app, house_entity);
        run!(app, 20);

        release_keyboard_key!(app, KeyCode::S);
        run!(app, 1);
        select_plane!(app, street_entity_1);
        run!(app, 1);
        select_plane!(app, street_entity_2);
        run!(app, 1);
        select_plane!(app, street_entity_3);
        run!(app, 1);

        run!(app, 40);

        // Home is fulfilled
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 8);

        release_keyboard_key!(app, KeyCode::H);
        run!(app, 1);
        select_plane!(app, house_entity_2);
        run!(app, 20);

        // palatability is not sufficient, so population count doesn't change
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 8);

        release_keyboard_key!(app, KeyCode::G);
        run!(app, 1);
        select_plane!(app, garden_entity);
        run!(app, 40);

        // Homes are fulfilled
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 16);
    }

    #[test]
    fn test_position_is_already_occupated() {
        let mut app = create_app!();

        let entities = get_entities!(planes app);
        let position = position_to_index(1, 2);
        let house_entity = entities.get(position).unwrap();

        release_keyboard_key!(app, KeyCode::H);
        run!(app, 1);
        select_plane!(app, house_entity);
        run!(app, 20);

        let houses = get_entities!(houses app);
        assert_eq!(houses.len(), 1);

        release_keyboard_key!(app, KeyCode::H);
        run!(app, 1);
        select_plane!(app, house_entity);
        run!(app, 20);

        let houses = get_entities!(houses app);
        assert_eq!(houses.len(), 1);
    }

    fn position_to_index(x: usize, y: usize) -> usize {
        CONFIGURATION.game.width_table * x + y
    }

    mod helpers {

        macro_rules! run {
            ($app: ident, $n: expr) => {
                (0..$n).for_each(|_| {
                    use bevy::ecs::event::Events;

                    let world = &mut $app.world;
                    let mut game_tick = world.get_resource_mut::<Events<GameTick>>().unwrap();

                    game_tick.send(GameTick(0));
                    $app.update();
                });
            };
        }

        macro_rules! release_keyboard_key {
            ($app: ident, $code: path) => {{
                use bevy::{
                    ecs::event::Events,
                    input::{keyboard::KeyboardInput, ElementState},
                    prelude::KeyCode,
                };

                let world = &mut $app.world;
                let mut keyboard_input = world.get_resource_mut::<Events<KeyboardInput>>().unwrap();
                keyboard_input.send(KeyboardInput {
                    scan_code: 0,
                    key_code: Some($code),
                    state: ElementState::Released,
                });
            }};
        }

        macro_rules! select_plane {
            ($app: ident, $entity: ident) => {{
                use bevy::ecs::event::Events;
                use bevy_mod_picking::PickingEvent;

                let world = &mut $app.world;
                let mut picking_event = world.get_resource_mut::<Events<PickingEvent>>().unwrap();
                picking_event.send(PickingEvent::Clicked(*$entity));
            }};
        }

        macro_rules! create_app {
            () => {{
                use bevy::{
                    asset::AssetPlugin,
                    core::CorePlugin,
                    core_pipeline::CorePipelinePlugin,
                    hierarchy::HierarchyPlugin,
                    input::InputPlugin,
                    log::LogSettings,
                    pbr::PbrPlugin,
                    prelude::{App, Camera},
                    render::{camera::RenderTarget, RenderPlugin},
                    scene::ScenePlugin,
                    sprite::SpritePlugin,
                    text::TextPlugin,
                    transform::TransformPlugin,
                    ui::UiPlugin,
                    window::{WindowId, WindowPlugin},
                };

                let mut app = App::new();

                app.world.clear_entities();
                app.world.clear_trackers();

                let mut log_settings = LogSettings::default();
                log_settings.filter = format!("{},bevy_mod_raycast=off", log_settings.filter);
                app.insert_resource(log_settings);

                // uncomment the following line after https://github.com/bevyengine/bevy/issues/4934
                // app.add_plugin(LogPlugin::default());
                app.add_plugin(CorePlugin::default());
                app.add_plugin(TransformPlugin::default());
                app.add_plugin(HierarchyPlugin::default());
                // app.add_plugin(bevy_diagnostic::DiagnosticsPlugin::default());
                app.add_plugin(InputPlugin::default());
                app.add_plugin(WindowPlugin {
                    add_primary_window: true,
                    exit_on_close: false,
                });
                app.add_plugin(AssetPlugin::default());
                // app.add_plugin(DebugAssetServerPlugin::default());
                app.add_plugin(ScenePlugin::default());
                // app.add_plugin(WinitPlugin::default());
                app.add_plugin(RenderPlugin::default());
                app.add_plugin(CorePipelinePlugin::default());
                app.add_plugin(SpritePlugin::default());
                app.add_plugin(TextPlugin::default());
                app.add_plugin(UiPlugin::default());
                app.add_plugin(PbrPlugin::default());
                // app.add_plugin(GltfPlugin::default());
                // app.add_plugin(bevy_audio::AudioPlugin::default());
                // app.add_plugin(GilrsPlugin::default());
                // app.add_plugin(bevy_animation::AnimationPlugin::default());

                {
                    let mut camera = Camera::default();
                    // let mut camera = app.world.get_resource_mut::<Camera>().unwrap();
                    camera.target = RenderTarget::Window(WindowId::primary());
                    app.insert_resource(camera);
                }

                app.add_plugin(MainPlugin);

                run!(app, 1);

                app
            }};
        }

        macro_rules! get_entities {
            ($app: ident, $t: path) => {{
                use bevy::prelude::{Entity, With};

                let world = &mut $app.world;
                let mut query = world.query_filtered::<Entity, With<$t>>();
                let query = query.iter(world);
                query.collect::<Vec<_>>()
            }};
            (planes $app: ident) => {{
                get_entities!($app, crate::building::plugin::PlaneComponent)
            }};
            (houses $app: ident) => {{
                get_entities!($app, crate::building::plugin::HouseComponent)
            }};
        }

        pub(crate) use create_app;
        pub(crate) use get_entities;
        pub(crate) use release_keyboard_key;
        pub(crate) use run;
        pub(crate) use select_plane;
    }
}
