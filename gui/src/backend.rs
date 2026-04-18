use std::{sync::mpsc, thread};

use simulator::{BuildingType, RemoveOutcome, Simulator, StockpileResource};

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
    CalculateWorkerDistances,
}

#[derive(Clone)]
pub struct BackendHandle {
    sender: mpsc::Sender<BackendCommand>,
}

impl BackendHandle {
    pub fn spawn(
        map_size: usize,
        on_update: impl Fn(Simulator, String) + Send + 'static,
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
    on_update: impl Fn(Simulator, String),
) {
    let mut simulator = match Simulator::new(map_size) {
        Ok(simulator) => simulator,
        Err(error) => {
            eprintln!("failed to start simulator backend: {error}");
            return;
        }
    };

    for command in receiver {
        let message = match command {
            BackendCommand::PlaceBuilding {
                building_type,
                x,
                y,
            } => match simulator.place_building(building_type, x, y) {
                Ok(id) => format!(
                    "Placed {} #{} at ({}, {})",
                    building_type.display_name(),
                    id,
                    x,
                    y
                ),
                Err(error) => format!("Placement failed: {}", error),
            },
            BackendCommand::PlaceWall { start, end } => {
                match simulator.place_wall(start.0, start.1, end.0, end.1) {
                    Ok(id) => format!(
                        "Placed Wall #{} from ({}, {}) to ({}, {})",
                        id, start.0, start.1, end.0, end.1
                    ),
                    Err(error) => format!("Placement failed: {}", error),
                }
            }
            BackendCommand::RemoveAt { x, y } => match simulator.remove_at(x, y) {
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
            BackendCommand::RemoveAllWalls => {
                let removed = simulator.remove_all_walls();
                if removed == 0 {
                    "No walls to remove".to_string()
                } else {
                    format!("Removed {} wall segment(s)", removed)
                }
            }
            BackendCommand::SetStockpileResource { x, y, resource } => {
                match simulator.set_stockpile_resource_at(x, y, resource) {
                    Ok(id) => format!("Marked stockpile #{} as {}", id, resource.display_name()),
                    Err(error) => format!("Placement failed: {}", error),
                }
            }
            BackendCommand::CalculateWorkerDistances => {
                let count = simulator.calculate_worker_distances();
                let reachable = simulator
                    .worker_distances()
                    .values()
                    .filter(|distance| distance.reachable())
                    .count();
                format!(
                    "Calculated {} worker distance(s), {} reachable",
                    count, reachable
                )
            }
        };

        on_update(simulator.clone(), message);
    }
}
