use crate::{building::*, common::position::Position};
use bevy::{
    ecs::query::{ReadOnlyWorldQuery, WorldQuery},
    prelude::{Component, Entity, KeyCode, Resource},
};

use std::sync::Arc;

use crate::{
    common::configuration::CONFIGURATION, resources::ConfigurationResource, GameTick, MainPlugin,
};
use bevy::{
    input::ButtonState,
    prelude::{default, App, ImagePlugin},
    time::TimePlugin,
};

pub fn create_house_at(app: &mut App, position: &Position) {
    create_building_at(app, position, KeyCode::H);
}

pub fn create_street_at(app: &mut App, position: &Position) {
    create_building_at(app, position, KeyCode::S);
}

pub fn create_office_at(app: &mut App, position: &Position) {
    create_building_at(app, position, KeyCode::O);
}
pub fn create_garden_at(app: &mut App, position: &Position) {
    create_building_at(app, position, KeyCode::G);
}

fn create_building_at(app: &mut App, position: &Position, code: KeyCode) {
    release_keyboard_key(app, code);
    run(app, 1);
    let plane_entity = get_plane_at(app, position);
    select_plane(app, &plane_entity);
    run(app, 1);
}

pub fn get_manager_resource_mut<T: Resource>(app: &mut App) -> &'_ mut T {
    app.world.get_resource_mut::<T>().unwrap().into_inner()
}

pub fn assert_house_is_built_at(app: &mut App, position: &Position) {
    let _house = get_house_snapshot_at(app, position);
}
pub fn assert_street_is_built_at(app: &mut App, position: &Position) {
    let _street = get_street_snapshot_at(app, position);
}
pub fn assert_office_is_built_at(app: &mut App, position: &Position) {
    let _office = get_office_snapshot_at(app, position);
}

pub fn get_house_snapshot_at(app: &mut App, position: &Position) -> Option<HouseSnapshot> {
    let house = get_snapshot_at(app, position);
    match house {
        None => None,
        Some(BuildingSnapshot::House(h)) => Some(h),
        _ => panic!(
            "building at {:?} is not an house but is {:?}",
            position, house
        ),
    }
}
pub fn get_street_snapshot_at(app: &mut App, position: &Position) -> Option<StreetSnapshot> {
    let street = get_snapshot_at(app, position);
    match street {
        None => None,
        Some(BuildingSnapshot::Street(s)) => Some(s),
        _ => panic!(
            "building at {:?} is not an house but is {:?}",
            position, street
        ),
    }
}
pub fn get_office_snapshot_at(app: &mut App, position: &Position) -> Option<OfficeSnapshot> {
    let office = get_snapshot_at(app, position);
    match office {
        None => None,
        Some(BuildingSnapshot::Office(o)) => Some(o),
        _ => panic!(
            "building at {:?} is not an office but is {:?}",
            position, office
        ),
    }
}
pub fn get_garden_snapshot_at(app: &mut App, position: &Position) -> Option<GardenSnapshot> {
    let garden = get_snapshot_at(app, position);
    match garden {
        None => None,
        Some(BuildingSnapshot::Garden(g)) => Some(g),
        _ => panic!(
            "building at {:?} is not an garden but is {:?}",
            position, garden
        ),
    }
}
fn get_snapshot_at(app: &mut App, position: &Position) -> Option<BuildingSnapshot> {
    let plane_entity = get_plane_at(app, position);
    let building_manager = app.world.get_resource::<BuildingManagerResource>().unwrap();
    building_manager
        .get_building(&plane_entity.to_bits())
        .map(BuildingSnapshot::from)
}

pub fn get_plane_at(app: &mut App, position: &Position) -> Entity {
    let world = &mut app.world;
    let mut query = world.query::<(Entity, &PlaneComponent)>();
    query
        .iter(world)
        .find(|(_, p)| p.0 == *position)
        .map(|(e, _)| e)
        .unwrap()
}

pub fn get_component_at<'app, T: Component>(
    app: &'app mut App,
    position: &Position,
) -> Option<&'app T> {
    let world = &mut app.world;
    let mut query = world.query::<(Entity, &PlaneComponent, &T)>();
    query
        .iter(world)
        .find(|(_, p, _)| p.0 == *position)
        .map(|(_, _, c)| c)
}

pub fn get_entities<Comp: WorldQuery, WithComp: ReadOnlyWorldQuery>(
    app: &mut App,
) -> Vec<<<Comp as WorldQuery>::ReadOnly as WorldQuery>::Item<'_>> {
    let world = &mut app.world;
    let mut query = world.query_filtered::<Comp, WithComp>();
    let items = query.iter(world).collect::<Vec<_>>();
    items
}

pub fn run(app: &mut App, run: usize) {
    let mut c = 0;
    run_till(app, |_| {
        c += 1;
        c > run
    })
}

pub fn run_till<F>(app: &mut App, mut f: F)
where
    F: FnMut(&mut App) -> bool,
{
    use bevy::ecs::event::Events;
    let mut i = 1;
    loop {
        let world = &mut app.world;
        let mut game_tick = world.get_resource_mut::<Events<GameTick>>().unwrap();

        game_tick.send(GameTick(0));
        app.update();

        if !f(app) {
            break;
        }

        i += 1;
        assert!(i < 100);
    }
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
        asset::AssetPlugin, core::CorePlugin, core_pipeline::CorePipelinePlugin,
        hierarchy::HierarchyPlugin, input::InputPlugin, pbr::PbrPlugin, render::RenderPlugin,
        scene::ScenePlugin, sprite::SpritePlugin, text::TextPlugin, transform::TransformPlugin,
        ui::UiPlugin, utils::tracing::subscriber::set_global_default, window::WindowPlugin,
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

    app.add_plugin(CorePlugin::default())
        .add_plugin(TimePlugin::default())
        .add_plugin(TransformPlugin::default())
        .add_plugin(HierarchyPlugin::default())
        .add_plugin(InputPlugin::default())
        .add_plugin(WindowPlugin {
            add_primary_window: true,
            ..default() // exit_on_close: false,
        })
        .add_plugin(AssetPlugin::default())
        .add_plugin(ScenePlugin::default())
        .add_plugin(RenderPlugin::default())
        .add_plugin(CorePipelinePlugin::default())
        .add_plugin(TextPlugin::default())
        .add_plugin(UiPlugin::default())
        .add_plugin(ImagePlugin::default())
        .add_plugin(PbrPlugin::default())
        .add_plugin(SpritePlugin::default());

    app.insert_resource(ConfigurationResource(Arc::new(CONFIGURATION)));

    app.add_plugin(MainPlugin);

    run(&mut app, 1);

    app
}
