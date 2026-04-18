use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow,
    backend::{BackendCommand, BackendHandle},
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
        move |simulator, message| {
            let backend_state = Arc::clone(&backend_state);
            let message = message;
            let _ = backend_window.upgrade_in_event_loop(move |window| {
                let mut state = backend_state
                    .lock()
                    .expect("editor state lock should not be poisoned");
                state.set_simulator(simulator);
                refresh_view(&window, &state, &message);
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
    let state_for_worker_distances = Arc::clone(&state);
    let backend_for_worker_distances = backend;
    window.on_calculate_worker_distances(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_worker_distances
                .lock()
                .expect("editor state lock should not be poisoned");
            state.clear_pending_wall();
            let message =
                match backend_for_worker_distances.send(BackendCommand::CalculateWorkerDistances) {
                    Ok(()) => "Processing...".to_string(),
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
}

fn refresh_static_view(window: &MainWindow, state: &EditorState) {
    let map_size = state.map_size() as i32;
    window.set_map_size(map_size);
    window.set_selected_building(state.selected_id().unwrap_or_default().into());

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
