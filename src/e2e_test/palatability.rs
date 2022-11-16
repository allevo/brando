use crate::{
    building::BuildingUnderConstructionComponent, common::position::Position, e2e_test::utils::*,
    palatability::PalatabilityManagerResource,
};

#[test]
fn test_house_creation_increment_population_and_unemployee() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);
    create_street_at(&mut app, &Position { x: 0, y: 0 });
    create_street_at(&mut app, &Position { x: 0, y: 1 });
    create_street_at(&mut app, &Position { x: 0, y: 2 });

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });

    run_till(&mut app, |app| {
        let palatability: &mut PalatabilityManagerResource = get_manager_resource_mut(app);
        palatability.total_populations() != 8 && palatability.unemployed_inhabitants().len() == 8
    });
}

#[test]
fn test_office_creation_decrement_unemployee() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);
    create_street_at(&mut app, &Position { x: 0, y: 0 });
    create_street_at(&mut app, &Position { x: 0, y: 1 });
    create_street_at(&mut app, &Position { x: 0, y: 2 });

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });
    run_till(&mut app, |app| {
        let palatability: &mut PalatabilityManagerResource = get_manager_resource_mut(app);
        palatability.unemployed_inhabitants().len() == 8
    });

    let office_position = Position { x: 1, y: 3 };
    create_office_at(&mut app, &office_position);
    create_street_at(&mut app, &Position { x: 0, y: 3 });

    run_till(&mut app, |app| {
        get_office_snapshot_at(app, &office_position).is_none()
    });

    run_till(&mut app, |app| {
        let palatability: &mut PalatabilityManagerResource = get_manager_resource_mut(app);
        palatability.unemployed_inhabitants().is_empty()
    });
}

#[test]
fn test_building_stop_creation_process_is_palatability_is_not_sufficient() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);
    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });
    assert_house_is_built_at(&mut app, &house_position);

    let house_position = Position { x: 2, y: 2 };
    create_house_at(&mut app, &house_position);
    run(&mut app, 100);
    assert!(get_house_snapshot_at(&mut app, &house_position).is_none());

    let c =
        get_component_at::<BuildingUnderConstructionComponent>(&mut app, &house_position).unwrap();
    let a = c.building_under_construction.get_status();
    assert_ne!(a.0, a.1);

    let garden_position = Position { x: 1, y: 1 };
    create_garden_at(&mut app, &garden_position);

    run_till(&mut app, |app| {
        get_garden_snapshot_at(app, &garden_position).is_none()
    });
    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });
    assert_house_is_built_at(&mut app, &house_position);
}
