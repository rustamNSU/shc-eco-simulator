pub mod buildings;
pub mod map;
mod project;
pub mod simulation;
pub mod walls;

pub use buildings::{
    BuildingComponent, BuildingComponentType, BuildingCost, BuildingPlacement, BuildingType,
    EntryPoint, Footprint, StockpileResource, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick,
};
pub use map::{CellMap, MapError};
pub use project::{PROJECT_FORMAT_VERSION, ProjectFile, SavedBuilding};
pub use simulation::{
    BAKERY_WORK_TICKS, BREAD_BUY_GOLD, BREAD_SELL_GOLD, BreadBuildingRate, BreadEconomyReport,
    BuildingDistance, DistanceKey, FLOUR_BUY_GOLD, FLOUR_SELL_GOLD, GameTime, IRON_BUY_GOLD,
    MILL_PROCESS_TICKS, MILL_WORKER_COUNT, ProductionCycle, ProductionCycleError,
    ProductionRouteUsage, RemoveOutcome, SimulationSettings, Simulator, SimulatorError,
    WHEAT_BUY_GOLD, WHEAT_FARM_WALKS_PER_CYCLE, WHEAT_FARM_WORK_TICKS, WHEAT_SELL_GOLD,
    WOOD_BUY_GOLD, WeaponRecipe, WeaponType, bread_output_per_flour, clamped_fear_factor,
    wheat_output_per_farm_cycle, workshop_fear_output_ring,
};
pub use walls::WallSegment;

pub const DEFAULT_MAP_SIZE: usize = 100;
