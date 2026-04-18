use crate::buildings::{BuildingPlacement, BuildingType, StockpileResource};

use super::{BuildingDistance, DistanceKey};

const TICKS_PER_CELL_FACTOR: u64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponType {
    Bow,
    Crossbow,
    Spear,
    Pike,
    Sword,
    Mace,
    Armor,
}

impl WeaponType {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Bow => "Bow",
            Self::Crossbow => "Crossbow",
            Self::Spear => "Spear",
            Self::Pike => "Pike",
            Self::Sword => "Sword",
            Self::Mace => "Mace",
            Self::Armor => "Armor",
        }
    }

    pub const fn all() -> [Self; 7] {
        [
            Self::Bow,
            Self::Crossbow,
            Self::Spear,
            Self::Pike,
            Self::Sword,
            Self::Mace,
            Self::Armor,
        ]
    }

    pub fn recipe(self) -> WeaponRecipe {
        match self {
            Self::Bow => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::FletchersWorkshop,
                wood_required: 2,
                iron_required: 0,
                make_time_ticks: 400,
                sell_gold: 15,
            },
            Self::Crossbow => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::FletchersWorkshop,
                wood_required: 3,
                iron_required: 0,
                make_time_ticks: 550,
                sell_gold: 30,
            },
            Self::Spear => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::PoleturnersWorkshop,
                wood_required: 1,
                iron_required: 0,
                make_time_ticks: 300,
                sell_gold: 10,
            },
            Self::Pike => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::PoleturnersWorkshop,
                wood_required: 2,
                iron_required: 0,
                make_time_ticks: 600,
                sell_gold: 18,
            },
            Self::Sword => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::BlacksmithsWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 600,
                sell_gold: 30,
            },
            Self::Mace => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::BlacksmithsWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 600,
                sell_gold: 30,
            },
            Self::Armor => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::ArmourersWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 550,
                sell_gold: 30,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponRecipe {
    pub weapon_type: WeaponType,
    pub workshop_type: BuildingType,
    pub wood_required: u32,
    pub iron_required: u32,
    pub make_time_ticks: u64,
    pub sell_gold: u32,
}

impl WeaponRecipe {
    pub fn total_required_units(self) -> u32 {
        self.wood_required + self.iron_required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationSettings {
    pub game_speed_ticks_per_second: u32,
    pub optimized_fletcher_routing: bool,
    pub fletchers_weapon: WeaponType,
    pub poleturners_weapon: WeaponType,
    pub blacksmiths_weapon: WeaponType,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            game_speed_ticks_per_second: 50,
            optimized_fletcher_routing: false,
            fletchers_weapon: WeaponType::Bow,
            poleturners_weapon: WeaponType::Spear,
            blacksmiths_weapon: WeaponType::Sword,
        }
    }
}

impl SimulationSettings {
    pub fn selected_weapon_for(self, building_type: BuildingType) -> Option<WeaponType> {
        match building_type {
            BuildingType::FletchersWorkshop => Some(self.fletchers_weapon),
            BuildingType::PoleturnersWorkshop => Some(self.poleturners_weapon),
            BuildingType::BlacksmithsWorkshop => Some(self.blacksmiths_weapon),
            BuildingType::ArmourersWorkshop => Some(WeaponType::Armor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionRouteUsage {
    pub start_building_id: u32,
    pub finish_building_id: u32,
    pub trips: u32,
    pub distance_cells_per_trip: u32,
    pub total_distance_cells: u32,
    pub total_ticks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionCycle {
    pub recipe: WeaponRecipe,
    pub workshop_id: u32,
    pub armoury_id: u32,
    pub route_usage: Vec<ProductionRouteUsage>,
    pub travel_distance_cells: u32,
    pub travel_ticks: u64,
    pub make_ticks: u64,
    pub total_ticks: u64,
}

impl ProductionCycle {
    pub fn duration_seconds(&self, game_speed_ticks_per_second: u32) -> Option<f64> {
        if game_speed_ticks_per_second == 0 {
            return None;
        }

        Some(self.total_ticks as f64 / game_speed_ticks_per_second as f64)
    }

    pub(crate) fn from_route_usage(
        recipe: WeaponRecipe,
        workshop_id: u32,
        armoury_id: u32,
        route_usage: Vec<ProductionRouteUsage>,
    ) -> Self {
        let travel_distance_cells = route_usage
            .iter()
            .map(|usage| usage.total_distance_cells)
            .sum();
        let travel_ticks = route_usage.iter().map(|usage| usage.total_ticks).sum();
        let make_ticks = recipe.make_time_ticks;

        Self {
            recipe,
            workshop_id,
            armoury_id,
            route_usage,
            travel_distance_cells,
            travel_ticks,
            make_ticks,
            total_ticks: travel_ticks + make_ticks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionCycleError {
    WorkshopNotFound {
        workshop_id: u32,
    },
    ArmouryNotFound {
        armoury_id: u32,
    },
    ExpectedWorkshop {
        workshop_id: u32,
        actual_type: BuildingType,
        expected_type: BuildingType,
    },
    ExpectedArmoury {
        armoury_id: u32,
        actual_type: BuildingType,
    },
    MissingStockpile {
        resource: StockpileResource,
    },
    MissingDistance {
        start_building_id: u32,
        finish_building_id: u32,
    },
    UnreachableDistance {
        start_building_id: u32,
        finish_building_id: u32,
    },
}

impl core::fmt::Display for ProductionCycleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WorkshopNotFound { workshop_id } => {
                write!(f, "workshop with id {workshop_id} was not found")
            }
            Self::ArmouryNotFound { armoury_id } => {
                write!(f, "armoury with id {armoury_id} was not found")
            }
            Self::ExpectedWorkshop {
                workshop_id,
                actual_type,
                expected_type,
            } => write!(
                f,
                "building {workshop_id} is {}, but {} is required",
                actual_type.display_name(),
                expected_type.display_name()
            ),
            Self::ExpectedArmoury {
                armoury_id,
                actual_type,
            } => write!(
                f,
                "building {armoury_id} is {}, but Armoury is required",
                actual_type.display_name()
            ),
            Self::MissingStockpile { resource } => {
                write!(f, "no stockpile is marked for {}", resource.display_name())
            }
            Self::MissingDistance {
                start_building_id,
                finish_building_id,
            } => write!(
                f,
                "distance {start_building_id} -> {finish_building_id} is missing"
            ),
            Self::UnreachableDistance {
                start_building_id,
                finish_building_id,
            } => write!(
                f,
                "distance {start_building_id} -> {finish_building_id} is unreachable"
            ),
        }
    }
}

impl std::error::Error for ProductionCycleError {}

pub(crate) fn find_building(
    buildings: &[BuildingPlacement],
    building_id: u32,
) -> Option<&BuildingPlacement> {
    buildings.iter().find(|building| building.id == building_id)
}

pub(crate) fn find_stockpile_for_resource(
    buildings: &[BuildingPlacement],
    resource: StockpileResource,
) -> Option<&BuildingPlacement> {
    buildings
        .iter()
        .find(|building| building.stockpile_resource == Some(resource))
}

pub(crate) fn distance_cells(
    distances: &std::collections::HashMap<DistanceKey, BuildingDistance>,
    start_building_id: u32,
    finish_building_id: u32,
) -> Result<u32, ProductionCycleError> {
    let key = DistanceKey::new(start_building_id, finish_building_id);
    let Some(distance) = distances.get(&key) else {
        return Err(ProductionCycleError::MissingDistance {
            start_building_id,
            finish_building_id,
        });
    };

    distance
        .distance_cells
        .ok_or(ProductionCycleError::UnreachableDistance {
            start_building_id,
            finish_building_id,
        })
}

pub(crate) fn travel_ticks_for_distance(building_type: BuildingType, distance_cells: u32) -> u64 {
    let slowdown_base = building_type.worker_slowdown_base().unwrap_or(0);
    u64::from(distance_cells) * TICKS_PER_CELL_FACTOR * u64::from(slowdown_base + 1)
}

#[cfg(test)]
mod tests {
    use crate::buildings::{BuildingType, WORKSHOP_SLOWDOWN_BASE};

    use super::{SimulationSettings, WeaponType, travel_ticks_for_distance};

    #[test]
    fn travel_ticks_match_workshop_speed_formula() {
        let ticks = travel_ticks_for_distance(BuildingType::FletchersWorkshop, 50);
        assert_eq!(ticks, 50_u64 * 8 * u64::from(WORKSHOP_SLOWDOWN_BASE + 1));
    }

    #[test]
    fn default_settings_keep_fletcher_fix_disabled() {
        let settings = SimulationSettings::default();
        assert_eq!(settings.game_speed_ticks_per_second, 50);
        assert!(!settings.optimized_fletcher_routing);
        assert_eq!(settings.fletchers_weapon, WeaponType::Bow);
        assert_eq!(settings.poleturners_weapon, WeaponType::Spear);
        assert_eq!(settings.blacksmiths_weapon, WeaponType::Sword);
    }

    #[test]
    fn weapon_recipe_total_units_matches_resource_counts() {
        assert_eq!(WeaponType::Bow.recipe().total_required_units(), 2);
        assert_eq!(WeaponType::Sword.recipe().total_required_units(), 1);
    }
}
