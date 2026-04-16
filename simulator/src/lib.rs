pub mod buildings;
pub mod map;
pub mod simulation;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingPlacement, BuildingType, Footprint,
};
pub use map::{CellMap, MapError};
pub use simulation::{GameTime, Simulator, SimulatorError};

pub const DEFAULT_MAP_SIZE: usize = 100;
