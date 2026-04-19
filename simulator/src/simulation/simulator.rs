use std::collections::{HashMap, HashSet};

use crate::{
    buildings::{BuildingFactory, BuildingPlacement, BuildingType, StockpileResource},
    map::{CellMap, MapError},
    walls::{WallSegment, line_cells},
};

use super::{
    BuildingDistance, DistanceKey, GameTime,
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
}

#[derive(Debug)]
pub enum SimulatorError {
    Map(MapError),
    InvalidMapSize,
    InvalidWallDirection,
    StockpileDesignationRequiresStockpile,
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

    map.is_in_bounds(entry_point.x, entry_point.y) && !map.is_occupied(entry_point.x, entry_point.y)
}

impl Simulator {
    pub fn new(map_size: usize) -> Result<Self, SimulatorError> {
        if map_size == 0 {
            return Err(SimulatorError::InvalidMapSize);
        }

        Ok(Self {
            map: CellMap::new(map_size),
            factory: BuildingFactory::new(),
            time: GameTime::new(),
            buildings: Vec::new(),
            walls: Vec::new(),
            next_wall_id: 1,
            distances: HashMap::new(),
            worker_distances: HashMap::new(),
        })
    }

    pub fn map_size(&self) -> usize {
        self.map.size()
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

    pub fn is_cell_occupied(&self, x: usize, y: usize) -> bool {
        self.map.is_occupied(x, y)
    }

    pub fn place_building(
        &mut self,
        building_type: BuildingType,
        x: usize,
        y: usize,
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
        start_x: usize,
        start_y: usize,
        end_x: usize,
        end_y: usize,
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

    pub fn remove_at(&mut self, x: usize, y: usize) -> RemoveOutcome {
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

    pub fn tick(&mut self, delta_ticks: u64) {
        self.time.advance(delta_ticks);
    }

    pub fn calculate_worker_distances(&mut self) -> usize {
        self.worker_distances = build_worker_distances(&self.buildings, &self.distances);
        self.worker_distances.len()
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
        x: usize,
        y: usize,
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
    use crate::buildings::{BuildingType, EntryPoint, StockpileResource};

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
    fn rejects_overlap() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 5, 5)
            .expect("first placement should succeed");

        let second = simulator.place_building(BuildingType::BlacksmithsWorkshop, 6, 6);
        assert!(second.is_err());
    }

    #[test]
    fn rejects_out_of_bounds() {
        let mut simulator = Simulator::new(10).expect("simulator should be created");
        let result = simulator.place_building(BuildingType::ArmourersWorkshop, 8, 8);
        assert!(result.is_err());
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

        let positions: Vec<(usize, usize)> = stockpiles.iter().map(|s| (s.x, s.y)).collect();
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
    fn workshop_wall_contact_right_side_rotates_default_to_left() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_wall(14, 10, 14, 13)
            .expect("wall should be placed");

        simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 10)
            .expect("workshop should be placed");

        let workshop = simulator
            .buildings()
            .iter()
            .find(|b| b.x == 10 && b.y == 10)
            .expect("workshop should exist");

        assert_eq!(workshop.entry_point, Some(EntryPoint { x: 9, y: 12 }));
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
    fn wall_can_make_distance_unreachable() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let start_id = simulator
            .place_building(BuildingType::FletchersWorkshop, 1, 3)
            .expect("start building should be placed");
        let finish_id = simulator
            .place_building(BuildingType::Armoury, 12, 3)
            .expect("finish building should be placed");

        simulator
            .place_wall(8, 0, 8, 19)
            .expect("blocking wall should be placed");

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
        assert_eq!(before, Some(EntryPoint { x: 9, y: 12 }));

        let removed = simulator.remove_all_walls();
        assert_eq!(removed, 1);

        let after = simulator
            .buildings()
            .iter()
            .find(|b| b.id == workshop_id)
            .expect("workshop should exist")
            .entry_point;
        assert_eq!(after, Some(EntryPoint { x: 9, y: 12 }));
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
