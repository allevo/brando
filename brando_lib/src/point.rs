use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Point((i32, i32));
impl Point {
    pub fn new(width: i32, heigth: i32) -> Self {
        Self((width, heigth))
    }

    #[inline]
    pub fn x(&self) -> i32 {
        self.0 .0
    }

    #[inline]
    pub fn y(&self) -> i32 {
        self.0 .1
    }
}
