use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl MapBounds {
    pub fn square(size: usize) -> Option<Self> {
        let size = i32::try_from(size).ok()?;
        (size > 0).then_some(Self {
            min_x: 0,
            min_y: 0,
            max_x: size,
            max_y: size,
        })
    }

    pub fn is_valid(self) -> bool {
        self.min_x < self.max_x && self.min_y < self.max_y
    }

    pub fn width(self) -> usize {
        usize::try_from(i64::from(self.max_x) - i64::from(self.min_x)).unwrap_or(0)
    }

    pub fn height(self) -> usize {
        usize::try_from(i64::from(self.max_y) - i64::from(self.min_y)).unwrap_or(0)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y
    }
}
