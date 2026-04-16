#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingComponentType {
    GoodsYardStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingComponent {
    pub id: u8,
    pub component_type: BuildingComponentType,
    pub x: usize,
    pub y: usize,
    pub size: usize,
}
