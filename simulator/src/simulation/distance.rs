use crate::EntryPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DistanceKey {
    pub start_building_id: u32,
    pub finish_building_id: u32,
}

impl DistanceKey {
    pub fn new(start_building_id: u32, finish_building_id: u32) -> Self {
        Self {
            start_building_id,
            finish_building_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDistance {
    pub key: DistanceKey,
    pub start_entry: Option<EntryPoint>,
    pub finish_entry: Option<EntryPoint>,
    pub distance_cells: Option<u32>,
}

impl BuildingDistance {
    pub fn reachable(&self) -> bool {
        self.distance_cells.is_some()
    }
}
