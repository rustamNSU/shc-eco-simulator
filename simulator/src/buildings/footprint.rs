use super::BuildingType;

#[derive(Debug, Clone)]
pub struct Footprint {
    width: usize,
    height: usize,
    occupied: Vec<bool>,
}

impl Footprint {
    pub fn square(size: usize) -> Self {
        Self {
            width: size,
            height: size,
            occupied: vec![true; size * size],
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
            occupied,
        }
    }

    pub fn for_type(building_type: BuildingType) -> Self {
        match building_type {
            BuildingType::GoodsYard => Self::goods_yard(),
            BuildingType::Armoury
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
}
