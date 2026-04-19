pub mod buildings;
pub mod map;
pub mod simulation;
pub mod walls;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingCost, BuildingPlacement, BuildingType,
    EntryPoint, Footprint, StockpileResource, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick,
};
pub use map::{CellMap, MapError};
pub use simulation::{
    BuildingDistance, DistanceKey, GameTime, IRON_BUY_GOLD, ProductionCycle, ProductionCycleError,
    ProductionRouteUsage, RemoveOutcome, SimulationSettings, Simulator, SimulatorError,
    WOOD_BUY_GOLD, WeaponRecipe, WeaponType, clamped_fear_factor, workshop_fear_output_ring,
};
pub use walls::WallSegment;

pub const DEFAULT_MAP_SIZE: usize = 100;
