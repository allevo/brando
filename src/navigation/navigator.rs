use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use tracing::info;

use crate::{
    building::House,
    common::{configuration::Configuration, position::Position},
};

pub struct Navigator {
    start_point: Position,
    positions_to_add: HashSet<Position>,
    nodes: HashMap<Position, HashSet<Position>>,
}
impl Navigator {
    pub fn new(start_point: Position) -> Self {
        let mut nodes: HashMap<Position, HashSet<Position>> = Default::default();
        nodes.entry(start_point).or_default();
        Self {
            start_point,
            positions_to_add: Default::default(),
            nodes,
        }
    }

    pub fn add_node(&mut self, position: Position) {
        self.positions_to_add.insert(position);
    }

    pub fn get_navigation_descriptor(&self, end: &impl Reachable) -> Option<NavigationDescriptor> {
        use pathfinding::prelude::astar;

        let end = end.to_position();
        let neighbors: HashSet<_> = end.neighbors().collect();

        let result = astar(
            &self.start_point,
            |p| {
                self.nodes[p]
                    .iter()
                    .map(|p| (*p, 1_i64))
                    .collect::<Vec<_>>()
            },
            |p| {
                let delta_x = if p.x > end.x {
                    p.x - end.x
                } else {
                    end.x - p.x
                };
                let delta_y = if p.y > end.y {
                    p.y - end.y
                } else {
                    end.y - p.y
                };
                (delta_x + delta_y) / 3
            },
            |p| neighbors.contains(p),
        );

        let mut r = match result {
            None => return None,
            Some(r) => r,
        };

        r.0.push(end);
        r.1 += 1;

        // We want to reverse the vector in order to use "pop" method on "make_progress"
        let path = r.0.into_iter().rev().collect();
        let descriptor = NavigationDescriptor { path };

        info!("Found descriptor {:?}", descriptor);

        Some(descriptor)
    }

    pub fn rebuild(&mut self) -> usize {
        // TODO: order positions_to_add
        // The optimization here is related to points that are not connected to the "good" graph part
        // What happen if the user creates multiple nodes and arcs without connecting to the `start_point`
        // and connect to it later?
        // In that situation `positions_to_add` contains a points that are connected among them but not to
        // `start_point`. When the user adds the link between the `start_point` with a point inside `positions_to_add`,
        // the loop below processes just one "point".
        // As consequence of that, "optmize" should be called multiple times and every time add only a few of the points.
        // The optimization that can be done here is to order `position_to_add` in order to process multiple points with
        // just one invocation of `rebuild`.
        let positions_to_add = std::mem::take(&mut self.positions_to_add);

        let mut addressed_positions = HashSet::new();
        for position in &positions_to_add {
            let neighbors = position.neighbors();
            for neighbor in neighbors {
                if !self.nodes.contains_key(&neighbor) {
                    continue;
                }

                // Double reference. For the time being the street and double versed.
                // This makes undirected graph
                self.nodes.entry(neighbor).or_default().insert(*position);
                self.nodes.entry(*position).or_default().insert(neighbor);

                addressed_positions.insert(*position);
            }
        }

        let addressed_positions_count = addressed_positions.len();

        self.positions_to_add.extend(
            positions_to_add
                .into_iter()
                .filter(|p| !addressed_positions.contains(p)),
        );

        addressed_positions_count
    }

    pub fn make_progress(&self, navigator_descriptor: &mut NavigationDescriptor) {
        navigator_descriptor.path.pop();
    }

    pub fn calculate_delta(&self, requested: u8, configuration: &Configuration) -> u8 {
        configuration
            .buildings
            .house
            .max_inhabitant_per_travel
            .min(requested)
    }
}

pub trait Reachable {
    // TODO: this probably is wrong
    // In the future we might have building that has more than one position
    // (ie occupant more that 1 square)
    // For the time being KISS
    fn to_position(&self) -> Position;
}

impl Reachable for House {
    fn to_position(&self) -> Position {
        self.position
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NavigationDescriptor {
    path: Vec<Position>,
}

impl NavigationDescriptor {
    pub fn is_completed(&self) -> bool {
        self.path.is_empty()
    }
}

impl Display for NavigationDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "path length {}", self.path.len())
    }
}

#[cfg(test)]
mod tests {
    use crate::common::configuration::CONFIGURATION;

    use super::*;

    struct Foo(Position);
    impl Reachable for Foo {
        fn to_position(&self) -> Position {
            self.0
        }
    }

    #[test]
    fn test_navigate_ok() {
        let mut navigator = Navigator::new(Position { x: 0, y: 0 });
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });

        navigator.rebuild();

        let mut desc = navigator
            .get_navigation_descriptor(&Foo(Position { x: 3, y: 0 }))
            .unwrap();

        assert!(!desc.is_completed());

        navigator.make_progress(&mut desc);
        assert!(!desc.is_completed());
        navigator.make_progress(&mut desc);
        assert!(!desc.is_completed());
        navigator.make_progress(&mut desc);
        assert!(!desc.is_completed());
        navigator.make_progress(&mut desc);
        assert!(desc.is_completed());

        navigator.make_progress(&mut desc);
        assert!(desc.is_completed());
    }

    #[test]
    fn test_navigate_ko() {
        let mut navigator = Navigator::new(Position { x: 0, y: 0 });
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });

        navigator.rebuild();

        let desc = navigator.get_navigation_descriptor(&Foo(Position { x: 42, y: 0 }));

        assert_eq!(desc, None);
    }

    #[test]
    fn test_calculate_delta() {
        let navigator = Navigator::new(Position { x: 0, y: 0 });

        let delta = navigator.calculate_delta(5, &CONFIGURATION);
        assert_eq!(delta, 5);
        let delta = navigator.calculate_delta(6, &CONFIGURATION);
        assert_eq!(delta, 6);
        let delta = navigator.calculate_delta(10, &CONFIGURATION);
        assert_eq!(delta, 6);
    }
}
