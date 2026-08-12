use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use simulator::{
    BREAD_SELL_GOLD, BuildingType, FLOUR_BUY_GOLD, FLOUR_SELL_GOLD, WHEAT_BUY_GOLD,
    WHEAT_SELL_GOLD, WOOD_BUY_GOLD, WeaponType,
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, SimulationCycleItem, SimulationInfoLine,
    backend::{BackendCommand, BackendHandle, BackendUpdate},
    editor_state::{EditorState, PlacementOutcome},
    project_files::ProjectSession,
    visuals,
};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    let state = Arc::new(Mutex::new(EditorState::new()?));
    let project_session = Arc::new(Mutex::new(ProjectSession::new()));

    {
        let state = state
            .lock()
            .expect("editor state lock should not be poisoned");
        refresh_view(&window, &state, "Ready");
    }
    {
        let session = project_session
            .lock()
            .expect("project session lock should not be poisoned");
        refresh_project_file_view(&window, &session);
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
                if let Some(cycle_rows) = update.cycle_rows {
                    state.set_cycle_rows(cycle_rows);
                    refresh_view(&window, &state, &update.message);
                } else {
                    if update.geometry_changed {
                        state.mark_simulation_results_stale();
                    }
                    refresh_geometry_view(&window, &state, &update.message);
                }
            });
        },
    )?;

    let weak_window = window.as_weak();
    let state_for_save = Arc::clone(&state);
    let session_for_save = Arc::clone(&project_session);
    window.on_save_project(move || {
        if let Some(window) = weak_window.upgrade() {
            save_project(&window, &state_for_save, &session_for_save, false);
        }
    });

    let weak_window = window.as_weak();
    let state_for_save_as = Arc::clone(&state);
    let session_for_save_as = Arc::clone(&project_session);
    window.on_save_project_as(move || {
        if let Some(window) = weak_window.upgrade() {
            save_project(&window, &state_for_save_as, &session_for_save_as, true);
        }
    });

    let weak_window = window.as_weak();
    let state_for_open = Arc::clone(&state);
    let session_for_open = Arc::clone(&project_session);
    let backend_for_open = backend.clone();
    window.on_open_project(move || {
        if let Some(window) = weak_window.upgrade() {
            let result = session_for_open
                .lock()
                .expect("project session lock should not be poisoned")
                .open_dialog();
            handle_open_result(
                &window,
                &state_for_open,
                &session_for_open,
                &backend_for_open,
                result,
            );
        }
    });

    let weak_window = window.as_weak();
    let state_for_recent = Arc::clone(&state);
    let session_for_recent = Arc::clone(&project_session);
    let backend_for_recent = backend.clone();
    window.on_open_recent_project(move || {
        if let Some(window) = weak_window.upgrade() {
            let result = session_for_recent
                .lock()
                .expect("project session lock should not be poisoned")
                .open_latest_recent();
            handle_open_result(
                &window,
                &state_for_recent,
                &session_for_recent,
                &backend_for_recent,
                result,
            );
        }
    });

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
            refresh_geometry_view(&window, &state, &message);
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
                    refresh_geometry_view(&window, &state, &message);
                }
                Ok(PlacementOutcome::BackendCommand(command)) => {
                    let send_result = backend_for_place.send(command);
                    let message = match send_result {
                        Ok(()) => "Processing...".to_string(),
                        Err(error) => error,
                    };
                    refresh_geometry_view(&window, &state, &message);
                }
                Err(error) => {
                    refresh_geometry_view(&window, &state, &format!("Placement failed: {}", error));
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
            refresh_geometry_view(&window, &state, "Build mode disabled");
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
            refresh_tooltip_view(&window, &state);
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
            refresh_geometry_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_remove_all = Arc::clone(&state);
    let backend_for_remove_all = backend.clone();
    window.on_remove_all(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_remove_all
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message = match backend_for_remove_all.send(BackendCommand::RemoveAll) {
                Ok(()) => "Processing...".to_string(),
                Err(error) => error,
            };
            refresh_geometry_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_undo = Arc::clone(&state);
    let backend_for_undo = backend.clone();
    window.on_undo(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_undo
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message = match backend_for_undo.send(BackendCommand::Undo) {
                Ok(()) => "Processing undo...".to_string(),
                Err(error) => error,
            };
            refresh_geometry_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_redo = Arc::clone(&state);
    let backend_for_redo = backend.clone();
    window.on_redo(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_redo
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message = match backend_for_redo.send(BackendCommand::Redo) {
                Ok(()) => "Processing redo...".to_string(),
                Err(error) => error,
            };
            refresh_geometry_view(&window, &state, &message);
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
    let state_for_fear_factor = Arc::clone(&state);
    let backend_for_fear_factor = backend.clone();
    window.on_set_fear_factor(move |value| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_fear_factor
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_fear_factor(value);
            if changed {
                let message =
                    match backend_for_fear_factor.send(BackendCommand::RunCycleSimulation {
                        settings: state.simulation_settings(),
                    }) {
                        Ok(()) => format!(
                            "Fear factor set to {}. Processing simulation...",
                            state.fear_factor()
                        ),
                        Err(error) => error,
                    };
                refresh_view(&window, &state, &message);
            } else {
                window
                    .set_status_text(format!("Fear factor remains {}", state.fear_factor()).into());
                refresh_simulation_view(&window, &state);
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_buy_wood = Arc::clone(&state);
    window.on_set_buy_wood(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_buy_wood
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_buy_wood(enabled);
            let message = if changed {
                if enabled {
                    "Buy Wood enabled"
                } else {
                    "Buy Wood disabled"
                }
            } else {
                "Buy Wood unchanged"
            };
            refresh_view(&window, &state, message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_buy_iron = Arc::clone(&state);
    window.on_set_buy_iron(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_buy_iron
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_buy_iron(enabled);
            let message = if changed {
                if enabled {
                    "Buy Iron enabled"
                } else {
                    "Buy Iron disabled"
                }
            } else {
                "Buy Iron unchanged"
            };
            refresh_view(&window, &state, message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_buy_wheat = Arc::clone(&state);
    window.on_set_buy_wheat(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_buy_wheat
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_buy_wheat(enabled);
            let message = buy_setting_message("Wheat", enabled, changed);
            refresh_view(&window, &state, message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_buy_flour = Arc::clone(&state);
    window.on_set_buy_flour(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_buy_flour
                .lock()
                .expect("editor state lock should not be poisoned");
            let changed = state.set_buy_flour(enabled);
            let message = buy_setting_message("Flour", enabled, changed);
            refresh_view(&window, &state, message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_population_economy = Arc::clone(&state);
    window.on_set_population_economy_enabled(move |enabled| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_population_economy
                .lock()
                .expect("editor state lock should not be poisoned");
            state.set_population_economy_enabled(enabled);
            refresh_view(
                &window,
                &state,
                if enabled {
                    "Population economy included"
                } else {
                    "Population economy excluded"
                },
            );
        }
    });

    let weak_window = window.as_weak();
    let state_for_population = Arc::clone(&state);
    window.on_set_population(move |value| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_population
                .lock()
                .expect("editor state lock should not be poisoned");
            state.set_population(value);
            refresh_view(&window, &state, "Population updated");
        }
    });

    let weak_window = window.as_weak();
    let state_for_max_population = Arc::clone(&state);
    window.on_set_max_population(move |text| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_max_population
                .lock()
                .expect("editor state lock should not be poisoned");
            match parse_count(text.as_str(), "maximum population") {
                Ok(value) => {
                    state.set_max_population(value);
                    refresh_view(&window, &state, "Population scale updated");
                }
                Err(error) => window.set_status_text(error.into()),
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_inns = Arc::clone(&state);
    window.on_set_inn_count(move |text| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_inns
                .lock()
                .expect("editor state lock should not be poisoned");
            match parse_count(text.as_str(), "inn count") {
                Ok(value) => {
                    state.set_inn_count(value);
                    refresh_view(&window, &state, "Inn count updated");
                }
                Err(error) => window.set_status_text(error.into()),
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_stone = Arc::clone(&state);
    window.on_set_stone_quarry_count(move |text| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_stone
                .lock()
                .expect("editor state lock should not be poisoned");
            match parse_count(text.as_str(), "stone quarry count") {
                Ok(value) => {
                    state.set_stone_quarry_count(value);
                    refresh_view(&window, &state, "Stone quarry count updated");
                }
                Err(error) => window.set_status_text(error.into()),
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_iron_mines = Arc::clone(&state);
    window.on_set_iron_mine_count(move |text| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_iron_mines
                .lock()
                .expect("editor state lock should not be poisoned");
            match parse_count(text.as_str(), "iron mine count") {
                Ok(value) => {
                    state.set_iron_mine_count(value);
                    refresh_view(&window, &state, "Iron mine count updated");
                }
                Err(error) => window.set_status_text(error.into()),
            }
        }
    });

    let weak_window = window.as_weak();
    let state_for_tax = Arc::clone(&state);
    window.on_set_tax_index(move |value| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_tax
                .lock()
                .expect("editor state lock should not be poisoned");
            state.set_tax_index(value);
            refresh_view(&window, &state, "Tax level updated");
        }
    });

    let weak_window = window.as_weak();
    let state_for_food_ratio = Arc::clone(&state);
    window.on_set_food_ratio_index(move |value| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_food_ratio
                .lock()
                .expect("editor state lock should not be poisoned");
            state.set_food_ratio_index(value);
            refresh_view(&window, &state, "Food ratio updated");
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

fn save_project(
    window: &MainWindow,
    state: &Arc<Mutex<EditorState>>,
    project_session: &Arc<Mutex<ProjectSession>>,
    save_as: bool,
) {
    let project = {
        let state = state
            .lock()
            .expect("editor state lock should not be poisoned");
        simulator::ProjectFile::capture(
            state.simulator(),
            state.simulation_settings(),
            state.population_economy_settings(),
        )
    };

    let mut session = project_session
        .lock()
        .expect("project session lock should not be poisoned");
    let result = if save_as {
        session.save_as(&project)
    } else {
        session.save(&project)
    };

    let message = match result {
        Ok(Some(path)) => format!("Saved {}", display_file_name(&path)),
        Ok(None) => "Save canceled".to_string(),
        Err(error) => format!("Save failed: {error}"),
    };
    refresh_project_file_view(window, &session);
    window.set_status_text(message.into());
}

fn handle_open_result(
    window: &MainWindow,
    state: &Arc<Mutex<EditorState>>,
    project_session: &Arc<Mutex<ProjectSession>>,
    backend: &BackendHandle,
    result: Result<Option<(simulator::ProjectFile, PathBuf)>, String>,
) {
    let Some((project, path)) = (match result {
        Ok(project) => project,
        Err(error) => {
            window.set_status_text(format!("Open failed: {error}").into());
            return;
        }
    }) else {
        window.set_status_text("Open canceled".into());
        return;
    };

    let (simulator, settings, population_economy_settings) = match project.into_simulator() {
        Ok(project) => project,
        Err(error) => {
            window.set_status_text(format!("Open failed: {error}").into());
            return;
        }
    };

    if let Err(error) = backend.send(BackendCommand::LoadProject {
        simulator: Box::new(simulator),
        settings,
    }) {
        window.set_status_text(format!("Open failed: {error}").into());
        return;
    }

    {
        let mut state = state
            .lock()
            .expect("editor state lock should not be poisoned");
        state.set_simulation_settings(settings);
        state.set_population_economy_settings(population_economy_settings);
        state.mark_simulation_results_stale();
        state.clear_selection();
        refresh_geometry_view(window, &state, "Opening project and recalculating...");
    }

    let file_name = display_file_name(&path);
    let mut session = project_session
        .lock()
        .expect("project session lock should not be poisoned");
    let recent_result = session.mark_opened(path);
    refresh_project_file_view(window, &session);
    let message = match recent_result {
        Ok(()) => format!("Opened {file_name}"),
        Err(error) => format!("Opened {file_name}; recent list was not saved: {error}"),
    };
    window.set_status_text(message.into());
}

fn refresh_project_file_view(window: &MainWindow, session: &ProjectSession) {
    window.set_current_project_name(session.current_name().into());
    window.set_recent_project_name(session.latest_recent_name().into());
}

fn display_file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn buy_setting_message(resource: &str, enabled: bool, changed: bool) -> &'static str {
    match (resource, enabled, changed) {
        ("Wheat", true, true) => "Buy Wheat enabled",
        ("Wheat", false, true) => "Buy Wheat disabled",
        ("Wheat", _, false) => "Buy Wheat unchanged",
        ("Flour", true, true) => "Buy Flour enabled",
        ("Flour", false, true) => "Buy Flour disabled",
        ("Flour", _, false) => "Buy Flour unchanged",
        _ => "Buy setting unchanged",
    }
}

fn parse_count(text: &str, label: &str) -> Result<u32, String> {
    text.trim()
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a non-negative whole number"))
}

fn refresh_view(window: &MainWindow, state: &EditorState, status: &str) {
    window.set_status_text(status.into());
    refresh_static_view(window, state);
    refresh_preview(window, state);
    refresh_simulation_view(window, state);
}

fn refresh_geometry_view(window: &MainWindow, state: &EditorState, status: &str) {
    window.set_status_text(status.into());
    refresh_static_view(window, state);
    refresh_preview(window, state);
    window.set_simulation_results_stale(state.simulation_results_stale());
    if state.simulation_results_stale() {
        window.set_armoury_summary(
            "Simulation results are out of date.\nPress Recalculate after finishing map changes."
                .into(),
        );
        window.set_population_economy_summary(
            "Simulation results are out of date.\nGeometry changed; economy was not recalculated."
                .into(),
        );
        window.set_simulation_tooltip_visible(false);
    }
}

fn refresh_static_view(window: &MainWindow, state: &EditorState) {
    let bounds = state.map_bounds();
    let old_min_x = window.get_map_min_x();
    let old_max_y = window.get_map_max_y();
    let old_pan_x = window.get_canvas_pan_x();
    let old_pan_y = window.get_canvas_pan_y();
    window.set_map_min_x(bounds.min_x);
    window.set_map_min_y(bounds.min_y);
    window.set_map_max_x(bounds.max_x);
    window.set_map_max_y(bounds.max_y);
    window.set_map_width(bounds.width() as i32);
    window.set_map_height(bounds.height() as i32);

    let cell_px = 10.0 * window.get_zoom();
    let added_left = old_min_x - bounds.min_x;
    let added_top = bounds.max_y - old_max_y;
    if added_left != 0 {
        window.set_canvas_pan_x(old_pan_x - added_left as f32 * cell_px);
    }
    if added_top != 0 {
        window.set_canvas_pan_y(old_pan_y - added_top as f32 * cell_px);
    }
    window.set_selected_building(state.selected_id().unwrap_or_default().into());
    window.set_optimize_fletcher_routing(state.optimized_fletcher_routing());
    window.set_game_speed(state.game_speed() as i32);
    window.set_fear_factor(state.fear_factor());
    window.set_buy_wood(state.buy_wood());
    window.set_buy_iron(state.buy_iron());
    window.set_buy_wheat(state.buy_wheat());
    window.set_buy_flour(state.buy_flour());
    window.set_eco_setup_cost(build_eco_setup_summary(state).into());
    window.set_workshop_count_summary(build_workshop_count_summary(state).into());
    window.set_bread_count_summary(build_bread_count_summary(state).into());
    window.set_fletchers_weapon(weapon_id(state.fletchers_weapon()).into());
    window.set_poleturners_weapon(weapon_id(state.poleturners_weapon()).into());
    window.set_blacksmiths_weapon(weapon_id(state.blacksmiths_weapon()).into());
    window.set_simulation_tooltips_enabled(state.simulation_tooltips_enabled());

    let cells = visuals::build_occupied_cells(state.simulator());
    let model = VecModel::from(cells);
    window.set_occupied_cells(ModelRc::new(model));

    let boundaries = visuals::build_building_boundaries(state.simulator());
    let boundary_model = VecModel::from(boundaries);
    window.set_building_boundaries(ModelRc::new(boundary_model));

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
    window.set_simulation_results_stale(state.simulation_results_stale());
    window.set_optimize_fletcher_routing(state.optimized_fletcher_routing());
    window.set_game_speed(state.game_speed() as i32);
    window.set_fear_factor(state.fear_factor());
    window.set_buy_wood(state.buy_wood());
    window.set_buy_iron(state.buy_iron());
    window.set_buy_wheat(state.buy_wheat());
    window.set_buy_flour(state.buy_flour());
    window.set_eco_setup_cost(build_eco_setup_summary(state).into());
    window.set_workshop_count_summary(build_workshop_count_summary(state).into());
    window.set_bread_count_summary(build_bread_count_summary(state).into());
    window.set_fletchers_weapon(weapon_id(state.fletchers_weapon()).into());
    window.set_poleturners_weapon(weapon_id(state.poleturners_weapon()).into());
    window.set_blacksmiths_weapon(weapon_id(state.blacksmiths_weapon()).into());
    window.set_simulation_tooltips_enabled(state.simulation_tooltips_enabled());

    if state.simulation_results_stale() {
        window.set_armoury_summary(
            "Simulation results are out of date.\nPress Recalculate after finishing map changes."
                .into(),
        );
        window.set_population_economy_summary(
            "Simulation results are out of date.\nGeometry changed; economy was not recalculated."
                .into(),
        );
        window.set_simulation_tooltip_visible(false);
        window.set_simulation_cycles(ModelRc::new(VecModel::from(
            Vec::<SimulationCycleItem>::new(),
        )));
        return;
    }

    window.set_armoury_summary(build_armoury_summary(state).into());

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
                        "Travel: {} ticks, craft: {} ticks, avg output: {:.2}",
                        travel_ticks, make_ticks, row.average_weapons_per_cycle
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

    refresh_tooltip_view(window, state);
    refresh_population_economy_view(window, state);
}

fn refresh_tooltip_view(window: &MainWindow, state: &EditorState) {
    if state.simulation_results_stale() {
        window.set_simulation_tooltip_visible(false);
        return;
    }

    let (title, subtitle, lines) = build_hover_simulation_info(state);
    let tooltip_visible = state.simulation_tooltips_enabled() && !title.is_empty();
    let tooltip_height = estimate_tooltip_height(&subtitle, &lines);
    window.set_simulation_info_title(title.into());
    window.set_simulation_info_subtitle(subtitle.into());
    window.set_simulation_tooltip_visible(tooltip_visible);
    if tooltip_visible {
        if let Some((hover_x, hover_y)) = state.hover_cell() {
            let cell_px = (10.0 * window.get_zoom()).round() as i32;
            let bounds = state.map_bounds();
            let tooltip_x = (hover_x - bounds.min_x) * cell_px + (cell_px / 2) - 125;
            let tooltip_y = (bounds.max_y - hover_y - 1) * cell_px - tooltip_height - 12;
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

fn refresh_population_economy_view(window: &MainWindow, state: &EditorState) {
    let report = build_population_economy_report(state);
    let settings = report.settings;
    window.set_population_economy_enabled(settings.enabled);
    window.set_population_max(settings.max_population as i32);
    window.set_population_current(settings.population as i32);
    window.set_population_workers(report.total_workers as i32);
    window.set_inn_count(settings.inn_count as i32);
    window.set_stone_quarry_count(settings.stone_quarry_count as i32);
    window.set_iron_mine_count(settings.iron_mine_count as i32);
    window.set_tax_index(i32::from(settings.tax_index));
    window.set_food_ratio_index(i32::from(settings.food_ratio_index));
    window.set_tax_label(
        format!(
            "Popularity {:+} | coefficient {:.1}",
            report.tax.popularity, report.tax.coefficient
        )
        .into(),
    );
    window.set_food_ratio_label(
        format!(
            "{} | popularity {:+} | {:.1}x food",
            report.food_ratio.name, report.food_ratio.popularity, report.food_ratio.multiplier
        )
        .into(),
    );
    window.set_total_popularity(report.total_popularity);
    window.set_popularity_good(report.total_popularity >= 0);
    window.set_population_worker_summary(build_population_worker_summary(&report).into());
    window.set_population_economy_summary(build_combined_economy_summary(state, &report).into());
}

fn build_combined_economy_summary(
    state: &EditorState,
    report: &simulator::PopulationEconomyReport,
) -> String {
    let mut sections = vec![
        build_eco_setup_summary(state),
        build_workshop_count_summary(state),
        build_bread_count_summary(state),
        build_armoury_summary(state),
    ];

    if report.settings.enabled {
        sections.push(build_population_worker_summary(report));
        sections.push(build_population_economy_summary(report));
    } else {
        sections.push(build_additional_resources_summary(report));
        sections.push(
            "Population economy is not counted. Population, food consumption, tax, and inns are excluded."
                .to_string(),
        );
    }

    sections.join("\n\n")
}

fn build_population_economy_report(state: &EditorState) -> simulator::PopulationEconomyReport {
    let bread = state
        .simulator()
        .calculate_bread_economy(state.simulation_settings());
    let population_settings = state.population_economy_settings();
    let iron_stockpile_available =
        state.simulator().buildings().iter().any(|building| {
            building.stockpile_resource == Some(simulator::StockpileResource::Iron)
        });
    let iron = calculate_layout_iron_economy(state);
    let workshop_gold_per_minute = state
        .cycle_rows()
        .iter()
        .filter_map(|row| {
            let total_ticks = row.total_ticks?;
            let scale = if row.iron_per_cycle > 0 {
                iron.workshop_output_scale
            } else {
                1.0
            };
            Some(net_gold_per_minute(row, state, total_ticks) * scale)
        })
        .sum::<f64>();
    let food_gold_per_minute = bread_economy_gold_per_minute(&bread);

    simulator::calculate_population_economy(
        population_settings,
        simulator::PopulationEconomyContext {
            game_speed_ticks_per_second: state.game_speed(),
            fear_factor: state.fear_factor(),
            placed_workers: placed_economy_workers(state),
            food_produced_per_minute: bread.bread_per_minute,
            food_sell_gold_per_unit: BREAD_SELL_GOLD,
            layout_gold_per_minute: workshop_gold_per_minute + food_gold_per_minute,
            workshop_gold_per_minute,
            food_gold_per_minute,
            workshop_iron_demand_per_minute: iron.required_per_minute,
            workshops_buy_iron: state.buy_iron(),
            iron_stockpile_available,
        },
    )
}

fn placed_economy_workers(state: &EditorState) -> u32 {
    state
        .simulator()
        .buildings()
        .iter()
        .map(|building| match building.building_type {
            BuildingType::Windmill => 3,
            BuildingType::WheatFarm
            | BuildingType::Bakery
            | BuildingType::FletchersWorkshop
            | BuildingType::BlacksmithsWorkshop
            | BuildingType::PoleturnersWorkshop
            | BuildingType::ArmourersWorkshop => 1,
            _ => 0,
        })
        .sum()
}

fn build_population_worker_summary(report: &simulator::PopulationEconomyReport) -> String {
    let available = report
        .settings
        .population
        .saturating_sub(report.total_workers);
    let shortage = report
        .total_workers
        .saturating_sub(report.settings.population);
    let availability = if shortage > 0 {
        format!("Worker shortage: {shortage}")
    } else {
        format!("Free population: {available}")
    };

    format!(
        "Workers\nPlaced economy: {}\nAdditional buildings: {}\nTotal workers: {} / {} population\n{}",
        report.placed_workers,
        report.additional_workers,
        report.total_workers,
        report.settings.population,
        availability
    )
}

fn build_additional_resources_summary(report: &simulator::PopulationEconomyReport) -> String {
    let iron = if report.iron_stockpile_available {
        format!(
            "Iron produced: {:.2} / min\nWorkshop iron demand: {:.2} / min\nIron used by workshops: {:.2} / min\nIron bought (deficit): {:.2} / min\nIron surplus: {:.2} / min",
            report.iron_per_minute,
            report.workshop_iron_demand_per_minute,
            report.iron_used_per_minute,
            report.iron_bought_per_minute,
            report.iron_surplus_per_minute,
        )
    } else {
        "Iron mines inactive: designate an Iron Stock on the map".to_string()
    };

    format!(
        "Additional resources\nStone produced: {:.2} / min\nStone sell gold: {:.2} / min\n{}\nIron value: {:.2} gold / min\nResource workers: {}\nResource setup: {} wood",
        report.stone_per_minute,
        report.stone_gold_per_minute,
        iron,
        report.iron_gold_benefit_per_minute,
        report.additional_workers,
        report.additional_build_wood,
    )
}

fn build_population_economy_summary(report: &simulator::PopulationEconomyReport) -> String {
    let iron_note = if report.iron_stockpile_available {
        format!(
            "Iron produced: {:.2} / min\nWorkshop iron demand: {:.2} / min\nIron used by workshops: {:.2} / min\nIron bought (deficit): {:.2} / min\nIron surplus: {:.2} / min",
            report.iron_per_minute,
            report.workshop_iron_demand_per_minute,
            report.iron_used_per_minute,
            report.iron_bought_per_minute,
            report.iron_surplus_per_minute,
        )
    } else {
        "Iron mines inactive: designate an Iron Stock on the map".to_string()
    };

    format!(
        "Popularity\nTax: {:+}\nFood: {:+}\nInns: {:+} ({:.1}% coverage)\nFear factor: {:+}\n\nFood\nRequired: {:.2} / min\nBread available: {:.2} / min\nBread consumed: {:.2} / min\nBread left to sell: {:.2} / min\nBalance: {:+.2} / min\n\nProduction\nStone: {:.2} / min\n{}\n\nGold per minute\nWorkshop: {:+.2}\nFood eaten: -{:.2}\nFood sell: {:+.2}\nInn beer: -{:.2}\nTax: {:+.2}\nStone: {:+.2}\nIron: {:+.2}\nTotal: {:+.2}\n\nAdditional setup: {} wood + {} gold",
        report.tax.popularity,
        report.food_ratio.popularity,
        report.inn_popularity,
        report.inn_coverage_percent,
        report.fear_popularity,
        report.food_required_per_minute,
        report.food_produced_per_minute,
        report.food_consumed_per_minute,
        report.food_sellable_per_minute,
        report.food_balance_per_minute,
        report.stone_per_minute,
        iron_note,
        report.workshop_gold_per_minute,
        report.food_sale_reduction_per_minute,
        report.food_gold_per_minute,
        report.inn_gold_per_minute,
        report.tax_gold_per_minute,
        report.stone_gold_per_minute,
        report.iron_gold_benefit_per_minute,
        report.total_gold_per_minute,
        report.additional_build_wood,
        report.additional_build_gold
    )
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

fn build_eco_setup_summary(state: &EditorState) -> String {
    let costs = eco_setup_costs_for_types(
        state
            .simulator()
            .buildings()
            .iter()
            .map(|building| building.building_type),
    );

    format!(
        "Eco setup cost: {} gold ({})\nWorkshop eco: {} gold ({})\nFood eco: {} gold ({})",
        costs.total_gold_with_bought_wood(),
        format_build_cost(costs.total_gold, costs.total_wood),
        costs.workshop_gold_with_bought_wood(),
        format_build_cost(costs.workshop_gold, costs.workshop_wood),
        costs.food_gold_with_bought_wood(),
        format_build_cost(costs.food_gold, costs.food_wood),
    )
}

#[derive(Debug, Default, PartialEq, Eq)]
struct EcoSetupCosts {
    workshop_gold: u32,
    workshop_wood: u32,
    food_gold: u32,
    food_wood: u32,
    total_gold: u32,
    total_wood: u32,
}

impl EcoSetupCosts {
    fn workshop_gold_with_bought_wood(&self) -> u32 {
        self.workshop_gold + self.workshop_wood * WOOD_BUY_GOLD
    }

    fn food_gold_with_bought_wood(&self) -> u32 {
        self.food_gold + self.food_wood * WOOD_BUY_GOLD
    }

    fn total_gold_with_bought_wood(&self) -> u32 {
        self.total_gold + self.total_wood * WOOD_BUY_GOLD
    }
}

fn eco_setup_costs_for_types(
    building_types: impl IntoIterator<Item = BuildingType>,
) -> EcoSetupCosts {
    let mut costs = EcoSetupCosts::default();

    for building_type in building_types {
        let cost = building_type.build_cost();
        costs.total_gold += cost.gold;
        costs.total_wood += cost.wood;

        if matches!(
            building_type,
            BuildingType::WheatFarm
                | BuildingType::Windmill
                | BuildingType::Bakery
                | BuildingType::Granary
        ) {
            costs.food_gold += cost.gold;
            costs.food_wood += cost.wood;
        } else {
            costs.workshop_gold += cost.gold;
            costs.workshop_wood += cost.wood;
        }
    }

    costs
}

fn format_build_cost(gold: u32, wood: u32) -> String {
    match (gold, wood) {
        (0, wood) => format!("{wood} wood"),
        (gold, 0) => format!("{gold} gold"),
        (gold, wood) => format!("{gold} gold + {wood} wood"),
    }
}

fn build_workshop_count_summary(state: &EditorState) -> String {
    let count = |building_type| {
        state
            .simulator()
            .buildings()
            .iter()
            .filter(|building| building.building_type == building_type)
            .count()
    };

    format!(
        "Workshop count\nFletchers: {}\nPoleturners: {}\nBlacksmiths: {}\nArmourers: {}\nArmouries: {}",
        count(BuildingType::FletchersWorkshop),
        count(BuildingType::PoleturnersWorkshop),
        count(BuildingType::BlacksmithsWorkshop),
        count(BuildingType::ArmourersWorkshop),
        count(BuildingType::Armoury)
    )
}

fn build_bread_count_summary(state: &EditorState) -> String {
    let count = |building_type| {
        state
            .simulator()
            .buildings()
            .iter()
            .filter(|building| building.building_type == building_type)
            .count()
    };

    format!(
        "Bread economy\nWheat Farms: {}\nWind Mills: {}\nBakeries: {}\nGranaries: {}",
        count(BuildingType::WheatFarm),
        count(BuildingType::Windmill),
        count(BuildingType::Bakery),
        count(BuildingType::Granary)
    )
}

fn build_armoury_summary(state: &EditorState) -> String {
    let armouries = state
        .simulator()
        .buildings()
        .iter()
        .filter(|building| building.building_type == BuildingType::Armoury)
        .collect::<Vec<_>>();

    let armoury_text = if armouries.is_empty() {
        "Armoury production\nNo armoury placed".to_string()
    } else {
        let mut sections = Vec::with_capacity(armouries.len());
        for armoury in armouries {
            let (title, _, lines) = build_armoury_hover_info(state, armoury.id);
            sections.push(format!("{}\n{}", title, lines.join("\n")));
        }
        format!("Armoury production\n{}", sections.join("\n\n"))
    };

    format!(
        "{}\n\n{}\n\n{}",
        armoury_text,
        build_iron_armoury_summary(state),
        build_bread_summary(state)
    )
}

fn build_iron_armoury_summary(state: &EditorState) -> String {
    let iron = calculate_layout_iron_economy(state);
    let workshop_gold_per_minute = state
        .cycle_rows()
        .iter()
        .filter_map(|row| {
            let total_ticks = row.total_ticks?;
            Some(effective_workshop_gold_per_minute(
                row,
                state,
                total_ticks,
                iron,
            ))
        })
        .sum::<f64>();
    let total_with_surplus = workshop_gold_per_minute + iron.surplus_sale_gold_per_minute;

    format!(
        "Iron supply (buy 45 / sell 23)\nProduced: {:.2} / min\nRequired at full workshop output: {:.2} / min\nUsed by workshops: {:.2} / min\nBought deficit: {:.2} / min (-{:.2} gold)\nSold surplus: {:.2} / min (+{:.2} gold)\nIron workshop output: {:.1}%\nWorkshop gold after iron: {:.2} / min\nWorkshop + iron surplus: {:.2} / min",
        iron.produced_per_minute,
        iron.required_per_minute,
        iron.used_per_minute,
        iron.bought_per_minute,
        iron.bought_per_minute * f64::from(simulator::IRON_BUY_GOLD),
        iron.surplus_per_minute,
        iron.surplus_sale_gold_per_minute,
        iron.workshop_output_scale * 100.0,
        workshop_gold_per_minute,
        total_with_surplus,
    )
}

fn build_bread_summary(state: &EditorState) -> String {
    let report = state
        .simulator()
        .calculate_bread_economy(state.simulation_settings());
    let mut lines = vec!["Bread production".to_string()];
    lines.extend(bread_economy_lines(&report));

    if !report.issues.is_empty() {
        lines.push(String::new());
        lines.push("Issues:".to_string());
        lines.extend(report.issues);
    }

    lines.join("\n")
}

fn bread_economy_lines(report: &simulator::BreadEconomyReport) -> Vec<String> {
    let mut lines = vec![
        format!("Wheat produced / min: {:.2}", report.wheat_per_minute),
        format!(
            "Wheat surplus / min: {:.2}",
            report.surplus_wheat_per_minute
        ),
    ];
    if report.purchased_wheat_per_minute > 0.0 {
        lines.push(format!(
            "Wheat bought / min: {:.2}",
            report.purchased_wheat_per_minute
        ));
    }
    lines.extend([
        format!("Flour produced / min: {:.2}", report.flour_per_minute),
        format!(
            "Flour surplus / min: {:.2}",
            report.surplus_flour_per_minute
        ),
    ]);
    if report.purchased_flour_per_minute > 0.0 {
        lines.push(format!(
            "Flour bought / min: {:.2}",
            report.purchased_flour_per_minute
        ));
    }
    lines.extend([
        format!("Bread produced / min: {:.2}", report.bread_per_minute),
        format!(
            "Bread sell gold / min: {:.2}",
            report.bread_per_minute * BREAD_SELL_GOLD
        ),
    ]);
    let buy_gold = bread_input_buy_gold_per_minute(report);
    if buy_gold > 0.0 {
        lines.push(format!("Input buy gold / min: {:.2}", buy_gold));
    }
    lines.push(format!(
        "Total gold / min: {:.2}",
        bread_economy_gold_per_minute(report)
    ));
    lines.push(format!("Bottleneck: {}", report.limiting_stage));

    lines
}

fn net_gold_per_cycle(row: &crate::backend::CycleSimulationRow, state: &EditorState) -> f64 {
    let recipe = row.weapon_type.recipe();
    let gross_gold = row.average_weapons_per_cycle * recipe.sell_gold as f64;
    let bought_resource_gold = state
        .simulation_settings()
        .resource_buy_gold_per_cycle(recipe) as f64;

    gross_gold - bought_resource_gold
}

fn net_gold_per_minute(
    row: &crate::backend::CycleSimulationRow,
    state: &EditorState,
    total_ticks: u64,
) -> f64 {
    net_gold_per_cycle(row, state) / total_ticks as f64 * state.game_speed() as f64 * 60.0
}

fn calculate_layout_iron_economy(state: &EditorState) -> simulator::IronEconomy {
    let iron_stockpile_available =
        state.simulator().buildings().iter().any(|building| {
            building.stockpile_resource == Some(simulator::StockpileResource::Iron)
        });
    let produced_per_minute = if iron_stockpile_available {
        let fear_multiplier = 1.0 + f64::from(state.fear_factor().unsigned_abs()) / 5.0 * 0.33;
        f64::from(state.population_economy_settings().iron_mine_count)
            * simulator::IRON_PER_MINUTE
            * fear_multiplier
    } else {
        0.0
    };
    let required_per_minute = state
        .cycle_rows()
        .iter()
        .filter_map(|row| {
            let total_ticks = row.total_ticks?;
            Some(iron_required_per_minute(row, state, total_ticks))
        })
        .sum();

    simulator::calculate_iron_economy(produced_per_minute, required_per_minute, state.buy_iron())
}

fn iron_required_per_minute(
    row: &crate::backend::CycleSimulationRow,
    state: &EditorState,
    total_ticks: u64,
) -> f64 {
    f64::from(row.iron_per_cycle) / total_ticks as f64 * f64::from(state.game_speed()) * 60.0
}

fn effective_workshop_output_scale(
    row: &crate::backend::CycleSimulationRow,
    iron: simulator::IronEconomy,
) -> f64 {
    if row.iron_per_cycle == 0 {
        1.0
    } else {
        iron.workshop_output_scale
    }
}

fn effective_workshop_gold_per_minute(
    row: &crate::backend::CycleSimulationRow,
    state: &EditorState,
    total_ticks: u64,
    iron: simulator::IronEconomy,
) -> f64 {
    let base_gold = net_gold_per_minute(row, state, total_ticks);
    if row.iron_per_cycle == 0 {
        return base_gold;
    }

    if state.buy_iron() {
        let produced_coverage = if iron.required_per_minute == 0.0 {
            0.0
        } else {
            iron.used_per_minute / iron.required_per_minute
        };
        let supplied_per_minute =
            iron_required_per_minute(row, state, total_ticks) * produced_coverage;
        base_gold + supplied_per_minute * f64::from(simulator::IRON_BUY_GOLD)
    } else {
        base_gold * iron.workshop_output_scale
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
        simulator::BuildingType::WheatFarm
        | simulator::BuildingType::Windmill
        | simulator::BuildingType::Bakery
        | simulator::BuildingType::Granary => build_bread_hover_info(state, building),
        simulator::BuildingType::Stockpile => {
            build_stockpile_hover_info(state, building.id, building.stockpile_resource)
        }
        _ => (String::new(), String::new(), Vec::new()),
    }
}

fn build_bread_hover_info(
    state: &EditorState,
    building: &simulator::BuildingPlacement,
) -> (String, String, Vec<String>) {
    let report = state
        .simulator()
        .calculate_bread_economy(state.simulation_settings());
    let lines = match building.building_type {
        simulator::BuildingType::WheatFarm => report
            .farm_rates
            .iter()
            .find(|rate| rate.building_id == building.id)
            .map_or_else(
                || vec!["No reachable Wheat stockpile route".to_string()],
                |rate| goods_output_lines("Wheat", rate.actual_per_minute, WHEAT_SELL_GOLD),
            ),
        simulator::BuildingType::Windmill => report
            .mill_rates
            .iter()
            .find(|rate| rate.building_id == building.id)
            .map_or_else(
                || vec!["No reachable Wheat/Flour stockpile route".to_string()],
                |rate| goods_output_lines("Flour", rate.actual_per_minute, FLOUR_SELL_GOLD),
            ),
        simulator::BuildingType::Bakery => report
            .bakery_rates
            .iter()
            .find(|rate| rate.building_id == building.id)
            .map_or_else(
                || vec!["No reachable Flour stockpile and Granary route".to_string()],
                |rate| {
                    goods_output_lines(
                        "Bread",
                        rate.actual_per_minute * report.bread_per_flour,
                        BREAD_SELL_GOLD,
                    )
                },
            ),
        simulator::BuildingType::Granary => bread_economy_lines(&report),
        _ => Vec::new(),
    };

    (
        format!("#{} {}", building.id, building.building_type.display_name()),
        String::new(),
        lines,
    )
}

fn goods_output_lines(goods_name: &str, actual_per_minute: f64, sell_gold: f64) -> Vec<String> {
    vec![
        format!("{} produced / min: {:.2}", goods_name, actual_per_minute),
        format!(
            "{} sell gold / min: {:.2}",
            goods_name,
            actual_per_minute * sell_gold
        ),
    ]
}

fn bread_economy_gold_per_minute(report: &simulator::BreadEconomyReport) -> f64 {
    report.surplus_wheat_per_minute * WHEAT_SELL_GOLD
        + report.surplus_flour_per_minute * FLOUR_SELL_GOLD
        + report.bread_per_minute * BREAD_SELL_GOLD
        - bread_input_buy_gold_per_minute(report)
}

fn bread_input_buy_gold_per_minute(report: &simulator::BreadEconomyReport) -> f64 {
    report.purchased_wheat_per_minute * WHEAT_BUY_GOLD
        + report.purchased_flour_per_minute * FLOUR_BUY_GOLD
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
    let iron = calculate_layout_iron_economy(state);
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

            lines.push(format!(
                "Average output / cycle: {:.2}",
                row.average_weapons_per_cycle
            ));
            let output_scale = effective_workshop_output_scale(row, iron);
            let weapons_per_tick =
                row.average_weapons_per_cycle / total_ticks as f64 * output_scale;
            lines.push(format!(
                "Output / tick: {}",
                format_rate_tick(weapons_per_tick)
            ));
            lines.push(format!(
                "Output / min: {}",
                format_rate_minute(weapons_per_tick * state.game_speed() as f64 * 60.0)
            ));
            lines.push(format!(
                "Net gold / min: {}",
                format_rate_minute(effective_workshop_gold_per_minute(
                    row,
                    state,
                    total_ticks,
                    iron,
                ))
            ));
            if row.iron_per_cycle > 0 {
                lines.push(format!("Iron supply: {:.1}%", output_scale * 100.0));
            }
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
                simulator::StockpileResource::Wheat | simulator::StockpileResource::Flour => 0,
            };
            if amount == 0 {
                return None;
            }
            Some(amount as f64 / total_ticks as f64)
        })
        .sum::<f64>();

    let total_per_tick = match resource {
        simulator::StockpileResource::Wheat => {
            let report = state
                .simulator()
                .calculate_bread_economy(state.simulation_settings());
            report.flour_per_minute / (state.game_speed() as f64 * 60.0)
        }
        simulator::StockpileResource::Flour => {
            let report = state
                .simulator()
                .calculate_bread_economy(state.simulation_settings());
            if report.bread_per_flour == 0.0 {
                0.0
            } else {
                report.bread_per_minute
                    / report.bread_per_flour
                    / (state.game_speed() as f64 * 60.0)
            }
        }
        _ => total_per_tick,
    };

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
    let mut total_gold_per_minute = 0.0;
    let iron = calculate_layout_iron_economy(state);

    for row in state
        .cycle_rows()
        .iter()
        .filter(|row| row.armoury_id == Some(armoury_id))
    {
        let Some(total_ticks) = row.total_ticks else {
            continue;
        };

        let output_scale = effective_workshop_output_scale(row, iron);
        let per_tick = row.average_weapons_per_cycle / total_ticks as f64 * output_scale;
        let gold_per_minute = effective_workshop_gold_per_minute(row, state, total_ticks, iron);
        total_gold_per_minute += gold_per_minute;
        let key = row.weapon_type.display_name().to_string();
        let totals = weapon_totals.entry(key).or_insert((0.0, 0.0));
        totals.0 += per_tick;
        totals.1 += gold_per_minute;
    }

    let mut lines = Vec::new();

    if weapon_totals.is_empty() {
        lines.push("No completed workshop cycles are routed here".to_string());
    } else {
        lines.push(format!(
            "Total gold / min: {}",
            format_rate_minute(total_gold_per_minute)
        ));

        for (weapon_name, (per_tick, gold_per_minute)) in weapon_totals.into_iter() {
            if !lines.is_empty() {
                lines.push(String::new());
            }

            lines.push(format!(
                "{} output / min: {}",
                weapon_name,
                format_rate_minute(per_tick * state.game_speed() as f64 * 60.0)
            ));
            lines.push(format!(
                "{} gold / min: {}",
                weapon_name,
                format_rate_minute(gold_per_minute)
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

#[cfg(test)]
mod tests {
    use simulator::{BuildingType, Simulator, StockpileResource, WeaponType};

    use crate::{backend::CycleSimulationRow, editor_state::EditorState};

    use super::build_population_economy_summary;
    use super::{
        calculate_layout_iron_economy, eco_setup_costs_for_types,
        effective_workshop_gold_per_minute, format_build_cost,
    };

    #[test]
    fn eco_setup_cost_splits_workshop_and_food_and_always_prices_build_wood() {
        let building_types = std::iter::repeat_n(BuildingType::FletchersWorkshop, 12)
            .chain(std::iter::repeat_n(BuildingType::PoleturnersWorkshop, 17))
            .chain(std::iter::once(BuildingType::Armoury))
            .chain(std::iter::repeat_n(BuildingType::WheatFarm, 3))
            .chain(std::iter::repeat_n(BuildingType::Windmill, 3))
            .chain(std::iter::repeat_n(BuildingType::Bakery, 22))
            .chain(std::iter::once(BuildingType::Granary));
        let costs = eco_setup_costs_for_types(building_types);

        assert_eq!(costs.workshop_gold, 2_900);
        assert_eq!(costs.workshop_wood, 415);
        assert_eq!(costs.food_gold, 0);
        assert_eq!(costs.food_wood, 330);
        assert_eq!(costs.workshop_gold_with_bought_wood(), 4_560);
        assert_eq!(costs.food_gold_with_bought_wood(), 1_320);
        assert_eq!(costs.total_gold_with_bought_wood(), 5_880);
        assert_eq!(format_build_cost(2_900, 415), "2900 gold + 415 wood");
        assert_eq!(format_build_cost(0, 330), "330 wood");
    }

    fn iron_workshop_state(buy_iron: bool) -> (EditorState, CycleSimulationRow) {
        let mut simulator = Simulator::new(40).expect("simulator should be created");
        simulator
            .place_building(BuildingType::GoodsYard, 10, 10)
            .expect("goods yard should be placed");
        simulator
            .set_stockpile_resource_at(10, 10, StockpileResource::Iron)
            .expect("iron stock should be assigned");

        let row = CycleSimulationRow {
            workshop_id: 1,
            workshop_name: "Blacksmith".to_string(),
            weapon_type: WeaponType::Sword,
            armoury_id: Some(2),
            total_ticks: Some(1_000),
            travel_ticks: Some(0),
            make_ticks: Some(1_000),
            average_weapons_per_cycle: 1.0,
            wood_per_cycle: 0,
            iron_per_cycle: 1,
            error: None,
        };

        let mut state = EditorState::new().expect("editor state should be created");
        state.set_simulator(simulator);
        state.set_iron_mine_count(1);
        state.set_buy_iron(buy_iron);
        state.set_cycle_rows(vec![row.clone()]);
        (state, row)
    }

    #[test]
    fn armoury_output_is_capped_by_produced_iron_when_buying_is_disabled() {
        let (state, row) = iron_workshop_state(false);
        let iron = calculate_layout_iron_economy(&state);
        let gold = effective_workshop_gold_per_minute(&row, &state, 1_000, iron);

        assert!((iron.produced_per_minute - 2.63).abs() < 0.000_001);
        assert!((iron.required_per_minute - 3.0).abs() < 0.000_001);
        assert!((iron.workshop_output_scale - 2.63 / 3.0).abs() < 0.000_001);
        assert!((gold - 78.9).abs() < 0.000_001);
    }

    #[test]
    fn armoury_buys_only_the_iron_deficit_when_buying_is_enabled() {
        let (state, row) = iron_workshop_state(true);
        let iron = calculate_layout_iron_economy(&state);
        let gold = effective_workshop_gold_per_minute(&row, &state, 1_000, iron);

        assert!((iron.bought_per_minute - 0.37).abs() < 0.000_001);
        assert_eq!(iron.workshop_output_scale, 1.0);
        assert!((gold - 73.35).abs() < 0.000_001);
    }

    #[test]
    fn population_gold_summary_is_an_additive_ledger() {
        let report = simulator::calculate_population_economy(
            simulator::PopulationEconomySettings {
                enabled: true,
                population: 10,
                inn_count: 1,
                ..simulator::PopulationEconomySettings::default()
            },
            simulator::PopulationEconomyContext {
                food_produced_per_minute: 20.0,
                food_sell_gold_per_unit: 4.0,
                layout_gold_per_minute: 300.0,
                workshop_gold_per_minute: 200.0,
                food_gold_per_minute: 100.0,
                ..simulator::PopulationEconomyContext::default()
            },
        );
        let summary = build_population_economy_summary(&report);

        assert!(summary.contains("Workshop: +200.00"));
        assert!(summary.contains("Food eaten: -24.00"));
        assert!(summary.contains("Food sell: +100.00"));
        assert!(summary.contains("Inn beer: -11.20"));
        assert!(summary.contains("Stone: +0.00"));
        assert!(summary.contains("Total: +264.80"));
    }
}
