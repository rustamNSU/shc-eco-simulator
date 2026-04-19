pub const WORKSHOP_SLOWDOWN_BASE: u32 = 2;

pub fn unit_speed_cells_per_tick(slowdown_base: u32) -> f64 {
    1.0 / (8.0 * ((slowdown_base as f64) + 1.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingCost {
    pub wood: u32,
    pub gold: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingType {
    GoodsYard,
    Stockpile,
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
            Self::Stockpile => "stockpile",
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
            Self::Stockpile => "Stockpile",
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

    pub fn worker_slowdown_base(self) -> Option<u32> {
        match self {
            Self::FletchersWorkshop
            | Self::BlacksmithsWorkshop
            | Self::PoleturnersWorkshop
            | Self::ArmourersWorkshop => Some(WORKSHOP_SLOWDOWN_BASE),
            _ => None,
        }
    }

    pub fn worker_speed_cells_per_tick(self) -> Option<f64> {
        self.worker_slowdown_base().map(unit_speed_cells_per_tick)
    }

    pub fn build_cost(self) -> BuildingCost {
        match self {
            Self::GoodsYard | Self::Stockpile => BuildingCost { wood: 0, gold: 0 },
            Self::Armoury => BuildingCost { wood: 5, gold: 0 },
            Self::FletchersWorkshop => BuildingCost {
                wood: 20,
                gold: 100,
            },
            Self::BlacksmithsWorkshop => BuildingCost {
                wood: 20,
                gold: 200,
            },
            Self::PoleturnersWorkshop => BuildingCost {
                wood: 10,
                gold: 100,
            },
            Self::ArmourersWorkshop => BuildingCost {
                wood: 20,
                gold: 100,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BuildingCost, BuildingType, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick};

    #[test]
    fn speed_formula_matches_spec() {
        let value = unit_speed_cells_per_tick(2);
        let expected = 1.0 / 24.0;
        assert!((value - expected).abs() < 1e-12);
    }

    #[test]
    fn workshops_have_slowdown_base_two() {
        for workshop in [
            BuildingType::FletchersWorkshop,
            BuildingType::BlacksmithsWorkshop,
            BuildingType::PoleturnersWorkshop,
            BuildingType::ArmourersWorkshop,
        ] {
            assert_eq!(
                workshop.worker_slowdown_base(),
                Some(WORKSHOP_SLOWDOWN_BASE)
            );
        }
    }

    #[test]
    fn non_workshops_have_no_worker_speed() {
        assert_eq!(BuildingType::GoodsYard.worker_slowdown_base(), None);
        assert_eq!(BuildingType::Stockpile.worker_slowdown_base(), None);
        assert_eq!(BuildingType::Armoury.worker_slowdown_base(), None);
        assert_eq!(BuildingType::GoodsYard.worker_speed_cells_per_tick(), None);
        assert_eq!(BuildingType::Stockpile.worker_speed_cells_per_tick(), None);
        assert_eq!(BuildingType::Armoury.worker_speed_cells_per_tick(), None);
    }

    #[test]
    fn build_costs_match_domain_table() {
        assert_eq!(
            BuildingType::Armoury.build_cost(),
            BuildingCost { wood: 5, gold: 0 }
        );
        assert_eq!(
            BuildingType::FletchersWorkshop.build_cost(),
            BuildingCost {
                wood: 20,
                gold: 100
            }
        );
        assert_eq!(
            BuildingType::BlacksmithsWorkshop.build_cost(),
            BuildingCost {
                wood: 20,
                gold: 200
            }
        );
        assert_eq!(
            BuildingType::PoleturnersWorkshop.build_cost(),
            BuildingCost {
                wood: 10,
                gold: 100
            }
        );
        assert_eq!(
            BuildingType::ArmourersWorkshop.build_cost(),
            BuildingCost {
                wood: 20,
                gold: 100
            }
        );
    }
}
