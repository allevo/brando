use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
};
use bevy_mod_picking::{DefaultPickingPlugins, PickableBundle, PickingEvent};

use crate::{
    building::{manager::Building, BuildingSnapshot},
    common::{
        position::Position,
        position_utils::{convert_bevy_coords_into_position, convert_position_into_bevy_coords},
        EntityId,
    },
    inhabitant::events::{HomeAssignedToInhabitantEvent, JobAssignedToInhabitantEvent},
    palatability::PalatabilityManagerResource,
    resources::ConfigurationResource,
    GameTick, PbrBundles,
};

#[cfg(test)]
pub use components::*;
#[cfg(not(test))]
use components::*;

use events::*;
pub use resources::*;

use super::manager::BuildingManager;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        let configuration: &ConfigurationResource = app.world.resource();
        let manager = BuildingManagerResource(BuildingManager::new((*configuration).clone()));

        app.insert_resource(EditMode::None)
            .insert_resource(manager)
            .add_event::<BuildingCreatedEvent>()
            .add_plugins(DefaultPickingPlugins)
            .add_startup_system(setup)
            .add_system(start_building_creation)
            .add_system(switch_edit_mode)
            .add_system(make_progress_for_building_under_construction)
            .add_system(habit_house)
            .add_system(work_on_office);
    }
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
            (ButtonState::Released, Some(KeyCode::B)) => Some(EditMode::BiomassPowerPlant),
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
    mut building_manager: ResMut<BuildingManagerResource>,
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

    let id: EntityId = entity.to_bits();

    // TODO: move away from here!
    let building: Building = match *edit_mode {
        EditMode::House => Building::House(building_manager.house(id, position)),
        EditMode::Garden => Building::Garden(building_manager.garden(id, position)),
        EditMode::Street => Building::Street(building_manager.street(id, position)),
        EditMode::Office => Building::Office(building_manager.office(id, position)),
        EditMode::BiomassPowerPlant => {
            Building::BiomassPowerPlant(building_manager.biomass_power_plant(id, position))
        }
        EditMode::None => unreachable!("EditMode::None is handled before"),
    };

    info!("Building {:?} at {:?}", building, position);

    let building_under_construction = match building_manager.start_building_creation(building) {
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
            parent.spawn(sprite);
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
    mut building_manager: ResMut<BuildingManagerResource>,
    palatability: Res<PalatabilityManagerResource>,
    mut commands: Commands,
    bundles: Res<PbrBundles>,
    _configuration: Res<ConfigurationResource>,
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
        debug_assert_eq!(
            entity.to_bits(),
            building.building_under_construction.get_building().get_id()
        );

        let building_under_construction = &mut building.building_under_construction;
        let building = building_under_construction.get_building();
        let id = building.get_id();
        let position = building.get_position();

        // TODO: is this line impact too much the performance?
        // Technically we need to create the snapshot only when the building is finalized
        let building_snapshot = BuildingSnapshot::from(building);

        let building_palatability = palatability.get_palatability(&building_snapshot);
        if !building_palatability.is_positive() {
            debug!(
                "Building palatability {building_palatability:?}: insufficient at ({position:?}"
            );
            continue;
        }

        let is_completed = building_manager.make_progress(building_under_construction);

        if !is_completed {
            continue;
        }

        building_manager.finalize_building_creation(building_under_construction);

        info!(
            "{:?} completed at {:?}",
            building_under_construction, building_under_construction,
        );

        let building = building_under_construction.get_building();

        let bundle = match building {
            Building::House(_) => bundles.house(),
            Building::Garden(_) => bundles.garden(),
            Building::Street(_) => bundles.street(),
            Building::Office(_) => bundles.office(),
            Building::BiomassPowerPlant(_) => bundles.biomass_power_plant(),
        };

        let mut command = commands.entity(entity);
        command.despawn_descendants();
        command
            .remove::<BuildingUnderConstructionComponent>()
            .with_children(|parent| {
                parent.spawn(bundle);
            });

        // TODO: rework this part
        // This part need to be reworked in order to let it scalable
        // on the BuildingType enumeration growing.
        match building {
            Building::House(_) => command.insert(HouseComponent(id)),
            Building::Garden(_) => command.insert(GardenComponent(id)),
            Building::Street(_) => command.insert(StreetComponent(id)),
            Building::Office(_) => command.insert(OfficeComponent(id)),
            Building::BiomassPowerPlant(_) => command.insert(BiomassPowerPlantComponent(id)),
        };

        building_created_writer.send(BuildingCreatedEvent { building_snapshot });
    }
}

/// marks the house as inhabited
fn habit_house(
    mut houses: Query<&mut HouseComponent>,
    mut building_manager: ResMut<BuildingManagerResource>,
    mut inhabitant_arrived_reader: EventReader<HomeAssignedToInhabitantEvent>,
) {
    for arrived in inhabitant_arrived_reader.iter() {
        let hc = match houses.get_mut(Entity::from_bits(arrived.building_entity_id)) {
            Ok(c) => c,
            Err(e) => {
                error!("error on getting house component {e:?}");
                continue;
            }
        };

        building_manager.inhabitants_arrived_at_home(
            hc.0,
            arrived
                .inhabitants_entity_ids
                .len()
                .try_into()
                .expect("unable to convert usize into u32"),
        );
    }
}

/// marks the office as fulfilled
fn work_on_office(
    mut offices: Query<&mut OfficeComponent>,
    mut building_manager: ResMut<BuildingManagerResource>,
    mut inhabitant_find_job_reader: EventReader<JobAssignedToInhabitantEvent>,
) {
    for arrived in inhabitant_find_job_reader.iter() {
        let hc = match offices.get_mut(Entity::from_bits(arrived.building_entity_id)) {
            Ok(c) => c,
            Err(e) => {
                error!("error on getting house component {e:?}");
                continue;
            }
        };

        building_manager.workers_found_job(
            hc.0,
            arrived
                .workers_entity_ids
                .len()
                .try_into()
                .expect("unable to convert usize into u32"),
        );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    configuration: Res<ConfigurationResource>,
) {
    let grid_positions: Vec<_> = (0..configuration.game.width_table)
        .flat_map(|x| (0..configuration.game.depth_table).map(move |y| (x as i64, y as i64)))
        .map(|(x, y)| convert_position_into_bevy_coords(&configuration, &Position { x, y }))
        .collect();

    for translation in grid_positions {
        let position = convert_bevy_coords_into_position(&configuration, &translation);

        let transform = Transform::from_translation(translation);
        commands
            .spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Plane {
                    size: configuration.cube_size,
                })),
                material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
                transform,
                ..default()
            })
            .insert(PlaneComponent(position))
            .insert(PickableBundle::default());
    }
}

mod resources {
    use std::ops::{Deref, DerefMut};

    use bevy::prelude::Resource;

    use crate::building::manager::BuildingManager;

    #[derive(Debug, Hash, PartialEq, Eq, Resource)]
    pub enum EditMode {
        None,
        House,
        Garden,
        Street,
        Office,
        BiomassPowerPlant,
    }

    #[derive(Resource)]
    pub struct BuildingManagerResource(pub BuildingManager);

    impl Deref for BuildingManagerResource {
        type Target = BuildingManager;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for BuildingManagerResource {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

pub mod events {
    use crate::building::BuildingSnapshot;
    use bevy::prelude::Component;

    #[derive(Component)]
    pub struct BuildingCreatedEvent {
        pub building_snapshot: BuildingSnapshot,
    }
}

mod components {
    use bevy::prelude::Component;

    use crate::{
        building::manager::BuildingUnderConstruction,
        common::{position::Position, EntityId},
    };

    #[derive(Component, Debug)]
    pub struct PlaneComponent(pub Position);

    #[derive(Component)]
    pub struct HouseComponent(pub EntityId);

    #[derive(Component)]
    pub struct StreetComponent(pub EntityId);
    #[derive(Component)]
    pub struct GardenComponent(pub EntityId);
    #[derive(Component)]
    pub struct OfficeComponent(pub EntityId);
    #[derive(Component)]
    pub struct BiomassPowerPlantComponent(pub EntityId);

    #[derive(Component)]
    pub struct BuildingUnderConstructionComponent {
        pub building_under_construction: BuildingUnderConstruction,
    }
}
