use crate::position::Position;

pub struct Palatability {
    total_populations: u64,
    // TODO: change approach to store the rendered values
    houses_sources: Vec<HouseSourcePalatabilityDescriptor>,
}
impl Palatability {
    pub fn new() -> Self {
        Self {
            total_populations: 0,
            houses_sources: vec![],
        }
    }

    pub fn add_house_source(&mut self, source: impl Into<HouseSourcePalatabilityDescriptor>) {
        self.houses_sources.push(source.into());
    }

    pub fn get_house_palatability(&self, position: &Position) -> HousePalatability {
        let value: i32 = self
            .houses_sources
            .iter()
            .map(|hsp| hsp.calculate(position))
            .sum();
        HousePalatability { value }
    }

    pub fn increment_populations(&mut self, delta: i32) {
        self.total_populations = (self.total_populations as i128 + delta as i128).max(0) as u64;
    }

    pub fn total_populations(&self) -> u64 {
        self.total_populations
    }
}

pub struct HouseSourcePalatabilityDescriptor {
    pub origin: Position,
    pub value: i32,
    pub max_horizontal_distribution_distance: usize,
    pub max_linear_distribution_distance: usize,
    pub linear_factor: i32,
}

impl HouseSourcePalatabilityDescriptor {
    fn calculate(&self, position: &Position) -> i32 {
        let distance = self.origin.distance(position);

        if distance < self.max_horizontal_distribution_distance {
            return self.value;
        }

        if distance < self.max_linear_distribution_distance {
            let d = (distance - self.max_horizontal_distribution_distance) as i32;

            let value = self.value - self.linear_factor * d;
            if self.value > 0 {
                return value.max(0);
            } else {
                return value.min(0);
            }
        }

        0
    }
}

pub struct HousePalatability {
    value: i32,
}

impl HousePalatability {
    #[inline]
    pub fn is_positive(&self) -> bool {
        // we consider 0 as positive palatability
        !self.value.is_negative()
    }
}
