use std::collections::{HashMap, VecDeque};

use crate::{EntryPoint, buildings::BuildingPlacement, map::CellMap};

use super::{BuildingDistance, DistanceKey};

pub(crate) fn recompute_building_distances(
    buildings: &[BuildingPlacement],
    map: &CellMap,
) -> HashMap<DistanceKey, BuildingDistance> {
    let mut distances = HashMap::new();

    for start in buildings {
        for finish in buildings {
            if start.id == finish.id {
                continue;
            }

            let key = DistanceKey::new(start.id, finish.id);
            let distance_cells = match (start.entry_point, finish.entry_point) {
                (Some(start_entry), Some(finish_entry)) => {
                    shortest_path_len(map, start_entry, finish_entry)
                }
                _ => None,
            };

            distances.insert(
                key,
                BuildingDistance {
                    key,
                    start_entry: start.entry_point,
                    finish_entry: finish.entry_point,
                    distance_cells,
                },
            );
        }
    }

    distances
}

pub(crate) fn shortest_path_len(
    map: &CellMap,
    start: EntryPoint,
    finish: EntryPoint,
) -> Option<u32> {
    if !map.is_in_bounds(start.x, start.y) || !map.is_in_bounds(finish.x, finish.y) {
        return None;
    }

    if start == finish {
        return Some(0);
    }

    if map.is_blocked(start.x, start.y) || map.is_blocked(finish.x, finish.y) {
        return None;
    }

    let width = map.width();
    let mut dist = vec![u32::MAX; width * map.height()];
    let mut queue = VecDeque::new();

    let start_idx = map.index(start.x, start.y)?;
    dist[start_idx] = 0;
    queue.push_back((start.x, start.y));

    while let Some((x, y)) = queue.pop_front() {
        let current_dist = dist[map.index(x, y)?];

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = x + dx;
                let ny = y + dy;
                if !map.is_in_bounds(nx, ny) || map.is_blocked(nx, ny) {
                    continue;
                }

                let next_idx = map.index(nx, ny)?;
                if dist[next_idx] != u32::MAX {
                    continue;
                }

                let next_dist = current_dist + 1;
                if nx == finish.x && ny == finish.y {
                    return Some(next_dist);
                }

                dist[next_idx] = next_dist;
                queue.push_back((nx, ny));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::{CellMap, EntryPoint};

    use super::shortest_path_len;

    #[test]
    fn same_start_and_finish_cell_has_zero_distance() {
        let map = CellMap::new(10);
        let point = EntryPoint { x: 1, y: 1 };
        assert_eq!(shortest_path_len(&map, point, point), Some(0));
    }
}
