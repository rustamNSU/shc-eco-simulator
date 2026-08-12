use crate::backend::{BackendCommand, CycleSimulationRow};
use simulator::{
    BuildingType, DEFAULT_MAP_SIZE, Footprint, PopulationEconomySettings, SimulationSettings,
    Simulator, StockpileResource, WeaponType, clamped_fear_factor, walls::line_cells,
};

enum SelectedTool {
    Building(BuildingType),
    Wall,
    Remove,
    SetWoodStock,
    SetIronStock,
    SetWheatStock,
    SetFlourStock,
}

pub enum PlacementOutcome {
    BackendCommand(BackendCommand),
    Status(String),
}

pub struct EditorState {
    simulator: Simulator,
    selected: Option<SelectedTool>,
    hover_cell: Option<(i32, i32)>,
    wall_start: Option<(i32, i32)>,
    simulation_tooltips_enabled: bool,
    simulation_settings: SimulationSettings,
    population_economy_settings: PopulationEconomySettings,
    cycle_rows: Vec<CycleSimulationRow>,
    simulation_results_stale: bool,
}

impl EditorState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            simulator: Simulator::new(DEFAULT_MAP_SIZE)?,
            selected: None,
            hover_cell: None,
            wall_start: None,
            simulation_tooltips_enabled: true,
            simulation_settings: SimulationSettings::default(),
            population_economy_settings: PopulationEconomySettings::default(),
            cycle_rows: Vec::new(),
            simulation_results_stale: true,
        })
    }

    pub fn map_size(&self) -> usize {
        self.simulator.map_size()
    }

    pub fn map_bounds(&self) -> simulator::MapBounds {
        self.simulator.map_bounds()
    }

    pub fn set_simulator(&mut self, simulator: Simulator) {
        self.simulator = simulator;
    }

    pub fn set_cycle_rows(&mut self, cycle_rows: Vec<CycleSimulationRow>) {
        self.cycle_rows = cycle_rows;
        self.simulation_results_stale = false;
    }

    pub fn mark_simulation_results_stale(&mut self) {
        self.cycle_rows.clear();
        self.simulation_results_stale = true;
    }

    pub fn simulation_results_stale(&self) -> bool {
        self.simulation_results_stale
    }

    pub fn selected_id(&self) -> Option<&'static str> {
        match self.selected {
            Some(SelectedTool::Building(building_type)) => Some(building_type.id()),
            Some(SelectedTool::Wall) => Some("wall"),
            Some(SelectedTool::Remove) => Some("remove"),
            Some(SelectedTool::SetWoodStock) => Some("set_wood_stock"),
            Some(SelectedTool::SetIronStock) => Some("set_iron_stock"),
            Some(SelectedTool::SetWheatStock) => Some("set_wheat_stock"),
            Some(SelectedTool::SetFlourStock) => Some("set_flour_stock"),
            None => None,
        }
    }

    pub fn set_selected_from_id(&mut self, value: &str) -> bool {
        self.wall_start = None;

        if value == "wall" {
            self.selected = Some(SelectedTool::Wall);
            return true;
        }

        if value == "remove" {
            self.selected = Some(SelectedTool::Remove);
            return true;
        }

        if value == "set_wood_stock" {
            self.selected = Some(SelectedTool::SetWoodStock);
            return true;
        }

        if value == "set_iron_stock" {
            self.selected = Some(SelectedTool::SetIronStock);
            return true;
        }

        if value == "set_wheat_stock" {
            self.selected = Some(SelectedTool::SetWheatStock);
            return true;
        }

        if value == "set_flour_stock" {
            self.selected = Some(SelectedTool::SetFlourStock);
            return true;
        }

        if let Some(building) = BuildingType::from_id(value) {
            if building == BuildingType::Stockpile {
                return false;
            }
            self.selected = Some(SelectedTool::Building(building));
            return true;
        }

        false
    }

    pub fn selected_label(&self) -> &'static str {
        match self.selected {
            Some(SelectedTool::Building(building_type)) => building_type.display_name(),
            Some(SelectedTool::Wall) => "Wall",
            Some(SelectedTool::Remove) => "Remove",
            Some(SelectedTool::SetWoodStock) => "Set Wood Stock",
            Some(SelectedTool::SetIronStock) => "Set Iron Stock",
            Some(SelectedTool::SetWheatStock) => "Set Wheat Stock",
            Some(SelectedTool::SetFlourStock) => "Set Flour Stock",
            None => "None",
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.wall_start = None;
    }

    pub fn set_hover_cell(&mut self, x: f32, y: f32) {
        let x = x.floor() as i32;
        let y = y.floor() as i32;

        if !self.simulator.map_bounds().contains(x, y) {
            self.hover_cell = None;
            return;
        }

        self.hover_cell = Some((x, y));
    }

    pub fn place_selected(&mut self, x: f32, y: f32) -> Result<PlacementOutcome, String> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;

        if !self.simulator.map_bounds().contains(x, y) {
            return Err("cell is outside map".to_string());
        }

        match self.selected {
            Some(SelectedTool::Building(building_type)) => Ok(PlacementOutcome::BackendCommand(
                BackendCommand::PlaceBuilding {
                    building_type,
                    x,
                    y,
                },
            )),
            Some(SelectedTool::Wall) => self.place_wall_click(x, y),
            Some(SelectedTool::Remove) => {
                Ok(PlacementOutcome::BackendCommand(BackendCommand::RemoveAt {
                    x,
                    y,
                }))
            }
            Some(SelectedTool::SetWoodStock) => self.mark_stockpile(x, y, StockpileResource::Wood),
            Some(SelectedTool::SetIronStock) => self.mark_stockpile(x, y, StockpileResource::Iron),
            Some(SelectedTool::SetWheatStock) => {
                self.mark_stockpile(x, y, StockpileResource::Wheat)
            }
            Some(SelectedTool::SetFlourStock) => {
                self.mark_stockpile(x, y, StockpileResource::Flour)
            }
            None => Err("no tool selected".to_string()),
        }
    }

    fn place_wall_click(&mut self, x: i32, y: i32) -> Result<PlacementOutcome, String> {
        match self.wall_start {
            None => {
                self.wall_start = Some((x, y));
                Ok(PlacementOutcome::Status(format!(
                    "Wall start set at ({}, {})",
                    x, y
                )))
            }
            Some((sx, sy)) => {
                if sx != x && sy != y {
                    return Err(
                        "wall end cell must be horizontal or vertical from start".to_string()
                    );
                }

                self.wall_start = None;
                Ok(PlacementOutcome::BackendCommand(
                    BackendCommand::PlaceWall {
                        start: (sx, sy),
                        end: (x, y),
                    },
                ))
            }
        }
    }

    pub fn simulator(&self) -> &Simulator {
        &self.simulator
    }

    pub fn simulation_settings(&self) -> SimulationSettings {
        self.simulation_settings
    }

    pub fn set_simulation_settings(&mut self, settings: SimulationSettings) {
        self.simulation_settings = settings;
        self.cycle_rows.clear();
        self.wall_start = None;
    }

    pub fn population_economy_settings(&self) -> PopulationEconomySettings {
        self.population_economy_settings
    }

    pub fn set_population_economy_enabled(&mut self, enabled: bool) -> bool {
        if self.population_economy_settings.enabled == enabled {
            return false;
        }
        self.population_economy_settings.enabled = enabled;
        true
    }

    pub fn set_population_economy_settings(&mut self, settings: PopulationEconomySettings) {
        self.population_economy_settings = settings.normalized();
    }

    pub fn set_max_population(&mut self, value: u32) {
        self.population_economy_settings.max_population = value;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_population(&mut self, value: f32) {
        self.population_economy_settings.population = value.round().max(0.0) as u32;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_inn_count(&mut self, value: u32) {
        self.population_economy_settings.inn_count = value;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_stone_quarry_count(&mut self, value: u32) {
        self.population_economy_settings.stone_quarry_count = value;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_iron_mine_count(&mut self, value: u32) {
        self.population_economy_settings.iron_mine_count = value;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_tax_index(&mut self, value: f32) {
        self.population_economy_settings.tax_index = value.round().max(0.0) as u8;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn set_food_ratio_index(&mut self, value: f32) {
        self.population_economy_settings.food_ratio_index = value.round().max(0.0) as u8;
        self.population_economy_settings = self.population_economy_settings.normalized();
    }

    pub fn game_speed(&self) -> u32 {
        self.simulation_settings.game_speed_ticks_per_second
    }

    pub fn fear_factor(&self) -> i32 {
        self.simulation_settings.fear_factor
    }

    pub fn buy_wood(&self) -> bool {
        self.simulation_settings.buy_wood
    }

    pub fn buy_iron(&self) -> bool {
        self.simulation_settings.buy_iron
    }

    pub fn buy_wheat(&self) -> bool {
        self.simulation_settings.buy_wheat
    }

    pub fn buy_flour(&self) -> bool {
        self.simulation_settings.buy_flour
    }

    pub fn optimized_fletcher_routing(&self) -> bool {
        self.simulation_settings.optimized_fletcher_routing
    }

    pub fn set_game_speed(&mut self, value: f32) -> bool {
        let snapped = ((value / 5.0).round() as i32 * 5).clamp(20, 90) as u32;
        if self.simulation_settings.game_speed_ticks_per_second == snapped {
            return false;
        }

        self.simulation_settings.game_speed_ticks_per_second = snapped;
        true
    }

    pub fn set_fear_factor(&mut self, value: f32) -> bool {
        let snapped = clamped_fear_factor(value.round() as i32);
        if self.simulation_settings.fear_factor == snapped {
            return false;
        }

        self.simulation_settings.fear_factor = snapped;
        true
    }

    pub fn set_buy_wood(&mut self, enabled: bool) -> bool {
        if self.simulation_settings.buy_wood == enabled {
            return false;
        }

        self.simulation_settings.buy_wood = enabled;
        true
    }

    pub fn set_buy_iron(&mut self, enabled: bool) -> bool {
        if self.simulation_settings.buy_iron == enabled {
            return false;
        }

        self.simulation_settings.buy_iron = enabled;
        true
    }

    pub fn set_buy_wheat(&mut self, enabled: bool) -> bool {
        if self.simulation_settings.buy_wheat == enabled {
            return false;
        }

        self.simulation_settings.buy_wheat = enabled;
        true
    }

    pub fn set_buy_flour(&mut self, enabled: bool) -> bool {
        if self.simulation_settings.buy_flour == enabled {
            return false;
        }

        self.simulation_settings.buy_flour = enabled;
        true
    }

    pub fn set_optimized_fletcher_routing(&mut self, enabled: bool) -> bool {
        if self.simulation_settings.optimized_fletcher_routing == enabled {
            return false;
        }

        self.simulation_settings.optimized_fletcher_routing = enabled;
        true
    }

    pub fn cycle_rows(&self) -> &[CycleSimulationRow] {
        &self.cycle_rows
    }

    pub fn simulation_tooltips_enabled(&self) -> bool {
        self.simulation_tooltips_enabled
    }

    pub fn set_simulation_tooltips_enabled(&mut self, enabled: bool) -> bool {
        if self.simulation_tooltips_enabled == enabled {
            return false;
        }

        self.simulation_tooltips_enabled = enabled;
        true
    }

    pub fn hovered_building(&self) -> Option<&simulator::BuildingPlacement> {
        let (hover_x, hover_y) = self.hover_cell?;
        self.simulator.buildings().iter().find(|building| {
            building
                .occupied_cells()
                .any(|cell| cell == (hover_x, hover_y))
        })
    }

    pub fn hover_cell(&self) -> Option<(i32, i32)> {
        self.hover_cell
    }

    pub fn fletchers_weapon(&self) -> WeaponType {
        self.simulation_settings.fletchers_weapon
    }

    pub fn poleturners_weapon(&self) -> WeaponType {
        self.simulation_settings.poleturners_weapon
    }

    pub fn blacksmiths_weapon(&self) -> WeaponType {
        self.simulation_settings.blacksmiths_weapon
    }

    pub fn toggle_fletchers_weapon(&mut self) -> WeaponType {
        self.simulation_settings.fletchers_weapon = match self.simulation_settings.fletchers_weapon
        {
            WeaponType::Bow => WeaponType::Crossbow,
            _ => WeaponType::Bow,
        };
        self.simulation_settings.fletchers_weapon
    }

    pub fn toggle_poleturners_weapon(&mut self) -> WeaponType {
        self.simulation_settings.poleturners_weapon =
            match self.simulation_settings.poleturners_weapon {
                WeaponType::Spear => WeaponType::Pike,
                _ => WeaponType::Spear,
            };
        self.simulation_settings.poleturners_weapon
    }

    pub fn toggle_blacksmiths_weapon(&mut self) -> WeaponType {
        self.simulation_settings.blacksmiths_weapon =
            match self.simulation_settings.blacksmiths_weapon {
                WeaponType::Sword => WeaponType::Mace,
                _ => WeaponType::Sword,
            };
        self.simulation_settings.blacksmiths_weapon
    }

    pub fn clear_pending_wall(&mut self) {
        self.wall_start = None;
    }

    fn mark_stockpile(
        &mut self,
        x: i32,
        y: i32,
        resource: StockpileResource,
    ) -> Result<PlacementOutcome, String> {
        self.wall_start = None;
        Ok(PlacementOutcome::BackendCommand(
            BackendCommand::SetStockpileResource { x, y, resource },
        ))
    }

    pub fn preview_cells(&self) -> Vec<(i32, i32)> {
        let Some((anchor_x, anchor_y)) = self.hover_cell else {
            return Vec::new();
        };

        match self.selected {
            Some(SelectedTool::Building(selected)) => {
                let bounds = self.simulator.map_bounds();
                let footprint = Footprint::for_type(selected);
                let mut cells = Vec::new();

                for (dx, dy) in footprint.occupied_offsets() {
                    let x = anchor_x + dx as i32;
                    let y = anchor_y + dy as i32;
                    if bounds.contains(x, y) {
                        cells.push((x, y));
                    }
                }

                cells
            }
            Some(SelectedTool::Wall) => {
                if let Some((sx, sy)) = self.wall_start {
                    if sx == anchor_x || sy == anchor_y {
                        return line_cells(sx, sy, anchor_x, anchor_y);
                    }
                    return Vec::new();
                }
                vec![(anchor_x, anchor_y)]
            }
            Some(SelectedTool::Remove)
            | Some(SelectedTool::SetWoodStock)
            | Some(SelectedTool::SetIronStock)
            | Some(SelectedTool::SetWheatStock)
            | Some(SelectedTool::SetFlourStock) => vec![(anchor_x, anchor_y)],
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use simulator::{BuildingType, Simulator};

    use crate::backend::BackendCommand;

    use super::{EditorState, PlacementOutcome};

    #[test]
    fn negative_canvas_coordinates_reach_the_backend_unchanged() {
        let mut simulator = Simulator::new(20).expect("simulator should be created");
        simulator
            .place_building(BuildingType::Armoury, 0, 0)
            .expect("edge placement should create negative canvas space");
        let mut state = EditorState::new().expect("editor state should be created");
        state.set_simulator(simulator);
        assert!(state.set_selected_from_id("bakery"));

        state.set_hover_cell(-25.2, -10.1);
        assert_eq!(state.hover_cell(), Some((-26, -11)));
        let outcome = state
            .place_selected(-25.2, -10.1)
            .expect("negative coordinate should be accepted");

        match outcome {
            PlacementOutcome::BackendCommand(BackendCommand::PlaceBuilding {
                building_type,
                x,
                y,
            }) => {
                assert_eq!(building_type, BuildingType::Bakery);
                assert_eq!((x, y), (-26, -11));
            }
            _ => panic!("expected a building placement command"),
        }
    }

    #[test]
    fn geometry_changes_mark_simulation_results_stale_until_rows_are_replaced() {
        let mut state = EditorState::new().expect("editor state should be created");
        assert!(state.simulation_results_stale());

        state.set_cycle_rows(Vec::new());
        assert!(!state.simulation_results_stale());

        state.mark_simulation_results_stale();
        assert!(state.simulation_results_stale());
        assert!(state.cycle_rows().is_empty());
    }
}
