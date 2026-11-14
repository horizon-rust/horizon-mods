pub mod material_list;

pub use material_list::{MaterialEntry, MaterialList};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use malilib::util::BlockPos;
    use crate::schematic::{BlockState, LitematicaSchematic, SubRegion};

    #[test]
    fn test_material_list_calculation_and_inventory() {
        let mut schematic = LitematicaSchematic::new("Test", "Author");
        let mut region = SubRegion::new("Main", BlockPos::ORIGIN, BlockPos::new(2, 2, 2));

        let stone = BlockState::new("minecraft:stone");
        let torch = BlockState::new("minecraft:wall_torch");

        region.set_block_state(0, 0, 0, stone.clone());
        region.set_block_state(0, 0, 1, stone.clone());
        region.set_block_state(1, 0, 0, torch.clone());

        schematic.regions.insert("Main".to_string(), region);

        let mut mat_list = MaterialList::from_schematic(&schematic, None);

        assert_eq!(mat_list.total_items_required, 3);
        assert_eq!(mat_list.entries.get("minecraft:stone").unwrap().total_required, 2);
        assert_eq!(mat_list.entries.get("minecraft:torch").unwrap().total_required, 1);

        let mut inv = HashMap::new();
        inv.insert("minecraft:stone".to_string(), 1);

        mat_list.update_inventory(&inv);

        assert_eq!(mat_list.entries.get("minecraft:stone").unwrap().missing_count, 1);
        assert_eq!(mat_list.entries.get("minecraft:torch").unwrap().missing_count, 1);
        assert_eq!(mat_list.total_items_missing, 2);
    }
}
