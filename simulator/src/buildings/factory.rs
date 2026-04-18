use super::{BuildingPlacement, BuildingType, Footprint};

#[derive(Debug, Default)]
pub struct BuildingFactory {
    next_id: u32,
    next_goods_yard_group_id: u32,
}

impl BuildingFactory {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            next_goods_yard_group_id: 1,
        }
    }

    pub fn create(&mut self, building_type: BuildingType, x: usize, y: usize) -> BuildingPlacement {
        let placement = BuildingPlacement {
            id: self.next_id,
            building_type,
            x,
            y,
            goods_yard_group_id: None,
            stockpile_resource: None,
            entry_point: None,
            footprint: Footprint::for_type(building_type),
            components: Vec::new(),
        };
        self.next_id += 1;
        placement
    }

    pub fn create_goods_yard_stacks(
        &mut self,
        x: usize,
        y: usize,
    ) -> (u32, Vec<BuildingPlacement>) {
        let group_id = self.next_goods_yard_group_id;
        self.next_goods_yard_group_id += 1;

        let mut create_stack = |stack_x: usize, stack_y: usize| {
            let placement = BuildingPlacement {
                id: self.next_id,
                building_type: BuildingType::Stockpile,
                x: stack_x,
                y: stack_y,
                goods_yard_group_id: Some(group_id),
                stockpile_resource: None,
                entry_point: None,
                footprint: Footprint::for_type(BuildingType::Stockpile),
                components: Vec::new(),
            };
            self.next_id += 1;
            placement
        };

        let stacks = vec![
            create_stack(x, y),
            create_stack(x + 3, y),
            create_stack(x, y + 3),
            create_stack(x + 3, y + 3),
        ];

        (group_id, stacks)
    }
}
