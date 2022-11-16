use bevy::prelude::Entity;

use crate::{
    building::{HouseComponent, PlaneComponent},
    common::position::Position,
    e2e_test::utils::*,
};

#[test]
fn test_create_building_take_time() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);
    assert!(
        get_house_snapshot_at(&mut app, &house_position).is_none(),
        "Should be not built yet"
    );

    run(&mut app, 1);
    assert!(
        get_house_snapshot_at(&mut app, &house_position).is_none(),
        "Should be not built yet"
    );

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });
    assert!(
        get_house_snapshot_at(&mut app, &house_position).is_some(),
        "Should be built"
    );
}

#[test]
fn test_create_house() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });

    assert_house_is_built_at(&mut app, &house_position);

    let house = get_house_snapshot_at(&mut app, &house_position).unwrap();
    assert_eq!(house.get_current_residents(), &0_u32);
}

#[test]
fn test_house_can_be_fulfilled() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 2 };
    create_house_at(&mut app, &house_position);
    create_street_at(&mut app, &Position { x: 0, y: 0 });
    create_street_at(&mut app, &Position { x: 0, y: 1 });
    create_street_at(&mut app, &Position { x: 0, y: 2 });

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });

    assert_house_is_built_at(&mut app, &house_position);
    assert_street_is_built_at(&mut app, &Position { x: 0, y: 0 });
    assert_street_is_built_at(&mut app, &Position { x: 0, y: 1 });
    assert_street_is_built_at(&mut app, &Position { x: 0, y: 2 });

    run_till(&mut app, |app| {
        let house = get_house_snapshot_at(app, &house_position).unwrap();
        house.get_current_residents() == house.get_max_residents()
    });
}

#[test]
fn test_create_office() {
    let mut app = create_app();

    let office_position = Position { x: 1, y: 2 };
    create_office_at(&mut app, &office_position);

    run_till(&mut app, |app| {
        get_office_snapshot_at(app, &office_position).is_none()
    });

    assert_office_is_built_at(&mut app, &office_position);

    let office = get_office_snapshot_at(&mut app, &office_position).unwrap();
    assert_eq!(office.get_current_workers(), &0_u32);
}

#[test]
fn test_office_can_be_fulfilled() {
    let mut app = create_app();

    let office_position = Position { x: 1, y: 3 };

    create_street_at(&mut app, &Position { x: 0, y: 0 });
    create_street_at(&mut app, &Position { x: 0, y: 1 });
    create_street_at(&mut app, &Position { x: 0, y: 2 });
    create_street_at(&mut app, &Position { x: 0, y: 3 });
    create_house_at(&mut app, &Position { x: 1, y: 2 });
    create_office_at(&mut app, &office_position);

    run_till(&mut app, |app| {
        get_office_snapshot_at(app, &office_position).is_none()
    });

    run_till(&mut app, |app| {
        let office = get_office_snapshot_at(app, &office_position).unwrap();
        office.get_current_workers() == office.get_max_workers()
    });
}

#[test]
fn test_position_is_already_used() {
    let mut app = create_app();

    let house_position = Position { x: 1, y: 3 };
    create_house_at(&mut app, &house_position);
    create_house_at(&mut app, &house_position);
    create_house_at(&mut app, &house_position);

    run_till(&mut app, |app| {
        get_house_snapshot_at(app, &house_position).is_none()
    });

    let items = get_entities::<(Entity, &PlaneComponent), &HouseComponent>(&mut app);

    assert_eq!(items.len(), 1);
}
