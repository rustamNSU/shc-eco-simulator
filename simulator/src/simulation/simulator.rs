use crate::{
    buildings::{BuildingFactory, BuildingPlacement, BuildingType},
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
        let placement = self.factory.create(building_type, x, y);
        self.map.place(&placement)?;
        let id = placement.id;
        self.buildings.push(placement);
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

        Ok(wall.id)
    }

    pub fn tick(&mut self, delta_ticks: u64) {
        self.time.advance(delta_ticks);
    }
}

#[cfg(test)]
mod tests {
    use crate::buildings::{BuildingComponentType, BuildingType};

    use super::{Simulator, SimulatorError};

    #[test]
    fn places_workshop_when_space_is_free() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        let result = simulator.place_building(BuildingType::FletchersWorkshop, 2, 3);
        assert!(result.is_ok());
        assert_eq!(simulator.buildings().len(), 1);
        assert_eq!(simulator.buildings()[0].entry_point, None);
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
                .all(|component| component.entry_point.is_none())
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
}
