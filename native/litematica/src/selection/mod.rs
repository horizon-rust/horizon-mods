pub mod selection;

pub use selection::{AreaSelection, SelectionManager};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use malilib::util::BlockPos;
    use crate::schematic::BlockState;

    #[test]
    fn test_area_selection_and_capture() {
        let sel = AreaSelection::with_corners("Sub1", BlockPos::new(10, 64, 10), BlockPos::new(12, 65, 11));
        let bbox = sel.bounding_box().unwrap();
        assert_eq!(bbox.size_x(), 3);
        assert_eq!(bbox.size_y(), 2);
        assert_eq!(bbox.size_z(), 2);

        let mut world_blocks = HashMap::new();
        world_blocks.insert(BlockPos::new(10, 64, 10), BlockState::new("minecraft:diamond_block"));

        let schematic = sel
            .capture_to_schematic("CaptureTest", "Tester", |p| {
                world_blocks.get(&p).cloned().unwrap_or_else(|| BlockState::new(BlockState::AIR))
            })
            .unwrap();

        assert_eq!(schematic.name, "CaptureTest");
        let reg = schematic.regions.get("Sub1").unwrap();
        assert_eq!(reg.size, BlockPos::new(3, 2, 2));
        assert_eq!(reg.get_block_state(0, 0, 0), Some(&BlockState::new("minecraft:diamond_block")));
    }
}
