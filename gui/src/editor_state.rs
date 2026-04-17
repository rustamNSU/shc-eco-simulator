use simulator::{
    BuildingType, DEFAULT_MAP_SIZE, Footprint, RemoveOutcome, Simulator, walls::line_cells,
};

enum SelectedTool {
    Building(BuildingType),
    Wall,
    Remove,
}

pub enum PlacementOutcome {
    Building {
        id: u32,
        name: &'static str,
    },
    WallStart {
        x: i32,
        y: i32,
    },
    WallPlaced {
        id: u32,
        start: (i32, i32),
        end: (i32, i32),
    },
    RemovedBuildings {
        removed_ids: Vec<u32>,
        goods_yard_group_id: Option<u32>,
    },
    RemovedWall {
        id: u32,
    },
    NothingToRemove,
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

    pub fn selected_id(&self) -> Option<&'static str> {
        match self.selected {
            Some(SelectedTool::Building(building_type)) => Some(building_type.id()),
            Some(SelectedTool::Wall) => Some("wall"),
            Some(SelectedTool::Remove) => Some("remove"),
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
            Some(SelectedTool::Building(building_type)) => {
                let id = self
                    .simulator
                    .place_building(building_type, ux, uy)
                    .map_err(|error| error.to_string())?;
                Ok(PlacementOutcome::Building {
                    id,
                    name: building_type.display_name(),
                })
            }
            Some(SelectedTool::Wall) => self.place_wall_click(x, y),
            Some(SelectedTool::Remove) => Ok(self.remove_at(ux, uy)),
            None => Err("no tool selected".to_string()),
        }
    }

    pub fn remove_all_walls(&mut self) -> usize {
        self.wall_start = None;
        self.simulator.remove_all_walls()
    }

    fn place_wall_click(&mut self, x: i32, y: i32) -> Result<PlacementOutcome, String> {
        match self.wall_start {
            None => {
                self.wall_start = Some((x, y));
                Ok(PlacementOutcome::WallStart { x, y })
            }
            Some((sx, sy)) => {
                if sx != x && sy != y {
                    return Err(
                        "wall end cell must be horizontal or vertical from start".to_string()
                    );
                }

                let wall_id = self
                    .simulator
                    .place_wall(sx as usize, sy as usize, x as usize, y as usize)
                    .map_err(|error| error.to_string())?;
                self.wall_start = None;
                Ok(PlacementOutcome::WallPlaced {
                    id: wall_id,
                    start: (sx, sy),
                    end: (x, y),
                })
            }
        }
    }

    pub fn simulator(&self) -> &Simulator {
        &self.simulator
    }

    fn remove_at(&mut self, x: usize, y: usize) -> PlacementOutcome {
        self.wall_start = None;
        match self.simulator.remove_at(x, y) {
            RemoveOutcome::None => PlacementOutcome::NothingToRemove,
            RemoveOutcome::Wall { id } => PlacementOutcome::RemovedWall { id },
            RemoveOutcome::Buildings {
                removed_ids,
                goods_yard_group_id,
            } => PlacementOutcome::RemovedBuildings {
                removed_ids,
                goods_yard_group_id,
            },
        }
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
            Some(SelectedTool::Remove) => vec![(anchor_x, anchor_y)],
            None => Vec::new(),
        }
    }
}
