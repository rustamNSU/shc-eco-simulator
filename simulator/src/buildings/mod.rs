mod components;
mod factory;
mod footprint;
mod placement;
mod types;

pub use components::{BuildingComponent, BuildingComponentType};
pub use factory::BuildingFactory;
pub use footprint::Footprint;
pub use placement::BuildingPlacement;
pub use types::BuildingType;
