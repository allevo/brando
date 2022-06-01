pub struct Configuration {
    pub cube_size: f32,
    pub width_table: usize,
    pub depth_table: usize,
    pub camera_velocity: f32,
}

impl Configuration {
    pub const fn half(&self) -> (f32, f32) {
        (
            self.width_table as f32 / 2. * self.cube_size,
            self.depth_table as f32 / 2. * self.cube_size,
        )
    }
}

pub const CONFIGURATION: Configuration = Configuration {
    cube_size: 0.3,
    width_table: 32,
    depth_table: 32,
    camera_velocity: 0.75,
};
