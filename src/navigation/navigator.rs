use std::collections::{HashMap, HashSet};

use tracing::info;

use crate::common::position::Position;

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

    pub fn get_navigation_descriptor(
        &mut self,
        end: impl Into<Reachable>,
    ) -> Option<NavigationDescriptor> {
        use pathfinding::prelude::astar;

        let reachable: Reachable = end.into();
        let end = reachable.position;
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
        let descriptor = NavigationDescriptor {
            path,
            count: reachable.count,
        };

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

    pub fn make_progress(
        &self,
        navigator_descriptor: &mut NavigationDescriptor,
    ) -> Result<(), &'static str> {
        navigator_descriptor.path.pop();

        Ok(())
    }
}
pub struct Reachable {
    pub position: Position,
    pub count: u8,
}

// TODO: avoid "pub" here
#[derive(Debug)]
pub struct NavigationDescriptor {
    pub path: Vec<Position>,
    pub count: u8,
}

impl NavigationDescriptor {
    pub fn is_completed(&self) -> bool {
        self.path.is_empty()
    }
}
