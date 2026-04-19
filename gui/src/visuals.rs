use std::collections::{BTreeMap, BTreeSet};

use simulator::{BuildingType, Simulator, StockpileResource};

use crate::{BuildingBoundary, BuildingListItem, MapMarker, OccupiedCellVisual};

pub fn building_color(building_type: BuildingType) -> slint::Color {
    match building_type {
        BuildingType::GoodsYard => slint::Color::from_rgb_u8(179, 120, 78),
        BuildingType::Stockpile => slint::Color::from_rgb_u8(179, 120, 78),
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

pub fn build_building_boundaries(simulator: &Simulator) -> Vec<BuildingBoundary> {
    let mut goods_yard_origins = BTreeMap::new();
    let mut edges = BTreeSet::new();

    for building in simulator.buildings() {
        if let Some(group_id) = building.goods_yard_group_id {
            let entry = goods_yard_origins
                .entry(group_id)
                .or_insert((building.x, building.y));
            entry.0 = entry.0.min(building.x);
            entry.1 = entry.1.min(building.y);
            continue;
        }

        append_boundary_edges(
            &mut edges,
            building.x,
            building.y,
            building.width(),
            building.height(),
        );
    }

    for (_, (x, y)) in goods_yard_origins {
        append_boundary_edges(&mut edges, x, y, 5, 5);
    }

    edges
        .into_iter()
        .map(|edge| BuildingBoundary {
            x: edge.x as i32,
            y: edge.y as i32,
            horizontal: edge.horizontal,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BoundaryEdge {
    x: usize,
    y: usize,
    horizontal: bool,
}

fn append_boundary_edges(
    edges: &mut BTreeSet<BoundaryEdge>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    if width == 0 || height == 0 {
        return;
    }

    for dx in 0..width {
        edges.insert(BoundaryEdge {
            x: x + dx,
            y,
            horizontal: true,
        });
        edges.insert(BoundaryEdge {
            x: x + dx,
            y: y + height,
            horizontal: true,
        });
    }

    for dy in 0..height {
        edges.insert(BoundaryEdge {
            x,
            y: y + dy,
            horizontal: false,
        });
        edges.insert(BoundaryEdge {
            x: x + width,
            y: y + dy,
            horizontal: false,
        });
    }
}

pub fn build_building_list(simulator: &Simulator) -> Vec<BuildingListItem> {
    let mut result = Vec::with_capacity(simulator.buildings().len());

    for building in simulator.buildings() {
        result.push(BuildingListItem {
            id: building.id as i32,
            name: building_list_name(building).into(),
            x: building.x as i32,
            y: building.y as i32,
            color: building_color(building.building_type),
        });
    }

    result
}

pub fn build_anchor_labels(simulator: &Simulator) -> Vec<MapMarker> {
    let mut result = Vec::new();

    for building in simulator.buildings() {
        result.push(MapMarker {
            x: building.x as i32,
            y: building.y as i32,
            text: building.id.to_string().into(),
            color: slint::Color::from_rgb_u8(20, 20, 20),
            bg: slint::Color::from_argb_u8(0, 0, 0, 0),
        });
    }

    result
}

pub fn build_entry_labels(simulator: &Simulator) -> Vec<MapMarker> {
    let mut result = Vec::new();
    let light_green = slint::Color::from_argb_u8(180, 164, 236, 164);

    for building in simulator.buildings() {
        if let Some(entry) = building.entry_point {
            result.push(MapMarker {
                x: entry.x as i32,
                y: entry.y as i32,
                text: building.id.to_string().into(),
                color: slint::Color::from_rgb_u8(0, 80, 0),
                bg: light_green,
            });
        }

        for component in building.components() {
            if let Some(entry) = component.entry_point {
                result.push(MapMarker {
                    x: entry.x as i32,
                    y: entry.y as i32,
                    text: building.id.to_string().into(),
                    color: slint::Color::from_rgb_u8(0, 80, 0),
                    bg: light_green,
                });
            }
        }
    }

    result
}

pub fn build_stockpile_resource_labels(simulator: &Simulator) -> Vec<MapMarker> {
    let mut result = Vec::new();

    for building in simulator.buildings() {
        let Some(resource) = building.stockpile_resource else {
            continue;
        };

        result.push(MapMarker {
            x: (building.x + 1) as i32,
            y: (building.y + 1) as i32,
            text: resource.short_label().into(),
            color: slint::Color::from_rgb_u8(20, 20, 20),
            bg: stockpile_resource_color(resource),
        });
    }

    result
}

pub fn build_no_entry_markers(simulator: &Simulator) -> Vec<OccupiedCellVisual> {
    let mut result = Vec::new();
    let red = slint::Color::from_rgb_u8(220, 40, 40);
    let map_size = simulator.map_size();

    for building in simulator.buildings() {
        if building.building_type != BuildingType::GoodsYard && building.entry_point.is_none() {
            append_diagonal_cells(
                &mut result,
                building.x,
                building.y,
                building.width(),
                red,
                map_size,
            );
        }

        for component in building.components() {
            if component.entry_point.is_none() {
                append_diagonal_cells(
                    &mut result,
                    component.x,
                    component.y,
                    component.size,
                    red,
                    map_size,
                );
            }
        }
    }

    result
}

fn building_list_name(building: &simulator::BuildingPlacement) -> String {
    match building.stockpile_resource {
        Some(resource) => {
            format!(
                "{} [{}]",
                building.building_type.display_name(),
                resource.display_name()
            )
        }
        None => building.building_type.display_name().to_string(),
    }
}

fn stockpile_resource_color(resource: StockpileResource) -> slint::Color {
    match resource {
        StockpileResource::Wood => slint::Color::from_argb_u8(210, 196, 150, 92),
        StockpileResource::Iron => slint::Color::from_argb_u8(210, 170, 170, 178),
    }
}

fn append_diagonal_cells(
    out: &mut Vec<OccupiedCellVisual>,
    x: usize,
    y: usize,
    size: usize,
    color: slint::Color,
    map_size: usize,
) {
    if size == 0 {
        return;
    }

    for i in 0..size {
        let p1 = (x + i, y + i);
        let p2 = (x + i, y + (size - 1 - i));

        if p1.0 < map_size && p1.1 < map_size {
            out.push(OccupiedCellVisual {
                x: p1.0 as i32,
                y: p1.1 as i32,
                color,
            });
        }

        if p2.0 < map_size && p2.1 < map_size && p2 != p1 {
            out.push(OccupiedCellVisual {
                x: p2.0 as i32,
                y: p2.1 as i32,
                color,
            });
        }
    }
}
