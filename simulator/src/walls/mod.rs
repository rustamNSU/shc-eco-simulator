#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WallSegment {
    pub id: u32,
    pub start_x: usize,
    pub start_y: usize,
    pub end_x: usize,
    pub end_y: usize,
}

impl WallSegment {
    pub fn new(id: u32, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> Self {
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

    pub fn cells(&self) -> Vec<(usize, usize)> {
        line_cells(self.start_x, self.start_y, self.end_x, self.end_y)
    }
}

pub fn line_cells(
    start_x: usize,
    start_y: usize,
    end_x: usize,
    end_y: usize,
) -> Vec<(usize, usize)> {
    if start_x == end_x {
        let min_y = start_y.min(end_y);
        let max_y = start_y.max(end_y);
        let mut cells = Vec::with_capacity((max_y - min_y) + 1);
        for y in min_y..=max_y {
            cells.push((start_x, y));
        }
        return cells;
    }

    if start_y == end_y {
        let min_x = start_x.min(end_x);
        let max_x = start_x.max(end_x);
        let mut cells = Vec::with_capacity((max_x - min_x) + 1);
        for x in min_x..=max_x {
            cells.push((x, start_y));
        }
        return cells;
    }

    Vec::new()
}
