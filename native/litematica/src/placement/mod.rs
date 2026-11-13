pub mod placement;
pub mod manager;

pub use placement::{SchematicPlacement, SubRegionPlacement};
pub use manager::PlacementManager;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use malilib::util::{BlockPos, BlockRotation};
    use crate::schematic::{BlockState, LitematicaSchematic, SubRegion};

    #[test]
    fn test_placement_rotation_and_world_coords() {
        let mut schematic = LitematicaSchematic::new("Test", "Author");
        let mut region = SubRegion::new("Main", BlockPos::ORIGIN, BlockPos::new(2, 2, 2));

        let stone = BlockState::new("minecraft:stone");
        let chest = BlockState::new("minecraft:chest").with_property("facing", "north");

        region.set_block_state(1, 0, 0, stone.clone());
        region.set_block_state(0, 0, 1, chest.clone());

        schematic.regions.insert("Main".to_string(), region);

        let mut placement = SchematicPlacement::new("p1", Arc::new(schematic), BlockPos::new(100, 64, 100));
        placement.rotation = BlockRotation::Clockwise90;

        // When rotated 90 deg clockwise:
        // (1, 0, 0) -> (-0, 0, 1) = (0, 0, 1) relative to origin -> world (100, 64, 101)
        let world_p1 = placement.schematic_to_world("Main", BlockPos::new(1, 0, 0));
        assert_eq!(world_p1, BlockPos::new(100, 64, 101));

        let block = placement.get_block_at_world_pos(world_p1);
        assert_eq!(block.as_ref(), Some(&stone));

        // (0, 0, 1) -> (-1, 0, 0) relative to origin -> world (99, 64, 100)
        let world_chest = placement.schematic_to_world("Main", BlockPos::new(0, 0, 1));
        assert_eq!(world_chest, BlockPos::new(99, 64, 100));

        let chest_block = placement.get_block_at_world_pos(world_chest).unwrap();
        assert_eq!(chest_block.name, "minecraft:chest");
        assert_eq!(chest_block.properties.get("facing").map(|s| s.as_str()), Some("east"));
    }
}
