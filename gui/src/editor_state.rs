use crate::backend::BackendCommand;
use simulator::{
    BuildingType, DEFAULT_MAP_SIZE, Footprint, Simulator, StockpileResource, walls::line_cells,
};

enum SelectedTool {
    Building(BuildingType),
    Wall,
    Remove,
    SetWoodStock,
    SetIronStock,
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
}

impl EditorState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            simulator: Simulator::new(DEFAULT_MAP_SIZE)?,
            selected: None,
            hover_cell: None,
            wall_start: None,
        })
    }

    pub fn map_size(&self) -> usize {
        self.simulator.map_size()
    }

    pub fn set_simulator(&mut self, simulator: Simulator) {
        self.simulator = simulator;
    }

    pub fn selected_id(&self) -> Option<&'static str> {
        match self.selected {
            Some(SelectedTool::Building(building_type)) => Some(building_type.id()),
            Some(SelectedTool::Wall) => Some("wall"),
            Some(SelectedTool::Remove) => Some("remove"),
            Some(SelectedTool::SetWoodStock) => Some("set_wood_stock"),
            Some(SelectedTool::SetIronStock) => Some("set_iron_stock"),
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

        if x < 0 || y < 0 {
            self.hover_cell = None;
            return;
        }

        self.hover_cell = Some((x, y));
    }

    pub fn place_selected(&mut self, x: f32, y: f32) -> Result<PlacementOutcome, String> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;

        if x < 0 || y < 0 {
            return Err("cell is outside map".to_string());
        }

        let ux = x as usize;
        let uy = y as usize;
        if ux >= self.map_size() || uy >= self.map_size() {
            return Err("cell is outside map".to_string());
        }

        match self.selected {
            Some(SelectedTool::Building(building_type)) => Ok(PlacementOutcome::BackendCommand(
                BackendCommand::PlaceBuilding {
                    building_type,
                    x: ux,
                    y: uy,
                },
            )),
            Some(SelectedTool::Wall) => self.place_wall_click(x, y),
            Some(SelectedTool::Remove) => {
                Ok(PlacementOutcome::BackendCommand(BackendCommand::RemoveAt {
                    x: ux,
                    y: uy,
                }))
            }
            Some(SelectedTool::SetWoodStock) => {
                self.mark_stockpile(ux, uy, StockpileResource::Wood)
            }
            Some(SelectedTool::SetIronStock) => {
                self.mark_stockpile(ux, uy, StockpileResource::Iron)
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
                        start: (sx as usize, sy as usize),
                        end: (x as usize, y as usize),
                    },
                ))
            }
        }
    }

    pub fn simulator(&self) -> &Simulator {
        &self.simulator
    }

    pub fn clear_pending_wall(&mut self) {
        self.wall_start = None;
    }

    fn mark_stockpile(
        &mut self,
        x: usize,
        y: usize,
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
                let map_size = self.simulator.map_size() as i32;
                let footprint = Footprint::for_type(selected);
                let mut cells = Vec::new();

                for (dx, dy) in footprint.occupied_offsets() {
                    let x = anchor_x + dx as i32;
                    let y = anchor_y + dy as i32;
                    if x >= 0 && y >= 0 && x < map_size && y < map_size {
                        cells.push((x, y));
                    }
                }

                cells
            }
            Some(SelectedTool::Wall) => {
                if let Some((sx, sy)) = self.wall_start {
                    if sx == anchor_x || sy == anchor_y {
                        return line_cells(
                            sx as usize,
                            sy as usize,
                            anchor_x as usize,
                            anchor_y as usize,
                        )
                        .into_iter()
                        .map(|(x, y)| (x as i32, y as i32))
                        .collect();
                    }
                    return Vec::new();
                }
                vec![(anchor_x, anchor_y)]
            }
            Some(SelectedTool::Remove)
            | Some(SelectedTool::SetWoodStock)
            | Some(SelectedTool::SetIronStock) => vec![(anchor_x, anchor_y)],
            None => Vec::new(),
        }
    }
}
