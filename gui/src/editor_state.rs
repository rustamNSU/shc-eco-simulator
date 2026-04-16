use simulator::{BuildingType, DEFAULT_MAP_SIZE, Footprint, Simulator};

pub struct EditorState {
    simulator: Simulator,
    selected: Option<BuildingType>,
    hover_cell: Option<(i32, i32)>,
}

impl EditorState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            simulator: Simulator::new(DEFAULT_MAP_SIZE)?,
            selected: None,
            hover_cell: None,
        })
    }

    pub fn map_size(&self) -> usize {
        self.simulator.map_size()
    }

    pub fn selected(&self) -> Option<BuildingType> {
        self.selected
    }

    pub fn set_selected_from_id(&mut self, value: &str) -> bool {
        if let Some(building) = BuildingType::from_id(value) {
            self.selected = Some(building);
            return true;
        }
        false
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
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

    pub fn place_selected(&mut self, x: f32, y: f32) -> Result<u32, String> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;

        if x < 0 || y < 0 {
            return Err("cell is outside map".to_string());
        }

        let ux = x as usize;
        let uy = y as usize;

        let Some(selected) = self.selected else {
            return Err("no building selected".to_string());
        };

        self.simulator
            .place_building(selected, ux, uy)
            .map_err(|error| error.to_string())
    }

    pub fn simulator(&self) -> &Simulator {
        &self.simulator
    }

    pub fn preview_cells(&self) -> Vec<(i32, i32)> {
        let Some((anchor_x, anchor_y)) = self.hover_cell else {
            return Vec::new();
        };
        let Some(selected) = self.selected else {
            return Vec::new();
        };

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
}
