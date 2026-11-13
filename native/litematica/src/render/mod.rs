pub mod overlay;

pub use overlay::{GhostOverlayMesh, LineVertex, WireframeBuilder};

#[cfg(test)]
mod tests {
    use super::*;
    use malilib::gui::Color;
    use malilib::util::{BlockPos, IntBoundingBox};

    #[test]
    fn test_wireframe_builder() {
        let b = IntBoundingBox::new(BlockPos::new(0, 0, 0), BlockPos::new(1, 1, 1));
        let lines = WireframeBuilder::build_box_lines(&b, Color::WHITE);
        assert_eq!(lines.len(), 24); // 12 edges * 2 vertices
    }
}
