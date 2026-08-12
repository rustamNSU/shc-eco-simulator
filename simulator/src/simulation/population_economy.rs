use serde::{Deserialize, Serialize};

use super::{IRON_BUY_GOLD, clamped_fear_factor};

pub const DEFAULT_MAX_POPULATION: u32 = 500;
pub const FOOD_PER_PERSON_PER_MINUTE: f64 = 0.6;
pub const INN_CAPACITY: u32 = 30;
pub const INN_GOLD_PER_MINUTE: f64 = 11.2;
pub const STONE_PER_MINUTE: f64 = 18.6;
pub const STONE_SELL_GOLD: f64 = 7.0;
pub const IRON_PER_MINUTE: f64 = 2.63;
pub const IRON_SELL_GOLD: f64 = 23.0;
pub const STONE_WORKERS: u32 = 4;
pub const IRON_MINE_WORKERS: u32 = 2;
pub const INN_WORKERS: u32 = 1;
pub const GAME_MONTH_TICKS: u32 = 800;
pub const INN_BUILD_WOOD: u32 = 20;
pub const INN_BUILD_GOLD: u32 = 100;
pub const STONE_QUARRY_BUILD_WOOD: u32 = 25;
pub const IRON_MINE_BUILD_WOOD: u32 = 20;

const MAX_CALCULATOR_COUNT: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PopulationEconomySettings {
    pub enabled: bool,
    pub max_population: u32,
    pub population: u32,
    pub inn_count: u32,
    pub stone_quarry_count: u32,
    pub iron_mine_count: u32,
    pub tax_index: u8,
    pub food_ratio_index: u8,
}

impl Default for PopulationEconomySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_population: DEFAULT_MAX_POPULATION,
            population: 0,
            inn_count: 0,
            stone_quarry_count: 0,
            iron_mine_count: 0,
            tax_index: 3,
            food_ratio_index: 2,
        }
    }
}

impl PopulationEconomySettings {
    pub fn normalized(mut self) -> Self {
        self.max_population = self.max_population.clamp(1, MAX_CALCULATOR_COUNT);
        self.population = self.population.min(self.max_population);
        self.inn_count = self.inn_count.min(MAX_CALCULATOR_COUNT);
        self.stone_quarry_count = self.stone_quarry_count.min(MAX_CALCULATOR_COUNT);
        self.iron_mine_count = self.iron_mine_count.min(MAX_CALCULATOR_COUNT);
        self.tax_index = self.tax_index.min((TAX_LEVELS.len() - 1) as u8);
        self.food_ratio_index = self.food_ratio_index.min((FOOD_RATIOS.len() - 1) as u8);
        self
    }

    pub fn tax_level(self) -> TaxLevel {
        TAX_LEVELS[usize::from(self.normalized().tax_index)]
    }

    pub fn food_ratio(self) -> FoodRatio {
        FOOD_RATIOS[usize::from(self.normalized().food_ratio_index)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaxLevel {
    pub popularity: i32,
    pub coefficient: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoodRatio {
    pub name: &'static str,
    pub multiplier: f64,
    pub popularity: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PopulationEconomyContext {
    pub game_speed_ticks_per_second: u32,
    pub fear_factor: i32,
    pub placed_workers: u32,
    pub food_produced_per_minute: f64,
    pub food_sell_gold_per_unit: f64,
    pub layout_gold_per_minute: f64,
    pub workshop_gold_per_minute: f64,
    pub food_gold_per_minute: f64,
    pub workshop_iron_demand_per_minute: f64,
    pub workshops_buy_iron: bool,
    pub iron_stockpile_available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopulationEconomyReport {
    pub settings: PopulationEconomySettings,
    pub tax: TaxLevel,
    pub food_ratio: FoodRatio,
    pub tax_gold_per_minute: f64,
    pub food_required_per_minute: f64,
    pub food_produced_per_minute: f64,
    pub food_balance_per_minute: f64,
    pub food_consumed_per_minute: f64,
    pub food_sellable_per_minute: f64,
    pub food_sale_reduction_per_minute: f64,
    pub inn_coverage_percent: f64,
    pub inn_popularity: i32,
    pub inn_gold_per_minute: f64,
    pub fear_popularity: i32,
    pub total_popularity: i32,
    pub stone_per_minute: f64,
    pub stone_gold_per_minute: f64,
    pub iron_per_minute: f64,
    pub iron_used_per_minute: f64,
    pub iron_bought_per_minute: f64,
    pub iron_surplus_per_minute: f64,
    pub iron_stockpile_available: bool,
    pub workshop_iron_demand_per_minute: f64,
    pub iron_gold_benefit_per_minute: f64,
    pub placed_workers: u32,
    pub additional_workers: u32,
    pub total_workers: u32,
    pub layout_gold_per_minute: f64,
    pub workshop_gold_per_minute: f64,
    pub food_gold_per_minute: f64,
    pub layout_gold_after_food_per_minute: f64,
    pub total_gold_per_minute: f64,
    pub additional_build_wood: u32,
    pub additional_build_gold: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IronEconomy {
    pub produced_per_minute: f64,
    pub required_per_minute: f64,
    pub used_per_minute: f64,
    pub bought_per_minute: f64,
    pub surplus_per_minute: f64,
    pub workshop_output_scale: f64,
    pub purchase_savings_gold_per_minute: f64,
    pub surplus_sale_gold_per_minute: f64,
}

impl IronEconomy {
    pub fn total_gold_benefit_per_minute(self) -> f64 {
        self.purchase_savings_gold_per_minute + self.surplus_sale_gold_per_minute
    }
}

pub fn calculate_iron_economy(
    produced_per_minute: f64,
    required_per_minute: f64,
    buy_iron: bool,
) -> IronEconomy {
    let produced_per_minute = produced_per_minute.max(0.0);
    let required_per_minute = required_per_minute.max(0.0);
    let used_per_minute = produced_per_minute.min(required_per_minute);
    let bought_per_minute = if buy_iron {
        (required_per_minute - used_per_minute).max(0.0)
    } else {
        0.0
    };
    let surplus_per_minute = (produced_per_minute - required_per_minute).max(0.0);
    let workshop_output_scale = if buy_iron || required_per_minute == 0.0 {
        1.0
    } else {
        used_per_minute / required_per_minute
    };
    let purchase_savings_gold_per_minute = if buy_iron {
        used_per_minute * f64::from(IRON_BUY_GOLD)
    } else {
        0.0
    };
    let surplus_sale_gold_per_minute = surplus_per_minute * IRON_SELL_GOLD;

    IronEconomy {
        produced_per_minute,
        required_per_minute,
        used_per_minute,
        bought_per_minute,
        surplus_per_minute,
        workshop_output_scale,
        purchase_savings_gold_per_minute,
        surplus_sale_gold_per_minute,
    }
}

pub const TAX_LEVELS: [TaxLevel; 12] = [
    TaxLevel {
        popularity: 7,
        coefficient: -1.0,
    },
    TaxLevel {
        popularity: 5,
        coefficient: -0.8,
    },
    TaxLevel {
        popularity: 3,
        coefficient: -0.6,
    },
    TaxLevel {
        popularity: 1,
        coefficient: 0.0,
    },
    TaxLevel {
        popularity: -2,
        coefficient: 0.6,
    },
    TaxLevel {
        popularity: -4,
        coefficient: 0.8,
    },
    TaxLevel {
        popularity: -6,
        coefficient: 1.0,
    },
    TaxLevel {
        popularity: -8,
        coefficient: 1.2,
    },
    TaxLevel {
        popularity: -12,
        coefficient: 1.4,
    },
    TaxLevel {
        popularity: -16,
        coefficient: 1.6,
    },
    TaxLevel {
        popularity: -20,
        coefficient: 1.8,
    },
    TaxLevel {
        popularity: -24,
        coefficient: 2.0,
    },
];

pub const FOOD_RATIOS: [FoodRatio; 5] = [
    FoodRatio {
        name: "No food",
        multiplier: 0.0,
        popularity: -8,
    },
    FoodRatio {
        name: "Half rations",
        multiplier: 0.5,
        popularity: -4,
    },
    FoodRatio {
        name: "Normal rations",
        multiplier: 1.0,
        popularity: 0,
    },
    FoodRatio {
        name: "Extra rations",
        multiplier: 1.5,
        popularity: 4,
    },
    FoodRatio {
        name: "Double rations",
        multiplier: 2.0,
        popularity: 8,
    },
];

pub fn calculate_population_economy(
    settings: PopulationEconomySettings,
    context: PopulationEconomyContext,
) -> PopulationEconomyReport {
    let settings = settings.normalized();
    let tax = settings.tax_level();
    let food_ratio = settings.food_ratio();
    let ticks_per_minute = f64::from(context.game_speed_ticks_per_second) * 60.0;
    let months_per_minute = ticks_per_minute / f64::from(GAME_MONTH_TICKS);
    let tax_gold_per_minute = if settings.enabled {
        f64::from(settings.population) * tax.coefficient * months_per_minute
    } else {
        0.0
    };
    let food_required_per_minute = if settings.enabled {
        f64::from(settings.population) * FOOD_PER_PERSON_PER_MINUTE * food_ratio.multiplier
    } else {
        0.0
    };
    let food_balance_per_minute = context.food_produced_per_minute - food_required_per_minute;
    let food_consumed_per_minute = context
        .food_produced_per_minute
        .min(food_required_per_minute);
    let food_sellable_per_minute =
        (context.food_produced_per_minute - food_consumed_per_minute).max(0.0);
    let food_sale_reduction_per_minute = food_consumed_per_minute * context.food_sell_gold_per_unit;
    let layout_gold_after_food_per_minute =
        context.layout_gold_per_minute - food_sale_reduction_per_minute;
    let inn_capacity = settings.inn_count.saturating_mul(INN_CAPACITY);
    let inn_coverage = if !settings.enabled || settings.population == 0 {
        0.0
    } else {
        (f64::from(inn_capacity) / f64::from(settings.population)).min(1.0)
    };
    let inn_popularity = if settings.enabled {
        inn_popularity(inn_coverage)
    } else {
        0
    };
    let inn_gold_per_minute = if settings.enabled {
        f64::from(settings.inn_count) * INN_GOLD_PER_MINUTE
    } else {
        0.0
    };
    let fear_popularity = clamped_fear_factor(context.fear_factor);
    let total_popularity = if settings.enabled {
        tax.popularity + food_ratio.popularity + inn_popularity + fear_popularity
    } else {
        0
    };
    let production_multiplier = 1.0 + f64::from(fear_popularity.unsigned_abs()) / 5.0 * 0.33;
    let stone_per_minute =
        f64::from(settings.stone_quarry_count) * STONE_PER_MINUTE * production_multiplier;
    let stone_gold_per_minute = stone_per_minute * STONE_SELL_GOLD;
    let iron_per_minute = if context.iron_stockpile_available {
        f64::from(settings.iron_mine_count) * IRON_PER_MINUTE * production_multiplier
    } else {
        0.0
    };
    let iron = calculate_iron_economy(
        iron_per_minute,
        context.workshop_iron_demand_per_minute,
        context.workshops_buy_iron,
    );
    let iron_gold_benefit_per_minute = iron.total_gold_benefit_per_minute();
    let inn_workers = if settings.enabled {
        settings.inn_count.saturating_mul(INN_WORKERS)
    } else {
        0
    };
    let additional_workers = inn_workers
        .saturating_add(settings.stone_quarry_count.saturating_mul(STONE_WORKERS))
        .saturating_add(settings.iron_mine_count.saturating_mul(IRON_MINE_WORKERS));
    let total_workers = context.placed_workers.saturating_add(additional_workers);
    let total_gold_per_minute = layout_gold_after_food_per_minute
        + tax_gold_per_minute
        + iron_gold_benefit_per_minute
        + stone_gold_per_minute
        - inn_gold_per_minute;
    let inn_build_wood = if settings.enabled {
        settings.inn_count.saturating_mul(INN_BUILD_WOOD)
    } else {
        0
    };
    let additional_build_wood = inn_build_wood
        .saturating_add(
            settings
                .stone_quarry_count
                .saturating_mul(STONE_QUARRY_BUILD_WOOD),
        )
        .saturating_add(
            settings
                .iron_mine_count
                .saturating_mul(IRON_MINE_BUILD_WOOD),
        );
    let additional_build_gold = if settings.enabled {
        settings.inn_count.saturating_mul(INN_BUILD_GOLD)
    } else {
        0
    };

    PopulationEconomyReport {
        settings,
        tax,
        food_ratio,
        tax_gold_per_minute,
        food_required_per_minute,
        food_produced_per_minute: context.food_produced_per_minute,
        food_balance_per_minute,
        food_consumed_per_minute,
        food_sellable_per_minute,
        food_sale_reduction_per_minute,
        inn_coverage_percent: inn_coverage * 100.0,
        inn_popularity,
        inn_gold_per_minute,
        fear_popularity,
        total_popularity,
        stone_per_minute,
        stone_gold_per_minute,
        iron_per_minute,
        iron_used_per_minute: iron.used_per_minute,
        iron_bought_per_minute: iron.bought_per_minute,
        iron_surplus_per_minute: iron.surplus_per_minute,
        iron_stockpile_available: context.iron_stockpile_available,
        workshop_iron_demand_per_minute: context.workshop_iron_demand_per_minute,
        iron_gold_benefit_per_minute,
        placed_workers: context.placed_workers,
        additional_workers,
        total_workers,
        layout_gold_per_minute: context.layout_gold_per_minute,
        workshop_gold_per_minute: context.workshop_gold_per_minute,
        food_gold_per_minute: context.food_gold_per_minute,
        layout_gold_after_food_per_minute,
        total_gold_per_minute,
        additional_build_wood,
        additional_build_gold,
    }
}

fn inn_popularity(coverage: f64) -> i32 {
    if coverage >= 1.0 {
        8
    } else if coverage > 0.75 {
        6
    } else if coverage > 0.5 {
        4
    } else if coverage > 0.25 {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IRON_BUY_GOLD, PopulationEconomyContext, PopulationEconomySettings, calculate_iron_economy,
        calculate_population_economy,
    };

    #[test]
    fn iron_flow_caps_workshops_buys_only_deficit_and_sells_only_surplus() {
        let produced_only = calculate_iron_economy(4.0, 10.0, false);
        assert_eq!(produced_only.used_per_minute, 4.0);
        assert_eq!(produced_only.bought_per_minute, 0.0);
        assert_eq!(produced_only.workshop_output_scale, 0.4);

        let with_buying = calculate_iron_economy(4.0, 10.0, true);
        assert_eq!(with_buying.used_per_minute, 4.0);
        assert_eq!(with_buying.bought_per_minute, 6.0);
        assert_eq!(with_buying.workshop_output_scale, 1.0);
        assert_eq!(
            with_buying.purchase_savings_gold_per_minute,
            4.0 * f64::from(IRON_BUY_GOLD)
        );

        let surplus = calculate_iron_economy(12.0, 10.0, false);
        assert_eq!(surplus.used_per_minute, 10.0);
        assert_eq!(surplus.surplus_per_minute, 2.0);
        assert_eq!(surplus.surplus_sale_gold_per_minute, 46.0);
    }

    #[test]
    fn tax_food_inn_and_fear_popularity_are_combined() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                population: 100,
                inn_count: 4,
                tax_index: 6,
                food_ratio_index: 3,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext {
                game_speed_ticks_per_second: 50,
                fear_factor: -2,
                food_produced_per_minute: 100.0,
                ..PopulationEconomyContext::default()
            },
        );

        assert_eq!(report.tax.popularity, -6);
        assert_eq!(report.food_ratio.popularity, 4);
        assert_eq!(report.inn_popularity, 8);
        assert_eq!(report.fear_popularity, -2);
        assert_eq!(report.total_popularity, 4);
        assert_eq!(report.food_required_per_minute, 90.0);
        assert_eq!(report.tax_gold_per_minute, 375.0);
    }

    #[test]
    fn inn_popularity_uses_coverage_thresholds() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                population: 32,
                inn_count: 1,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext::default(),
        );
        assert_eq!(report.inn_popularity, 6);

        let full = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                population: 32,
                inn_count: 2,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext::default(),
        );
        assert_eq!(full.inn_popularity, 8);
    }

    #[test]
    fn mines_scale_with_fear_and_add_workers() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                stone_quarry_count: 1,
                iron_mine_count: 1,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext {
                fear_factor: -5,
                placed_workers: 10,
                iron_stockpile_available: true,
                ..PopulationEconomyContext::default()
            },
        );

        assert!((report.stone_per_minute - 18.6 * 1.33).abs() < f64::EPSILON);
        assert!((report.stone_gold_per_minute - 18.6 * 1.33 * 7.0).abs() < f64::EPSILON);
        assert!((report.iron_per_minute - 2.63 * 1.33).abs() < f64::EPSILON);
        assert_eq!(report.additional_workers, 6);
        assert_eq!(report.total_workers, 16);
    }

    #[test]
    fn iron_mines_are_independent_of_population_but_require_an_iron_stockpile() {
        let settings = PopulationEconomySettings {
            iron_mine_count: 2,
            ..PopulationEconomySettings::default()
        };
        let stockpile = PopulationEconomyContext {
            iron_stockpile_available: true,
            ..PopulationEconomyContext::default()
        };

        let report = calculate_population_economy(settings, stockpile);
        assert!((report.iron_per_minute - 5.26).abs() < f64::EPSILON);
        assert_eq!(report.additional_workers, 4);
        assert_eq!(report.additional_build_wood, 40);
        assert_eq!(
            calculate_population_economy(
                PopulationEconomySettings {
                    enabled: true,
                    ..settings
                },
                PopulationEconomyContext::default(),
            )
            .iron_per_minute,
            0.0,
        );
    }

    #[test]
    fn produced_iron_replaces_purchases_and_only_the_deficit_is_bought() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                iron_mine_count: 2,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext {
                workshop_iron_demand_per_minute: 10.0,
                workshops_buy_iron: true,
                iron_stockpile_available: true,
                ..PopulationEconomyContext::default()
            },
        );

        assert!((report.iron_per_minute - 5.26).abs() < f64::EPSILON);
        assert!((report.iron_used_per_minute - 5.26).abs() < f64::EPSILON);
        assert!((report.iron_bought_per_minute - 4.74).abs() < 0.000_001);
        assert!(
            (report.iron_gold_benefit_per_minute - 5.26 * f64::from(IRON_BUY_GOLD)).abs()
                < 0.000_001
        );
    }

    #[test]
    fn food_eaten_by_population_is_removed_from_sellable_layout_income() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                population: 100,
                tax_index: 3,
                food_ratio_index: 2,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext {
                game_speed_ticks_per_second: 50,
                food_produced_per_minute: 80.0,
                food_sell_gold_per_unit: 4.0,
                layout_gold_per_minute: 400.0,
                ..PopulationEconomyContext::default()
            },
        );

        assert_eq!(report.food_required_per_minute, 60.0);
        assert_eq!(report.food_consumed_per_minute, 60.0);
        assert_eq!(report.food_sellable_per_minute, 20.0);
        assert_eq!(report.food_sale_reduction_per_minute, 240.0);
        assert_eq!(report.layout_gold_after_food_per_minute, 160.0);
        assert_eq!(report.total_gold_per_minute, 160.0);
    }

    #[test]
    fn food_shortage_never_leaves_food_to_sell() {
        let report = calculate_population_economy(
            PopulationEconomySettings {
                enabled: true,
                population: 100,
                food_ratio_index: 4,
                ..PopulationEconomySettings::default()
            },
            PopulationEconomyContext {
                food_produced_per_minute: 50.0,
                food_sell_gold_per_unit: 4.0,
                layout_gold_per_minute: 200.0,
                ..PopulationEconomyContext::default()
            },
        );

        assert_eq!(report.food_required_per_minute, 120.0);
        assert_eq!(report.food_consumed_per_minute, 50.0);
        assert_eq!(report.food_sellable_per_minute, 0.0);
        assert_eq!(report.layout_gold_after_food_per_minute, 0.0);
    }
}
