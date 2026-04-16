pub mod buildings;
pub mod map;
pub mod simulation;
pub mod walls;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingPlacement, BuildingType, EntryPoint,
    Footprint,
};
pub use map::{CellMap, MapError};
pub use simulation::{GameTime, Simulator, SimulatorError};
pub use walls::WallSegment;

pub const DEFAULT_MAP_SIZE: usize = 100;
