#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EntryPoint {
    pub x: i32,
    pub y: i32,
}
