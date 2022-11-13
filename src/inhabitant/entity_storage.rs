use std::collections::HashMap;

use bevy::utils::HashSet;
use tracing::info;

use crate::common::{enums::EducationLevel, position::Position, EntityId};

use super::inhabitant::Inhabitant;

#[derive(Debug)]
pub struct BuildingNeedToBeFulfilled {
    building_entity_id: EntityId,
    building_position: Position,
    remain: u32,
}

impl BuildingNeedToBeFulfilled {
    pub fn new(building_entity_id: EntityId, building_position: Position, remain: u32) -> Self {
        Self {
            building_entity_id,
            building_position,
            remain,
        }
    }
}

#[derive(Default, Debug)]
pub struct EntityStorage {
    inhabitants: HashMap<EntityId, Inhabitant>,

    /// Inhabitants that are waiting for being introducing the the game
    inhabitants_need_to_be_introduced: HashSet<EntityId>,
    /// Houses that need to be fulfilled with inhabitants
    houses_needs_to_be_fulfilled: HashMap<EntityId, BuildingNeedToBeFulfilled>,
    /// Offices that need to be fulfilled with workers
    offices_needs_to_be_fulfilled: HashMap<EntityId, BuildingNeedToBeFulfilled>,
    /// Inhabitants that are waiting for a job
    inhabitants_need_to_work: HashSet<EntityId>,
}

impl EntityStorage {
    pub fn introduce_inhabitant(&mut self, inhabitant: Inhabitant) {
        let id = *inhabitant.get_id();
        self.inhabitants_need_to_be_introduced.insert(id);
        self.inhabitants.entry(id).or_insert(inhabitant);
    }

    pub fn register_house(&mut self, house_to_be_fulfilled: BuildingNeedToBeFulfilled) {
        let entry = self
            .houses_needs_to_be_fulfilled
            .entry(house_to_be_fulfilled.building_entity_id);
        entry.or_insert(house_to_be_fulfilled);
    }

    pub fn register_office(&mut self, office_to_be_fulfilled: BuildingNeedToBeFulfilled) {
        let entry = self
            .offices_needs_to_be_fulfilled
            .entry(office_to_be_fulfilled.building_entity_id);
        entry.or_insert(office_to_be_fulfilled);
    }

    pub fn register_unemployee(&mut self, id: EntityId) {
        debug_assert!(self.inhabitants[&id].get_work_place_id().is_none());
        self.inhabitants_need_to_work.insert(id);
    }

    pub fn found_home_for_inhabitant(
        &mut self,
        inhabitant_id: &EntityId,
        house_id: EntityId,
        house_position: Position,
    ) -> &Inhabitant {
        info!("Found home for {}", inhabitant_id);
        self.inhabitants
            .get_mut(inhabitant_id)
            .unwrap()
            .home_found(house_id, house_position);

        self.inhabitants.get(inhabitant_id).unwrap()
    }

    pub fn found_job_for_unemployee(
        &mut self,
        inhabitant_id: &EntityId,
        office_id: EntityId,
        office_position: Position,
    ) {
        info!("Found work for {}", inhabitant_id);
        self.inhabitants
            .get_mut(inhabitant_id)
            .unwrap()
            .work_place_found(office_id, office_position);
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
        let building_entity_id = *self.houses_needs_to_be_fulfilled.keys().next().unwrap();
        let building_needed_to_be_fulfilled = self
            .houses_needs_to_be_fulfilled
            .get_mut(&building_entity_id)
            .unwrap();
        if building_needed_to_be_fulfilled.remain == 0 {
            self.houses_needs_to_be_fulfilled
                .remove(&building_entity_id);
            return self.get_inhabitant_house_assignment();
        }

        let from = *self
            .inhabitants_need_to_be_introduced
            .iter()
            .next()
            .unwrap();
        self.inhabitants_need_to_be_introduced.remove(&from);

        // TODO: only one?
        building_needed_to_be_fulfilled.remain -= 1;

        vec![AssignmentResult {
            from,
            from_position: Position { x: 0, y: 0 },
            to: building_needed_to_be_fulfilled.building_entity_id,
            to_position: building_needed_to_be_fulfilled.building_position,
            count: 1,
            assignment_type: AssignmentType::InhabitantHouse,
        }]
    }

    pub fn get_inhabitant_job_assignment(&mut self) -> Vec<AssignmentResult> {
        if self.offices_needs_to_be_fulfilled.is_empty() || self.inhabitants_need_to_work.is_empty()
        {
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

        let from = self
            .inhabitants_need_to_work
            .iter()
            .filter(|i| {
                let inhabitant_education_level: &EducationLevel =
                    self.inhabitants[*i].get_education_level();
                inhabitant_education_level >= building_required_education_level
            })
            .next();

        let from = match from {
            None => return vec![],
            Some(from) => *from,
        };
        self.inhabitants_need_to_work.remove(&from);

        // TODO: only one?
        building_needed_to_be_fulfilled.remain -= 1;

        let house_position = *self.inhabitants[&from]
            .get_home()
            .as_ref()
            .unwrap()
            .get_house_position();

        vec![AssignmentResult {
            from,
            from_position: house_position,
            to: building_needed_to_be_fulfilled.building_entity_id,
            to_position: building_needed_to_be_fulfilled.building_position,
            count: 1,
            assignment_type: AssignmentType::InhabitantOffice,
        }]
    }

    pub fn resign_assign_result(&mut self, assign_result: AssignmentResult) {
        let inhabitants_list: &mut HashSet<EntityId> = match assign_result.assignment_type {
            AssignmentType::InhabitantHouse => &mut self.inhabitants_need_to_be_introduced,
            AssignmentType::InhabitantOffice => &mut self.inhabitants_need_to_work,
        };
        inhabitants_list.insert(assign_result.from);

        let building_map_to_be_fulfilled: &mut HashMap<EntityId, BuildingNeedToBeFulfilled> =
            match assign_result.assignment_type {
                AssignmentType::InhabitantHouse => &mut self.houses_needs_to_be_fulfilled,
                AssignmentType::InhabitantOffice => &mut self.offices_needs_to_be_fulfilled,
            };

        let building_need_to_be_fulfilled = building_map_to_be_fulfilled
            .entry(assign_result.to)
            .or_insert(BuildingNeedToBeFulfilled {
                building_entity_id: assign_result.to,
                building_position: assign_result.to_position,
                remain: 0,
            });
        building_need_to_be_fulfilled.remain += assign_result.count;
    }
}

#[derive(Debug)]
pub struct AssignmentResult {
    pub assignment_type: AssignmentType,
    pub from: EntityId,
    pub from_position: Position,
    pub to: EntityId,
    pub to_position: Position,
    pub count: u32,
}

#[derive(Debug)]
pub enum AssignmentType {
    InhabitantHouse,
    InhabitantOffice,
}

#[cfg(test)]
mod tests {
    use crate::{
        common::{enums::EducationLevel, position::Position},
        inhabitant::inhabitant::Inhabitant,
    };

    use super::{BuildingNeedToBeFulfilled, EntityStorage};

    #[test]
    fn test_consume_assignments() {
        let mut entity_storage = EntityStorage::default();

        let house = 0_u64;
        let house_position = Position { x: 0, y: 0 };
        entity_storage.register_house(BuildingNeedToBeFulfilled {
            building_entity_id: house,
            building_position: house_position,
            remain: 1,
        });

        let inhabitant = 1_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant, EducationLevel::None));

        let mut assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        let assignment = assignments.pop().unwrap();
        assert_eq!(assignment.from, inhabitant);
        assert_eq!(assignment.to, house);

        entity_storage.resign_assign_result(assignment);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 0);
    }

    #[test]
    fn test_consume_assignments_erase_all() {
        let mut entity_storage = EntityStorage::default();

        let house = 0_u64;
        let house_position = Position { x: 0, y: 0 };
        entity_storage.register_house(BuildingNeedToBeFulfilled {
            building_entity_id: house,
            building_position: house_position,
            remain: 5,
        });

        let inhabitant1 = 1_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant1, EducationLevel::None));
        let inhabitant2 = 2_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant2, EducationLevel::None));
        let inhabitant3 = 3_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant3, EducationLevel::None));

        let mut assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        let assignment = assignments.pop().unwrap();
        assert!(vec![inhabitant1, inhabitant2, inhabitant3].contains(&assignment.from));
        assert_eq!(assignment.to, house);

        entity_storage.resign_assign_result(assignment);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        // inhabitants are missing
        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 0);

        let inhabitant4 = 4_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant4, EducationLevel::None));
        let inhabitant5 = 5_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant5, EducationLevel::None));
        let inhabitant6 = 6_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant6, EducationLevel::None));

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);
        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 1);

        // houses are missing
        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 0);
    }

    #[test]
    fn test_consume_assignments_0() {
        let mut entity_storage = EntityStorage::default();

        let house = 0_u64;
        let house_position = Position { x: 0, y: 0 };
        entity_storage.register_house(BuildingNeedToBeFulfilled {
            building_entity_id: house,
            building_position: house_position,
            remain: 0,
        });

        let inhabitant = 1_u64;
        entity_storage.introduce_inhabitant(Inhabitant::new(inhabitant, EducationLevel::None));

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 0);

        let assignments = entity_storage.get_inhabitant_house_assignment();
        assert_eq!(assignments.len(), 0);
    }
}
