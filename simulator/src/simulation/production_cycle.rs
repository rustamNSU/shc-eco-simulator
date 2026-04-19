use crate::buildings::{BuildingPlacement, BuildingType, StockpileResource};

use super::{BuildingDistance, DistanceKey};

const TICKS_PER_CELL_FACTOR: u64 = 8;
const MIN_FEAR_FACTOR: i32 = -5;
const MAX_FEAR_FACTOR: i32 = 0;
const WORKSHOP_FEAR_RING_LEN: usize = 10;
const WORKSHOP_BASE_OUTPUT_RING: [u32; WORKSHOP_FEAR_RING_LEN] = [1, 2, 1, 2, 1, 2, 1, 2, 1, 2];

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
                make_time_ticks: 638,
                sell_gold: 15,
            },
            Self::Crossbow => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::FletchersWorkshop,
                wood_required: 3,
                iron_required: 0,
                make_time_ticks: 565,
                sell_gold: 30,
            },
            Self::Spear => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::PoleturnersWorkshop,
                wood_required: 1,
                iron_required: 0,
                make_time_ticks: 332,
                sell_gold: 10,
            },
            Self::Pike => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::PoleturnersWorkshop,
                wood_required: 2,
                iron_required: 0,
                make_time_ticks: 872,
                sell_gold: 18,
            },
            Self::Sword => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::BlacksmithsWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 1090,
                sell_gold: 30,
            },
            Self::Mace => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::BlacksmithsWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 910,
                sell_gold: 30,
            },
            Self::Armor => WeaponRecipe {
                weapon_type: self,
                workshop_type: BuildingType::ArmourersWorkshop,
                wood_required: 0,
                iron_required: 1,
                make_time_ticks: 625,
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
    pub fear_factor: i32,
    pub optimized_fletcher_routing: bool,
    pub fletchers_weapon: WeaponType,
    pub poleturners_weapon: WeaponType,
    pub blacksmiths_weapon: WeaponType,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            game_speed_ticks_per_second: 50,
            fear_factor: 0,
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

    pub fn average_weapon_output_per_cycle(self, building_type: BuildingType) -> f64 {
        if !matches!(
            building_type,
            BuildingType::FletchersWorkshop
                | BuildingType::BlacksmithsWorkshop
                | BuildingType::PoleturnersWorkshop
                | BuildingType::ArmourersWorkshop
        ) {
            return 0.0;
        }

        let ring = workshop_fear_output_ring(self.fear_factor);
        let total = ring.iter().sum::<u32>();
        total as f64 / WORKSHOP_FEAR_RING_LEN as f64
    }
}

pub fn clamped_fear_factor(fear_factor: i32) -> i32 {
    fear_factor.clamp(MIN_FEAR_FACTOR, MAX_FEAR_FACTOR)
}

pub fn workshop_fear_output_ring(fear_factor: i32) -> [u32; WORKSHOP_FEAR_RING_LEN] {
    let mut ring = WORKSHOP_BASE_OUTPUT_RING;
    let improved_one_count = clamped_fear_factor(fear_factor).unsigned_abs() as usize;

    for output in ring
        .iter_mut()
        .filter(|output| **output == 1)
        .take(improved_one_count)
    {
        *output = 2;
    }

    ring
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

    use super::{
        SimulationSettings, WeaponType, clamped_fear_factor, travel_ticks_for_distance,
        workshop_fear_output_ring,
    };

    #[test]
    fn travel_ticks_match_workshop_speed_formula() {
        let ticks = travel_ticks_for_distance(BuildingType::FletchersWorkshop, 50);
        assert_eq!(ticks, 50_u64 * 8 * u64::from(WORKSHOP_SLOWDOWN_BASE + 1));
    }

    #[test]
    fn default_settings_keep_fletcher_fix_disabled() {
        let settings = SimulationSettings::default();
        assert_eq!(settings.game_speed_ticks_per_second, 50);
        assert_eq!(settings.fear_factor, 0);
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

    #[test]
    fn fear_factor_is_clamped_to_supported_range() {
        assert_eq!(clamped_fear_factor(2), 0);
        assert_eq!(clamped_fear_factor(-3), -3);
        assert_eq!(clamped_fear_factor(-9), -5);
    }

    #[test]
    fn workshop_fear_output_ring_improves_one_entries_from_front() {
        assert_eq!(workshop_fear_output_ring(0), [1, 2, 1, 2, 1, 2, 1, 2, 1, 2]);
        assert_eq!(
            workshop_fear_output_ring(-2),
            [2, 2, 2, 2, 1, 2, 1, 2, 1, 2]
        );
        assert_eq!(
            workshop_fear_output_ring(-5),
            [2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
        );
    }

    #[test]
    fn fear_factor_average_output_matches_ten_cycle_ring() {
        let settings = SimulationSettings::default();
        assert_eq!(
            settings.average_weapon_output_per_cycle(BuildingType::FletchersWorkshop),
            1.5
        );

        let settings = SimulationSettings {
            fear_factor: -1,
            ..SimulationSettings::default()
        };
        assert_eq!(
            settings.average_weapon_output_per_cycle(BuildingType::FletchersWorkshop),
            1.6
        );

        let settings = SimulationSettings {
            fear_factor: -5,
            ..SimulationSettings::default()
        };
        assert_eq!(
            settings.average_weapon_output_per_cycle(BuildingType::FletchersWorkshop),
            2.0
        );
    }
}
