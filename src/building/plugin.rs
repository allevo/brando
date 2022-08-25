use std::sync::Arc;

use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};
use bevy_mod_picking::{DefaultPickingPlugins, PickableBundle, PickingEvent};

use crate::{
    building::{builder::BuildingBuilder, BuildRequest, BuildingType, BuildingUnderConstruction},
    common::{
        configuration::Configuration,
        position::Position,
        position_utils::{convert_bevy_coords_into_position, convert_position_into_bevy_coords},
    },
    navigation::plugin::events::{InhabitantArrivedAtHomeEvent, InhabitantFoundJobEvent},
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
        let builder = BuildingBuilder::new(configuration.clone());

        app.insert_resource(EditMode::None)
            .insert_resource(builder)
            .add_event::<BuildingCreatedEvent>()
            .add_plugins(DefaultPickingPlugins)
            .add_system_to_stage(CoreStage::PostUpdate, start_building_creation)
            .add_startup_system(setup)
            .add_system_to_stage(CoreStage::PostUpdate, switch_edit_mode)
            .add_system_to_stage(
                CoreStage::PostUpdate,
                make_progress_for_building_under_construction,
            )
            .add_system_to_stage(CoreStage::PostUpdate, habit_house)
            .add_system_to_stage(CoreStage::PostUpdate, work_on_office);
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
            (ButtonState::Released, Some(KeyCode::S)) => Some(EditMode::Street),
            (ButtonState::Released, Some(KeyCode::G)) => Some(EditMode::Garden),
            (ButtonState::Released, Some(KeyCode::H)) => Some(EditMode::House),
            (ButtonState::Released, Some(KeyCode::O)) => Some(EditMode::Office),
            (ButtonState::Released, Some(KeyCode::Escape)) => Some(EditMode::None),
            _ => None,
        })
        .next()
    {
        info!("Switch EditMode to {:?}", e);
        *edit_mode = e;
    }
}

/// Spawn entity with `BuildingInConstructionComponent`
fn start_building_creation(
    mut events: EventReader<PickingEvent>,
    planes: Query<&PlaneComponent>,
    edit_mode: Res<EditMode>,
    mut brando: ResMut<BuildingBuilder>,
    mut commands: Commands,
    bundles: Res<PbrBundles>,
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

    let position: &PlaneComponent = planes.get(*entity).unwrap();
    let position = position.0;

    let building_type = match *edit_mode {
        EditMode::House => BuildingType::House,
        EditMode::Garden => BuildingType::Garden,
        EditMode::Street => BuildingType::Street,
        EditMode::Office => BuildingType::Office,
        EditMode::None => unreachable!("EditMode::None is handled before"),
    };

    info!("Building {:?} at {:?}", building_type, position);

    let request = BuildRequest::new(position, building_type);
    let building_under_construction = match brando.create_building(request) {
        Ok(res) => res,
        Err(s) => {
            error!("Error on creation building: {}", s);
            return;
        }
    };

    commands
        .entity(*entity)
        .insert(BuildingUnderConstructionComponent {
            building_under_construction,
        })
        .with_children(|parent| {
            let mut sprite = bundles.in_progress();
            sprite.transform.translation = Vec3::new(0., 0.0001, 0.);
            parent.spawn_bundle(sprite);
        });
}

// TODO: split this function: too many arguments
/// Make BuildingInConstruction progress. Then:
/// - if the building is not yet finished, stop
/// - otherwise place the building
/// NB: the progress is made if and only if there's sufficient palatability
#[allow(clippy::too_many_arguments)]
fn make_progress_for_building_under_construction(
    game_tick: EventReader<GameTick>,
    mut buildings_in_progress: Query<(Entity, &mut BuildingUnderConstructionComponent)>,
    mut builder: ResMut<BuildingBuilder>,
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
    if game_tick.is_empty() {
        return;
    }

    for (entity, mut building) in buildings_in_progress.iter_mut() {
        let position = building.building_under_construction.request.position;

        // TODO: generalize this
        // Currently we implement only a type of building that, to be built,
        // needs to have a proper palatability.
        // So, for the time being, an "if" is enough
        match building.building_under_construction.request.building_type {
            BuildingType::House => {
                let p = palatability.get_house_palatability(&position);
                // TODO: tag this entity in order to retry later
                // If an house hasn't enough palatability, we retry again and again
                // Probably this is not good at all: we can put a dedicated component to the entity
                // in order to deselect it avoiding the reprocessing.
                if !p.is_positive() {
                    debug!("house palatability: insufficient at ({position:?})");
                    continue;
                }
            }
            BuildingType::Office => {
                let p = palatability.get_office_palatability(&position);
                // TODO: tag this entity in order to retry later
                // If an house hasn't enough palatability, we retry again and again
                // Probably this is not good at all: we can put a dedicated component to the entity
                // in order to deselect it avoiding the reprocessing.
                if !p.is_positive() {
                    debug!("office palatability: insufficient at ({position:?})");
                    continue;
                }
            }
            BuildingType::Garden | BuildingType::Street => {}
        }

        let building_under_construction: &mut BuildingUnderConstruction =
            &mut building.building_under_construction;

        builder
            .make_progress(building_under_construction)
            .expect("make progress never fails");

        if !building_under_construction.is_completed() {
            continue;
        }

        info!(
            "{:?} completed at {:?}",
            building_under_construction.request.building_type,
            building_under_construction.request.position,
        );

        let building_type = &building_under_construction.request.building_type;
        let bundle = match building_type {
            BuildingType::House => bundles.house(),
            BuildingType::Garden => bundles.garden(),
            BuildingType::Street => bundles.street(),
            BuildingType::Office => bundles.office(),
        };

        let mut command = commands.entity(entity);
        command.despawn_descendants();
        command
            .remove::<BuildingUnderConstructionComponent>()
            .with_children(|parent| {
                parent.spawn_bundle(bundle);
            });

        let entity_id: u64 = entity.to_bits();

        // TODO: the clone of the building_under_construction is ugly
        // Actually if I remove a component from the entity (like here),
        // I should have back also che ownership of the component.
        // That'd allow to avoid the clone here.
        let building = builder.build(
            entity_id,
            building_under_construction.clone(),
            &configuration,
        );
        let building_id = building.id();

        // TODO: rework this part
        // This part need to be reworked in order to let it scalable
        // on the BuildingType enumeration growing.
        let entity = match building_type {
            BuildingType::House => command.insert(HouseComponent(building_id)).id(),
            BuildingType::Garden => command.insert(GardenComponent(building_id)).id(),
            BuildingType::Street => command.insert(StreetComponent(building_id)).id(),
            BuildingType::Office => command.insert(OfficeComponent(building_id)).id(),
        };

        let snapshot = building.snapshot();

        // let building: Building = Building::clone(building);
        building_created_writer.send(BuildingCreatedEvent {
            building: snapshot,
            building_entity: entity,
            position,
        });
    }
}

/// marks the house as inhabited
fn habit_house(
    mut houses: Query<&mut HouseComponent>,
    mut builder: ResMut<BuildingBuilder>,
    mut inhabitant_arrived_reader: EventReader<InhabitantArrivedAtHomeEvent>,
) {
    for arrived in inhabitant_arrived_reader.iter() {
        let hc = match houses.get_mut(arrived.building_entity) {
            Ok(c) => c,
            Err(e) => {
                error!("error on getting house component {e:?}");
                continue;
            }
        };

        builder
            .go_to_live_home(hc.0, arrived.inhabitants_entities.len())
            .expect("error on updating house property");
    }
}

/// marks the office as fulfilled
fn work_on_office(
    mut offices: Query<&mut OfficeComponent>,
    mut builder: ResMut<BuildingBuilder>,
    mut inhabitant_find_job_reader: EventReader<InhabitantFoundJobEvent>,
) {
    for arrived in inhabitant_find_job_reader.iter() {
        let hc = match offices.get_mut(arrived.building_entity) {
            Ok(c) => c,
            Err(e) => {
                error!("error on getting house component {e:?}");
                continue;
            }
        };

        builder
            .job_found(hc.0, arrived.workers_entities.len())
            .expect("error on updating office property");
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
        let position = convert_bevy_coords_into_position(&configuration, &translation);

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
            .insert(PlaneComponent(position))
            .insert_bundle(PickableBundle::default());
    }
}

mod events {
    use crate::{
        building::{ResidentProperty, WorkProperty},
        common::position::Position,
    };
    use bevy::prelude::{Component, Entity};

    #[derive(Component)]
    pub struct BuildingCreatedEvent {
        pub position: Position,
        pub building: BuildingSnapshot,
        pub building_entity: Entity,
    }

    pub enum BuildingSnapshot {
        House(HouseSnapshot),
        Office(OfficeSnapshot),
        Street(StreetSnapshot),
        Garden(GardenSnapshot),
    }

    #[allow(dead_code)]
    impl BuildingSnapshot {
        pub fn into_house(self) -> HouseSnapshot {
            match self {
                BuildingSnapshot::House(h) => h,
                _ => unreachable!("BuildingSnapshot is not an HouseSnapshot"),
            }
        }

        pub fn into_office(self) -> OfficeSnapshot {
            match self {
                BuildingSnapshot::Office(o) => o,
                _ => unreachable!("BuildingSnapshot is not an OfficeSnapshot"),
            }
        }
    }

    pub struct HouseSnapshot {
        pub position: Position,
        pub resident_property: ResidentProperty,
    }
    pub struct OfficeSnapshot {
        pub position: Position,
        pub work_property: WorkProperty,
    }
    pub struct StreetSnapshot {
        pub position: Position,
    }
    pub struct GardenSnapshot {
        pub position: Position,
    }
}

mod components {
    use bevy::prelude::Component;

    use crate::{
        building::{BuildingId, BuildingUnderConstruction},
        common::position::Position,
    };

    #[derive(Component, Debug)]
    pub struct PlaneComponent(pub Position);

    #[derive(Component)]
    pub struct HouseComponent(pub BuildingId);

    #[derive(Component)]
    pub struct StreetComponent(pub BuildingId);
    #[derive(Component)]
    pub struct GardenComponent(pub BuildingId);
    #[derive(Component)]
    pub struct OfficeComponent(pub BuildingId);

    #[derive(Component)]
    pub struct BuildingUnderConstructionComponent {
        pub building_under_construction: BuildingUnderConstruction,
    }
}
