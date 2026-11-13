use malilib::util::BlockPos;
use super::placement::SchematicPlacement;
use crate::schematic::BlockState;

#[derive(Debug, Default)]
pub struct PlacementManager {
    placements: Vec<SchematicPlacement>,
    selected_index: Option<usize>,
}

impl PlacementManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn placements(&self) -> &[SchematicPlacement] {
        &self.placements
    }

    pub fn placements_mut(&mut self) -> &mut [SchematicPlacement] {
        &mut self.placements
    }

    pub fn add_placement(&mut self, placement: SchematicPlacement) -> usize {
        let idx = self.placements.len();
        self.placements.push(placement);
        if self.selected_index.is_none() {
            self.selected_index = Some(idx);
        }
        idx
    }

    pub fn remove_placement(&mut self, index: usize) -> Option<SchematicPlacement> {
        if index < self.placements.len() {
            let removed = self.placements.remove(index);
            if let Some(selected) = self.selected_index {
                if selected >= self.placements.len() {
                    self.selected_index = if self.placements.is_empty() {
                        None
                    } else {
                        Some(self.placements.len() - 1)
                    };
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn select(&mut self, index: usize) {
        if index < self.placements.len() {
            self.selected_index = Some(index);
        }
    }

    pub fn get_selected(&self) -> Option<&SchematicPlacement> {
        self.selected_index.and_then(|i| self.placements.get(i))
    }

    pub fn get_selected_mut(&mut self) -> Option<&mut SchematicPlacement> {
        self.selected_index.and_then(|i| self.placements.get_mut(i))
    }

    pub fn get_block_at_world_pos(&self, pos: BlockPos) -> Option<BlockState> {
        for placement in &self.placements {
            if let Some(st) = placement.get_block_at_world_pos(pos) {
                if !st.is_air() {
                    return Some(st);
                }
            }
        }
        None
    }
}
