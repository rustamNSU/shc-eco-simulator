use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    buildings::{BuildingFactory, BuildingPlacement, BuildingType, EntryPoint},
    map::{CellMap, MapError},
    walls::{WallSegment, line_cells},
};

use super::{BuildingDistance, DistanceKey, GameTime};

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

    fn assign_entry_points(&self, placement: &mut BuildingPlacement) {
        placement.entry_point = self.calculate_building_entry(
            placement.building_type,
            placement.x,
            placement.y,
            placement.width(),
        );

        for component in &mut placement.components {
            component.entry_point =
                self.resolve_entry_point_for_square(component.x, component.y, component.size, 0);
        }
    }

    fn recalculate_entry_points(&mut self) {
        for index in 0..self.buildings.len() {
            let (building_entry, component_entries) = {
                let building = &self.buildings[index];
                let building_entry = self.calculate_building_entry(
                    building.building_type,
                    building.x,
                    building.y,
                    building.width(),
                );
                let component_entries = building
                    .components()
                    .iter()
                    .map(|component| {
                        self.resolve_entry_point_for_square(
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

    fn calculate_building_entry(
        &self,
        building_type: BuildingType,
        x: usize,
        y: usize,
        size: usize,
    ) -> Option<EntryPoint> {
        if building_type == BuildingType::GoodsYard {
            return None;
        }

        let rotation_steps = if is_workshop(building_type) {
            self.workshop_wall_rotation_steps(x, y, size)
        } else {
            0
        };

        self.resolve_entry_point_for_square(x, y, size, rotation_steps)
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
        let mut map = HashMap::new();

        for start in &self.buildings {
            for finish in &self.buildings {
                if start.id == finish.id {
                    continue;
                }

                let key = DistanceKey::new(start.id, finish.id);
                let distance_cells = match (start.entry_point, finish.entry_point) {
                    (Some(start_entry), Some(finish_entry)) => {
                        self.shortest_path_len(start_entry, finish_entry)
                    }
                    _ => None,
                };

                map.insert(
                    key,
                    BuildingDistance {
                        key,
                        start_entry: start.entry_point,
                        finish_entry: finish.entry_point,
                        distance_cells,
                    },
                );
            }
        }

        self.distances = map;
    }

    fn shortest_path_len(&self, start: EntryPoint, finish: EntryPoint) -> Option<u32> {
        if !self.map.is_in_bounds(start.x, start.y) || !self.map.is_in_bounds(finish.x, finish.y) {
            return None;
        }

        if start == finish {
            return Some(0);
        }

        if self.map.is_occupied(start.x, start.y) || self.map.is_occupied(finish.x, finish.y) {
            return None;
        }

        let size = self.map.size();
        let mut dist = vec![u32::MAX; size * size];
        let mut queue = VecDeque::new();

        let start_idx = start.y * size + start.x;
        dist[start_idx] = 0;
        queue.push_back((start.x, start.y));

        while let Some((x, y)) = queue.pop_front() {
            let current_dist = dist[y * size + x];

            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    let ux = nx as usize;
                    let uy = ny as usize;
                    if !self.map.is_in_bounds(ux, uy) {
                        continue;
                    }
                    if self.map.is_occupied(ux, uy) {
                        continue;
                    }

                    let next_idx = uy * size + ux;
                    if dist[next_idx] != u32::MAX {
                        continue;
                    }

                    let next_dist = current_dist + 1;
                    if ux == finish.x && uy == finish.y {
                        return Some(next_dist);
                    }

                    dist[next_idx] = next_dist;
                    queue.push_back((ux, uy));
                }
            }
        }

        None
    }

    fn resolve_entry_point_for_square(
        &self,
        x: usize,
        y: usize,
        size: usize,
        rotation_steps: usize,
    ) -> Option<EntryPoint> {
        let default_cell = default_entry_cell_rotated(x, y, size, rotation_steps);
        let side_cells = side_perimeter_cells_clockwise(x, y, size, default_cell);
        if let Some(point) = self.first_available(side_cells) {
            return Some(point);
        }

        let corner_cells = corner_cells_clockwise_from_bottom_right(x, y, size, rotation_steps);
        self.first_available(corner_cells)
    }

    fn first_available(&self, cells: Vec<(i32, i32)>) -> Option<EntryPoint> {
        for (cx, cy) in cells {
            if cx < 0 || cy < 0 {
                continue;
            }
            let ux = cx as usize;
            let uy = cy as usize;
            if self.map.is_in_bounds(ux, uy) && !self.map.is_occupied(ux, uy) {
                return Some(EntryPoint { x: ux, y: uy });
            }
        }
        None
    }

    fn workshop_wall_rotation_steps(&self, x: usize, y: usize, size: usize) -> usize {
        let xi = x as i32;
        let yi = y as i32;
        let ni = size as i32;

        if self.side_has_wall_contact((xi..(xi + ni)).map(|cx| (cx, yi + ni)).collect()) {
            return 0;
        }
        if self.side_has_wall_contact((yi..(yi + ni)).map(|cy| (xi + ni, cy)).collect()) {
            return 1;
        }
        if self.side_has_wall_contact((xi..(xi + ni)).map(|cx| (cx, yi - 1)).collect()) {
            return 2;
        }
        if self.side_has_wall_contact((yi..(yi + ni)).map(|cy| (xi - 1, cy)).collect()) {
            return 3;
        }

        0
    }

    fn side_has_wall_contact(&self, cells: Vec<(i32, i32)>) -> bool {
        cells.into_iter().any(|(x, y)| self.is_wall_at(x, y))
    }

    fn is_wall_at(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let ux = x as usize;
        let uy = y as usize;
        self.walls
            .iter()
            .any(|wall| wall_contains_cell(wall, ux, uy))
    }
}

fn default_entry_cell_rotated(
    x: usize,
    y: usize,
    size: usize,
    rotation_steps: usize,
) -> Option<(i32, i32)> {
    if y == 0 || size == 0 {
        return None;
    }

    let offset = if size == 2 { 0 } else { (size / 2) as i32 };
    let xi = x as i32;
    let yi = y as i32;
    let ni = size as i32;

    match rotation_steps % 4 {
        0 => Some((xi + offset, yi - 1)),
        1 => Some((xi - 1, yi + offset)),
        2 => Some((xi + offset, yi + ni)),
        _ => Some((xi + ni, yi + offset)),
    }
}

fn side_perimeter_cells_clockwise(
    x: usize,
    y: usize,
    size: usize,
    start: Option<(i32, i32)>,
) -> Vec<(i32, i32)> {
    if size == 0 {
        return Vec::new();
    }

    let xi = x as i32;
    let yi = y as i32;
    let ni = size as i32;
    let top = yi + ni;
    let right = xi + ni;

    let mut ring = Vec::with_capacity(size * 4);

    for cx in (xi..(xi + ni)).rev() {
        ring.push((cx, yi - 1));
    }
    for cy in yi..(yi + ni) {
        ring.push((xi - 1, cy));
    }
    for cx in xi..(xi + ni) {
        ring.push((cx, top));
    }
    for cy in (yi..(yi + ni)).rev() {
        ring.push((right, cy));
    }

    if let Some(start_cell) = start {
        if let Some(idx) = ring.iter().position(|cell| *cell == start_cell) {
            ring.rotate_left(idx);
        }
    }

    ring
}

fn corner_cells_clockwise_from_bottom_right(
    x: usize,
    y: usize,
    size: usize,
    rotation_steps: usize,
) -> Vec<(i32, i32)> {
    let xi = x as i32;
    let yi = y as i32;
    let ni = size as i32;

    let mut corners = vec![
        (xi + ni, yi - 1),
        (xi - 1, yi - 1),
        (xi - 1, yi + ni),
        (xi + ni, yi + ni),
    ];

    corners.rotate_left(rotation_steps % 4);
    corners
}

fn wall_contains_cell(wall: &WallSegment, x: usize, y: usize) -> bool {
    if wall.start_x == wall.end_x {
        if x != wall.start_x {
            return false;
        }
        let min_y = wall.start_y.min(wall.end_y);
        let max_y = wall.start_y.max(wall.end_y);
        return y >= min_y && y <= max_y;
    }

    if wall.start_y == wall.end_y {
        if y != wall.start_y {
            return false;
        }
        let min_x = wall.start_x.min(wall.end_x);
        let max_x = wall.start_x.max(wall.end_x);
        return x >= min_x && x <= max_x;
    }

    false
}

fn is_workshop(building_type: BuildingType) -> bool {
    matches!(
        building_type,
        BuildingType::FletchersWorkshop
            | BuildingType::BlacksmithsWorkshop
            | BuildingType::PoleturnersWorkshop
            | BuildingType::ArmourersWorkshop
    )
}

#[cfg(test)]
mod tests {
    use crate::buildings::{BuildingType, EntryPoint};

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
    fn same_start_and_finish_cell_has_zero_distance() {
        let simulator = Simulator::new(10).expect("simulator should be created");
        let point = EntryPoint { x: 1, y: 1 };
        assert_eq!(simulator.shortest_path_len(point, point), Some(0));
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
}
