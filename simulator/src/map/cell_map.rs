use crate::buildings::BuildingPlacement;

use super::MapError;

#[derive(Debug, Clone)]
pub struct CellMap {
    size: usize,
    cells: Vec<Option<u32>>,
}

impl CellMap {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            cells: vec![None; size * size],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_occupied(&self, x: usize, y: usize) -> bool {
        self.get_cell(x, y).is_some()
    }

    pub fn can_place(&self, placement: &BuildingPlacement) -> Result<(), MapError> {
        for (x, y) in placement.occupied_cells() {
            if x >= self.size || y >= self.size {
                return Err(MapError::OutOfBounds);
            }
            if self.is_occupied(x, y) {
                return Err(MapError::Occupied);
            }
        }
        Ok(())
    }

    pub fn place(&mut self, placement: &BuildingPlacement) -> Result<(), MapError> {
        self.can_place(placement)?;

        for (x, y) in placement.occupied_cells() {
            let idx = self.index(x, y);
            self.cells[idx] = Some(placement.id);
        }

        Ok(())
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
