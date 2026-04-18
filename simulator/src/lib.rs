pub mod buildings;
pub mod map;
pub mod simulation;
pub mod walls;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingPlacement, BuildingType, EntryPoint,
    Footprint, StockpileResource, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick,
};
pub use map::{CellMap, MapError};
pub use simulation::{
    BuildingDistance, DistanceKey, GameTime, ProductionCycle, ProductionCycleError,
    ProductionRouteUsage, RemoveOutcome, SimulationSettings, Simulator, SimulatorError,
    WeaponRecipe, WeaponType,
};
pub use walls::WallSegment;

pub const DEFAULT_MAP_SIZE: usize = 100;
