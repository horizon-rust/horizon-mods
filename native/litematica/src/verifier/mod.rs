pub mod verifier;

pub use verifier::{MismatchEntry, MismatchType, SchematicVerifier, VerificationResult};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use malilib::util::BlockPos;
    use crate::placement::SchematicPlacement;
    use crate::schematic::{BlockState, LitematicaSchematic, SubRegion};

    #[test]
    fn test_schematic_verifier() {
        let mut schematic = LitematicaSchematic::new("Test", "Author");
        let mut region = SubRegion::new("Main", BlockPos::ORIGIN, BlockPos::new(2, 2, 1));

        let stone = BlockState::new("minecraft:stone");
        let dirt = BlockState::new("minecraft:dirt");
        let oak_stairs_north = BlockState::new("minecraft:oak_stairs").with_property("facing", "north");

        region.set_block_state(0, 0, 0, stone.clone());
        region.set_block_state(1, 0, 0, dirt.clone());
        region.set_block_state(0, 1, 0, oak_stairs_north.clone());

        schematic.regions.insert("Main".to_string(), region);

        let placement = SchematicPlacement::new("p1", Arc::new(schematic), BlockPos::ORIGIN);

        // Mock world:
        let mut world_blocks: HashMap<BlockPos, BlockState> = HashMap::new();
        world_blocks.insert(BlockPos::new(0, 0, 0), stone.clone()); // Correct
        // (1, 0, 0) is missing in world -> air
        world_blocks.insert(BlockPos::new(0, 1, 0), BlockState::new("minecraft:oak_stairs").with_property("facing", "south")); // WrongState
        world_blocks.insert(BlockPos::new(1, 1, 0), BlockState::new("minecraft:cobblestone")); // ExtraBlock

        let res = SchematicVerifier::verify_placement(&placement, None, |p| {
            world_blocks.get(&p).cloned().unwrap_or_else(|| BlockState::new(BlockState::AIR))
        });

        assert_eq!(res.total_checked, 4);
        assert_eq!(res.correct_count, 1);
        assert_eq!(res.missing_count, 1);
        assert_eq!(res.wrong_state_count, 1);
        assert_eq!(res.extra_count, 1);
        assert_eq!(res.wrong_block_count, 0);
    }
}
