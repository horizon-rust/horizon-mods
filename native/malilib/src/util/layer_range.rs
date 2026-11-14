use serde::{Deserialize, Serialize};
use super::position::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LayerMode {
    #[default]
    All,
    Single,
    AllBelow,
    AllAbove,
    Range,
}

impl LayerMode {
    pub fn next(&self) -> Self {
        match self {
            LayerMode::All => LayerMode::Single,
            LayerMode::Single => LayerMode::AllBelow,
            LayerMode::AllBelow => LayerMode::AllAbove,
            LayerMode::AllAbove => LayerMode::Range,
            LayerMode::Range => LayerMode::All,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerRange {
    pub mode: LayerMode,
    pub current_layer: i32,
    pub layer_min: i32,
    pub layer_max: i32,
}

impl Default for LayerRange {
    fn default() -> Self {
        Self {
            mode: LayerMode::All,
            current_layer: 0,
            layer_min: 0,
            layer_max: 255,
        }
    }
}

impl LayerRange {
    pub fn new(min: i32, max: i32) -> Self {
        Self {
            mode: LayerMode::All,
            current_layer: min,
            layer_min: min,
            layer_max: max,
        }
    }

    pub fn is_position_within(&self, pos: BlockPos) -> bool {
        match self.mode {
            LayerMode::All => true,
            LayerMode::Single => pos.y == self.current_layer,
            LayerMode::AllBelow => pos.y <= self.current_layer,
            LayerMode::AllAbove => pos.y >= self.current_layer,
            LayerMode::Range => pos.y >= self.layer_min && pos.y <= self.layer_max,
        }
    }

    pub fn set_current_layer(&mut self, y: i32) {
        self.current_layer = y.clamp(self.layer_min, self.layer_max);
    }

    pub fn move_layer(&mut self, delta: i32) {
        self.set_current_layer(self.current_layer + delta);
    }
}
