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
};

#[derive(Debug)]
pub struct Simulator {
    map: CellMap,
    factory: BuildingFactory,
    time: GameTime,
    buildings: Vec<BuildingPlacement>,
    walls: Vec<WallSegment>,
    next_wall_id: u32,
    distances: HashMap<DistanceKey, BuildingDistance>,
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

    pub fn distance_between(
        &self,
        start_building_id: u32,
        finish_building_id: u32,
    ) -> Option<&BuildingDistance> {
        self.distances
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
            self.recalculate_entry_points();
            self.recompute_distances();
            return Ok(first_id);
        }

        let mut placement = self.factory.create(building_type, x, y);
        self.map.place(&placement)?;
        self.assign_entry_points(&mut placement);
        let id = placement.id;
        self.buildings.push(placement);
        self.recalculate_entry_points();
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
        self.recalculate_entry_points();
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

        self.recompute_distances();
        count
    }

    pub fn tick(&mut self, delta_ticks: u64) {
        self.time.advance(delta_ticks);
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

    fn recalculate_entry_points(&mut self) {
        for index in 0..self.buildings.len() {
            let (building_entry, component_entries) = {
                let building = &self.buildings[index];
                let building_entry = calculate_building_entry(
                    &self.map,
                    &self.walls,
                    building.building_type,
                    building.x,
                    building.y,
                    building.width(),
                );
                let component_entries = building
                    .components()
                    .iter()
                    .map(|component| {
                        resolve_entry_point_for_square(
                            &self.map,
                            component.x,
                            component.y,
                            component.size,
                            0,
                        )
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
            self.recompute_distances();
        }

        removed_ids
    }

    fn remove_wall_by_index(&mut self, index: usize) -> u32 {
        let wall = self.walls.remove(index);
        self.map.clear_cells(wall.cells());
        self.recompute_distances();
        wall.id
    }

    fn recompute_distances(&mut self) {
        self.distances = recompute_building_distances(&self.buildings, &self.map);
    }
}

#[cfg(test)]
mod tests {
    use crate::buildings::{BuildingType, EntryPoint, StockpileResource};

    use super::{DistanceKey, RemoveOutcome, Simulator, SimulatorError};

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
}
