use super::{BuildingComponent, BuildingType, EntryPoint, Footprint};

#[derive(Debug, Clone)]
pub struct BuildingPlacement {
    pub id: u32,
    pub building_type: BuildingType,
    pub x: usize,
    pub y: usize,
    pub entry_point: Option<EntryPoint>,
    pub footprint: Footprint,
    pub components: Vec<BuildingComponent>,
}

impl BuildingPlacement {
    pub fn occupied_cells(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.footprint
            .occupied_offsets()
            .map(move |(dx, dy)| (self.x + dx, self.y + dy))
    }

    pub fn width(&self) -> usize {
        self.footprint.width()
    }

    pub fn height(&self) -> usize {
        self.footprint.height()
    }

    pub fn components(&self) -> &[BuildingComponent] {
        &self.components
    }
}
