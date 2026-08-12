use super::{BuildingComponent, BuildingType, EntryPoint, Footprint, StockpileResource};

#[derive(Debug, Clone)]
pub struct BuildingPlacement {
    pub id: u32,
    pub building_type: BuildingType,
    pub x: i32,
    pub y: i32,
    pub goods_yard_group_id: Option<u32>,
    pub stockpile_resource: Option<StockpileResource>,
    pub entry_point: Option<EntryPoint>,
    pub footprint: Footprint,
    pub components: Vec<BuildingComponent>,
}

impl BuildingPlacement {
    pub fn occupied_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.footprint
            .occupied_offsets()
            .map(move |(dx, dy)| (self.x + dx as i32, self.y + dy as i32))
    }

    pub fn blocking_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.footprint
            .blocking_offsets()
            .map(move |(dx, dy)| (self.x + dx as i32, self.y + dy as i32))
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
