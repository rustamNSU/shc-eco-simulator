use std::collections::HashMap;

use crate::buildings::{BuildingPlacement, BuildingType, StockpileResource};

use super::{BuildingDistance, DistanceKey, SimulationSettings};

pub const WHEAT_FARM_WORK_TICKS: u64 = 6_950;
pub const WHEAT_FARM_WALKS_PER_CYCLE: u32 = 12;
pub const MILL_PROCESS_TICKS: u64 = 312;
pub const MILL_WORKER_COUNT: u32 = 3;
pub const BAKERY_WORK_TICKS: u64 = 1_700;

const FARM_LOADED_TICKS_PER_CELL: u64 = 16;
const FARM_EMPTY_TICKS_PER_CELL: u64 = 12;
const MILL_TICKS_PER_CELL: u64 = 16;
const BAKERY_TICKS_PER_CELL: u64 = 24;

pub const WHEAT_SELL_GOLD: f64 = 8.0;
pub const WHEAT_BUY_GOLD: f64 = 23.0;
pub const FLOUR_SELL_GOLD: f64 = 10.0;
pub const FLOUR_BUY_GOLD: f64 = 32.0;
pub const BREAD_SELL_GOLD: f64 = 4.0;
pub const BREAD_BUY_GOLD: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct BreadBuildingRate {
    pub building_id: u32,
    pub capacity_per_minute: f64,
    pub actual_per_minute: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreadEconomyReport {
    pub wheat_per_minute: f64,
    pub flour_capacity_per_minute: f64,
    pub flour_per_minute: f64,
    pub bakery_batches_per_minute: f64,
    pub bread_per_minute: f64,
    pub bread_batches_per_minute: f64,
    pub surplus_wheat_per_minute: f64,
    pub surplus_flour_per_minute: f64,
    pub purchased_wheat_per_minute: f64,
    pub purchased_flour_per_minute: f64,
    pub wheat_per_farm_cycle: f64,
    pub bread_per_flour: f64,
    pub limiting_stage: &'static str,
    pub farm_count: usize,
    pub mill_count: usize,
    pub bakery_count: usize,
    pub granary_count: usize,
    pub farm_rates: Vec<BreadBuildingRate>,
    pub mill_rates: Vec<BreadBuildingRate>,
    pub bakery_rates: Vec<BreadBuildingRate>,
    pub issues: Vec<String>,
}

pub(crate) fn calculate_bread_economy(
    buildings: &[BuildingPlacement],
    distances: &HashMap<DistanceKey, BuildingDistance>,
    settings: SimulationSettings,
) -> BreadEconomyReport {
    let farms = buildings_of_type(buildings, BuildingType::WheatFarm);
    let mills = buildings_of_type(buildings, BuildingType::Windmill);
    let bakeries = buildings_of_type(buildings, BuildingType::Bakery);
    let granaries = buildings_of_type(buildings, BuildingType::Granary);
    let wheat_stock = stockpile(buildings, StockpileResource::Wheat);
    let flour_stock = stockpile(buildings, StockpileResource::Flour);
    let mut issues = Vec::new();

    if wheat_stock.is_none() {
        issues.push("No stockpile is marked for Wheat".to_string());
    }
    if flour_stock.is_none() {
        issues.push("No stockpile is marked for Flour".to_string());
    }
    if granaries.is_empty() {
        issues.push("No Granary placed".to_string());
    }

    let wheat_per_farm_cycle = wheat_output_per_farm_cycle(settings.fear_factor);
    let bread_per_flour = bread_output_per_flour(settings.fear_factor);

    let farm_rates_per_tick = wheat_stock.map_or_else(Vec::new, |stock| {
        farms
            .iter()
            .filter_map(|farm| {
                let distance = route_distance(distances, farm.id, stock.id, &mut issues)?;
                let travel_ticks = farm_travel_ticks(distance);
                Some((
                    farm.id,
                    wheat_per_farm_cycle / (WHEAT_FARM_WORK_TICKS + travel_ticks) as f64,
                ))
            })
            .collect()
    });
    let wheat_per_tick = sum_rates(&farm_rates_per_tick);

    let mill_rates_per_tick = match (wheat_stock, flour_stock) {
        (Some(wheat), Some(flour)) => mills
            .iter()
            .filter_map(|mill| {
                let route_cells = route_distance(distances, flour.id, wheat.id, &mut issues)?
                    + route_distance(distances, wheat.id, mill.id, &mut issues)?
                    + route_distance(distances, mill.id, flour.id, &mut issues)?;
                let worker_cycle_ticks =
                    MILL_PROCESS_TICKS + u64::from(route_cells) * MILL_TICKS_PER_CELL;
                Some((
                    mill.id,
                    mill_capacity_per_tick(worker_cycle_ticks - MILL_PROCESS_TICKS),
                ))
            })
            .collect(),
        _ => Vec::new(),
    };
    let flour_capacity_per_tick = sum_rates(&mill_rates_per_tick);

    let bakery_rates_per_tick = flour_stock.map_or_else(Vec::new, |flour| {
        bakeries
            .iter()
            .filter_map(|bakery| {
                let route_cells = granaries
                    .iter()
                    .filter_map(|granary| {
                        let to_flour = distance(distances, granary.id, flour.id)?;
                        let to_bakery = distance(distances, flour.id, bakery.id)?;
                        let to_granary = distance(distances, bakery.id, granary.id)?;
                        Some(to_flour + to_bakery + to_granary)
                    })
                    .min();

                let Some(route_cells) = route_cells else {
                    issues.push(format!(
                        "Bakery #{} has no reachable Flour stockpile and Granary route",
                        bakery.id
                    ));
                    return None;
                };

                let cycle_ticks =
                    BAKERY_WORK_TICKS + u64::from(route_cells) * BAKERY_TICKS_PER_CELL;
                Some((bakery.id, 1.0 / cycle_ticks as f64))
            })
            .collect()
    });
    let bakery_capacity_per_tick = sum_rates(&bakery_rates_per_tick);

    let farm_wheat_processed_per_tick = wheat_per_tick.min(flour_capacity_per_tick);
    let purchased_wheat_per_tick = if settings.buy_wheat {
        (bakery_capacity_per_tick - farm_wheat_processed_per_tick)
            .max(0.0)
            .min((flour_capacity_per_tick - farm_wheat_processed_per_tick).max(0.0))
    } else {
        0.0
    };
    let flour_per_tick = farm_wheat_processed_per_tick + purchased_wheat_per_tick;
    let purchased_flour_per_tick = if settings.buy_flour {
        (bakery_capacity_per_tick - flour_per_tick).max(0.0)
    } else {
        0.0
    };
    let bread_batches_per_tick =
        (flour_per_tick + purchased_flour_per_tick).min(bakery_capacity_per_tick);
    let ticks_per_minute = f64::from(settings.game_speed_ticks_per_second) * 60.0;
    let limiting_stage = limiting_stage(
        wheat_per_tick,
        flour_capacity_per_tick,
        bakery_capacity_per_tick,
        settings.buy_wheat,
        settings.buy_flour,
    );

    BreadEconomyReport {
        wheat_per_minute: wheat_per_tick * ticks_per_minute,
        flour_capacity_per_minute: flour_capacity_per_tick * ticks_per_minute,
        flour_per_minute: flour_per_tick * ticks_per_minute,
        bakery_batches_per_minute: bakery_capacity_per_tick * ticks_per_minute,
        bread_per_minute: bread_batches_per_tick * bread_per_flour * ticks_per_minute,
        bread_batches_per_minute: bread_batches_per_tick * ticks_per_minute,
        surplus_wheat_per_minute: (wheat_per_tick - farm_wheat_processed_per_tick).max(0.0)
            * ticks_per_minute,
        surplus_flour_per_minute: (flour_per_tick - bread_batches_per_tick).max(0.0)
            * ticks_per_minute,
        purchased_wheat_per_minute: purchased_wheat_per_tick * ticks_per_minute,
        purchased_flour_per_minute: purchased_flour_per_tick * ticks_per_minute,
        wheat_per_farm_cycle,
        bread_per_flour,
        limiting_stage,
        farm_count: farms.len(),
        mill_count: mills.len(),
        bakery_count: bakeries.len(),
        granary_count: granaries.len(),
        farm_rates: allocate_rates(&farm_rates_per_tick, wheat_per_tick, ticks_per_minute),
        mill_rates: allocate_rates(&mill_rates_per_tick, flour_per_tick, ticks_per_minute),
        bakery_rates: allocate_rates(
            &bakery_rates_per_tick,
            bread_batches_per_tick,
            ticks_per_minute,
        ),
        issues,
    }
}

fn sum_rates(rates: &[(u32, f64)]) -> f64 {
    rates.iter().map(|(_, rate)| rate).sum()
}

fn allocate_rates(
    rates: &[(u32, f64)],
    actual_total_per_tick: f64,
    ticks_per_minute: f64,
) -> Vec<BreadBuildingRate> {
    let capacity_total = sum_rates(rates);
    rates
        .iter()
        .map(|(building_id, capacity)| BreadBuildingRate {
            building_id: *building_id,
            capacity_per_minute: capacity * ticks_per_minute,
            actual_per_minute: if capacity_total == 0.0 {
                0.0
            } else {
                actual_total_per_tick * capacity / capacity_total * ticks_per_minute
            },
        })
        .collect()
}

pub fn wheat_output_per_farm_cycle(fear_factor: i32) -> f64 {
    24.0 + fear_progress(fear_factor) * 12.0
}

pub fn bread_output_per_flour(fear_factor: i32) -> f64 {
    8.0 + fear_progress(fear_factor) * 4.0
}

fn fear_progress(fear_factor: i32) -> f64 {
    f64::from(fear_factor.clamp(-5, 0).unsigned_abs()) / 5.0
}

fn buildings_of_type(
    buildings: &[BuildingPlacement],
    building_type: BuildingType,
) -> Vec<&BuildingPlacement> {
    buildings
        .iter()
        .filter(|building| building.building_type == building_type)
        .collect()
}

fn stockpile(
    buildings: &[BuildingPlacement],
    resource: StockpileResource,
) -> Option<&BuildingPlacement> {
    buildings
        .iter()
        .find(|building| building.stockpile_resource == Some(resource))
}

fn route_distance(
    distances: &HashMap<DistanceKey, BuildingDistance>,
    start_id: u32,
    finish_id: u32,
    issues: &mut Vec<String>,
) -> Option<u32> {
    let result = distance(distances, start_id, finish_id);
    if result.is_none() {
        issues.push(format!(
            "Route #{} -> #{} is unreachable",
            start_id, finish_id
        ));
    }
    result
}

fn distance(
    distances: &HashMap<DistanceKey, BuildingDistance>,
    start_id: u32,
    finish_id: u32,
) -> Option<u32> {
    distances
        .get(&DistanceKey::new(start_id, finish_id))?
        .distance_cells
}

fn limiting_stage(
    wheat: f64,
    mill: f64,
    bakery: f64,
    buy_wheat: bool,
    buy_flour: bool,
) -> &'static str {
    if buy_flour {
        return "Bakery";
    }
    if buy_wheat {
        return if mill <= bakery {
            "Wind mill"
        } else {
            "Bakery"
        };
    }
    if wheat <= mill && wheat <= bakery {
        "Wheat"
    } else if mill <= bakery {
        "Wind mill"
    } else {
        "Bakery"
    }
}

fn mill_capacity_per_tick(travel_ticks: u64) -> f64 {
    let worker_capacity = f64::from(MILL_WORKER_COUNT) / (MILL_PROCESS_TICKS + travel_ticks) as f64;
    let processor_capacity = 1.0 / MILL_PROCESS_TICKS as f64;
    worker_capacity.min(processor_capacity)
}

fn farm_travel_ticks(distance_cells: u32) -> u64 {
    u64::from(distance_cells)
        * (FARM_LOADED_TICKS_PER_CELL + FARM_EMPTY_TICKS_PER_CELL)
        * u64::from(WHEAT_FARM_WALKS_PER_CYCLE)
}

#[cfg(test)]
mod tests {
    use super::{
        MILL_PROCESS_TICKS, WHEAT_FARM_WORK_TICKS, bread_output_per_flour, farm_travel_ticks,
        mill_capacity_per_tick, wheat_output_per_farm_cycle,
    };

    #[test]
    fn fear_factor_linearly_changes_bread_and_wheat_output() {
        assert_eq!(wheat_output_per_farm_cycle(0), 24.0);
        assert_eq!(wheat_output_per_farm_cycle(-5), 36.0);
        assert_eq!(wheat_output_per_farm_cycle(-2), 28.8);
        assert_eq!(bread_output_per_flour(0), 8.0);
        assert_eq!(bread_output_per_flour(-5), 12.0);
        assert_eq!(bread_output_per_flour(-2), 9.6);
    }

    #[test]
    fn mill_capacity_respects_single_processor_and_three_workers() {
        assert_eq!(mill_capacity_per_tick(0), 1.0 / MILL_PROCESS_TICKS as f64);
        assert_eq!(mill_capacity_per_tick(1_000), 3.0 / 1_312.0);
    }

    #[test]
    fn wheat_farmer_returns_faster_when_empty() {
        assert_eq!(farm_travel_ticks(1), 12 * (16 + 12));
    }

    #[test]
    fn calibrated_three_farm_layout_produces_about_twenty_two_wheat_per_minute() {
        let wheat_per_minute = [16, 4, 8]
            .into_iter()
            .map(|distance| {
                24.0 / (WHEAT_FARM_WORK_TICKS + farm_travel_ticks(distance)) as f64 * 3_000.0
            })
            .sum::<f64>();

        assert!((wheat_per_minute - 22.0).abs() < 0.05);
    }
}
