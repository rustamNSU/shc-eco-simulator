use std::collections::HashMap;

use crate::buildings::{BuildingPlacement, BuildingType, StockpileResource};

use super::{BuildingDistance, DistanceKey};

pub(crate) fn build_worker_distances(
    buildings: &[BuildingPlacement],
    distances: &HashMap<DistanceKey, BuildingDistance>,
) -> HashMap<DistanceKey, BuildingDistance> {
    let mut worker_distances = HashMap::new();

    let wood_stockpile_ids = buildings
        .iter()
        .filter(|building| building.stockpile_resource == Some(StockpileResource::Wood))
        .map(|building| building.id)
        .collect::<Vec<_>>();
    let iron_stockpile_ids = buildings
        .iter()
        .filter(|building| building.stockpile_resource == Some(StockpileResource::Iron))
        .map(|building| building.id)
        .collect::<Vec<_>>();
    let armoury_ids = buildings
        .iter()
        .filter(|building| building.building_type == BuildingType::Armoury)
        .map(|building| building.id)
        .collect::<Vec<_>>();

    for workshop in buildings
        .iter()
        .filter(|building| is_workshop(building.building_type))
    {
        let stockpile_ids = match workshop.building_type {
            BuildingType::FletchersWorkshop | BuildingType::PoleturnersWorkshop => {
                &wood_stockpile_ids
            }
            BuildingType::BlacksmithsWorkshop | BuildingType::ArmourersWorkshop => {
                &iron_stockpile_ids
            }
            _ => continue,
        };

        for stockpile_id in stockpile_ids {
            insert_distance(&mut worker_distances, distances, workshop.id, *stockpile_id);
            insert_distance(&mut worker_distances, distances, *stockpile_id, workshop.id);
        }

        for armoury_id in &armoury_ids {
            insert_distance(&mut worker_distances, distances, workshop.id, *armoury_id);
            insert_distance(&mut worker_distances, distances, *armoury_id, workshop.id);
        }
    }

    for armoury_id in &armoury_ids {
        for stockpile_id in &wood_stockpile_ids {
            insert_distance(&mut worker_distances, distances, *armoury_id, *stockpile_id);
        }

        for stockpile_id in &iron_stockpile_ids {
            insert_distance(&mut worker_distances, distances, *armoury_id, *stockpile_id);
        }
    }

    worker_distances
}

fn insert_distance(
    out: &mut HashMap<DistanceKey, BuildingDistance>,
    all_distances: &HashMap<DistanceKey, BuildingDistance>,
    start_building_id: u32,
    finish_building_id: u32,
) {
    let key = DistanceKey::new(start_building_id, finish_building_id);
    if let Some(distance) = all_distances.get(&key) {
        out.insert(key, distance.clone());
    }
}

fn is_workshop(building_type: BuildingType) -> bool {
    matches!(
        building_type,
        BuildingType::FletchersWorkshop
            | BuildingType::BlacksmithsWorkshop
            | BuildingType::PoleturnersWorkshop
            | BuildingType::ArmourersWorkshop
    )
}
