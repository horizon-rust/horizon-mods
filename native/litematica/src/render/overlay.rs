use malilib::gui::Color;
use malilib::util::{IntBoundingBox, LayerRange};
use crate::placement::PlacementManager;
use crate::verifier::{MismatchType, VerificationResult};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl LineVertex {
    pub fn new(x: f32, y: f32, z: f32, color: Color) -> Self {
        Self {
            position: [x, y, z],
            color: [
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ],
        }
    }
}

pub struct WireframeBuilder;

impl WireframeBuilder {
    pub fn build_box_lines(bbox: &IntBoundingBox, color: Color) -> Vec<LineVertex> {
        let min_x = bbox.min_x as f32;
        let min_y = bbox.min_y as f32;
        let min_z = bbox.min_z as f32;
        let max_x = (bbox.max_x + 1) as f32;
        let max_y = (bbox.max_y + 1) as f32;
        let max_z = (bbox.max_z + 1) as f32;

        let mut lines = Vec::with_capacity(24);

        // 4 bottom edges
        lines.push(LineVertex::new(min_x, min_y, min_z, color));
        lines.push(LineVertex::new(max_x, min_y, min_z, color));

        lines.push(LineVertex::new(max_x, min_y, min_z, color));
        lines.push(LineVertex::new(max_x, min_y, max_z, color));

        lines.push(LineVertex::new(max_x, min_y, max_z, color));
        lines.push(LineVertex::new(min_x, min_y, max_z, color));

        lines.push(LineVertex::new(min_x, min_y, max_z, color));
        lines.push(LineVertex::new(min_x, min_y, min_z, color));

        // 4 top edges
        lines.push(LineVertex::new(min_x, max_y, min_z, color));
        lines.push(LineVertex::new(max_x, max_y, min_z, color));

        lines.push(LineVertex::new(max_x, max_y, min_z, color));
        lines.push(LineVertex::new(max_x, max_y, max_z, color));

        lines.push(LineVertex::new(max_x, max_y, max_z, color));
        lines.push(LineVertex::new(min_x, max_y, max_z, color));

        lines.push(LineVertex::new(min_x, max_y, max_z, color));
        lines.push(LineVertex::new(min_x, max_y, min_z, color));

        // 4 vertical pillars
        lines.push(LineVertex::new(min_x, min_y, min_z, color));
        lines.push(LineVertex::new(min_x, max_y, min_z, color));

        lines.push(LineVertex::new(max_x, min_y, min_z, color));
        lines.push(LineVertex::new(max_x, max_y, min_z, color));

        lines.push(LineVertex::new(max_x, min_y, max_z, color));
        lines.push(LineVertex::new(max_x, max_y, max_z, color));

        lines.push(LineVertex::new(min_x, min_y, max_z, color));
        lines.push(LineVertex::new(min_x, max_y, max_z, color));

        lines
    }
}

pub struct GhostOverlayMesh {
    pub vertices: Vec<LineVertex>,
}

impl GhostOverlayMesh {
    pub fn generate_from_placements(
        placement_mgr: &PlacementManager,
        _layer_range: Option<&LayerRange>,
        verification: Option<&VerificationResult>,
    ) -> Self {
        let mut vertices = Vec::new();

        // 1. Placement bounding boxes
        for placement in placement_mgr.placements() {
            if placement.render_enclosing_box {
                let enc_box = placement.get_enclosing_world_box();
                let col = if placement.locked {
                    Color::rgba(255, 60, 60, 200)
                } else {
                    Color::rgba(0, 200, 255, 200)
                };
                vertices.extend(WireframeBuilder::build_box_lines(&enc_box, col));
            }
        }

        // 2. Verification highlight boxes
        if let Some(res) = verification {
            for err in &res.mismatches {
                let col = match err.mismatch {
                    MismatchType::MissingBlock { .. } => Color::rgba(0, 180, 255, 220), // Cyan/Blue
                    MismatchType::WrongBlock { .. } => Color::rgba(255, 30, 30, 220),   // Red
                    MismatchType::WrongState { .. } => Color::rgba(255, 220, 0, 220),  // Yellow
                    MismatchType::ExtraBlock { .. } => Color::rgba(255, 0, 220, 220),  // Magenta
                };
                let single_box = IntBoundingBox::new(err.pos, err.pos);
                vertices.extend(WireframeBuilder::build_box_lines(&single_box, col));
            }
        }

        Self { vertices }
    }
}
