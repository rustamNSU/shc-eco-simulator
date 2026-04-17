use crate::{
    buildings::{BuildingFactory, BuildingPlacement, BuildingType, EntryPoint},
    map::{CellMap, MapError},
    walls::{WallSegment, line_cells},
};

use super::GameTime;

#[derive(Debug)]
pub struct Simulator {
    map: CellMap,
    factory: BuildingFactory,
    time: GameTime,
    buildings: Vec<BuildingPlacement>,
    walls: Vec<WallSegment>,
    next_wall_id: u32,
}

#[derive(Debug)]
pub enum SimulatorError {
    Map(MapError),
    InvalidMapSize,
    InvalidWallDirection,
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

    pub fn is_cell_occupied(&self, x: usize, y: usize) -> bool {
        self.map.is_occupied(x, y)
    }

    pub fn place_building(
        &mut self,
        building_type: BuildingType,
        x: usize,
        y: usize,
    ) -> Result<u32, SimulatorError> {
        let mut placement = self.factory.create(building_type, x, y);
        self.map.place(&placement)?;
        self.assign_entry_points(&mut placement);
        let id = placement.id;
        self.buildings.push(placement);
        self.recalculate_blocked_entry_points();
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
        self.recalculate_blocked_entry_points();

        Ok(wall.id)
    }

    pub fn tick(&mut self, delta_ticks: u64) {
        self.time.advance(delta_ticks);
    }

    fn assign_entry_points(&self, placement: &mut BuildingPlacement) {
        let rotation_steps = if is_workshop(placement.building_type) {
            self.workshop_wall_rotation_steps(placement.x, placement.y, placement.width())
        } else {
            0
        };

        if placement.building_type != BuildingType::GoodsYard {
            placement.entry_point = self.resolve_entry_point_for_square(
                placement.x,
                placement.y,
                placement.width(),
                rotation_steps,
            );
        } else {
            placement.entry_point = None;
        }

        for component in &mut placement.components {
            component.entry_point =
                self.resolve_entry_point_for_square(component.x, component.y, component.size, 0);
        }
    }

    fn recalculate_blocked_entry_points(&mut self) {
        for index in 0..self.buildings.len() {
            let (new_building_entry, new_component_entries) = {
                let building = &self.buildings[index];

                let building_entry = if building.building_type != BuildingType::GoodsYard
                    && building
                        .entry_point
                        .is_some_and(|entry| self.map.is_occupied(entry.x, entry.y))
                {
                    self.resolve_entry_point_for_square(
                        building.x,
                        building.y,
                        building.width(),
                        if is_workshop(building.building_type) {
                            self.workshop_wall_rotation_steps(
                                building.x,
                                building.y,
                                building.width(),
                            )
                        } else {
                            0
                        },
                    )
                } else {
                    building.entry_point
                };

                let component_entries: Vec<Option<EntryPoint>> = building
                    .components()
                    .iter()
                    .map(|component| {
                        if component
                            .entry_point
                            .is_some_and(|entry| self.map.is_occupied(entry.x, entry.y))
                        {
                            self.resolve_entry_point_for_square(
                                component.x,
                                component.y,
                                component.size,
                                0,
                            )
                        } else {
                            component.entry_point
                        }
                    })
                    .collect();

                (building_entry, component_entries)
            };

            let building = &mut self.buildings[index];
            building.entry_point = new_building_entry;
            for (component, new_entry) in building
                .components
                .iter_mut()
                .zip(new_component_entries.into_iter())
            {
                component.entry_point = new_entry;
            }
        }
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
    use crate::buildings::{BuildingComponentType, BuildingType, EntryPoint};

    use super::{Simulator, SimulatorError};

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

        let goods_yard = simulator
            .buildings()
            .iter()
            .find(|entry| entry.building_type == BuildingType::GoodsYard)
            .expect("goods yard placement should exist");

        assert_eq!(goods_yard.components().len(), 4);
        assert_eq!(
            goods_yard
                .components()
                .iter()
                .filter(|c| c.component_type == BuildingComponentType::GoodsYardStack)
                .count(),
            4
        );

        let positions: Vec<(usize, usize)> =
            goods_yard.components().iter().map(|c| (c.x, c.y)).collect();
        assert!(positions.contains(&(10, 10)));
        assert!(positions.contains(&(13, 10)));
        assert!(positions.contains(&(10, 13)));
        assert!(positions.contains(&(13, 13)));
        assert_eq!(goods_yard.entry_point, None);
        assert!(
            goods_yard
                .components()
                .iter()
                .all(|component| component.entry_point.is_some())
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
        let goods_yard = simulator
            .buildings()
            .iter()
            .find(|entry| entry.building_type == BuildingType::GoodsYard)
            .expect("goods yard placement should exist");

        assert_eq!(goods_yard.entry_point, None);
        assert!(
            goods_yard
                .components()
                .iter()
                .all(|component| component.entry_point.is_some())
        );
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
}
