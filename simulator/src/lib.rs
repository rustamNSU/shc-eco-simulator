pub mod buildings;
pub mod map;
pub mod simulation;
pub mod walls;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingPlacement, BuildingType, EntryPoint,
    Footprint, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick,
};
pub use map::{CellMap, MapError};
pub use simulation::{
    BuildingDistance, DistanceKey, GameTime, RemoveOutcome, Simulator, SimulatorError,
};
pub use walls::WallSegment;

pub const DEFAULT_MAP_SIZE: usize = 100;
