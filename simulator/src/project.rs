use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    BuildingPlacement, BuildingType, EntryPoint, Footprint, SimulationSettings, Simulator,
    SimulatorError, StockpileResource, WallSegment,
};

pub const PROJECT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    pub map_size: usize,
    pub buildings: Vec<SavedBuilding>,
    pub walls: Vec<WallSegment>,
    pub simulation: SimulationSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedBuilding {
    pub id: u32,
    pub building_type: BuildingType,
    pub x: usize,
    pub y: usize,
    pub goods_yard_group_id: Option<u32>,
    pub stockpile_resource: Option<StockpileResource>,
    pub entry_point: Option<EntryPoint>,
}

impl ProjectFile {
    pub fn capture(simulator: &Simulator, simulation: SimulationSettings) -> Self {
        Self {
            version: PROJECT_FORMAT_VERSION,
            map_size: simulator.map_size(),
            buildings: simulator
                .buildings()
                .iter()
                .map(SavedBuilding::from)
                .collect(),
            walls: simulator.walls().to_vec(),
            simulation,
        }
    }

    pub fn into_simulator(self) -> Result<(Simulator, SimulationSettings), SimulatorError> {
        if self.version != PROJECT_FORMAT_VERSION {
            return Err(SimulatorError::UnsupportedProjectVersion {
                version: self.version,
            });
        }

        let simulator = Simulator::from_saved_layout(self.map_size, self.buildings, self.walls)?;
        Ok((simulator, self.simulation))
    }
}

impl From<&BuildingPlacement> for SavedBuilding {
    fn from(building: &BuildingPlacement) -> Self {
        Self {
            id: building.id,
            building_type: building.building_type,
            x: building.x,
            y: building.y,
            goods_yard_group_id: building.goods_yard_group_id,
            stockpile_resource: building.stockpile_resource,
            entry_point: building.entry_point,
        }
    }
}

impl SavedBuilding {
    pub(crate) fn into_placement(self) -> BuildingPlacement {
        BuildingPlacement {
            id: self.id,
            building_type: self.building_type,
            x: self.x,
            y: self.y,
            goods_yard_group_id: self.goods_yard_group_id,
            stockpile_resource: self.stockpile_resource,
            entry_point: self.entry_point,
            footprint: Footprint::for_type(self.building_type),
            components: Vec::new(),
        }
    }
}

pub(crate) fn validate_unique_ids(
    buildings: &[SavedBuilding],
    walls: &[WallSegment],
) -> Result<(), SimulatorError> {
    let mut building_ids = HashSet::new();
    if buildings
        .iter()
        .any(|building| !building_ids.insert(building.id))
    {
        return Err(SimulatorError::InvalidProject(
            "building IDs must be unique",
        ));
    }

    let mut wall_ids = HashSet::new();
    if walls.iter().any(|wall| !wall_ids.insert(wall.id)) {
        return Err(SimulatorError::InvalidProject("wall IDs must be unique"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{BuildingType, SimulationSettings, Simulator, StockpileResource};

    use super::{ProjectFile, SavedBuilding};

    #[test]
    fn json_round_trip_preserves_layout_entry_points_and_settings() {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_wall(14, 10, 14, 13)
            .expect("wall should be placed");
        simulator
            .place_building(BuildingType::FletchersWorkshop, 10, 10)
            .expect("workshop should be placed");
        simulator
            .place_building(BuildingType::GoodsYard, 20, 20)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(20, 20, StockpileResource::Wood)
            .expect("stockpile should be marked");

        let settings = SimulationSettings {
            buy_wood: false,
            buy_wheat: true,
            optimized_fletcher_routing: true,
            ..SimulationSettings::default()
        };
        let original = ProjectFile::capture(&simulator, settings);
        let json = serde_json::to_string_pretty(&original).expect("project should serialize");
        let decoded: ProjectFile = serde_json::from_str(&json).expect("project should deserialize");
        let (mut restored, restored_settings) = decoded
            .into_simulator()
            .expect("saved layout should restore");

        assert_eq!(restored.map_size(), simulator.map_size());
        assert_eq!(restored.walls(), simulator.walls());
        assert_eq!(restored.buildings().len(), simulator.buildings().len());
        for (actual, expected) in restored.buildings().iter().zip(simulator.buildings()) {
            assert_eq!(SavedBuilding::from(actual), SavedBuilding::from(expected));
        }
        assert_eq!(restored_settings, settings);
        assert!(!restored.distances().is_empty());
        assert!(!restored.worker_distances().is_empty());

        let previous_max_id = restored
            .buildings()
            .iter()
            .map(|building| building.id)
            .max()
            .expect("restored layout should contain buildings");
        let new_id = restored
            .place_building(BuildingType::Armoury, 30, 30)
            .expect("new placement should work after loading");
        assert!(new_id > previous_max_id);
    }

    #[test]
    fn older_json_defaults_new_resource_buy_options_to_disabled() {
        let simulator = Simulator::new(20).expect("simulator should be created");
        let project = ProjectFile::capture(&simulator, SimulationSettings::default());
        let mut json = serde_json::to_value(project).expect("project should serialize");
        let simulation = json["simulation"]
            .as_object_mut()
            .expect("simulation settings should be an object");
        simulation.remove("buy_wheat");
        simulation.remove("buy_flour");

        let decoded: ProjectFile =
            serde_json::from_value(json).expect("older project should deserialize");
        assert!(!decoded.simulation.buy_wheat);
        assert!(!decoded.simulation.buy_flour);
    }
}
