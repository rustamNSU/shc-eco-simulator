use simulator::{BuildingType, Simulator};

use crate::{AnchorLabel, BuildingListItem, OccupiedCellVisual};

pub fn building_color(building_type: BuildingType) -> slint::Color {
    match building_type {
        BuildingType::GoodsYard => slint::Color::from_rgb_u8(179, 120, 78),
        BuildingType::Armoury => slint::Color::from_rgb_u8(103, 134, 171),
        BuildingType::FletchersWorkshop => slint::Color::from_rgb_u8(82, 156, 94),
        BuildingType::BlacksmithsWorkshop => slint::Color::from_rgb_u8(74, 74, 74),
        BuildingType::PoleturnersWorkshop => slint::Color::from_rgb_u8(175, 145, 86),
        BuildingType::ArmourersWorkshop => slint::Color::from_rgb_u8(151, 111, 171),
    }
}

pub fn build_occupied_cells(simulator: &Simulator) -> Vec<OccupiedCellVisual> {
    let mut cells = Vec::new();

    for building in simulator.buildings() {
        let color = building_color(building.building_type);
        for (x, y) in building.occupied_cells() {
            cells.push(OccupiedCellVisual {
                x: x as i32,
                y: y as i32,
                color,
            });
        }
    }

    let wall_color = slint::Color::from_rgb_u8(120, 84, 62);
    for wall in simulator.walls() {
        for (x, y) in wall.cells() {
            cells.push(OccupiedCellVisual {
                x: x as i32,
                y: y as i32,
                color: wall_color,
            });
        }
    }

    cells
}

pub fn build_preview_cells(cells: &[(i32, i32)]) -> Vec<OccupiedCellVisual> {
    let mut result = Vec::with_capacity(cells.len());
    let color = slint::Color::from_argb_u8(150, 130, 130, 130);

    for (x, y) in cells {
        result.push(OccupiedCellVisual {
            x: *x,
            y: *y,
            color,
        });
    }

    result
}

pub fn build_building_list(simulator: &Simulator) -> Vec<BuildingListItem> {
    let mut result = Vec::with_capacity(simulator.buildings().len());

    for building in simulator.buildings() {
        result.push(BuildingListItem {
            id: building.id as i32,
            name: building.building_type.display_name().into(),
            x: building.x as i32,
            y: building.y as i32,
            color: building_color(building.building_type),
        });
    }

    result
}

pub fn build_entry_labels(simulator: &Simulator) -> Vec<AnchorLabel> {
    let mut result = Vec::new();

    for building in simulator.buildings() {
        if let Some(entry) = building.entry_point {
            result.push(AnchorLabel {
                id: building.id as i32,
                x: entry.x as i32,
                y: entry.y as i32,
                color: slint::Color::from_rgb_u8(20, 20, 20),
            });
        }

        for component in building.components() {
            if let Some(entry) = component.entry_point {
                result.push(AnchorLabel {
                    id: building.id as i32,
                    x: entry.x as i32,
                    y: entry.y as i32,
                    color: slint::Color::from_rgb_u8(20, 20, 20),
                });
            }
        }
    }

    result
}
