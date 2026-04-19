use std::{sync::mpsc, thread};

use simulator::{
    BuildingType, ProductionCycle, RemoveOutcome, SimulationSettings, Simulator, StockpileResource,
    WeaponType,
};

#[derive(Debug)]
pub enum BackendCommand {
    PlaceBuilding {
        building_type: BuildingType,
        x: usize,
        y: usize,
    },
    PlaceWall {
        start: (usize, usize),
        end: (usize, usize),
    },
    RemoveAt {
        x: usize,
        y: usize,
    },
    RemoveAllWalls,
    SetStockpileResource {
        x: usize,
        y: usize,
        resource: StockpileResource,
    },
    RunCycleSimulation {
        settings: SimulationSettings,
    },
}

#[derive(Debug, Clone)]
pub struct CycleSimulationRow {
    pub workshop_id: u32,
    pub workshop_name: String,
    pub weapon_type: WeaponType,
    pub armoury_id: Option<u32>,
    pub total_ticks: Option<u64>,
    pub travel_ticks: Option<u64>,
    pub make_ticks: Option<u64>,
    pub average_weapons_per_cycle: f64,
    pub wood_per_cycle: u32,
    pub iron_per_cycle: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackendUpdate {
    pub simulator: Simulator,
    pub message: String,
    pub cycle_rows: Option<Vec<CycleSimulationRow>>,
}

#[derive(Clone)]
pub struct BackendHandle {
    sender: mpsc::Sender<BackendCommand>,
}

impl BackendHandle {
    pub fn spawn(
        map_size: usize,
        on_update: impl Fn(BackendUpdate) + Send + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("simulator-backend".to_string())
            .spawn(move || run_backend(map_size, receiver, on_update))?;

        Ok(Self { sender })
    }

    pub fn send(&self, command: BackendCommand) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "backend thread is not available".to_string())
    }
}

fn run_backend(
    map_size: usize,
    receiver: mpsc::Receiver<BackendCommand>,
    on_update: impl Fn(BackendUpdate),
) {
    let mut simulator = match Simulator::new(map_size) {
        Ok(simulator) => simulator,
        Err(error) => {
            eprintln!("failed to start simulator backend: {error}");
            return;
        }
    };

    for command in receiver {
        let (message, cycle_rows) = match command {
            BackendCommand::PlaceBuilding {
                building_type,
                x,
                y,
            } => (
                match simulator.place_building(building_type, x, y) {
                    Ok(id) => format!(
                        "Placed {} #{} at ({}, {})",
                        building_type.display_name(),
                        id,
                        x,
                        y
                    ),
                    Err(error) => format!("Placement failed: {}", error),
                },
                None,
            ),
            BackendCommand::PlaceWall { start, end } => (
                match simulator.place_wall(start.0, start.1, end.0, end.1) {
                    Ok(id) => format!(
                        "Placed Wall #{} from ({}, {}) to ({}, {})",
                        id, start.0, start.1, end.0, end.1
                    ),
                    Err(error) => format!("Placement failed: {}", error),
                },
                None,
            ),
            BackendCommand::RemoveAt { x, y } => (
                match simulator.remove_at(x, y) {
                    RemoveOutcome::None => "Nothing to remove at this cell".to_string(),
                    RemoveOutcome::Wall { id } => format!("Removed Wall #{}", id),
                    RemoveOutcome::Buildings {
                        removed_ids,
                        goods_yard_group_id,
                    } => {
                        if removed_ids.is_empty() {
                            "Nothing removed".to_string()
                        } else if let Some(group_id) = goods_yard_group_id {
                            format!(
                                "Removed Goods Yard group #{} ({} stockpiles)",
                                group_id,
                                removed_ids.len()
                            )
                        } else {
                            format!("Removed building #{}", removed_ids[0])
                        }
                    }
                },
                None,
            ),
            BackendCommand::RemoveAllWalls => (
                {
                    let removed = simulator.remove_all_walls();
                    if removed == 0 {
                        "No walls to remove".to_string()
                    } else {
                        format!("Removed {} wall segment(s)", removed)
                    }
                },
                None,
            ),
            BackendCommand::SetStockpileResource { x, y, resource } => (
                match simulator.set_stockpile_resource_at(x, y, resource) {
                    Ok(id) => format!("Marked stockpile #{} as {}", id, resource.display_name()),
                    Err(error) => format!("Placement failed: {}", error),
                },
                None,
            ),
            BackendCommand::RunCycleSimulation { settings } => {
                simulator.calculate_worker_distances();
                let cycle_rows = build_cycle_simulation_rows(&simulator, settings);
                let success_count = cycle_rows
                    .iter()
                    .filter(|row| row.total_ticks.is_some())
                    .count();
                (
                    format!(
                        "Calculated {} workshop cycle(s), {} ready",
                        cycle_rows.len(),
                        success_count
                    ),
                    Some(cycle_rows),
                )
            }
        };

        on_update(BackendUpdate {
            simulator: simulator.clone(),
            message,
            cycle_rows,
        });
    }
}

fn build_cycle_simulation_rows(
    simulator: &Simulator,
    settings: SimulationSettings,
) -> Vec<CycleSimulationRow> {
    let armoury_ids = simulator
        .buildings()
        .iter()
        .filter(|building| building.building_type == BuildingType::Armoury)
        .map(|building| building.id)
        .collect::<Vec<_>>();

    let mut rows = Vec::new();

    for workshop in simulator.buildings().iter().filter(|building| {
        matches!(
            building.building_type,
            BuildingType::FletchersWorkshop
                | BuildingType::BlacksmithsWorkshop
                | BuildingType::PoleturnersWorkshop
                | BuildingType::ArmourersWorkshop
        )
    }) {
        let Some(weapon_type) = settings.selected_weapon_for(workshop.building_type) else {
            continue;
        };

        rows.push(build_cycle_row(
            simulator,
            workshop.id,
            workshop.building_type,
            &armoury_ids,
            weapon_type,
            settings,
        ));
    }

    rows
}

fn build_cycle_row(
    simulator: &Simulator,
    workshop_id: u32,
    workshop_type: BuildingType,
    armoury_ids: &[u32],
    weapon_type: WeaponType,
    settings: SimulationSettings,
) -> CycleSimulationRow {
    let workshop_name = format!("#{} {}", workshop_id, workshop_type.display_name());
    let recipe = weapon_type.recipe();
    let average_weapons_per_cycle = settings.average_weapon_output_per_cycle(workshop_type);

    if armoury_ids.is_empty() {
        return CycleSimulationRow {
            workshop_id,
            workshop_name,
            weapon_type,
            armoury_id: None,
            total_ticks: None,
            travel_ticks: None,
            make_ticks: None,
            average_weapons_per_cycle,
            wood_per_cycle: recipe.wood_required,
            iron_per_cycle: recipe.iron_required,
            error: Some("No armoury placed".to_string()),
        };
    }

    let mut best_cycle: Option<ProductionCycle> = None;
    let mut first_error = None;

    for armoury_id in armoury_ids {
        match simulator.calculate_production_cycle(weapon_type, workshop_id, *armoury_id, settings)
        {
            Ok(cycle) => {
                let should_replace = best_cycle
                    .as_ref()
                    .is_none_or(|current| cycle.total_ticks < current.total_ticks);
                if should_replace {
                    best_cycle = Some(cycle);
                }
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error.to_string());
                }
            }
        }
    }

    if let Some(cycle) = best_cycle {
        return CycleSimulationRow {
            workshop_id,
            workshop_name,
            weapon_type,
            armoury_id: Some(cycle.armoury_id),
            total_ticks: Some(cycle.total_ticks),
            travel_ticks: Some(cycle.travel_ticks),
            make_ticks: Some(cycle.make_ticks),
            average_weapons_per_cycle,
            wood_per_cycle: recipe.wood_required,
            iron_per_cycle: recipe.iron_required,
            error: None,
        };
    }

    CycleSimulationRow {
        workshop_id,
        workshop_name,
        weapon_type,
        armoury_id: None,
        total_ticks: None,
        travel_ticks: None,
        make_ticks: None,
        average_weapons_per_cycle,
        wood_per_cycle: recipe.wood_required,
        iron_per_cycle: recipe.iron_required,
        error: Some(first_error.unwrap_or_else(|| "No reachable cycle".to_string())),
    }
}
