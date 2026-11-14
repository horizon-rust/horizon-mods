use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::position::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlockRotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl BlockRotation {
    pub fn rotate_pos(&self, pos: BlockPos) -> BlockPos {
        match self {
            BlockRotation::None => pos,
            BlockRotation::Clockwise90 => BlockPos::new(-pos.z, pos.y, pos.x),
            BlockRotation::Clockwise180 => BlockPos::new(-pos.x, pos.y, -pos.z),
            BlockRotation::CounterClockwise90 => BlockPos::new(pos.z, pos.y, -pos.x),
        }
    }

    pub fn rotate_facing(&self, facing: &str) -> &'static str {
        match (self, facing) {
            (BlockRotation::None, f) => match f {
                "north" => "north",
                "south" => "south",
                "east" => "east",
                "west" => "west",
                "up" => "up",
                "down" => "down",
                _ => "north",
            },
            (BlockRotation::Clockwise90, "north") => "east",
            (BlockRotation::Clockwise90, "east") => "south",
            (BlockRotation::Clockwise90, "south") => "west",
            (BlockRotation::Clockwise90, "west") => "north",

            (BlockRotation::Clockwise180, "north") => "south",
            (BlockRotation::Clockwise180, "east") => "west",
            (BlockRotation::Clockwise180, "south") => "north",
            (BlockRotation::Clockwise180, "west") => "east",

            (BlockRotation::CounterClockwise90, "north") => "west",
            (BlockRotation::CounterClockwise90, "east") => "north",
            (BlockRotation::CounterClockwise90, "south") => "east",
            (BlockRotation::CounterClockwise90, "west") => "south",

            (_, f) => match f {
                "up" => "up",
                "down" => "down",
                _ => "north",
            },
        }
    }

    pub fn rotate_axis(&self, axis: &str) -> &'static str {
        match (self, axis) {
            (BlockRotation::Clockwise90 | BlockRotation::CounterClockwise90, "x") => "z",
            (BlockRotation::Clockwise90 | BlockRotation::CounterClockwise90, "z") => "x",
            (_, a) => match a {
                "x" => "x",
                "y" => "y",
                "z" => "z",
                _ => "y",
            },
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BlockRotation::None => BlockRotation::Clockwise90,
            BlockRotation::Clockwise90 => BlockRotation::Clockwise180,
            BlockRotation::Clockwise180 => BlockRotation::CounterClockwise90,
            BlockRotation::CounterClockwise90 => BlockRotation::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlockMirror {
    #[default]
    None,
    FrontBack, // Mirror across Z
    LeftRight, // Mirror across X
}

impl BlockMirror {
    pub fn mirror_pos(&self, pos: BlockPos) -> BlockPos {
        match self {
            BlockMirror::None => pos,
            BlockMirror::FrontBack => BlockPos::new(pos.x, pos.y, -pos.z),
            BlockMirror::LeftRight => BlockPos::new(-pos.x, pos.y, pos.z),
        }
    }

    pub fn mirror_facing(&self, facing: &str) -> &'static str {
        match (self, facing) {
            (BlockMirror::FrontBack, "north") => "south",
            (BlockMirror::FrontBack, "south") => "north",
            (BlockMirror::LeftRight, "east") => "west",
            (BlockMirror::LeftRight, "west") => "east",
            (_, f) => match f {
                "north" => "north",
                "south" => "south",
                "east" => "east",
                "west" => "west",
                "up" => "up",
                "down" => "down",
                _ => "north",
            },
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BlockMirror::None => BlockMirror::LeftRight,
            BlockMirror::LeftRight => BlockMirror::FrontBack,
            BlockMirror::FrontBack => BlockMirror::None,
        }
    }
}

pub fn transform_block_state(
    _block_name: &str,
    properties: &BTreeMap<String, String>,
    rotation: BlockRotation,
    mirror: BlockMirror,
) -> BTreeMap<String, String> {
    let mut transformed = properties.clone();

    if let Some(facing) = transformed.get("facing").cloned() {
        let f = mirror.mirror_facing(&facing);
        let f = rotation.rotate_facing(f);
        transformed.insert("facing".to_string(), f.to_string());
    }

    if let Some(axis) = transformed.get("axis").cloned() {
        let a = rotation.rotate_axis(&axis);
        transformed.insert("axis".to_string(), a.to_string());
    }

    if let Some(rot_str) = transformed.get("rotation").cloned() {
        if let Ok(rot_val) = rot_str.parse::<u32>() {
            let offset = match rotation {
                BlockRotation::None => 0,
                BlockRotation::Clockwise90 => 4,
                BlockRotation::Clockwise180 => 8,
                BlockRotation::CounterClockwise90 => 12,
            };
            let mut new_rot = (rot_val + offset) % 16;
            if mirror == BlockMirror::LeftRight {
                new_rot = (16 - new_rot) % 16;
            } else if mirror == BlockMirror::FrontBack {
                new_rot = (24 - new_rot) % 16;
            }
            transformed.insert("rotation".to_string(), new_rot.to_string());
        }
    }

    transformed
}
