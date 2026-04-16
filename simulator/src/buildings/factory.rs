use super::{BuildingComponent, BuildingComponentType, BuildingPlacement, BuildingType, Footprint};

#[derive(Debug, Default)]
pub struct BuildingFactory {
    next_id: u32,
}

impl BuildingFactory {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    pub fn create(&mut self, building_type: BuildingType, x: usize, y: usize) -> BuildingPlacement {
        let components = self.create_components(building_type, x, y);
        let placement = BuildingPlacement {
            id: self.next_id,
            building_type,
            x,
            y,
            entry_point: None,
            footprint: Footprint::for_type(building_type),
            components,
        };
        self.next_id += 1;
        placement
    }

    fn create_components(
        &self,
        building_type: BuildingType,
        x: usize,
        y: usize,
    ) -> Vec<BuildingComponent> {
        match building_type {
            BuildingType::GoodsYard => vec![
                BuildingComponent {
                    id: 1,
                    component_type: BuildingComponentType::GoodsYardStack,
                    x,
                    y,
                    size: 2,
                    entry_point: None,
                },
                BuildingComponent {
                    id: 2,
                    component_type: BuildingComponentType::GoodsYardStack,
                    x: x + 3,
                    y,
                    size: 2,
                    entry_point: None,
                },
                BuildingComponent {
                    id: 3,
                    component_type: BuildingComponentType::GoodsYardStack,
                    x,
                    y: y + 3,
                    size: 2,
                    entry_point: None,
                },
                BuildingComponent {
                    id: 4,
                    component_type: BuildingComponentType::GoodsYardStack,
                    x: x + 3,
                    y: y + 3,
                    size: 2,
                    entry_point: None,
                },
            ],
            _ => Vec::new(),
        }
    }
}
