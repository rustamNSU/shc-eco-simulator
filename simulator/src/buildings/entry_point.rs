#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EntryPoint {
    pub x: usize,
    pub y: usize,
}
