use std::collections::{HashMap, HashSet};

use crate::{
    SavedBuilding,
    buildings::{BuildingFactory, BuildingPlacement, BuildingType, StockpileResource},
    map::{CellMap, MapBounds, MapError},
    project::validate_unique_ids,
    walls::{WallSegment, line_cells},
};

use super::{
    BreadEconomyReport, BuildingDistance, DistanceKey, GameTime,
    bread::calculate_bread_economy,
    entry_logic::{calculate_building_entry, resolve_entry_point_for_square, wall_contains_cell},
    pathfinding::recompute_building_distances,
    production_cycle::{
        ProductionCycle, ProductionCycleError, ProductionRouteUsage, SimulationSettings,
        WeaponType, distance_cells, find_building, find_stockpile_for_resource,
        travel_ticks_for_distance,
    },
    worker_distance::build_worker_distances,
};

#[derive(Debug, Clone)]
pub struct Simulator {
    map: CellMap,
    factory: BuildingFactory,
    time: GameTime,
    buildings: Vec<BuildingPlacement>,
    walls: Vec<WallSegment>,
    next_wall_id: u32,
    distances: HashMap<DistanceKey, BuildingDistance>,
    worker_distances: HashMap<DistanceKey, BuildingDistance>,
    defer_path_calculation: bool,
}

#[derive(Debug)]
pub enum SimulatorError {
    Map(MapError),
    InvalidMapSize,
    InvalidWallDirection,
    StockpileDesignationRequiresStockpile,
    InvalidProject(&'static str),
    UnsupportedProjectVersion { version: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveOutcome {
    None,
    Buildings {
        removed_ids: Vec<u32>,
        goods_yard_group_id: Option<u32>,
    },
    Wall {
        id: u32,
    },
}

impl core::fmt::Display for SimulatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Map(error) => write!(f, "{error}"),
            Self::InvalidMapSize => write!(f, "map size must be greater than zero"),
            Self::InvalidWallDirection => write!(f, "wall must be horizontal or vertical"),
            Self::StockpileDesignationRequiresStockpile => {
                write!(
                    f,
                    "stockpile designation can only be applied to a stockpile"
                )
            }
            Self::InvalidProject(message) => write!(f, "invalid project: {message}"),
            Self::UnsupportedProjectVersion { version } => {
                write!(f, "project format version {version} is not supported")
            }
        }
    }
}

impl std::error::Error for SimulatorError {}

impl From<MapError> for SimulatorError {
    fn from(value: MapError) -> Self {
        Self::Map(value)
    }
}

fn is_entry_point_available(
    map: &CellMap,
    entry_point: Option<crate::buildings::EntryPoint>,
) -> bool {
    let Some(entry_point) = entry_point else {
        return false;
    };

    map.is_in_bounds(entry_point.x, entry_point.y) && !map.is_blocked(entry_point.x, entry_point.y)
}

impl Simulator {
    pub fn new(map_size: usize) -> Result<Self, SimulatorError> {
        let bounds = MapBounds::square(map_size).ok_or(SimulatorError::InvalidMapSize)?;
        Self::with_bounds(bounds)
    }

    pub fn with_bounds(bounds: MapBounds) -> Result<Self, SimulatorError> {
        let map = CellMap::with_bounds(bounds).ok_or(SimulatorError::InvalidMapSize)?;

        Ok(Self {
            map,
            factory: BuildingFactory::new(),
            time: GameTime::new(),
            buildings: Vec::new(),
            walls: Vec::new(),
            next_wall_id: 1,
            distances: HashMap::new(),
            worker_distances: HashMap::new(),
            defer_path_calculation: false,
        })
    }

    pub fn from_saved_layout(
        bounds: MapBounds,
        saved_buildings: Vec<SavedBuilding>,
        walls: Vec<WallSegment>,
    ) -> Result<Self, SimulatorError> {
        if !bounds.is_valid() {
            return Err(SimulatorError::InvalidMapSize);
        }
        validate_unique_ids(&saved_buildings, &walls)?;

        let next_building_id = saved_buildings
            .iter()
            .map(|building| building.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(SimulatorError::InvalidProject("building ID is too large"))?;
        let next_group_id = saved_buildings
            .iter()
            .filter_map(|building| building.goods_yard_group_id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(SimulatorError::InvalidProject(
                "goods yard group ID is too large",
            ))?;
        let next_wall_id = walls
            .iter()
            .map(|wall| wall.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(SimulatorError::InvalidProject("wall ID is too large"))?;

        let mut map = CellMap::with_bounds(bounds).ok_or(SimulatorError::InvalidMapSize)?;
        let buildings = saved_buildings
            .into_iter()
            .map(SavedBuilding::into_placement)
            .collect::<Vec<_>>();
        for building in &buildings {
            if let Some(entry) = building.entry_point
                && !map.is_in_bounds(entry.x, entry.y)
            {
                return Err(SimulatorError::InvalidProject(
                    "building entry point is outside the map",
                ));
            }
            map.place(building)?;
        }
        for wall in &walls {
            if !wall.is_axis_aligned() {
                return Err(SimulatorError::InvalidWallDirection);
            }
            map.place_cells(wall.id, wall.cells())?;
        }

        let mut simulator = Self {
            map,
            factory: BuildingFactory::with_next_ids(next_building_id, next_group_id),
            time: GameTime::new(),
            buildings,
            walls,
            next_wall_id,
            distances: HashMap::new(),
            worker_distances: HashMap::new(),
            defer_path_calculation: false,
        };
        simulator.recompute_distances();
        Ok(simulator)
    }

    pub fn map_size(&self) -> usize {
        self.map.width()
    }

    pub fn map_bounds(&self) -> MapBounds {
        self.map.bounds()
    }

    pub fn map_width(&self) -> usize {
        self.map.width()
    }

    pub fn map_height(&self) -> usize {
        self.map.height()
    }

    pub fn time(&self) -> GameTime {
        self.time
    }

    pub fn buildings(&self) -> &[BuildingPlacement] {
        &self.buildings
    }

    pub fn walls(&self) -> &[WallSegment] {
        &self.walls
    }

    pub fn distances(&self) -> &HashMap<DistanceKey, BuildingDistance> {
        &self.distances
    }

    pub fn worker_distances(&self) -> &HashMap<DistanceKey, BuildingDistance> {
        &self.worker_distances
    }

    pub fn distance_between(
        &self,
        start_building_id: u32,
        finish_building_id: u32,
    ) -> Option<&BuildingDistance> {
        self.distances
            .get(&DistanceKey::new(start_building_id, finish_building_id))
    }

    pub fn worker_distance_between(
        &self,
        start_building_id: u32,
        finish_building_id: u32,
    ) -> Option<&BuildingDistance> {
        self.worker_distances
            .get(&DistanceKey::new(start_building_id, finish_building_id))
    }

    pub fn is_cell_occupied(&self, x: i32, y: i32) -> bool {
        self.map.is_occupied(x, y)
    }

    pub fn place_building(
        &mut self,
        building_type: BuildingType,
        x: i32,
        y: i32,
    ) -> Result<u32, SimulatorError> {
        let footprint = crate::Footprint::for_type(building_type);
        let mut candidate = self.clone();
        candidate.ensure_bounds_for_extent(
            x,
            y,
            x + footprint.width() as i32,
            y + footprint.height() as i32,
        )?;
        let id = candidate.place_building_in_bounds(building_type, x, y)?;
        *self = candidate;
        Ok(id)
    }

    fn place_building_in_bounds(
        &mut self,
        building_type: BuildingType,
        x: i32,
        y: i32,
    ) -> Result<u32, SimulatorError> {
        if building_type == BuildingType::GoodsYard {
            let (_, mut stacks) = self.factory.create_goods_yard_stacks(x, y);
            for stack in &stacks {
                self.map.can_place(stack)?;
            }

            let first_id = stacks[0].id;
            for stack in &mut stacks {
                self.map.place(stack)?;
                self.assign_entry_points(stack);
            }

            self.buildings.extend(stacks);
            self.refresh_unavailable_entry_points();
            self.recompute_distances();
            return Ok(first_id);
        }

        let mut placement = self.factory.create(building_type, x, y);
        self.map.place(&placement)?;
        self.assign_entry_points(&mut placement);
        let id = placement.id;
        self.buildings.push(placement);
        self.refresh_unavailable_entry_points();
        self.recompute_distances();
        Ok(id)
    }

    pub fn place_wall(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) -> Result<u32, SimulatorError> {
        let mut candidate = self.clone();
        candidate.ensure_bounds_for_extent(
            start_x.min(end_x),
            start_y.min(end_y),
            start_x.max(end_x) + 1,
            start_y.max(end_y) + 1,
        )?;
        let id = candidate.place_wall_in_bounds(start_x, start_y, end_x, end_y)?;
        *self = candidate;
        Ok(id)
    }

    fn place_wall_in_bounds(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) -> Result<u32, SimulatorError> {
        let wall = WallSegment::new(self.next_wall_id, start_x, start_y, end_x, end_y);
        if !wall.is_axis_aligned() {
            return Err(SimulatorError::InvalidWallDirection);
        }

        let cells = line_cells(start_x, start_y, end_x, end_y);
        self.map.place_cells(wall.id, cells.iter().copied())?;
        self.walls.push(wall);
        self.next_wall_id += 1;
        self.refresh_unavailable_entry_points();
        self.recompute_distances();

        Ok(wall.id)
    }

    pub fn remove_building(&mut self, building_id: u32) -> bool {
        let Some(goods_yard_group_id) = self
            .buildings
            .iter()
            .find(|building| building.id == building_id)
            .map(|building| building.goods_yard_group_id)
        else {
            return false;
        };

        let removed = if let Some(group_id) = goods_yard_group_id {
            self.remove_buildings_by_group(group_id)
        } else {
            self.remove_buildings_by_ids([building_id])
        };

        !removed.is_empty()
    }

    pub fn remove_at(&mut self, x: i32, y: i32) -> RemoveOutcome {
        if let Some((target_id, goods_yard_group_id)) = self
            .buildings
            .iter()
            .find(|building| building.occupied_cells().any(|cell| cell == (x, y)))
            .map(|building| (building.id, building.goods_yard_group_id))
        {
            let removed_ids = if let Some(group_id) = goods_yard_group_id {
                self.remove_buildings_by_group(group_id)
            } else {
                self.remove_buildings_by_ids([target_id])
            };

            return RemoveOutcome::Buildings {
                removed_ids,
                goods_yard_group_id,
            };
        }

        if let Some(index) = self
            .walls
            .iter()
            .position(|wall| wall_contains_cell(wall, x, y))
        {
            let id = self.remove_wall_by_index(index);
            return RemoveOutcome::Wall { id };
        }

        RemoveOutcome::None
    }

    pub fn remove_all_walls(&mut self) -> usize {
        let count = self.walls.len();
        if count == 0 {
            return 0;
        }

        for wall in self.walls.drain(..) {
            self.map.clear_cells(wall.cells());
        }

        self.refresh_unavailable_entry_points();
        self.recompute_distances();
        count
    }

    pub fn remove_all(&mut self) -> (usize, usize) {
        let building_count = self.buildings.len();
        let wall_count = self.walls.len();

        for building in self.buildings.drain(..) {
            self.map.clear_cells(building.occupied_cells());
        }
        for wall in self.walls.drain(..) {
            self.map.clear_cells(wall.cells());
        }

        self.distances.clear();
        self.worker_distances.clear();

        (building_count, wall_count)
    }

    pub fn tick(&mut self, delta_ticks: u64) {
        self.time.advance(delta_ticks);
    }

    pub fn calculate_worker_distances(&mut self) -> usize {
        self.distances = recompute_building_distances(&self.buildings, &self.map);
        self.recompute_worker_distances();
        self.worker_distances.len()
    }

    pub fn set_defer_path_calculation(&mut self, defer: bool) {
        self.defer_path_calculation = defer;
        if defer {
            self.distances.clear();
            self.worker_distances.clear();
        }
    }

    pub fn calculate_bread_economy(&self, settings: SimulationSettings) -> BreadEconomyReport {
        calculate_bread_economy(&self.buildings, &self.distances, settings)
    }

    pub fn calculate_production_cycle(
        &self,
        weapon_type: WeaponType,
        workshop_id: u32,
        armoury_id: u32,
        settings: SimulationSettings,
    ) -> Result<ProductionCycle, ProductionCycleError> {
        let recipe = weapon_type.recipe();
        let workshop = find_building(&self.buildings, workshop_id)
            .ok_or(ProductionCycleError::WorkshopNotFound { workshop_id })?;
        let armoury = find_building(&self.buildings, armoury_id)
            .ok_or(ProductionCycleError::ArmouryNotFound { armoury_id })?;

        if workshop.building_type != recipe.workshop_type {
            return Err(ProductionCycleError::ExpectedWorkshop {
                workshop_id,
                actual_type: workshop.building_type,
                expected_type: recipe.workshop_type,
            });
        }

        if armoury.building_type != BuildingType::Armoury {
            return Err(ProductionCycleError::ExpectedArmoury {
                armoury_id,
                actual_type: armoury.building_type,
            });
        }

        let mut route_usage = Vec::new();
        self.append_resource_phase_routes(
            &mut route_usage,
            workshop_id,
            armoury_id,
            settings,
            StockpileResource::Wood,
            recipe.wood_required,
            recipe.workshop_type,
        )?;
        self.append_resource_phase_routes(
            &mut route_usage,
            workshop_id,
            armoury_id,
            settings,
            StockpileResource::Iron,
            recipe.iron_required,
            recipe.workshop_type,
        )?;
        self.push_route_usage(
            &mut route_usage,
            recipe.workshop_type,
            workshop_id,
            armoury_id,
            1,
        )?;

        Ok(ProductionCycle::from_route_usage(
            recipe,
            workshop_id,
            armoury_id,
            route_usage,
        ))
    }

    pub fn set_stockpile_resource_at(
        &mut self,
        x: i32,
        y: i32,
        resource: StockpileResource,
    ) -> Result<u32, SimulatorError> {
        let Some(target_id) = self
            .buildings
            .iter()
            .find(|building| {
                building.building_type == BuildingType::Stockpile
                    && building.occupied_cells().any(|cell| cell == (x, y))
            })
            .map(|building| building.id)
        else {
            return Err(SimulatorError::StockpileDesignationRequiresStockpile);
        };

        for building in &mut self.buildings {
            if building.stockpile_resource == Some(resource) {
                building.stockpile_resource = None;
            }
        }

        let target = self
            .buildings
            .iter_mut()
            .find(|building| building.id == target_id)
            .expect("target stockpile should still exist");
        target.stockpile_resource = Some(resource);
        self.recompute_worker_distances();

        Ok(target_id)
    }

    fn assign_entry_points(&self, placement: &mut BuildingPlacement) {
        placement.entry_point = calculate_building_entry(
            &self.map,
            &self.walls,
            placement.building_type,
            placement.x,
            placement.y,
            placement.width(),
        );

        for component in &mut placement.components {
            component.entry_point = resolve_entry_point_for_square(
                &self.map,
                component.x,
                component.y,
                component.size,
                0,
            );
        }
    }

    fn ensure_bounds_for_extent(
        &mut self,
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
    ) -> Result<(), SimulatorError> {
        const EXPANSION: i32 = 50;

        let mut bounds = self.map.bounds();
        while i64::from(min_x) - i64::from(bounds.min_x) < i64::from(EXPANSION) {
            bounds.min_x = bounds
                .min_x
                .checked_sub(EXPANSION)
                .ok_or(SimulatorError::InvalidMapSize)?;
        }
        while i64::from(min_y) - i64::from(bounds.min_y) < i64::from(EXPANSION) {
            bounds.min_y = bounds
                .min_y
                .checked_sub(EXPANSION)
                .ok_or(SimulatorError::InvalidMapSize)?;
        }
        while i64::from(bounds.max_x) - i64::from(max_x) < i64::from(EXPANSION) {
            bounds.max_x = bounds
                .max_x
                .checked_add(EXPANSION)
                .ok_or(SimulatorError::InvalidMapSize)?;
        }
        while i64::from(bounds.max_y) - i64::from(max_y) < i64::from(EXPANSION) {
            bounds.max_y = bounds
                .max_y
                .checked_add(EXPANSION)
                .ok_or(SimulatorError::InvalidMapSize)?;
        }

        if bounds == self.map.bounds() {
            return Ok(());
        }

        let mut expanded_map =
            CellMap::with_bounds(bounds).ok_or(SimulatorError::InvalidMapSize)?;
        for building in &self.buildings {
            expanded_map.place(building)?;
        }
        for wall in &self.walls {
            expanded_map.place_cells(wall.id, wall.cells())?;
        }
        self.map = expanded_map;
        Ok(())
    }

    fn refresh_unavailable_entry_points(&mut self) {
        for index in 0..self.buildings.len() {
            let (building_entry, component_entries) = {
                let building = &self.buildings[index];
                let building_entry = if is_entry_point_available(&self.map, building.entry_point) {
                    building.entry_point
                } else {
                    calculate_building_entry(
                        &self.map,
                        &self.walls,
                        building.building_type,
                        building.x,
                        building.y,
                        building.width(),
                    )
                };
                let component_entries = building
                    .components()
                    .iter()
                    .map(|component| {
                        if is_entry_point_available(&self.map, component.entry_point) {
                            component.entry_point
                        } else {
                            resolve_entry_point_for_square(
                                &self.map,
                                component.x,
                                component.y,
                                component.size,
                                0,
                            )
                        }
                    })
                    .collect::<Vec<_>>();
                (building_entry, component_entries)
            };

            let building = &mut self.buildings[index];
            building.entry_point = building_entry;
            for (component, new_entry) in building
                .components
                .iter_mut()
                .zip(component_entries.into_iter())
            {
                component.entry_point = new_entry;
            }
        }
    }

    fn remove_buildings_by_group(&mut self, group_id: u32) -> Vec<u32> {
        let ids = self
            .buildings
            .iter()
            .filter(|building| building.goods_yard_group_id == Some(group_id))
            .map(|building| building.id)
            .collect::<Vec<_>>();
        self.remove_buildings_by_ids(ids)
    }

    fn remove_buildings_by_ids<I>(&mut self, ids: I) -> Vec<u32>
    where
        I: IntoIterator<Item = u32>,
    {
        let id_set: HashSet<u32> = ids.into_iter().collect();
        if id_set.is_empty() {
            return Vec::new();
        }

        let mut kept = Vec::with_capacity(self.buildings.len());
        let mut removed_ids = Vec::new();

        for building in self.buildings.drain(..) {
            if id_set.contains(&building.id) {
                self.map.clear_cells(building.occupied_cells());
                removed_ids.push(building.id);
            } else {
                kept.push(building);
            }
        }

        self.buildings = kept;

        if !removed_ids.is_empty() {
            self.refresh_unavailable_entry_points();
            self.recompute_distances();
        }

        removed_ids
    }

    fn remove_wall_by_index(&mut self, index: usize) -> u32 {
        let wall = self.walls.remove(index);
        self.map.clear_cells(wall.cells());
        self.refresh_unavailable_entry_points();
        self.recompute_distances();
        wall.id
    }

    fn recompute_distances(&mut self) {
        if self.defer_path_calculation {
            self.distances.clear();
            self.worker_distances.clear();
            return;
        }
        self.distances = recompute_building_distances(&self.buildings, &self.map);
        self.recompute_worker_distances();
    }

    fn recompute_worker_distances(&mut self) {
        self.worker_distances = build_worker_distances(&self.buildings, &self.distances);
    }

    fn append_resource_phase_routes(
        &self,
        route_usage: &mut Vec<ProductionRouteUsage>,
        workshop_id: u32,
        armoury_id: u32,
        settings: SimulationSettings,
        resource: StockpileResource,
        required_units: u32,
        workshop_type: BuildingType,
    ) -> Result<(), ProductionCycleError> {
        if required_units == 0 {
            return Ok(());
        }

        let stockpile = find_stockpile_for_resource(&self.buildings, resource)
            .ok_or(ProductionCycleError::MissingStockpile { resource })?;
        let stockpile_id = stockpile.id;
        let starts_from_workshop = workshop_type == BuildingType::FletchersWorkshop
            && !settings.optimized_fletcher_routing;

        if starts_from_workshop {
            self.push_route_usage(route_usage, workshop_type, armoury_id, workshop_id, 1)?;
            self.push_route_usage(
                route_usage,
                workshop_type,
                workshop_id,
                stockpile_id,
                required_units,
            )?;
        } else {
            self.push_route_usage(route_usage, workshop_type, armoury_id, stockpile_id, 1)?;
            if required_units > 1 {
                self.push_route_usage(
                    route_usage,
                    workshop_type,
                    workshop_id,
                    stockpile_id,
                    required_units - 1,
                )?;
            }
        }

        self.push_route_usage(
            route_usage,
            workshop_type,
            stockpile_id,
            workshop_id,
            required_units,
        )?;

        Ok(())
    }

    fn push_route_usage(
        &self,
        route_usage: &mut Vec<ProductionRouteUsage>,
        workshop_type: BuildingType,
        start_building_id: u32,
        finish_building_id: u32,
        trips: u32,
    ) -> Result<(), ProductionCycleError> {
        if trips == 0 {
            return Ok(());
        }

        let distance_cells = distance_cells(
            &self.worker_distances,
            start_building_id,
            finish_building_id,
        )?;
        let total_distance_cells = distance_cells * trips;
        let total_ticks =
            travel_ticks_for_distance(workshop_type, distance_cells) * u64::from(trips);

        if let Some(existing) = route_usage.iter_mut().find(|usage| {
            usage.start_building_id == start_building_id
                && usage.finish_building_id == finish_building_id
        }) {
            existing.trips += trips;
            existing.total_distance_cells += total_distance_cells;
            existing.total_ticks += total_ticks;
            return Ok(());
        }

        route_usage.push(ProductionRouteUsage {
            start_building_id,
            finish_building_id,
            trips,
            distance_cells_per_trip: distance_cells,
            total_distance_cells,
            total_ticks,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        MapBounds,
        buildings::{BuildingType, EntryPoint, StockpileResource},
    };

    use super::{
        DistanceKey, RemoveOutcome, SimulationSettings, Simulator, SimulatorError, WeaponType,
    };

    #[test]
    fn places_workshop_when_space_is_free() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let result = simulator.place_building(BuildingType::FletchersWorkshop, 2, 3);
        assert!(result.is_ok());
        assert_eq!(simulator.buildings().len(), 1);
        assert_eq!(
            simulator.buildings()[0].entry_point,
            Some(EntryPoint { x: 4, y: 2 })
        );
    }

    #[test]
    fn calculates_complete_bread_economy_with_traversable_wheat_field() {
        let mut simulator = Simulator::new(60).expect("simulator should be created");
        simulator
            .place_building(BuildingType::WheatFarm, 2, 2)
            .expect("wheat farm should be placed");
        simulator
            .place_building(BuildingType::Windmill, 14, 3)
            .expect("wind mill should be placed");
        simulator
            .place_building(BuildingType::GoodsYard, 20, 20)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(20, 20, StockpileResource::Wheat)
            .expect("wheat stockpile should be assigned");
        simulator
            .set_stockpile_resource_at(23, 20, StockpileResource::Flour)
            .expect("flour stockpile should be assigned");
        simulator
            .place_building(BuildingType::Bakery, 30, 10)
            .expect("bakery should be placed");
        simulator
            .place_building(BuildingType::Granary, 40, 10)
            .expect("granary should be placed");

        let farm = simulator
            .buildings()
            .iter()
            .find(|building| building.building_type == BuildingType::WheatFarm)
            .expect("wheat farm should exist");
        assert_eq!(farm.entry_point, Some(EntryPoint { x: 6, y: 5 }));

        let report = simulator.calculate_bread_economy(SimulationSettings::default());
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert_eq!(report.wheat_per_farm_cycle, 24.0);
        assert_eq!(report.bread_per_flour, 8.0);
        assert!(report.wheat_per_minute > 0.0);
        assert!(report.flour_per_minute > 0.0);
        assert!(report.bread_per_minute > 0.0);
        assert_eq!(report.farm_rates.len(), 1);
        assert_eq!(report.mill_rates.len(), 1);
        assert_eq!(report.bakery_rates.len(), 1);
        assert_eq!(
            report.farm_rates[0].actual_per_minute,
            report.wheat_per_minute
        );
        assert_eq!(
            report.mill_rates[0].actual_per_minute,
            report.flour_per_minute
        );
        assert_eq!(
            report.bakery_rates[0].actual_per_minute,
            report.bread_batches_per_minute
        );
        assert!(
            (report.surplus_wheat_per_minute - (report.wheat_per_minute - report.flour_per_minute))
                .abs()
                < f64::EPSILON
        );
        assert!(
            (report.surplus_flour_per_minute
                - (report.flour_per_minute - report.bread_batches_per_minute))
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn mill_entry_can_use_a_traversable_wheat_field_cell() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::WheatFarm, 5, 5)
            .expect("wheat farm should be placed");
        simulator
            .place_building(BuildingType::Windmill, 9, 14)
            .expect("wind mill should be placed");

        let mill = simulator
            .buildings()
            .iter()
            .find(|building| building.building_type == BuildingType::Windmill)
            .expect("wind mill should exist");
        assert_eq!(mill.entry_point, Some(EntryPoint { x: 10, y: 13 }));
    }

    #[test]
    fn bought_wheat_or_flour_can_supply_bakeries_without_farms() {
        let mut simulator = Simulator::new(60).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Windmill, 14, 3)
            .expect("wind mill should be placed");
        simulator
            .place_building(BuildingType::GoodsYard, 20, 20)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(20, 20, StockpileResource::Wheat)
            .expect("wheat stockpile should be assigned");
        simulator
            .set_stockpile_resource_at(23, 20, StockpileResource::Flour)
            .expect("flour stockpile should be assigned");
        simulator
            .place_building(BuildingType::Bakery, 30, 10)
            .expect("bakery should be placed");
        simulator
            .place_building(BuildingType::Granary, 40, 10)
            .expect("granary should be placed");

        let without_buying = simulator.calculate_bread_economy(SimulationSettings::default());
        assert_eq!(without_buying.bread_per_minute, 0.0);

        let bought_wheat = simulator.calculate_bread_economy(SimulationSettings {
            buy_wheat: true,
            ..SimulationSettings::default()
        });
        assert!(bought_wheat.purchased_wheat_per_minute > 0.0);
        assert_eq!(bought_wheat.purchased_flour_per_minute, 0.0);
        assert!(bought_wheat.bread_per_minute > 0.0);

        let bought_flour = simulator.calculate_bread_economy(SimulationSettings {
            buy_flour: true,
            ..SimulationSettings::default()
        });
        assert_eq!(bought_flour.purchased_wheat_per_minute, 0.0);
        assert!(bought_flour.purchased_flour_per_minute > 0.0);
        assert!(bought_flour.bread_per_minute > 0.0);
    }

    #[test]
    fn bakery_uses_workshop_wall_orientation_rules() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_wall(14, 10, 14, 13)
            .expect("wall should be placed");
        simulator
            .place_building(BuildingType::Bakery, 10, 10)
            .expect("bakery should be placed");

        assert_eq!(
            simulator.buildings()[0].entry_point,
            Some(EntryPoint { x: 9, y: 11 })
        );
    }

    #[test]
    fn rejects_overlap() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 5, 5)
            .expect("first placement should succeed");

        let second = simulator.place_building(BuildingType::BlacksmithsWorkshop, 6, 6);
        assert!(second.is_err());
    }

    #[test]
    fn keeps_at_least_fifty_cells_between_a_building_and_every_boundary() {
        let mut simulator = Simulator::new(10).expect("simulator should be created");
        simulator
            .place_building(BuildingType::ArmourersWorkshop, 8, 8)
            .expect("placement should expand the canvas");

        assert_eq!(
            simulator.map_bounds(),
            MapBounds {
                min_x: -50,
                min_y: -50,
                max_x: 110,
                max_y: 110,
            }
        );
        let building = &simulator.buildings()[0];
        let bounds = simulator.map_bounds();
        assert!(building.x - bounds.min_x >= 50);
        assert!(building.y - bounds.min_y >= 50);
        assert!(bounds.max_x - (building.x + building.width() as i32) >= 50);
        assert!(bounds.max_y - (building.y + building.height() as i32) >= 50);
    }

    #[test]
    fn expands_when_a_building_is_close_to_but_not_touching_a_boundary() {
        let mut simulator = Simulator::new(100).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 40, 40)
            .expect("placement should succeed");

        assert_eq!(
            simulator.map_bounds(),
            MapBounds {
                min_x: -50,
                min_y: -50,
                max_x: 100,
                max_y: 100,
            }
        );
    }

    #[test]
    fn expands_left_and_bottom_and_accepts_negative_coordinates() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 0, 0)
            .expect("edge placement should expand the canvas");
        simulator
            .place_building(BuildingType::Bakery, -50, -50)
            .expect("negative placement should expand the canvas again");

        assert_eq!(simulator.map_bounds().min_x, -100);
        assert_eq!(simulator.map_bounds().min_y, -100);
        assert!(simulator.is_cell_occupied(-50, -50));
    }

    #[test]
    fn failed_placement_does_not_commit_canvas_expansion() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let before = simulator.map_bounds();

        let result = simulator.place_wall(-100, -100, 100, 100);

        assert!(result.is_err());
        assert_eq!(simulator.map_bounds(), before);
    }

    #[test]
    fn goods_yard_has_cross_gap() {
        let mut simulator = Simulator::new(12).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 1, 1)
            .expect("goods yard should be placed");

        assert!(!simulator.is_cell_occupied(3, 3));
        assert!(simulator.is_cell_occupied(1, 1));
        assert!(simulator.is_cell_occupied(5, 5));
    }

    #[test]
    fn goods_yard_has_four_internal_stacks() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");

        let stockpiles: Vec<_> = simulator
            .buildings()
            .iter()
            .filter(|entry| entry.building_type == BuildingType::Stockpile)
            .collect();
        assert_eq!(stockpiles.len(), 4);

        let positions: Vec<(i32, i32)> = stockpiles.iter().map(|s| (s.x, s.y)).collect();
        assert!(positions.contains(&(10, 10)));
        assert!(positions.contains(&(13, 10)));
        assert!(positions.contains(&(10, 13)));
        assert!(positions.contains(&(13, 13)));

        let group_id = stockpiles[0]
            .goods_yard_group_id
            .expect("goods yard stockpile should have group id");
        assert!(
            stockpiles
                .iter()
                .all(|stack| stack.goods_yard_group_id == Some(group_id))
        );
    }

    #[test]
    fn places_horizontal_wall() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let wall_id = simulator
            .place_wall(2, 4, 6, 4)
            .expect("wall placement should succeed");
        assert_eq!(wall_id, 1);
        assert_eq!(simulator.walls().len(), 1);
        assert!(simulator.is_cell_occupied(2, 4));
        assert!(simulator.is_cell_occupied(6, 4));
    }

    #[test]
    fn rejects_diagonal_wall() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let result = simulator.place_wall(1, 1, 3, 2);
        assert!(matches!(result, Err(SimulatorError::InvalidWallDirection)));
    }

    #[test]
    fn assigns_default_entry_point_for_square_building() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 6, 6)
            .expect("building should be placed");

        let placed = &simulator.buildings()[0];
        assert_eq!(placed.entry_point, Some(EntryPoint { x: 8, y: 5 }));
    }

    #[test]
    fn rotates_clockwise_when_default_entry_is_blocked() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 8, 2)
            .expect("blocking building should be placed");
        simulator
            .place_building(BuildingType::Armoury, 6, 6)
            .expect("target building should be placed");

        let target = simulator
            .buildings()
            .iter()
            .find(|b| b.x == 6 && b.y == 6)
            .expect("target building should exist");
        assert_eq!(target.entry_point, Some(EntryPoint { x: 7, y: 5 }));
    }

    #[test]
    fn goods_yard_stacks_receive_individual_entry_points() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");
        let stockpiles: Vec<_> = simulator
            .buildings()
            .iter()
            .filter(|entry| entry.building_type == BuildingType::Stockpile)
            .collect();
        assert_eq!(stockpiles.len(), 4);
        assert!(stockpiles.iter().all(|stack| stack.entry_point.is_some()));
    }

    #[test]
    fn recalculates_existing_entry_point_when_new_building_blocks_it() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 6, 6)
            .expect("first building should be placed");
        let initial_entry = simulator.buildings()[0]
            .entry_point
            .expect("entry point should exist");
        assert_eq!(initial_entry, EntryPoint { x: 8, y: 5 });

        simulator
            .place_building(BuildingType::Armoury, 8, 2)
            .expect("second building should be placed");

        let updated_entry = simulator
            .buildings()
            .iter()
            .find(|b| b.x == 6 && b.y == 6)
            .expect("first building should still exist")
            .entry_point
            .expect("entry point should still exist after recalculation");
        assert_eq!(updated_entry, EntryPoint { x: 7, y: 5 });
    }

    #[test]
    fn workshop_entry_rotates_with_wall_orientation() {
        let cases = [
            ((10, 14, 13, 14), EntryPoint { x: 12, y: 9 }),
            ((14, 10, 14, 13), EntryPoint { x: 9, y: 11 }),
            ((10, 9, 13, 9), EntryPoint { x: 11, y: 14 }),
            ((9, 10, 9, 13), EntryPoint { x: 14, y: 12 }),
        ];

        for ((start_x, start_y, end_x, end_y), expected_entry) in cases {
            let mut simulator = Simulator::new(40).expect("simulator should be created");
            simulator
                .place_wall(start_x, start_y, end_x, end_y)
                .expect("wall should be placed");
            simulator
                .place_building(BuildingType::FletchersWorkshop, 10, 10)
                .expect("workshop should be placed");

            let workshop = simulator
                .buildings()
                .iter()
                .find(|b| b.x == 10 && b.y == 10)
                .expect("workshop should exist");

            assert_eq!(workshop.entry_point, Some(expected_entry));
        }
    }

    #[test]
    fn stores_directional_distance_objects_for_both_directions() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        let a = simulator
            .place_building(BuildingType::FletchersWorkshop, 2, 3)
            .expect("building A should be placed");
        let b = simulator
            .place_building(BuildingType::Armoury, 10, 3)
            .expect("building B should be placed");

        assert!(simulator.distances().contains_key(&DistanceKey::new(a, b)));
        assert!(simulator.distances().contains_key(&DistanceKey::new(b, a)));
    }

    #[test]
    fn walls_can_enclose_a_building_and_make_distance_unreachable() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let start_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 5, 5)
            .expect("start building should be placed");
        let finish_id = simulator
            .place_building(BuildingType::Armoury, 12, 5)
            .expect("finish building should be placed");

        simulator
            .place_wall(4, 4, 9, 4)
            .expect("bottom wall should be placed");
        simulator
            .place_wall(4, 9, 9, 9)
            .expect("top wall should be placed");
        simulator
            .place_wall(4, 5, 4, 8)
            .expect("left wall should be placed");
        simulator
            .place_wall(9, 5, 9, 8)
            .expect("right wall should be placed");

        let distance = simulator
            .distance_between(start_id, finish_id)
            .expect("distance object should exist");
        assert_eq!(distance.distance_cells, None);
    }

    #[test]
    fn removing_one_stockpile_removes_whole_goods_yard_group() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");

        let outcome = simulator.remove_at(10, 10);
        match outcome {
            RemoveOutcome::Buildings {
                removed_ids,
                goods_yard_group_id,
            } => {
                assert_eq!(removed_ids.len(), 4);
                assert!(goods_yard_group_id.is_some());
            }
            _ => panic!("expected building removal"),
        }

        assert!(
            simulator
                .buildings()
                .iter()
                .all(|building| building.building_type != BuildingType::Stockpile)
        );
        assert!(!simulator.is_cell_occupied(10, 10));
        assert!(!simulator.is_cell_occupied(13, 10));
        assert!(!simulator.is_cell_occupied(10, 13));
        assert!(!simulator.is_cell_occupied(13, 13));
    }

    #[test]
    fn remove_all_walls_clears_wall_cells() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_wall(2, 4, 6, 4)
            .expect("wall placement should succeed");
        simulator
            .place_wall(8, 2, 8, 5)
            .expect("wall placement should succeed");

        let removed = simulator.remove_all_walls();
        assert_eq!(removed, 2);
        assert_eq!(simulator.walls().len(), 0);
        assert!(!simulator.is_cell_occupied(2, 4));
        assert!(!simulator.is_cell_occupied(8, 5));
    }

    #[test]
    fn remove_all_clears_buildings_walls_and_distances() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 2, 2)
            .expect("armoury should be placed");
        simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 2)
            .expect("workshop should be placed");
        simulator
            .place_wall(2, 10, 6, 10)
            .expect("wall should be placed");

        let removed = simulator.remove_all();

        assert_eq!(removed, (2, 1));
        assert!(simulator.buildings().is_empty());
        assert!(simulator.walls().is_empty());
        assert!(simulator.distances().is_empty());
        assert!(simulator.worker_distances().is_empty());
        assert!(!simulator.is_cell_occupied(2, 2));
        assert!(!simulator.is_cell_occupied(2, 10));
    }

    #[test]
    fn removing_wall_does_not_recalculate_workshop_entry_point() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_wall(14, 10, 14, 13)
            .expect("wall should be placed");
        let workshop_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 10)
            .expect("workshop should be placed");

        let before = simulator
            .buildings()
            .iter()
            .find(|b| b.id == workshop_id)
            .expect("workshop should exist")
            .entry_point;
        assert_eq!(before, Some(EntryPoint { x: 9, y: 11 }));

        let removed = simulator.remove_all_walls();
        assert_eq!(removed, 1);

        let after = simulator
            .buildings()
            .iter()
            .find(|b| b.id == workshop_id)
            .expect("workshop should exist")
            .entry_point;
        assert_eq!(after, Some(EntryPoint { x: 9, y: 11 }));
    }

    #[test]
    fn removing_wall_assigns_entry_only_when_building_had_none() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_wall(5, 5, 10, 5)
            .expect("bottom wall should be placed");
        simulator
            .place_wall(5, 10, 10, 10)
            .expect("top wall should be placed");
        simulator
            .place_wall(5, 6, 5, 9)
            .expect("left wall should be placed");
        simulator
            .place_wall(10, 6, 10, 9)
            .expect("right wall should be placed");

        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 6, 6)
            .expect("armoury should be placed inside blocked perimeter");
        let before = simulator
            .buildings()
            .iter()
            .find(|b| b.id == armoury_id)
            .expect("armoury should exist")
            .entry_point;
        assert_eq!(before, None);

        let outcome = simulator.remove_at(8, 5);
        assert!(matches!(outcome, RemoveOutcome::Wall { .. }));

        let after = simulator
            .buildings()
            .iter()
            .find(|b| b.id == armoury_id)
            .expect("armoury should exist")
            .entry_point;
        assert_eq!(after, Some(EntryPoint { x: 8, y: 5 }));
    }

    #[test]
    fn removing_building_does_not_recalculate_other_entry_points() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        let target_id = simulator
            .place_building(BuildingType::Armoury, 6, 6)
            .expect("target building should be placed");
        let blocker_id = simulator
            .place_building(BuildingType::Armoury, 8, 2)
            .expect("blocking building should be placed");

        let before = simulator
            .buildings()
            .iter()
            .find(|b| b.id == target_id)
            .expect("target should exist")
            .entry_point;
        assert_eq!(before, Some(EntryPoint { x: 7, y: 5 }));

        assert!(simulator.remove_building(blocker_id));

        let after = simulator
            .buildings()
            .iter()
            .find(|b| b.id == target_id)
            .expect("target should exist")
            .entry_point;
        assert_eq!(after, Some(EntryPoint { x: 7, y: 5 }));
    }

    #[test]
    fn stockpile_resource_moves_between_stockpiles() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");

        let first_id = simulator
            .set_stockpile_resource_at(10, 10, StockpileResource::Wood)
            .expect("first stockpile should accept wood");
        let second_id = simulator
            .set_stockpile_resource_at(13, 10, StockpileResource::Wood)
            .expect("second stockpile should accept wood");

        assert_ne!(first_id, second_id);
        assert_eq!(
            simulator
                .buildings()
                .iter()
                .find(|b| b.id == first_id)
                .expect("first stockpile should exist")
                .stockpile_resource,
            None
        );
        assert_eq!(
            simulator
                .buildings()
                .iter()
                .find(|b| b.id == second_id)
                .expect("second stockpile should exist")
                .stockpile_resource,
            Some(StockpileResource::Wood)
        );
    }

    #[test]
    fn stockpile_cannot_hold_wood_and_iron_together() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");

        let stockpile_id = simulator
            .set_stockpile_resource_at(10, 10, StockpileResource::Wood)
            .expect("stockpile should accept wood");
        simulator
            .set_stockpile_resource_at(10, 10, StockpileResource::Iron)
            .expect("stockpile should switch to iron");

        assert_eq!(
            simulator
                .buildings()
                .iter()
                .find(|b| b.id == stockpile_id)
                .expect("stockpile should exist")
                .stockpile_resource,
            Some(StockpileResource::Iron)
        );
    }

    #[test]
    fn stockpile_designation_rejects_non_stockpile_cells() {
        let mut simulator = Simulator::new(30).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 5, 5)
            .expect("armoury should be placed");

        let result = simulator.set_stockpile_resource_at(5, 5, StockpileResource::Wood);
        assert!(matches!(
            result,
            Err(SimulatorError::StockpileDesignationRequiresStockpile)
        ));
    }

    #[test]
    fn calculates_worker_distances_for_wood_workshop_and_armoury_routes() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(2, 2, StockpileResource::Wood)
            .expect("wood stockpile should be marked");
        let workshop_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 2)
            .expect("fletchers workshop should be placed");
        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");
        let wood_stockpile_id = simulator
            .buildings()
            .iter()
            .find(|building| building.stockpile_resource == Some(StockpileResource::Wood))
            .expect("wood stockpile should exist")
            .id;

        let count = simulator.calculate_worker_distances();
        assert_eq!(count, 5);
        assert!(
            simulator
                .worker_distance_between(workshop_id, wood_stockpile_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(wood_stockpile_id, workshop_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(workshop_id, armoury_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(armoury_id, workshop_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(armoury_id, wood_stockpile_id)
                .is_some()
        );
    }

    #[test]
    fn calculates_worker_distances_for_iron_routes() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(2, 2, StockpileResource::Iron)
            .expect("iron stockpile should be marked");
        let workshop_id = simulator
            .place_building(BuildingType::BlacksmithsWorkshop, 10, 2)
            .expect("blacksmiths workshop should be placed");
        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");
        let iron_stockpile_id = simulator
            .buildings()
            .iter()
            .find(|building| building.stockpile_resource == Some(StockpileResource::Iron))
            .expect("iron stockpile should exist")
            .id;

        simulator.calculate_worker_distances();

        assert!(
            simulator
                .worker_distance_between(workshop_id, iron_stockpile_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(iron_stockpile_id, workshop_id)
                .is_some()
        );
        assert!(
            simulator
                .worker_distance_between(armoury_id, iron_stockpile_id)
                .is_some()
        );
    }

    #[test]
    fn deferred_path_calculation_waits_for_explicit_calculate() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator.set_defer_path_calculation(true);
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 2)
            .expect("workshop should be placed");
        simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");

        assert!(simulator.distances().is_empty());
        assert!(simulator.worker_distances().is_empty());

        assert!(simulator.calculate_worker_distances() > 0);
        assert!(!simulator.distances().is_empty());
        assert!(!simulator.worker_distances().is_empty());
    }

    #[test]
    fn deferred_mode_keeps_large_layout_edits_free_of_path_recalculation() {
        let mut simulator = Simulator::new(200).expect("simulator should be created");
        simulator.set_defer_path_calculation(true);

        for index in 0..120 {
            let x = 5 + (index % 20) * 7;
            let y = 5 + (index / 20) * 7;
            simulator
                .place_building(BuildingType::Armoury, x, y)
                .expect("spaced armoury should be placed");
        }

        assert_eq!(simulator.buildings().len(), 120);
        assert!(simulator.distances().is_empty());
        assert!(simulator.worker_distances().is_empty());
    }

    #[test]
    fn bow_cycle_starts_from_armoury_and_returns_to_armoury() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(2, 2, StockpileResource::Wood)
            .expect("wood stockpile should be marked");
        let workshop_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 2)
            .expect("fletchers workshop should be placed");
        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");
        let wood_stockpile_id = simulator
            .buildings()
            .iter()
            .find(|building| building.stockpile_resource == Some(StockpileResource::Wood))
            .expect("wood stockpile should exist")
            .id;

        let cycle = simulator
            .calculate_production_cycle(
                WeaponType::Bow,
                workshop_id,
                armoury_id,
                SimulationSettings::default(),
            )
            .expect("bow cycle should be calculated");

        assert_eq!(cycle.recipe.wood_required, 2);
        assert_eq!(cycle.route_usage.len(), 4);
        assert_eq!(cycle.route_usage[0].start_building_id, armoury_id);
        assert_eq!(cycle.route_usage[0].finish_building_id, workshop_id);
        assert_eq!(cycle.route_usage[0].trips, 1);
        assert_eq!(cycle.route_usage[1].start_building_id, workshop_id);
        assert_eq!(cycle.route_usage[1].finish_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[1].trips, 2);
        assert_eq!(cycle.route_usage[2].start_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[2].finish_building_id, workshop_id);
        assert_eq!(cycle.route_usage[2].trips, 2);
        assert_eq!(cycle.route_usage[3].start_building_id, workshop_id);
        assert_eq!(cycle.route_usage[3].finish_building_id, armoury_id);
        assert_eq!(cycle.route_usage[3].trips, 1);
        assert_eq!(cycle.total_ticks, cycle.travel_ticks + cycle.make_ticks);
    }

    #[test]
    fn optimized_fletcher_cycle_goes_directly_from_armoury_to_stockpile() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(2, 2, StockpileResource::Wood)
            .expect("wood stockpile should be marked");
        let workshop_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 2)
            .expect("fletchers workshop should be placed");
        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");
        let wood_stockpile_id = simulator
            .buildings()
            .iter()
            .find(|building| building.stockpile_resource == Some(StockpileResource::Wood))
            .expect("wood stockpile should exist")
            .id;

        let cycle = simulator
            .calculate_production_cycle(
                WeaponType::Bow,
                workshop_id,
                armoury_id,
                SimulationSettings {
                    optimized_fletcher_routing: true,
                    ..SimulationSettings::default()
                },
            )
            .expect("optimized bow cycle should be calculated");

        assert_eq!(cycle.route_usage.len(), 4);
        assert_eq!(cycle.route_usage[0].start_building_id, armoury_id);
        assert_eq!(cycle.route_usage[0].finish_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[0].trips, 1);
        assert_eq!(cycle.route_usage[1].start_building_id, workshop_id);
        assert_eq!(cycle.route_usage[1].finish_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[1].trips, 1);
        assert_eq!(cycle.route_usage[2].start_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[2].finish_building_id, workshop_id);
        assert_eq!(cycle.route_usage[2].trips, 2);
        assert_eq!(cycle.route_usage[3].start_building_id, workshop_id);
        assert_eq!(cycle.route_usage[3].finish_building_id, armoury_id);
        assert_eq!(cycle.route_usage[3].trips, 1);
    }

    #[test]
    fn spear_cycle_uses_armoury_to_stockpile_to_workshop_pattern() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 2, 2)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(2, 2, StockpileResource::Wood)
            .expect("wood stockpile should be marked");
        let workshop_id = simulator
            .place_building(BuildingType::PoleturnersWorkshop, 10, 2)
            .expect("poleturners workshop should be placed");
        let armoury_id = simulator
            .place_building(BuildingType::Armoury, 18, 2)
            .expect("armoury should be placed");
        let wood_stockpile_id = simulator
            .buildings()
            .iter()
            .find(|building| building.stockpile_resource == Some(StockpileResource::Wood))
            .expect("wood stockpile should exist")
            .id;

        let cycle = simulator
            .calculate_production_cycle(
                WeaponType::Spear,
                workshop_id,
                armoury_id,
                SimulationSettings::default(),
            )
            .expect("spear cycle should be calculated");

        assert_eq!(cycle.route_usage.len(), 3);
        assert_eq!(cycle.route_usage[0].start_building_id, armoury_id);
        assert_eq!(cycle.route_usage[0].finish_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[0].trips, 1);
        assert_eq!(cycle.route_usage[1].start_building_id, wood_stockpile_id);
        assert_eq!(cycle.route_usage[1].finish_building_id, workshop_id);
        assert_eq!(cycle.route_usage[1].trips, 1);
        assert_eq!(cycle.route_usage[2].start_building_id, workshop_id);
        assert_eq!(cycle.route_usage[2].finish_building_id, armoury_id);
        assert_eq!(cycle.route_usage[2].trips, 1);
    }
}
