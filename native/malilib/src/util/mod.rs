pub mod position;
pub mod vec;
pub mod bounding_box;
pub mod transform;
pub mod layer_range;

pub use position::BlockPos;
pub use vec::{Vec3d, Vec3f};
pub use bounding_box::IntBoundingBox;
pub use transform::{BlockRotation, BlockMirror, transform_block_state};
pub use layer_range::{LayerMode, LayerRange};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_bounding_box_volume_and_contains() {
        let p1 = BlockPos::new(10, 20, 30);
        let p2 = BlockPos::new(12, 22, 32);
        let b = IntBoundingBox::new(p1, p2);

        assert_eq!(b.size_x(), 3);
        assert_eq!(b.size_y(), 3);
        assert_eq!(b.size_z(), 3);
        assert_eq!(b.volume(), 27);
        assert!(b.contains(BlockPos::new(11, 21, 31)));
        assert!(!b.contains(BlockPos::new(13, 21, 31)));

        let blocks: Vec<BlockPos> = b.iter_blocks().collect();
        assert_eq!(blocks.len(), 27);
    }

    #[test]
    fn test_rotation_transform() {
        let _origin = BlockPos::new(0, 0, 0);
        let pos = BlockPos::new(5, 10, 2);

        let r90 = BlockRotation::Clockwise90.rotate_pos(pos);
        assert_eq!(r90, BlockPos::new(-2, 10, 5));

        let r180 = BlockRotation::Clockwise180.rotate_pos(pos);
        assert_eq!(r180, BlockPos::new(-5, 10, -2));

        let r270 = BlockRotation::CounterClockwise90.rotate_pos(pos);
        assert_eq!(r270, BlockPos::new(2, 10, -5));

        let mut props = BTreeMap::new();
        props.insert("facing".to_string(), "north".to_string());
        let res = transform_block_state("minecraft:furnace", &props, BlockRotation::Clockwise90, BlockMirror::None);
        assert_eq!(res.get("facing").map(|s| s.as_str()), Some("east"));
    }

    #[test]
    fn test_layer_range_filtering() {
        let mut lr = LayerRange::new(0, 100);
        lr.mode = LayerMode::Single;
        lr.set_current_layer(64);

        assert!(lr.is_position_within(BlockPos::new(10, 64, 10)));
        assert!(!lr.is_position_within(BlockPos::new(10, 65, 10)));

        lr.mode = LayerMode::AllBelow;
        assert!(lr.is_position_within(BlockPos::new(10, 50, 10)));
        assert!(!lr.is_position_within(BlockPos::new(10, 65, 10)));
    }
}
