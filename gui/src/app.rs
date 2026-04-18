use std::sync::{Arc, Mutex};

use simulator::WeaponType;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, SimulationCycleItem, SimulationInfoLine,
    backend::{BackendCommand, BackendHandle, BackendUpdate},
    editor_state::{EditorState, PlacementOutcome},
    visuals,
};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    let state = Arc::new(Mutex::new(EditorState::new()?));

    {
        let state = state
            .lock()
            .expect("editor state lock should not be poisoned");
        refresh_view(&window, &state, "Ready");
    }

    let backend_window = window.as_weak();
    let backend_state = Arc::clone(&state);
    let backend = BackendHandle::spawn(
        {
            let state = state
                .lock()
                .expect("editor state lock should not be poisoned");
            state.map_size()
        },
        move |update: BackendUpdate| {
            let backend_state = Arc::clone(&backend_state);
            let _ = backend_window.upgrade_in_event_loop(move |window| {
                let mut state = backend_state
                    .lock()
                    .expect("editor state lock should not be poisoned");
                state.set_simulator(update.simulator);
                state.set_cycle_rows(update.cycle_rows);
                refresh_view(&window, &state, &update.message);
            });
        },
    )?;

    let weak_window = window.as_weak();
    let state_for_select = Arc::clone(&state);
    window.on_select_building(move |tool_id| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_select
                .lock()
                .expect("editor state lock should not be poisoned");
            let message = if state.set_selected_from_id(tool_id.as_str()) {
                format!("Selected: {}", state.selected_label())
            } else {
                format!("Unknown tool id: {}", tool_id)
            };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_place = Arc::clone(&state);
    let backend_for_place = backend.clone();
    window.on_place_at(move |x, y| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_place
                .lock()
                .expect("editor state lock should not be poisoned");
            match state.place_selected(x, y) {
                Ok(PlacementOutcome::Status(message)) => {
                    refresh_view(&window, &state, &message);
                }
                Ok(PlacementOutcome::BackendCommand(command)) => {
                    let send_result = backend_for_place.send(command);
                    let message = match send_result {
                        Ok(()) => "Processing...".to_string(),
                        Err(error) => error,
                    };
                    refresh_view(&window, &state, &message);
                }
                Err(error) => {
                    refresh_view(&window, &state, &format!("Placement failed: {}", error));
                }
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_clear = Arc::clone(&state);
    window.on_clear_selection(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_clear
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_selection();
            refresh_view(&window, &state, "Build mode disabled");
        }
    });

    let weak_window = window.as_weak();
    let state_for_hover = Arc::clone(&state);
    window.on_hover_at(move |x, y| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_hover
                .lock()
                .expect("editor state lock should not be poisoned");
            state.set_hover_cell(x, y);
            refresh_preview(&window, &state);
            refresh_simulation_view(&window, &state);
        }
    });

    let weak_window = window.as_weak();
    window.on_adjust_zoom(move |delta| {
        if let Some(window) = weak_window.upgrade() {
            let current = window.get_zoom();
            let next = (current + delta).clamp(0.2, 4.0);
            window.set_zoom(next);
        }
    });

    let weak_window = window.as_weak();
    let state_for_remove_walls = Arc::clone(&state);
    let backend_for_remove_walls = backend.clone();
    window.on_remove_all_walls(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_remove_walls
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message = match backend_for_remove_walls.send(BackendCommand::RemoveAllWalls) {
                Ok(()) => "Processing...".to_string(),
                Err(error) => error,
            };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_fletcher = Arc::clone(&state);
    let backend_for_fletcher = backend.clone();
    window.on_set_optimize_fletcher_routing(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_fletcher
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_optimized_fletcher_routing(enabled);
            let message = if changed {
                if enabled {
                    "Optimized Fletcher routing enabled.".to_string()
                } else {
                    "Optimized Fletcher routing disabled.".to_string()
                }
            } else {
                "Fletcher routing setting unchanged".to_string()
            };
            if changed {
                let send_result = backend_for_fletcher.send(BackendCommand::RunCycleSimulation {
                    settings: state.simulation_settings(),
                });
                let status = match send_result {
                    Ok(()) => "Processing simulation...".to_string(),
                    Err(error) => error,
                };
                refresh_view(&window, &state, &status);
            } else {
                window.set_status_text(message.into());
                refresh_simulation_view(&window, &state);
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_game_speed = Arc::clone(&state);
    window.on_set_game_speed(move |value| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_game_speed
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_game_speed(value);
            let message = if changed {
                format!("Game speed set to {} GS", state.game_speed())
            } else {
                format!("Game speed remains {} GS", state.game_speed())
            };
            window.set_status_text(message.into());
            refresh_simulation_view(&window, &state);
        }
    });

    let weak_window = window.as_weak();
    let state_for_tooltips = Arc::clone(&state);
    window.on_set_simulation_tooltips_enabled(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_tooltips
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_simulation_tooltips_enabled(enabled);
            let message = if enabled {
                "Simulation tooltips enabled"
            } else {
                "Simulation tooltips disabled"
            };
            if changed {
                refresh_view(&window, &state, message);
            } else {
                refresh_simulation_view(&window, &state);
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_run_simulation = Arc::clone(&state);
    let backend_for_run_simulation = backend.clone();
    window.on_run_simulation(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_run_simulation
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message =
                match backend_for_run_simulation.send(BackendCommand::RunCycleSimulation {
                    settings: state.simulation_settings(),
                }) {
                    Ok(()) => "Processing simulation...".to_string(),
                    Err(error) => error,
                };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_fletchers_weapon = Arc::clone(&state);
    let backend_for_fletchers_weapon = backend.clone();
    window.on_toggle_fletchers_weapon(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_fletchers_weapon
                .lock()
                .expect("editor state lock should not be poisoned");
            let weapon = state.toggle_fletchers_weapon();
            let message =
                match backend_for_fletchers_weapon.send(BackendCommand::RunCycleSimulation {
                    settings: state.simulation_settings(),
                }) {
                    Ok(()) => format!(
                        "Fletchers switched to {}. Processing simulation...",
                        weapon.display_name()
                    ),
                    Err(error) => error,
                };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_poleturners_weapon = Arc::clone(&state);
    let backend_for_poleturners_weapon = backend.clone();
    window.on_toggle_poleturners_weapon(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_poleturners_weapon
                .lock()
                .expect("editor state lock should not be poisoned");
            let weapon = state.toggle_poleturners_weapon();
            let message =
                match backend_for_poleturners_weapon.send(BackendCommand::RunCycleSimulation {
                    settings: state.simulation_settings(),
                }) {
                    Ok(()) => format!(
                        "Poleturners switched to {}. Processing simulation...",
                        weapon.display_name()
                    ),
                    Err(error) => error,
                };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_blacksmiths_weapon = Arc::clone(&state);
    let backend_for_blacksmiths_weapon = backend.clone();
    window.on_toggle_blacksmiths_weapon(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_blacksmiths_weapon
                .lock()
                .expect("editor state lock should not be poisoned");
            let weapon = state.toggle_blacksmiths_weapon();
            let message =
                match backend_for_blacksmiths_weapon.send(BackendCommand::RunCycleSimulation {
                    settings: state.simulation_settings(),
                }) {
                    Ok(()) => format!(
                        "Blacksmiths switched to {}. Processing simulation...",
                        weapon.display_name()
                    ),
                    Err(error) => error,
                };
            refresh_view(&window, &state, &message);
        }
    });

    window.run()?;
    Ok(())
}

fn refresh_view(window: &MainWindow, state: &EditorState, status: &str) {
    window.set_status_text(status.into());
    refresh_static_view(window, state);
    refresh_preview(window, state);
    refresh_simulation_view(window, state);
}

fn refresh_static_view(window: &MainWindow, state: &EditorState) {
    let map_size = state.map_size() as i32;
    window.set_map_size(map_size);
    window.set_selected_building(state.selected_id().unwrap_or_default().into());
    window.set_optimize_fletcher_routing(state.optimized_fletcher_routing());
    window.set_game_speed(state.game_speed() as i32);
    window.set_fletchers_weapon(weapon_id(state.fletchers_weapon()).into());
    window.set_poleturners_weapon(weapon_id(state.poleturners_weapon()).into());
    window.set_blacksmiths_weapon(weapon_id(state.blacksmiths_weapon()).into());
    window.set_simulation_tooltips_enabled(state.simulation_tooltips_enabled());

    let cells = visuals::build_occupied_cells(state.simulator());
    let model = VecModel::from(cells);
    window.set_occupied_cells(ModelRc::new(model));

    let list_entries = visuals::build_building_list(state.simulator());
    let list_model = VecModel::from(list_entries);
    window.set_placed_buildings(ModelRc::new(list_model));

    let anchor_labels = visuals::build_anchor_labels(state.simulator());
    let anchor_model = VecModel::from(anchor_labels);
    window.set_anchor_labels(ModelRc::new(anchor_model));

    let entry_labels = visuals::build_entry_labels(state.simulator());
    let entry_model = VecModel::from(entry_labels);
    window.set_entry_labels(ModelRc::new(entry_model));

    let no_entry_cells = visuals::build_no_entry_markers(state.simulator());
    let no_entry_model = VecModel::from(no_entry_cells);
    window.set_no_entry_cells(ModelRc::new(no_entry_model));

    let resource_labels = visuals::build_stockpile_resource_labels(state.simulator());
    let resource_model = VecModel::from(resource_labels);
    window.set_resource_labels(ModelRc::new(resource_model));
}

fn refresh_preview(window: &MainWindow, state: &EditorState) {
    let preview_cells = visuals::build_preview_cells(&state.preview_cells());
    let preview_model = VecModel::from(preview_cells);
    window.set_preview_cells(ModelRc::new(preview_model));
}

fn refresh_simulation_view(window: &MainWindow, state: &EditorState) {
    window.set_optimize_fletcher_routing(state.optimized_fletcher_routing());
    window.set_game_speed(state.game_speed() as i32);
    window.set_fletchers_weapon(weapon_id(state.fletchers_weapon()).into());
    window.set_poleturners_weapon(weapon_id(state.poleturners_weapon()).into());
    window.set_blacksmiths_weapon(weapon_id(state.blacksmiths_weapon()).into());
    window.set_simulation_tooltips_enabled(state.simulation_tooltips_enabled());

    let items = state
        .cycle_rows()
        .iter()
        .map(|row| {
            let armoury = row
                .armoury_id
                .map(|id| format!("#{}", id))
                .unwrap_or_else(|| "-".to_string());
            let ticks = row
                .total_ticks
                .map(|ticks| ticks.to_string())
                .unwrap_or_else(|| "-".to_string());
            let seconds = row
                .total_ticks
                .map(|ticks| format!("{:.2}", ticks as f64 / state.game_speed() as f64))
                .unwrap_or_else(|| "-".to_string());
            let weapon = row.weapon_type.display_name().to_string();
            let note = match (row.travel_ticks, row.make_ticks, row.error.as_ref()) {
                (Some(travel_ticks), Some(make_ticks), _) => {
                    format!(
                        "Travel: {} ticks, craft: {} ticks",
                        travel_ticks, make_ticks
                    )
                }
                (_, _, Some(error)) => error.clone(),
                _ => "No cycle data".to_string(),
            };

            SimulationCycleItem {
                workshop: row.workshop_name.clone().into(),
                weapon: weapon.into(),
                armoury: armoury.into(),
                ticks: ticks.into(),
                seconds: seconds.into(),
                note: note.into(),
            }
        })
        .collect::<Vec<_>>();

    let model = VecModel::from(items);
    window.set_simulation_cycles(ModelRc::new(model));

    let (title, subtitle, lines) = build_hover_simulation_info(state);
    let tooltip_visible = state.simulation_tooltips_enabled() && !title.is_empty();
    let tooltip_height = estimate_tooltip_height(&subtitle, &lines);
    window.set_simulation_info_title(title.into());
    window.set_simulation_info_subtitle(subtitle.into());
    window.set_simulation_tooltip_visible(tooltip_visible);
    if tooltip_visible {
        if let Some((hover_x, hover_y)) = state.hover_cell() {
            let cell_px = (10.0 * window.get_zoom()).round() as i32;
            let tooltip_x = hover_x * cell_px + (cell_px / 2) - 125;
            let tooltip_y = (state.map_size() as i32 - hover_y - 1) * cell_px - tooltip_height - 12;
            window.set_simulation_tooltip_x(tooltip_x);
            window.set_simulation_tooltip_y(tooltip_y);
            window.set_simulation_tooltip_height(tooltip_height);
        }
    } else {
        window.set_simulation_tooltip_x(0);
        window.set_simulation_tooltip_y(0);
        window.set_simulation_tooltip_height(80);
    }
    let info_model = VecModel::from(
        lines
            .into_iter()
            .map(|text| SimulationInfoLine { text: text.into() })
            .collect::<Vec<_>>(),
    );
    window.set_simulation_info_lines(ModelRc::new(info_model));
}

fn weapon_id(weapon: WeaponType) -> &'static str {
    match weapon {
        WeaponType::Bow => "bow",
        WeaponType::Crossbow => "crossbow",
        WeaponType::Spear => "spear",
        WeaponType::Pike => "pike",
        WeaponType::Sword => "sword",
        WeaponType::Mace => "mace",
        WeaponType::Armor => "armor",
    }
}

fn build_hover_simulation_info(state: &EditorState) -> (String, String, Vec<String>) {
    let Some(building) = state.hovered_building() else {
        return (String::new(), String::new(), Vec::new());
    };

    match building.building_type {
        simulator::BuildingType::FletchersWorkshop
        | simulator::BuildingType::BlacksmithsWorkshop
        | simulator::BuildingType::PoleturnersWorkshop
        | simulator::BuildingType::ArmourersWorkshop => {
            build_workshop_hover_info(state, building.id, building.building_type.display_name())
        }
        simulator::BuildingType::Armoury => build_armoury_hover_info(state, building.id),
        simulator::BuildingType::Stockpile => {
            build_stockpile_hover_info(state, building.id, building.stockpile_resource)
        }
        _ => (String::new(), String::new(), Vec::new()),
    }
}

fn build_workshop_hover_info(
    state: &EditorState,
    workshop_id: u32,
    display_name: &str,
) -> (String, String, Vec<String>) {
    let Some(row) = state
        .cycle_rows()
        .iter()
        .find(|row| row.workshop_id == workshop_id)
    else {
        return (
            format!("#{} {}", workshop_id, display_name),
            "Run simulation to inspect this workshop".to_string(),
            Vec::new(),
        );
    };

    let mut lines = Vec::new();
    let subtitle = format!("Current product: {}", row.weapon_type.display_name());

    match (row.total_ticks, row.armoury_id) {
        (Some(total_ticks), Some(armoury_id)) => {
            lines.push(format!("Armoury: #{}", armoury_id));
            lines.push(format!(
                "Cycle: {} ticks | {:.2} sec",
                total_ticks,
                total_ticks as f64 / state.game_speed() as f64
            ));
            if let (Some(travel_ticks), Some(make_ticks)) = (row.travel_ticks, row.make_ticks) {
                lines.push(format!(
                    "Travel: {} ticks | Craft: {} ticks",
                    travel_ticks, make_ticks
                ));
            }

            let weapons_per_tick = 1.0 / total_ticks as f64;
            lines.push(format!(
                "Output / tick: {}",
                format_rate_tick(weapons_per_tick)
            ));
            lines.push(format!(
                "Output / min: {}",
                format_rate_minute(weapons_per_tick * state.game_speed() as f64 * 60.0)
            ));
        }
        _ => {
            lines.push(
                row.error
                    .clone()
                    .unwrap_or_else(|| "No reachable cycle".to_string()),
            );
        }
    }

    (
        format!("#{} {}", workshop_id, display_name),
        subtitle,
        lines,
    )
}

fn build_stockpile_hover_info(
    state: &EditorState,
    stockpile_id: u32,
    resource: Option<simulator::StockpileResource>,
) -> (String, String, Vec<String>) {
    let Some(resource) = resource else {
        return (
            format!("#{} Stockpile", stockpile_id),
            "No resource assigned".to_string(),
            Vec::new(),
        );
    };

    let total_per_tick = state
        .cycle_rows()
        .iter()
        .filter_map(|row| {
            let total_ticks = row.total_ticks?;
            let amount = match resource {
                simulator::StockpileResource::Wood => row.wood_per_cycle,
                simulator::StockpileResource::Iron => row.iron_per_cycle,
            };
            if amount == 0 {
                return None;
            }
            Some(amount as f64 / total_ticks as f64)
        })
        .sum::<f64>();

    let per_minute = total_per_tick * state.game_speed() as f64 * 60.0;
    let lines = vec![
        format!(
            "{} spend / tick: {}",
            resource.display_name(),
            format_rate_tick(total_per_tick)
        ),
        format!(
            "{} spend / min: {}",
            resource.display_name(),
            format_rate_minute(per_minute)
        ),
    ];

    (
        format!("#{} Stockpile [{}]", stockpile_id, resource.display_name()),
        "Shared stock consumption".to_string(),
        lines,
    )
}

fn build_armoury_hover_info(state: &EditorState, armoury_id: u32) -> (String, String, Vec<String>) {
    let mut weapon_totals = std::collections::BTreeMap::new();

    for row in state
        .cycle_rows()
        .iter()
        .filter(|row| row.armoury_id == Some(armoury_id))
    {
        let Some(total_ticks) = row.total_ticks else {
            continue;
        };

        let per_tick = 1.0 / total_ticks as f64;
        let key = row.weapon_type.display_name().to_string();
        *weapon_totals.entry(key).or_insert(0.0) += per_tick;
    }

    let mut lines = Vec::new();

    if weapon_totals.is_empty() {
        lines.push("No completed workshop cycles are routed here".to_string());
    } else {
        for (index, (weapon_name, per_tick)) in weapon_totals.into_iter().enumerate() {
            if index > 0 {
                lines.push(String::new());
            }

            lines.push(format!(
                "{} per tick: {}",
                weapon_name,
                format_rate_tick(per_tick)
            ));
            lines.push(format!(
                "{} per min: {}",
                weapon_name,
                format_rate_minute(per_tick * state.game_speed() as f64 * 60.0)
            ));
        }
    }

    (
        format!("#{} Armoury", armoury_id),
        "Incoming weapon production by type".to_string(),
        lines,
    )
}

fn format_rate_tick(value: f64) -> String {
    format!("{:.2e}", value)
}

fn format_rate_minute(value: f64) -> String {
    format!("{:.2}", value)
}

fn estimate_tooltip_height(subtitle: &str, lines: &[String]) -> i32 {
    let visible_line_count = 1 + i32::from(!subtitle.is_empty()) + lines.len() as i32;
    18 + (visible_line_count * 18)
}
