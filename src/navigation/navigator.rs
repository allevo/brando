use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use tracing::info;

use crate::{
    building::House,
    common::{position::Position},
};

pub struct Navigator {
    positions_to_add: HashSet<Position>,
    nodes: HashMap<Position, HashSet<Position>>,
}
impl Navigator {
    pub(super) fn new() -> Self {
        let nodes: HashMap<Position, HashSet<Position>> = Default::default();
        Self {
            positions_to_add: Default::default(),
            nodes,
        }
    }

    pub(super) fn add_node(&mut self, position: Position) {
        self.positions_to_add.insert(position);
    }

    pub fn get_navigation_descriptor(
        &self,
        start_point: &Position,
        end: Position,
    ) -> Option<NavigationDescriptor> {
        use pathfinding::prelude::astar;

        let neighbors: HashSet<_> = end.neighbors().collect();

        let result = astar(
            start_point,
            |p| {
                let neighbors = match self.nodes.get(p) {
                    None => return vec![],
                    Some(n) => n,
                };
                neighbors.iter().map(|p| (*p, 1_i64)).collect::<Vec<_>>()
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

    pub(super) fn rebuild(&mut self) -> usize {
        let positions_to_add = std::mem::take(&mut self.positions_to_add);
        let tot = positions_to_add.len();

        for position in &positions_to_add {
            let linked_nodes: Vec<_> = position
                .neighbors()
                .filter(|n| positions_to_add.contains(n) || self.nodes.contains_key(n))
                .collect();

            if linked_nodes.is_empty() {
                self.positions_to_add.insert(*position);
                continue;
            }

            self.nodes
                .entry(*position)
                .or_default()
                .extend(linked_nodes.clone());

            for node in linked_nodes {
                self.nodes.entry(node).or_default().insert(*position);
            }
        }

        tot - self.positions_to_add.len()
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

impl Display for NavigationDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "path length {}", self.path.len())
    }
}

#[cfg(test)]
mod tests {
    

    use super::*;

    struct Foo(Position);
    impl Reachable for Foo {
        fn to_position(&self) -> Position {
            self.0
        }
    }

    #[test]
    fn test_navigate_ok() {
        let mut navigator = Navigator::new();
        navigator.add_node(Position { x: 0, y: 0 });
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });
        navigator.add_node(Position { x: 3, y: 1 });
        navigator.add_node(Position { x: 3, y: 2 });
        navigator.add_node(Position { x: 3, y: 3 });
        navigator.add_node(Position { x: 2, y: 3 });
        navigator.add_node(Position { x: 1, y: 3 });
        navigator.add_node(Position { x: 0, y: 3 });

        navigator.rebuild();

        let desc =
            navigator.get_navigation_descriptor(&Position { x: 0, y: 0 }, Position { x: 0, y: 3 });

        assert_eq!(desc.is_some(), true);
    }

    #[test]
    fn test_navigate_ko_end_point() {
        let mut navigator = Navigator::new();
        navigator.add_node(Position { x: 0, y: 0 });
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });

        navigator.rebuild();

        let desc =
            navigator.get_navigation_descriptor(&Position { x: 0, y: 0 }, Position { x: 42, y: 0 });

        assert_eq!(desc, None);
    }

    #[test]
    fn test_navigate_ko_stating_point() {
        let mut navigator = Navigator::new();
        navigator.add_node(Position { x: 0, y: 0 });
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });

        navigator.rebuild();

        let desc =
            navigator.get_navigation_descriptor(&Position { x: 42, y: 0 }, Position { x: 0, y: 0 });

        assert_eq!(desc, None);
    }

    #[test]
    fn test_build() {
        let mut navigator = Navigator::new();
        navigator.add_node(Position { x: 1, y: 0 });
        navigator.add_node(Position { x: 2, y: 0 });
        navigator.add_node(Position { x: 3, y: 0 });
        navigator.add_node(Position { x: 4, y: 4 });

        let resolved = navigator.rebuild();

        assert_eq!(resolved, 3);
    }
}
