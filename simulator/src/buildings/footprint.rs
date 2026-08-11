use super::BuildingType;

#[derive(Debug, Clone)]
pub struct Footprint {
    width: usize,
    height: usize,
    occupied: Vec<bool>,
    blocking: Vec<bool>,
}

impl Footprint {
    pub fn square(size: usize) -> Self {
        Self {
            width: size,
            height: size,
            occupied: vec![true; size * size],
            blocking: vec![true; size * size],
        }
    }

    pub fn goods_yard() -> Self {
        let mut occupied = vec![false; 5 * 5];

        for y in 0..5 {
            for x in 0..5 {
                let in_left = x <= 1;
                let in_right = x >= 3;
                let in_bottom = y <= 1;
                let in_top = y >= 3;
                let is_corner_stock = (in_left || in_right) && (in_bottom || in_top);
                occupied[(y * 5) + x] = is_corner_stock;
            }
        }

        Self {
            width: 5,
            height: 5,
            blocking: occupied.clone(),
            occupied,
        }
    }

    pub fn wheat_farm() -> Self {
        let mut blocking = vec![false; 9 * 9];
        for y in 6..9 {
            for x in 0..3 {
                blocking[(y * 9) + x] = true;
            }
        }

        Self {
            width: 9,
            height: 9,
            occupied: vec![true; 9 * 9],
            blocking,
        }
    }

    pub fn for_type(building_type: BuildingType) -> Self {
        match building_type {
            BuildingType::GoodsYard => Self::goods_yard(),
            BuildingType::Stockpile => Self::square(2),
            BuildingType::Windmill => Self::square(3),
            BuildingType::WheatFarm => Self::wheat_farm(),
            BuildingType::Armoury
            | BuildingType::Bakery
            | BuildingType::Granary
            | BuildingType::FletchersWorkshop
            | BuildingType::BlacksmithsWorkshop
            | BuildingType::PoleturnersWorkshop
            | BuildingType::ArmourersWorkshop => Self::square(4),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn occupied_offsets(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.occupied
            .iter()
            .enumerate()
            .filter_map(move |(index, is_used)| {
                if !*is_used {
                    return None;
                }
                let x = index % self.width;
                let y = index / self.width;
                Some((x, y))
            })
    }

    pub fn blocking_offsets(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.blocking
            .iter()
            .enumerate()
            .filter_map(move |(index, is_blocking)| {
                is_blocking.then_some((index % self.width, index / self.width))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::Footprint;

    #[test]
    fn wheat_farm_reserves_full_area_but_only_cabin_blocks_paths() {
        let footprint = Footprint::wheat_farm();
        let occupied = footprint.occupied_offsets().collect::<Vec<_>>();
        let blocking = footprint.blocking_offsets().collect::<Vec<_>>();

        assert_eq!(occupied.len(), 81);
        assert_eq!(blocking.len(), 9);
        assert!(blocking.contains(&(0, 8)));
        assert!(!blocking.contains(&(4, 3)));
    }
}
