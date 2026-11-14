pub mod tool_mode;
pub mod easy_place;

pub use tool_mode::ToolMode;
pub use easy_place::{EasyPlace, EasyPlaceVerdict};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use malilib::util::BlockPos;
    use crate::placement::{PlacementManager, SchematicPlacement};
    use crate::schematic::{BlockState, LitematicaSchematic, SubRegion};

    #[test]
    fn test_easy_place_verdict() {
        let mut schematic = LitematicaSchematic::new("Test", "Author");
        let mut region = SubRegion::new("Main", BlockPos::ORIGIN, BlockPos::new(1, 1, 1));
        region.set_block_state(0, 0, 0, BlockState::new("minecraft:glass"));
        schematic.regions.insert("Main".to_string(), region);

        let mut placement_mgr = PlacementManager::new();
        placement_mgr.add_placement(SchematicPlacement::new("p1", Arc::new(schematic), BlockPos::new(10, 64, 10)));

        let ep = EasyPlace::new();
        let target_pos = BlockPos::new(10, 64, 10);
        let air = BlockState::new(BlockState::AIR);

        let v1 = ep.evaluate_placement(target_pos, &air, "minecraft:stone", &placement_mgr);
        assert!(matches!(v1, EasyPlaceVerdict::WrongItem { .. }));

        let v2 = ep.evaluate_placement(target_pos, &air, "minecraft:glass", &placement_mgr);
        assert!(matches!(v2, EasyPlaceVerdict::Valid { .. }));

        let v3 = ep.evaluate_placement(BlockPos::new(20, 20, 20), &air, "minecraft:glass", &placement_mgr);
        assert!(matches!(v3, EasyPlaceVerdict::NotInSchematic));
    }
}
