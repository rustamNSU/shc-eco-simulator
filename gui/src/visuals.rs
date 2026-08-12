use std::collections::{BTreeMap, BTreeSet};

use simulator::{BuildingType, Simulator, StockpileResource};

use crate::{BuildingBoundary, MapMarker, OccupiedCellVisual};

pub fn building_color(building_type: BuildingType) -> slint::Color {
    match building_type {
        BuildingType::GoodsYard => slint::Color::from_rgb_u8(179, 120, 78),
        BuildingType::Stockpile => slint::Color::from_rgb_u8(179, 120, 78),
        BuildingType::Armoury => slint::Color::from_rgb_u8(103, 134, 171),
        BuildingType::FletchersWorkshop => slint::Color::from_rgb_u8(82, 156, 94),
        BuildingType::BlacksmithsWorkshop => slint::Color::from_rgb_u8(74, 74, 74),
        BuildingType::PoleturnersWorkshop => slint::Color::from_rgb_u8(175, 145, 86),
        BuildingType::ArmourersWorkshop => slint::Color::from_rgb_u8(151, 111, 171),
        BuildingType::WheatFarm => slint::Color::from_rgb_u8(194, 174, 76),
        BuildingType::Windmill => slint::Color::from_rgb_u8(164, 155, 138),
        BuildingType::Bakery => slint::Color::from_rgb_u8(190, 119, 66),
        BuildingType::Granary => slint::Color::from_rgb_u8(137, 92, 58),
    }
}

pub fn build_occupied_cells(simulator: &Simulator) -> Vec<OccupiedCellVisual> {
    let mut cells = Vec::new();

    for building in simulator.buildings() {
        let color = building_color(building.building_type);
        for (x, y) in building.occupied_cells() {
            let color = if building.building_type == BuildingType::WheatFarm
                && x < building.x + 3
                && y >= building.y + 6
            {
                slint::Color::from_rgb_u8(132, 92, 55)
            } else {
                color
            };
            cells.push(OccupiedCellVisual { x, y, color });
        }
    }

    let wall_color = slint::Color::from_rgb_u8(120, 84, 62);
    for wall in simulator.walls() {
        for (x, y) in wall.cells() {
            cells.push(OccupiedCellVisual {
                x,
                y,
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
            x: edge.x,
            y: edge.y,
            horizontal: edge.horizontal,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BoundaryEdge {
    x: i32,
    y: i32,
    horizontal: bool,
}

fn append_boundary_edges(
    edges: &mut BTreeSet<BoundaryEdge>,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
) {
    if width == 0 || height == 0 {
        return;
    }

    for dx in 0..width {
        edges.insert(BoundaryEdge {
            x: x + dx as i32,
            y,
            horizontal: true,
        });
        edges.insert(BoundaryEdge {
            x: x + dx as i32,
            y: y + height as i32,
            horizontal: true,
        });
    }

    for dy in 0..height {
        edges.insert(BoundaryEdge {
            x,
            y: y + dy as i32,
            horizontal: false,
        });
        edges.insert(BoundaryEdge {
            x: x + width as i32,
            y: y + dy as i32,
            horizontal: false,
        });
    }
}

pub fn build_anchor_labels(simulator: &Simulator) -> Vec<MapMarker> {
    let mut result = Vec::new();

    for building in simulator.buildings() {
        result.push(MapMarker {
            x: building.x,
            y: building.y,
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
                x: entry.x,
                y: entry.y,
                text: building.id.to_string().into(),
                color: slint::Color::from_rgb_u8(0, 80, 0),
                bg: light_green,
            });
        }

        for component in building.components() {
            if let Some(entry) = component.entry_point {
                result.push(MapMarker {
                    x: entry.x,
                    y: entry.y,
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
            x: building.x + 1,
            y: building.y + 1,
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
    let bounds = simulator.map_bounds();

    for building in simulator.buildings() {
        if building.building_type != BuildingType::GoodsYard && building.entry_point.is_none() {
            append_diagonal_cells(
                &mut result,
                building.x,
                building.y,
                building.width(),
                red,
                bounds,
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
                    bounds,
                );
            }
        }
    }

    result
}

fn stockpile_resource_color(resource: StockpileResource) -> slint::Color {
    match resource {
        StockpileResource::Wood => slint::Color::from_argb_u8(210, 196, 150, 92),
        StockpileResource::Iron => slint::Color::from_argb_u8(210, 170, 170, 178),
        StockpileResource::Wheat => slint::Color::from_argb_u8(210, 212, 184, 73),
        StockpileResource::Flour => slint::Color::from_argb_u8(210, 232, 224, 196),
    }
}

fn append_diagonal_cells(
    out: &mut Vec<OccupiedCellVisual>,
    x: i32,
    y: i32,
    size: usize,
    color: slint::Color,
    bounds: simulator::MapBounds,
) {
    if size == 0 {
        return;
    }

    for i in 0..size {
        let p1 = (x + i as i32, y + i as i32);
        let p2 = (x + i as i32, y + (size - 1 - i) as i32);

        if bounds.contains(p1.0, p1.1) {
            out.push(OccupiedCellVisual {
                x: p1.0,
                y: p1.1,
                color,
            });
        }

        if bounds.contains(p2.0, p2.1) && p2 != p1 {
            out.push(OccupiedCellVisual {
                x: p2.0,
                y: p2.1,
                color,
            });
        }
    }
}
