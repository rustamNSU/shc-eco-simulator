#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StockpileResource {
    Wood,
    Iron,
}

impl StockpileResource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wood => "Wood",
            Self::Iron => "Iron",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Wood => "W",
            Self::Iron => "I",
        }
    }
}
