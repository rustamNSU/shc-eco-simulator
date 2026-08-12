#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WallSegment {
    pub id: u32,
    pub start_x: i32,
    pub start_y: i32,
    pub end_x: i32,
    pub end_y: i32,
}

impl WallSegment {
    pub fn new(id: u32, start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Self {
        Self {
            id,
            start_x,
            start_y,
            end_x,
            end_y,
        }
    }

    pub fn is_axis_aligned(&self) -> bool {
        self.start_x == self.end_x || self.start_y == self.end_y
    }

    pub fn cells(&self) -> Vec<(i32, i32)> {
        line_cells(self.start_x, self.start_y, self.end_x, self.end_y)
    }
}

pub fn line_cells(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Vec<(i32, i32)> {
    if start_x == end_x {
        let min_y = start_y.min(end_y);
        let max_y = start_y.max(end_y);
        let mut cells = Vec::with_capacity((max_y - min_y + 1) as usize);
        for y in min_y..=max_y {
            cells.push((start_x, y));
        }
        return cells;
    }

    if start_y == end_y {
        let min_x = start_x.min(end_x);
        let max_x = start_x.max(end_x);
        let mut cells = Vec::with_capacity((max_x - min_x + 1) as usize);
        for x in min_x..=max_x {
            cells.push((x, start_y));
        }
        return cells;
    }

    Vec::new()
}
