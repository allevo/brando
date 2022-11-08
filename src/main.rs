#![feature(const_fn_floating_point_arithmetic)]

mod building;
mod common;
mod inhabitant;
mod navigation;
mod palatability;
mod power;

use std::{collections::HashSet, sync::Arc};

use bevy::{input::keyboard::KeyboardInput, prelude::*, render::camera::ScalingMode, time::Time};
use bevy_mod_picking::*;

use building::plugin::BuildingPlugin;
use common::configuration::{Configuration, CONFIGURATION};
use inhabitant::plugin::InhabitantPlugin;
use navigation::plugin::NavigatorPlugin;
use palatability::plugin::PalatabilityPlugin;
use power::plugin::PowerPlugin;
use tracing::debug;

#[derive(Component, Deref, DerefMut)]
struct GameTimer(Timer);
#[derive(Component)]
struct GameTick(u32);

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
        let configuration = world.resource::<Arc<Configuration>>().clone();

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
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(Arc::new(CONFIGURATION))
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
    configuration: Res<Arc<Configuration>>,
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
        .spawn()
        .insert(GameTimer(Timer::from_seconds(1.0, true)));
}

#[cfg(test)]
mod tests {
    use crate::{
        building::{
            manager::BuildingManager,
            plugin::{HouseComponent, OfficeComponent, PlaneComponent},
            BuildingSnapshot,
        },
        common::EntityId,
        palatability::manager::PalatabilityManager,
    };
    use bevy::prelude::{Entity, KeyCode, With};

    use helpers::*;

    #[test]
    fn test_main() {
        let mut app = create_app();

        let entities = get_entities!(app, (Entity, &PlaneComponent), PlaneComponent);
        let house_entity = get_at(&entities, 1, 2);
        let street_entity_1 = get_at(&entities, 0, 0);
        let street_entity_2 = get_at(&entities, 0, 1);
        let street_entity_3 = get_at(&entities, 0, 2);

        let house_entity_2 = get_at(&entities, 1, 1);
        let garden_entity = get_at(&entities, 2, 1);

        release_keyboard_key(&mut app, KeyCode::H);
        run(&mut app, 1);
        select_plane(&mut app, &house_entity);
        run(&mut app, 1);

        release_keyboard_key(&mut app, KeyCode::S);
        run(&mut app, 1);
        select_plane(&mut app, &street_entity_1);
        run(&mut app, 1);
        select_plane(&mut app, &street_entity_2);
        run(&mut app, 1);
        select_plane(&mut app, &street_entity_3);
        run(&mut app, 1);

        run(&mut app, 50);

        // Home is fulfilled
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 8);

        let houses: Vec<(Entity, &HouseComponent)> =
            get_entities!(app, (Entity, &HouseComponent), HouseComponent);
        let house_id: EntityId = houses.get(0).unwrap().1 .0;

        let house = {
            let building_manager = app.world.get_resource::<BuildingManager>().unwrap();
            let house: BuildingSnapshot =
                BuildingSnapshot::from(building_manager.get_building(&house_id));
            house.into_house()
        };
        assert_eq!(house.current_residents, house.max_residents);

        release_keyboard_key(&mut app, KeyCode::H);
        run(&mut app, 1);
        select_plane(&mut app, &house_entity_2);
        run(&mut app, 50);

        // palatability is not sufficient, so population count doesn't change
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 8);

        release_keyboard_key(&mut app, KeyCode::G);
        run(&mut app, 1);
        select_plane(&mut app, &garden_entity);
        run(&mut app, 60);

        // Homes are fulfilled
        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(palatability_manager.total_populations(), 16);

        // Check houses: both are fulfilled
        let houses: Vec<(Entity, &HouseComponent)> =
            get_entities!(app, (Entity, &HouseComponent), HouseComponent);
        let house1_id: EntityId = houses.get(0).unwrap().1 .0;
        let house2_id: EntityId = houses.get(1).unwrap().1 .0;

        let building_manager = app.world.get_resource::<BuildingManager>().unwrap();

        let house = BuildingSnapshot::from(building_manager.get_building(&house1_id)).into_house();
        assert_eq!(house.current_residents, house.max_residents);
        let house = BuildingSnapshot::from(building_manager.get_building(&house2_id)).into_house();
        assert_eq!(house.current_residents, house.max_residents);

        run(&mut app, 50);

        let palatability_manager = app.world.get_resource::<PalatabilityManager>().unwrap();
        assert_eq!(16, palatability_manager.unemployed_inhabitants().len());
        assert_eq!(0, palatability_manager.vacant_work());
        assert_eq!(0, palatability_manager.vacant_inhabitants());
    }

    #[test]
    fn test_create_office() {
        let mut app = create_app();
        let map = r#"
s
sg
sh
s
s
s"#;
        fill_map(&mut app, map, 8);

        run(&mut app, 20);

        let entities = get_entities!(app, (Entity, &PlaneComponent), PlaneComponent);

        let office_entity = get_at(&entities, 3, 1);
        release_keyboard_key(&mut app, KeyCode::O);
        run(&mut app, 1);
        select_plane(&mut app, &office_entity);
        run(&mut app, 1);
        release_keyboard_key(&mut app, KeyCode::O);
        run(&mut app, 1);

        run(&mut app, 20);

        let mut offices = get_entities!(app, (Entity, &OfficeComponent), OfficeComponent);
        assert_eq!(offices.len(), 1);
        let office_id: EntityId = offices.pop().unwrap().1 .0;

        let building_manager = app.world.get_resource::<BuildingManager>().unwrap();
        let office =
            BuildingSnapshot::from(building_manager.get_building(&office_id)).into_office();

        assert_eq!(office.max_workers, office.current_workers);
    }

    #[test]
    fn test_position_is_already_occupied() {
        let mut app = create_app();

        let entities = get_entities!(app, (Entity, &PlaneComponent), PlaneComponent);
        let house_entity = get_at(&entities, 1, 2);

        release_keyboard_key(&mut app, KeyCode::H);
        run(&mut app, 1);
        select_plane(&mut app, &house_entity);
        run(&mut app, 20);

        let houses = get_entities!(app, Entity, HouseComponent);
        assert_eq!(houses.len(), 1);

        release_keyboard_key(&mut app, KeyCode::H);
        run(&mut app, 1);
        select_plane(&mut app, &house_entity);
        run(&mut app, 20);

        let houses = get_entities!(app, Entity, HouseComponent);
        assert_eq!(houses.len(), 1);
    }

    mod helpers {
        use std::sync::Arc;

        use crate::{
            building::plugin::PlaneComponent, common::configuration::CONFIGURATION,
            palatability::manager::PalatabilityManager, GameTick, MainPlugin,
        };
        use bevy::{
            input::ButtonState,
            prelude::{App, Entity, KeyCode, With},
            time::TimePlugin,
        };

        macro_rules! get_entities {
            ($app: ident, $Q: tt, $F: ident) => {{
                let entities = {
                    let world = &mut $app.world;
                    let mut query = world.query_filtered::<$Q, With<$F>>();
                    let query = query.iter(world);
                    let entities = query.collect::<Vec<$Q>>();

                    entities.clone()
                };
                entities
            }};
        }
        pub(crate) use get_entities;

        #[inline]
        pub fn get_at(entities: &[(Entity, &PlaneComponent)], x: i64, y: i64) -> Entity {
            *entities
                .iter()
                .find(|(_, p)| p.0.x == x && p.0.y == y)
                .map(|(e, _)| e)
                .unwrap()
        }

        pub fn run(app: &mut App, run: usize) {
            (0..run).for_each(|_| {
                use bevy::ecs::event::Events;

                let world = &mut app.world;
                let mut game_tick = world.get_resource_mut::<Events<GameTick>>().unwrap();

                game_tick.send(GameTick(0));
                app.update();
            });
        }

        pub fn release_keyboard_key(app: &mut App, code: KeyCode) {
            use bevy::{ecs::event::Events, input::keyboard::KeyboardInput};

            let world = &mut app.world;
            let mut keyboard_input = world.get_resource_mut::<Events<KeyboardInput>>().unwrap();
            keyboard_input.send(KeyboardInput {
                scan_code: 0,
                key_code: Some(code),
                state: ButtonState::Released,
            });
        }

        pub fn select_plane(app: &mut App, entity: &Entity) {
            use bevy::ecs::event::Events;
            use bevy_mod_picking::PickingEvent;

            let world = &mut app.world;
            let mut picking_event = world.get_resource_mut::<Events<PickingEvent>>().unwrap();
            picking_event.send(PickingEvent::Clicked(*entity));
        }

        pub fn create_app() -> App {
            use bevy::{
                asset::AssetPlugin,
                core::CorePlugin,
                core_pipeline::CorePipelinePlugin,
                hierarchy::HierarchyPlugin,
                input::InputPlugin,
                log::LogSettings,
                pbr::PbrPlugin,
                prelude::Camera,
                render::{camera::RenderTarget, RenderPlugin},
                scene::ScenePlugin,
                sprite::SpritePlugin,
                text::TextPlugin,
                transform::TransformPlugin,
                ui::UiPlugin,
                utils::tracing::subscriber::set_global_default,
                window::{WindowId, WindowPlugin},
            };
            use tracing_log::LogTracer;
            use tracing_subscriber::{prelude::*, registry::Registry, EnvFilter};

            if LogTracer::init().is_ok() {
                let filter_layer = EnvFilter::try_from_default_env()
                    .or_else(|_| EnvFilter::try_new("OFF,brando=INFO"))
                    .unwrap();
                let subscriber = Registry::default().with(filter_layer);
                let fmt_layer = tracing_subscriber::fmt::Layer::default();
                let subscriber = subscriber.with(fmt_layer);
                set_global_default(subscriber).unwrap();
            }

            let mut app = App::new();

            app.world.clear_entities();
            app.world.clear_trackers();

            let mut log_settings = LogSettings::default();
            log_settings.filter = format!("{},bevy_mod_raycast=off", log_settings.filter);
            app.insert_resource(log_settings);

            app.add_plugin(CorePlugin::default());
            app.add_plugin(TimePlugin::default());
            app.add_plugin(TransformPlugin::default());
            app.add_plugin(HierarchyPlugin::default());
            // app.add_plugin(bevy_diagnostic::DiagnosticsPlugin::default());
            app.add_plugin(InputPlugin::default());
            app.add_plugin(WindowPlugin {
                // add_primary_window: true,
                // exit_on_close: false,
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
                let camera = Camera {
                    target: RenderTarget::Window(WindowId::primary()),
                    ..Camera::default()
                };
                app.insert_resource(camera);
            }

            app.insert_resource(Arc::new(CONFIGURATION));

            app.add_plugin(MainPlugin);

            run(&mut app, 1);

            app
        }

        pub fn fill_map(app: &mut App, map: &str, expected_population: u64) {
            for (x, line) in map.lines().skip(1).enumerate() {
                for (y, c) in line.chars().enumerate() {
                    let entities = get_entities!(app, (Entity, &PlaneComponent), PlaneComponent);
                    let entity = get_at(&entities, x as i64, y as i64);
                    match c {
                        's' => release_keyboard_key(app, KeyCode::S),
                        'g' => release_keyboard_key(app, KeyCode::G),
                        'h' => release_keyboard_key(app, KeyCode::H),
                        'o' => release_keyboard_key(app, KeyCode::O),
                        _ => continue,
                    }
                    run(app, 1);
                    select_plane(app, &entity);
                    run(app, 1);
                }
            }

            let mut max_iter = 50;
            loop {
                run(app, 3);
                let palatability = app.world.get_resource_mut::<PalatabilityManager>().unwrap();
                if palatability.total_populations() == expected_population {
                    break;
                }
                max_iter -= 1;
                if max_iter == 0 {
                    panic!(
                        "Unable to reach expected population of {}. Current: {}",
                        expected_population,
                        palatability.total_populations()
                    );
                }
            }
        }
    }
}
