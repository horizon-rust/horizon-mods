//! `io.github.a5b84.statuseffectbars.config.Direction`, 1:1.

use std::fmt;

use crate::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[default]
    LEFT_TO_RIGHT,
    RIGHT_TO_LEFT,
    BOTTOM_TO_TOP,
    TOP_TO_BOTTOM,
}

impl Direction {
    pub fn swap_xy(&self) -> bool {
        match self {
            Direction::LEFT_TO_RIGHT | Direction::RIGHT_TO_LEFT => false,
            Direction::BOTTOM_TO_TOP | Direction::TOP_TO_BOTTOM => true,
        }
    }

    pub fn reverse_axis(&self) -> bool {
        match self {
            Direction::LEFT_TO_RIGHT | Direction::TOP_TO_BOTTOM => false,
            Direction::RIGHT_TO_LEFT | Direction::BOTTOM_TO_TOP => true,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Direction::LEFT_TO_RIGHT => "LEFT_TO_RIGHT",
            Direction::RIGHT_TO_LEFT => "RIGHT_TO_LEFT",
            Direction::BOTTOM_TO_TOP => "BOTTOM_TO_TOP",
            Direction::TOP_TO_BOTTOM => "TOP_TO_BOTTOM",
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            i18n::get(&format!(
                "text.autoconfig.status-effect-bars.option.direction.{}",
                self.name()
            ))
        )
    }
}
