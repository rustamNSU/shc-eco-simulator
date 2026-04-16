#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
    Occupied,
}

impl core::fmt::Display for MapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfBounds => write!(f, "placement is out of map bounds"),
            Self::Occupied => write!(f, "one or more cells are already occupied"),
        }
    }
}

impl std::error::Error for MapError {}
