use std::{collections::HashMap};

use bevy::{prelude::Entity, utils::HashSet};

use crate::{common::position::Position, palatability::manager::EducationLevel};

#[derive(Debug)]
pub struct BuildingNeedToBeFulfilled {
    building_entity: Entity,
    building_position: Position,
    remain: usize,
}


impl BuildingNeedToBeFulfilled {
    pub fn new(building_entity: Entity, building_position: Position, remain: usize) -> Self {
        Self {
            building_entity,
            building_position,
            remain,
        }
    }
}

#[derive(Default, Debug)]
pub struct EntityStorage {
    inhabitants: HashMap<Entity, EducationLevel>,

    // Probably this shouldn't be a vec: let's see...
    /// Inhabitants that are waiting for being introducing the the game
    inhabitants_need_to_be_introduced: HashSet<Entity>,
    /// Houses that need to be fulfilled with inhabitants
    houses_needs_to_be_fulfilled: HashMap<Entity, BuildingNeedToBeFulfilled>,
    /// Offices that need to be fulfilled with workers
    offices_needs_to_be_fulfilled: HashMap<Entity, BuildingNeedToBeFulfilled>,
    /// Inhabitants that are waiting for a job
    inhabitants_need_to_work: HashSet<Entity>,

    /// Track which inhabitant live where
    inhabitant_house_position_map: HashMap<Entity, Position>,
}

impl EntityStorage {
    pub fn register_house(&mut self, house_to_be_fulfilled: BuildingNeedToBeFulfilled) {
        let entry = self
            .houses_needs_to_be_fulfilled
            .entry(house_to_be_fulfilled.building_entity);
        entry.or_insert(house_to_be_fulfilled);
    }

    pub fn register_office(&mut self, office_to_be_fulfilled: BuildingNeedToBeFulfilled) {
        let entry = self
            .offices_needs_to_be_fulfilled
            .entry(office_to_be_fulfilled.building_entity);
        entry.or_insert(office_to_be_fulfilled);
    }

    pub fn introduce_inhabitant(&mut self, entity: Entity, eduction_level: EducationLevel) {
        self.inhabitants_need_to_be_introduced.insert(entity);
        self.inhabitants.entry(entity)
            .or_insert(eduction_level);
    }

    pub fn register_unemployee(&mut self, entity: Entity) {
        self.inhabitants_need_to_work.insert(entity);
    }

    pub fn set_inhabitant_house_position(
        &mut self,
        inhabitant_entity: Entity,
        house_position: Position,
    ) {
        self.inhabitant_house_position_map
            .entry(inhabitant_entity)
            .or_insert(house_position);
    }

    pub fn get_inhabitant_house_assignment(&mut self) -> Vec<AssignmentResult> {
        if self.houses_needs_to_be_fulfilled.is_empty()
            || self.inhabitants_need_to_be_introduced.is_empty()
        {
            return vec![];
        }

        // We would like to find an house to fulfill: so find a "empty" house and
        // remove the number of inhabitants we would like to spawn

        // TODO: choose the house better, not "randomly"
        // Currently keys().next() is not a good algorithm: we can be smarter here
        let building_entity = *self.houses_needs_to_be_fulfilled.keys().next().unwrap();
        let building_needed_to_be_fulfilled = self
            .houses_needs_to_be_fulfilled
            .get_mut(&building_entity)
            .unwrap();
        if building_needed_to_be_fulfilled.remain == 0 {
            self.houses_needs_to_be_fulfilled.remove(&building_entity);
            return self.get_inhabitant_house_assignment();
        }

        let from = *self
            .inhabitants_need_to_be_introduced
            .iter()
            .next()
            .unwrap();

        // TODO: only one?
        building_needed_to_be_fulfilled.remain -= 1;

        vec![AssignmentResult {
            from,
            from_position: Position { x: 0, y: 0 },
            to: building_needed_to_be_fulfilled.building_entity,
            to_position: building_needed_to_be_fulfilled.building_position,
            count: 1,
            assignment_type: AssignmentType::InhabitantHouse,
        }]
    }

    pub fn get_inhabitant_job_assignment(&mut self) -> Vec<AssignmentResult> {
        if self.offices_needs_to_be_fulfilled.is_empty() || self.inhabitants_need_to_work.is_empty() {
            return vec![];
        }

        // We would like to find an office to fulfill: so find a "empty" office and
        // remove the number of inhabitants we would like to assign
        // NB: this method is really too equal of "get_inhabitant_house_assignment" method

        // TODO: choose the office better, not "randomly"
        // Currently keys().next() is not a good algorithm: we can be smarter here
        let building_entity = *self.offices_needs_to_be_fulfilled.keys().next().unwrap();
        let building_needed_to_be_fulfilled = self
            .offices_needs_to_be_fulfilled
            .get_mut(&building_entity)
            .unwrap();
        // Because the algorithm is not so "stable", we probably find some buildings
        // without remains.
        // TODO: avoid that
        if building_needed_to_be_fulfilled.remain == 0 {
            self.offices_needs_to_be_fulfilled.remove(&building_entity);
            return self.get_inhabitant_job_assignment();
        }

        let building_required_education_level = &EducationLevel::None;

        let from = self.inhabitants_need_to_work.iter()
            .filter(|i| {
                let inhabitant_education_level: &EducationLevel = &self.inhabitants[i];
                inhabitant_education_level >= building_required_education_level
            }).next();

        let from = match from {
            None => return vec![],
            Some(from) => *from,
        };

        // TODO: only one?
        building_needed_to_be_fulfilled.remain -= 1;

        let house_position = *self.inhabitant_house_position_map.get(&from).unwrap();

        vec![AssignmentResult {
            from,
            from_position: house_position,
            to: building_needed_to_be_fulfilled.building_entity,
            to_position: building_needed_to_be_fulfilled.building_position,
            count: 1,
            assignment_type: AssignmentType::InhabitantOffice,
        }]
    }

    pub fn resign_assign_result(&mut self, assign_result: AssignmentResult) {
        let inhabitants_list: &mut HashSet<Entity> = match assign_result.assignment_type {
            AssignmentType::InhabitantHouse => &mut self.inhabitants_need_to_be_introduced,
            AssignmentType::InhabitantOffice => &mut self.inhabitants_need_to_work,
        };
        inhabitants_list.insert(assign_result.from);

        let building_map: &mut HashMap<Entity, BuildingNeedToBeFulfilled> =
            match assign_result.assignment_type {
                AssignmentType::InhabitantHouse => &mut self.houses_needs_to_be_fulfilled,
                AssignmentType::InhabitantOffice => &mut self.offices_needs_to_be_fulfilled,
            };

        let building_need_to_be_fulfilled =
            building_map
                .entry(assign_result.to)
                .or_insert(BuildingNeedToBeFulfilled {
                    building_entity: assign_result.to,
                    building_position: assign_result.to_position,
                    remain: 0,
                });
        building_need_to_be_fulfilled.remain += assign_result.count;
    }
}

pub struct AssignmentResult {
    pub assignment_type: AssignmentType,
    pub from: Entity,
    pub from_position: Position,
    pub to: Entity,
    pub to_position: Position,
    pub count: usize,
}

pub enum AssignmentType {
    InhabitantHouse,
    InhabitantOffice,
}
