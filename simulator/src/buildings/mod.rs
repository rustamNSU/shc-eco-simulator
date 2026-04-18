mod components;
mod entry_point;
mod factory;
mod footprint;
mod placement;
mod stockpile_resource;
mod types;

pub use components::{BuildingComponent, BuildingComponentType};
pub use entry_point::EntryPoint;
pub use factory::BuildingFactory;
pub use footprint::Footprint;
pub use placement::BuildingPlacement;
pub use stockpile_resource::StockpileResource;
pub use types::{BuildingType, WORKSHOP_SLOWDOWN_BASE, unit_speed_cells_per_tick};
