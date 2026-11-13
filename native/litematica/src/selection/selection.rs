use malilib::util::{BlockPos, IntBoundingBox};
use crate::schematic::{BlockState, LitematicaSchematic, SubRegion};

#[derive(Debug, Clone, PartialEq)]
pub struct AreaSelection {
    pub name: String,
    pub corner1: Option<BlockPos>,
    pub corner2: Option<BlockPos>,
}

impl AreaSelection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            corner1: None,
            corner2: None,
        }
    }

    pub fn with_corners(name: impl Into<String>, c1: BlockPos, c2: BlockPos) -> Self {
        Self {
            name: name.into(),
            corner1: Some(c1),
            corner2: Some(c2),
        }
    }

    pub fn bounding_box(&self) -> Option<IntBoundingBox> {
        let (c1, c2) = (self.corner1?, self.corner2?);
        Some(IntBoundingBox::new(c1, c2))
    }

    pub fn capture_to_schematic<F: Fn(BlockPos) -> BlockState>(
        &self,
        schematic_name: &str,
        author: &str,
        get_world_block: F,
    ) -> Option<LitematicaSchematic> {
        let bbox = self.bounding_box()?;
        let size = BlockPos::new(bbox.size_x(), bbox.size_y(), bbox.size_z());

        let mut schematic = LitematicaSchematic::new(schematic_name, author);
        let mut region = SubRegion::new(self.name.clone(), BlockPos::ORIGIN, size);

        for world_pos in bbox.iter_blocks() {
            let state = get_world_block(world_pos);
            if !state.is_air() {
                let local_x = world_pos.x - bbox.min_x;
                let local_y = world_pos.y - bbox.min_y;
                let local_z = world_pos.z - bbox.min_z;
                region.set_block_state(local_x, local_y, local_z, state);
            }
        }

        schematic.regions.insert(self.name.clone(), region);
        Some(schematic)
    }
}

#[derive(Debug, Default)]
pub struct SelectionManager {
    selections: Vec<AreaSelection>,
    current_index: Option<usize>,
}

impl SelectionManager {
    pub fn new() -> Self {
        let mut mgr = Self::default();
        let default_sel = AreaSelection::new("Default");
        mgr.add_selection(default_sel);
        mgr
    }

    pub fn selections(&self) -> &[AreaSelection] {
        &self.selections
    }

    pub fn add_selection(&mut self, sel: AreaSelection) -> usize {
        let idx = self.selections.len();
        self.selections.push(sel);
        if self.current_index.is_none() {
            self.current_index = Some(idx);
        }
        idx
    }

    pub fn current_selection(&self) -> Option<&AreaSelection> {
        self.current_index.and_then(|i| self.selections.get(i))
    }

    pub fn current_selection_mut(&mut self) -> Option<&mut AreaSelection> {
        self.current_index.and_then(|i| self.selections.get_mut(i))
    }

    pub fn set_corner1(&mut self, pos: BlockPos) {
        if let Some(sel) = self.current_selection_mut() {
            sel.corner1 = Some(pos);
        }
    }

    pub fn set_corner2(&mut self, pos: BlockPos) {
        if let Some(sel) = self.current_selection_mut() {
            sel.corner2 = Some(pos);
        }
    }
}
