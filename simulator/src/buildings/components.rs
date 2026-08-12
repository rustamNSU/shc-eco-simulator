use super::EntryPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingComponentType {
    GoodsYardStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingComponent {
    pub id: u8,
    pub component_type: BuildingComponentType,
    pub x: i32,
    pub y: i32,
    pub size: usize,
    pub entry_point: Option<EntryPoint>,
}
