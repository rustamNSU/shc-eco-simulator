#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockpileResource {
    Wood,
    Iron,
    Wheat,
    Flour,
}

impl StockpileResource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wood => "Wood",
            Self::Iron => "Iron",
            Self::Wheat => "Wheat",
            Self::Flour => "Flour",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Wood => "W",
            Self::Iron => "I",
            Self::Wheat => "Wh",
            Self::Flour => "Fl",
        }
    }
}
