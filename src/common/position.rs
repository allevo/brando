static DELTA: [Position; 4] = [
    Position { x: -1, y: 0 },
    Position { x: 1, y: 0 },
    Position { x: 0, y: -1 },
    Position { x: 0, y: 1 },
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}
impl Position {
    pub fn neighbors(&self) -> impl Iterator<Item = Self> + '_ {
        DELTA.iter().map(|p| Position {
            x: p.x + self.x,
            y: p.y + self.y,
        })
    }

    pub fn distance(&self, position: &Position) -> u32 {
        ((position.x - self.x).abs() + (position.y - self.y).abs())
            .try_into()
            .expect("i64 cannot be converted to u32")
    }
}
