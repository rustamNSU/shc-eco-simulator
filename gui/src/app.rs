use std::{cell::RefCell, rc::Rc};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow,
    editor_state::{EditorState, PlacementOutcome},
    visuals,
};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    let state = Rc::new(RefCell::new(EditorState::new()?));

    refresh_view(&window, &state.borrow(), "Ready");

    let weak_window = window.as_weak();
    let state_for_select = Rc::clone(&state);
    window.on_select_building(move |tool_id| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_select.borrow_mut();
            let message = if state.set_selected_from_id(tool_id.as_str()) {
                format!("Selected: {}", state.selected_label())
            } else {
                format!("Unknown tool id: {}", tool_id)
            };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_place = Rc::clone(&state);
    window.on_place_at(move |x, y| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_place.borrow_mut();
            let cell_x = x.floor() as i32;
            let cell_y = y.floor() as i32;
            let message = match state.place_selected(x, y) {
                Ok(PlacementOutcome::Building { id, name }) => {
                    format!("Placed {} #{} at ({}, {})", name, id, cell_x, cell_y)
                }
                Ok(PlacementOutcome::WallStart { x, y }) => {
                    format!("Wall start set at ({}, {})", x, y)
                }
                Ok(PlacementOutcome::WallPlaced { id, start, end }) => {
                    format!(
                        "Placed Wall #{} from ({}, {}) to ({}, {})",
                        id, start.0, start.1, end.0, end.1
                    )
                }
                Err(error) => format!("Placement failed: {}", error),
            };
            refresh_view(&window, &state, &message);
        }
    });

    let weak_window = window.as_weak();
    let state_for_clear = Rc::clone(&state);
    window.on_clear_selection(move || {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_clear.borrow_mut();
            state.clear_selection();
            refresh_view(&window, &state, "Build mode disabled");
        }
    });

    let weak_window = window.as_weak();
    let state_for_hover = Rc::clone(&state);
    window.on_hover_at(move |x, y| {
        if let Some(window) = weak_window.upgrade() {
            let mut state = state_for_hover.borrow_mut();
            state.set_hover_cell(x, y);
            refresh_view(&window, &state, &window.get_status_text().to_string());
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

    window.run()?;
    Ok(())
}

fn refresh_view(window: &MainWindow, state: &EditorState, status: &str) {
    let map_size = state.map_size() as i32;
    window.set_map_size(map_size);
    window.set_selected_building(state.selected_id().unwrap_or_default().into());
    window.set_status_text(status.into());

    let cells = visuals::build_occupied_cells(state.simulator());
    let model = VecModel::from(cells);
    window.set_occupied_cells(ModelRc::new(model));

    let preview_cells = visuals::build_preview_cells(&state.preview_cells());
    let preview_model = VecModel::from(preview_cells);
    window.set_preview_cells(ModelRc::new(preview_model));

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
}
