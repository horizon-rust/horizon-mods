pub mod palette;
pub mod bit_packing;
pub mod format;
pub mod converter;

pub use palette::{BlockState, BlockStatePalette};
pub use bit_packing::PackedBitArray;
pub use format::{LitematicaSchematic, SubRegion};
pub use converter::SchematicConverter;

#[cfg(test)]
mod tests {
    use super::*;
    use malilib::util::BlockPos;

    #[test]
    fn test_schematic_roundtrip() {
        let mut schematic = LitematicaSchematic::new("TestHouse", "TestAuthor");
        schematic.description = "A cozy cottage".to_string();

        let size = BlockPos::new(3, 4, 5);
        let mut region = SubRegion::new("Main", BlockPos::ORIGIN, size);

        let stone = BlockState::new("minecraft:stone");
        let oak_stairs = BlockState::new("minecraft:oak_stairs").with_property("facing", "north");

        region.set_block_state(0, 0, 0, stone.clone());
        region.set_block_state(1, 2, 3, oak_stairs.clone());

        schematic.regions.insert("Main".to_string(), region);

        let nbt = schematic.to_nbt();
        let decoded = LitematicaSchematic::from_nbt(&nbt).unwrap();

        assert_eq!(decoded.name, "TestHouse");
        assert_eq!(decoded.author, "TestAuthor");
        assert_eq!(decoded.description, "A cozy cottage");

        let reg = decoded.regions.get("Main").unwrap();
        assert_eq!(reg.size, BlockPos::new(3, 4, 5));
        assert_eq!(reg.get_block_state(0, 0, 0), Some(&stone));
        assert_eq!(reg.get_block_state(1, 2, 3), Some(&oak_stairs));
        assert_eq!(reg.get_block_state(2, 2, 2).map(|s| s.is_air()), Some(true));
    }
}
