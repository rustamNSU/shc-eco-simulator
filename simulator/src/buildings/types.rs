#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingType {
    GoodsYard,
    Armoury,
    FletchersWorkshop,
    BlacksmithsWorkshop,
    PoleturnersWorkshop,
    ArmourersWorkshop,
}

impl BuildingType {
    pub fn id(self) -> &'static str {
        match self {
            Self::GoodsYard => "goods_yard",
            Self::Armoury => "armoury",
            Self::FletchersWorkshop => "fletchers_workshop",
            Self::BlacksmithsWorkshop => "blacksmiths_workshop",
            Self::PoleturnersWorkshop => "poleturners_workshop",
            Self::ArmourersWorkshop => "armourers_workshop",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::GoodsYard => "Goods Yard",
            Self::Armoury => "Armoury",
            Self::FletchersWorkshop => "Fletchers Workshop",
            Self::BlacksmithsWorkshop => "Blacksmiths Workshop",
            Self::PoleturnersWorkshop => "Poleturners Workshop",
            Self::ArmourersWorkshop => "Armourers Workshop",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "goods_yard" => Some(Self::GoodsYard),
            "armoury" => Some(Self::Armoury),
            "fletchers_workshop" => Some(Self::FletchersWorkshop),
            "blacksmiths_workshop" => Some(Self::BlacksmithsWorkshop),
            "poleturners_workshop" => Some(Self::PoleturnersWorkshop),
            "armourers_workshop" => Some(Self::ArmourersWorkshop),
            _ => None,
        }
    }

    pub const fn all() -> [Self; 6] {
        [
            Self::GoodsYard,
            Self::Armoury,
            Self::FletchersWorkshop,
            Self::BlacksmithsWorkshop,
            Self::PoleturnersWorkshop,
            Self::ArmourersWorkshop,
        ]
    }
}
