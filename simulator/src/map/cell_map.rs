use crate::buildings::BuildingPlacement;

use super::MapError;

#[derive(Debug, Clone)]
pub struct CellMap {
    size: usize,
    cells: Vec<Option<u32>>,
    blocked: Vec<bool>,
}

impl CellMap {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            cells: vec![None; size * size],
            blocked: vec![false; size * size],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_occupied(&self, x: usize, y: usize) -> bool {
        self.get_cell(x, y).is_some()
    }

    pub fn is_in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size
    }

    pub fn is_blocked(&self, x: usize, y: usize) -> bool {
        self.is_in_bounds(x, y) && self.blocked[self.index(x, y)]
    }

    pub fn can_place(&self, placement: &BuildingPlacement) -> Result<(), MapError> {
        self.can_place_cells(placement.occupied_cells())
    }

    pub fn place(&mut self, placement: &BuildingPlacement) -> Result<(), MapError> {
        self.can_place(placement)?;

        for (x, y) in placement.occupied_cells() {
            let idx = self.index(x, y);
            self.cells[idx] = Some(placement.id);
        }
        for (x, y) in placement.blocking_cells() {
            let idx = self.index(x, y);
            self.blocked[idx] = true;
        }

        Ok(())
    }

    pub fn can_place_cells(
        &self,
        cells: impl IntoIterator<Item = (usize, usize)>,
    ) -> Result<(), MapError> {
        for (x, y) in cells {
            if x >= self.size || y >= self.size {
                return Err(MapError::OutOfBounds);
            }
            if self.is_occupied(x, y) {
                return Err(MapError::Occupied);
            }
        }
        Ok(())
    }

    pub fn place_cells(
        &mut self,
        occupant_id: u32,
        cells: impl IntoIterator<Item = (usize, usize)>,
    ) -> Result<(), MapError> {
        let cells: Vec<(usize, usize)> = cells.into_iter().collect();
        self.can_place_cells(cells.iter().copied())?;

        for (x, y) in cells {
            let idx = self.index(x, y);
            self.cells[idx] = Some(occupant_id);
            self.blocked[idx] = true;
        }

        Ok(())
    }

    pub fn clear_cells(&mut self, cells: impl IntoIterator<Item = (usize, usize)>) {
        for (x, y) in cells {
            if x >= self.size || y >= self.size {
                continue;
            }
            let idx = self.index(x, y);
            self.cells[idx] = None;
            self.blocked[idx] = false;
        }
    }

    fn get_cell(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.size || y >= self.size {
            return None;
        }
        self.cells[self.index(x, y)]
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.size + x
    }
}
