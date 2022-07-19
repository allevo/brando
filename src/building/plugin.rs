use std::sync::Arc;

use bevy::{
    input::{keyboard::KeyboardInput, ElementState},
    prelude::*,
};
use bevy_mod_picking::{DefaultPickingPlugins, PickableBundle, PickingEvent};

use crate::{
    building::{
        builder::BuildingBuilder, BuildRequest, Building, BuildingInConstruction, BuildingType, IntoBuilding,
    },
    common::{
        position::Position,
        position_utils::{convert_bevy_coords_into_position, convert_position_into_bevy_coords}, configuration::{Configuration},
    },
    navigation::plugin::events::InhabitantArrivedAtHomeEvent,
    palatability::manager::PalatabilityManager,
    GameTick, PbrBundles,
};

#[cfg(test)]
pub use components::*;
#[cfg(not(test))]
use components::*;

pub use events::*;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &Arc<Configuration> = app.world.resource();
        let brando = BuildingBuilder::new(configuration.clone());

        app.insert_resource(EditMode::None)
            .insert_resource(brando)
            .add_event::<BuildingCreatedEvent>()
            .add_plugins(DefaultPickingPlugins)
            .add_system_to_stage(CoreStage::PostUpdate, build_building)
            .add_startup_system(setup)
            .add_system_to_stage(CoreStage::PostUpdate, switch_edit_mode)
            .add_system_to_stage(CoreStage::PostUpdate, make_progress)
            .add_system_to_stage(CoreStage::PostUpdate, habit_house);
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum EditMode {
    None,
    House,
    Garden,
    Street,
    Office,
}

/// Allow the user to switch edit mode base on the keyboard key
fn switch_edit_mode(
    mut keyboard_input_events: EventReader<KeyboardInput>,
    mut edit_mode: ResMut<EditMode>,
) {
    if let Some(e) = keyboard_input_events
        .iter()
        .filter_map(|e| match (e.state, e.key_code) {
            (ElementState::Released, Some(KeyCode::S)) => Some(EditMode::Street),
            (ElementState::Released, Some(KeyCode::G)) => Some(EditMode::Garden),
            (ElementState::Released, Some(KeyCode::H)) => Some(EditMode::House),
            (ElementState::Released, Some(KeyCode::O)) => Some(EditMode::Office),
            (ElementState::Released, Some(KeyCode::Escape)) => Some(EditMode::None),
            _ => None,
        })
        .next()
    {
        info!("Switch EditMode to {:?}", e);
        *edit_mode = e;
    }
}

/// Spawn entity with `BuildingInConstructionComponent`
fn build_building(
    mut events: EventReader<PickingEvent>,
    planes: Query<&Transform, With<PlaneComponent>>,
    edit_mode: Res<EditMode>,
    mut brando: ResMut<BuildingBuilder>,
    mut commands: Commands,
    bundles: Res<PbrBundles>,
    configuration: Res<Arc<Configuration>>,
) {
    if *edit_mode == EditMode::None {
        return;
    }

    let entity = events
        .iter()
        .filter_map(|e| match e {
            PickingEvent::Clicked(e) => Some(e),
            _ => None,
        })
        .next();

    let entity = match entity {
        None => return,
        Some(entity) => entity,
    };

    let transform = *planes.get(*entity).unwrap();

    let position = convert_bevy_coords_into_position(&configuration, &transform.translation);

    let building_type = match *edit_mode {
        EditMode::House => BuildingType::House,
        EditMode::Garden => BuildingType::Garden,
        EditMode::Street => BuildingType::Street,
        EditMode::Office => BuildingType::Office,
        EditMode::None => unreachable!("EditMode::None is handled before"),
    };

    info!("Building {:?} at {:?}", building_type, position);

    let request = BuildRequest::new(position, building_type);
    let res = match brando.create_building(request) {
        Ok(res) => res,
        Err(s) => {
            error!("Error on creation building: {}", s);
            return;
        }
    };

    commands
        .entity(*entity)
        .insert(BuildingInConstructionComponent(res))
        .with_children(|parent| {
            let mut sprite = bundles.in_progress();
            sprite.transform.translation = Vec3::new(0., 0.0001, 0.);
            parent.spawn_bundle(sprite);
        });
}

/// Make BuildingInConstruction progress:
/// - if the building is not constructored yet, stop
/// - otherwise place the building
/// NB: the progress is made if and only if there's sufficient palatability
fn make_progress(
    events: EventReader<GameTick>,
    mut buildings_in_progress: Query<(Entity, &mut BuildingInConstructionComponent)>,
    brando: Res<BuildingBuilder>,
    palatability: Res<PalatabilityManager>,
    mut commands: Commands,
    bundles: Res<PbrBundles>,
    configuration: Res<Arc<Configuration>>,
    mut building_created_writer: EventWriter<BuildingCreatedEvent>,
) {
    // TODO: split the following logic among frames
    // truly this could be not so performant if buildings_in_progress contains a lot of elements
    // because we manage all the elements in a unique frame, this can block the rendering pipelines
    // Probably a more convenient solution is to split the logic among the frames in order to
    // process little by little them.
    // we can create a dedicated entity to store the entities ids when the events count is not 0
    // and process them little by little in the following frames.
    if events.is_empty() {
        return;
    }

    for (entity, mut building) in buildings_in_progress.iter_mut() {
        let position = &(building.0.request.position);

        // TODO: generalize this
        // Currently we implement only a type of building that, to be built,
        // needs to have a proper palatability.
        // So, for the time being, an "if" is enough
        if building.0.request.building_type == BuildingType::House {
            let p = palatability.get_house_palatability(position);
            // TODO: tag this entity in order to retry later
            // If an house hasn't enough palatability, we retry again and again
            // Probably this is not good at all: we can put a dedicated component to the entity
            // in order to deselect it avoiding the reprocessing.
            if !p.is_positive() {
                debug!("house palatability: insufficient at ({position:?})");
                continue;
            }
        }
        if building.0.request.building_type == BuildingType::Office {
            let p = palatability.get_office_palatability(position);
            // TODO: tag this entity in order to retry later
            // If an house hasn't enough palatability, we retry again and again
            // Probably this is not good at all: we can put a dedicated component to the entity
            // in order to deselect it avoiding the reprocessing.
            if !p.is_positive() {
                debug!("office palatability: insufficient at ({position:?})");
                continue;
            }
        }

        let building_in_construction: &mut BuildingInConstruction = &mut building.0;

        brando
            .make_progress(building_in_construction)
            .expect("make progress never fails");

        if !building_in_construction.is_completed() {
            continue;
        }

        info!(
            "{:?} completed at {:?}",
            building_in_construction.request.building_type, building_in_construction.request.position,
        );

        let building_type = &building_in_construction.request.building_type;
        let bundle = match building_type {
            BuildingType::House => bundles.house(),
            BuildingType::Garden => bundles.garden(),
            BuildingType::Street => bundles.street(),
            BuildingType::Office => bundles.office(),
        };

        let mut command = commands.entity(entity);
        command.despawn_descendants();
        command
            .remove::<BuildingInConstructionComponent>()
            .with_children(|parent| {
                parent.spawn_bundle(bundle);
            });

        // TODO: rework this part
        // This part need to be reworked in order to let it scalable
        // on the BuildingType enumeration growing.
        let entity = match building_type {
            BuildingType::House => command
                .insert_bundle((HouseComponent(building_in_construction.into_building(&*configuration).unwrap()),))
                .id(),
            BuildingType::Garden => command
                .insert(GardenComponent(
                    building_in_construction.into_building(&*configuration).unwrap(),
                ))
                .id(),
            BuildingType::Street => command
                .insert(StreetComponent(
                    building_in_construction.into_building(&*configuration).unwrap(),
                ))
                .id(),
            BuildingType::Office => command
                .insert_bundle((OfficeComponent(
                    building_in_construction.into_building(&*configuration).unwrap(),
                ),))
                .id(),
        };

        let building: Building = building_in_construction
            .into_building(&*configuration)
            .expect("Something goes wrong on building creation");

        building_created_writer.send(BuildingCreatedEvent { building, entity });
    }
}

/// marks the house as inhabited
fn habit_house(
    mut houses: Query<&mut HouseComponent>,
    brando: Res<BuildingBuilder>,
    mut inhabitant_arrived_reader: EventReader<InhabitantArrivedAtHomeEvent>,
) {
    for arrived in inhabitant_arrived_reader.iter() {
        let mut hc = match houses.get_mut(arrived.entity) {
            Ok(c) => c,
            Err(e) => {
                error!("error on getting house component {e:?}");
                continue;
            }
        };

        brando
            .go_to_live_home(&mut hc.0, arrived.count)
            .expect("error on updating house property");
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    configuration: Res<Arc<Configuration>>,
) {
    let grid_positions: Vec<_> = (0..configuration.game.width_table)
        .flat_map(|x| (0..configuration.game.depth_table).map(move |y| (x as i64, y as i64)))
        .map(|(x, y)| convert_position_into_bevy_coords(&configuration, &Position { x, y }))
        .collect();

    for translation in grid_positions {
        let transform = Transform::from_translation(translation);
        commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Plane {
                    size: configuration.cube_size,
                })),
                material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
                transform,
                ..default()
            })
            .insert(PlaneComponent)
            .insert_bundle(PickableBundle::default());
    }
}

mod events {
    use crate::building::Building;
    use bevy::prelude::{Component, Entity};

    #[derive(Component)]
    pub struct BuildingCreatedEvent {
        pub building: Building,
        pub entity: Entity,
    }
}

mod components {
    use bevy::prelude::Component;

    use crate::building::{BuildingInConstruction, Garden, House, Office, Street};

    #[derive(Component, Debug)]
    pub struct PlaneComponent;

    #[derive(Component)]
    pub struct HouseComponent(pub House);

    #[derive(Component)]
    pub struct StreetComponent(pub Street);
    #[derive(Component)]
    pub struct GardenComponent(pub Garden);
    #[derive(Component)]
    pub struct OfficeComponent(pub Office);

    #[derive(Component)]
    pub struct BuildingInConstructionComponent(pub BuildingInConstruction);
}
