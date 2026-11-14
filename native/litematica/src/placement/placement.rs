use std::collections::HashMap;
use std::sync::Arc;
use malilib::util::{BlockMirror, BlockPos, BlockRotation, IntBoundingBox, transform_block_state};
use crate::schematic::{BlockState, LitematicaSchematic};

#[derive(Debug, Clone, PartialEq)]
pub struct SubRegionPlacement {
    pub name: String,
    pub pos_offset: BlockPos,
    pub enabled: bool,
}

impl SubRegionPlacement {
    pub fn new(name: impl Into<String>, pos_offset: BlockPos) -> Self {
        Self {
            name: name.into(),
            pos_offset,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchematicPlacement {
    pub id: String,
    pub schematic: Arc<LitematicaSchematic>,
    pub origin: BlockPos,
    pub rotation: BlockRotation,
    pub mirror: BlockMirror,
    pub locked: bool,
    pub render_enclosing_box: bool,
    pub sub_region_placements: HashMap<String, SubRegionPlacement>,
}

impl SchematicPlacement {
    pub fn new(id: impl Into<String>, schematic: Arc<LitematicaSchematic>, origin: BlockPos) -> Self {
        let mut sub_region_placements = HashMap::new();
        for (reg_name, reg) in &schematic.regions {
            sub_region_placements.insert(
                reg_name.clone(),
                SubRegionPlacement::new(reg_name.clone(), reg.pos),
            );
        }

        Self {
            id: id.into(),
            schematic,
            origin,
            rotation: BlockRotation::None,
            mirror: BlockMirror::None,
            locked: false,
            render_enclosing_box: true,
            sub_region_placements,
        }
    }

    pub fn get_sub_region_world_box(&self, region_name: &str) -> Option<IntBoundingBox> {
        let region = self.schematic.regions.get(region_name)?;
        let sub_placement = self.sub_region_placements.get(region_name)?;
        if !sub_placement.enabled {
            return None;
        }

        let p1 = self.schematic_to_world(region_name, BlockPos::ORIGIN);
        let p2 = self.schematic_to_world(
            region_name,
            BlockPos::new(
                region.size.x.abs() - 1,
                region.size.y.abs() - 1,
                region.size.z.abs() - 1,
            ),
        );

        Some(IntBoundingBox::new(p1, p2))
    }

    pub fn get_enclosing_world_box(&self) -> IntBoundingBox {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut min_z = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut max_z = i32::MIN;

        let mut has_any = false;
        for reg_name in self.schematic.regions.keys() {
            if let Some(b) = self.get_sub_region_world_box(reg_name) {
                has_any = true;
                min_x = min_x.min(b.min_x);
                min_y = min_y.min(b.min_y);
                min_z = min_z.min(b.min_z);
                max_x = max_x.max(b.max_x);
                max_y = max_y.max(b.max_y);
                max_z = max_z.max(b.max_z);
            }
        }

        if !has_any {
            IntBoundingBox::new(self.origin, self.origin)
        } else {
            IntBoundingBox {
                min_x,
                min_y,
                min_z,
                max_x,
                max_y,
                max_z,
            }
        }
    }

    pub fn schematic_to_world(&self, region_name: &str, schematic_pos: BlockPos) -> BlockPos {
        let sub_offset = self
            .sub_region_placements
            .get(region_name)
            .map(|p| p.pos_offset)
            .unwrap_or(BlockPos::ORIGIN);

        let local = schematic_pos + sub_offset;
        let mirrored = self.mirror.mirror_pos(local);
        let rotated = self.rotation.rotate_pos(mirrored);
        self.origin + rotated
    }

    pub fn world_to_schematic(&self, region_name: &str, world_pos: BlockPos) -> Option<BlockPos> {
        let region = self.schematic.regions.get(region_name)?;
        let sub_placement = self.sub_region_placements.get(region_name)?;
        if !sub_placement.enabled {
            return None;
        }

        let rel = world_pos - self.origin;
        // Inverse rotation:
        let inv_rot = match self.rotation {
            BlockRotation::None => BlockRotation::None,
            BlockRotation::Clockwise90 => BlockRotation::CounterClockwise90,
            BlockRotation::Clockwise180 => BlockRotation::Clockwise180,
            BlockRotation::CounterClockwise90 => BlockRotation::Clockwise90,
        };
        let unrotated = inv_rot.rotate_pos(rel);
        // Inverse mirror:
        let unmirrored = self.mirror.mirror_pos(unrotated);
        let schematic_pos = unmirrored - sub_placement.pos_offset;

        if schematic_pos.x >= 0 && schematic_pos.x < region.size.x.abs()
            && schematic_pos.y >= 0 && schematic_pos.y < region.size.y.abs()
            && schematic_pos.z >= 0 && schematic_pos.z < region.size.z.abs()
        {
            Some(schematic_pos)
        } else {
            None
        }
    }

    pub fn get_block_at_world_pos(&self, world_pos: BlockPos) -> Option<BlockState> {
        for (reg_name, region) in &self.schematic.regions {
            if let Some(schem_pos) = self.world_to_schematic(reg_name, world_pos) {
                if let Some(state) = region.get_block_state(schem_pos.x, schem_pos.y, schem_pos.z) {
                    if state.is_air() {
                        return Some(state.clone());
                    }
                    let transformed_props = transform_block_state(&state.name, &state.properties, self.rotation, self.mirror);
                    return Some(BlockState {
                        name: state.name.clone(),
                        properties: transformed_props,
                    });
                }
            }
        }
        None
    }
}
