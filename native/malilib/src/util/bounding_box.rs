use serde::{Deserialize, Serialize};
use super::position::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntBoundingBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl IntBoundingBox {
    pub fn new(p1: BlockPos, p2: BlockPos) -> Self {
        Self {
            min_x: p1.x.min(p2.x),
            min_y: p1.y.min(p2.y),
            min_z: p1.z.min(p2.z),
            max_x: p1.x.max(p2.x),
            max_y: p1.y.max(p2.y),
            max_z: p1.z.max(p2.z),
        }
    }

    pub fn from_origin_and_size(origin: BlockPos, size_x: i32, size_y: i32, size_z: i32) -> Self {
        let p2 = BlockPos {
            x: origin.x + size_x - 1,
            y: origin.y + size_y - 1,
            z: origin.z + size_z - 1,
        };
        Self::new(origin, p2)
    }

    pub fn min_pos(&self) -> BlockPos {
        BlockPos::new(self.min_x, self.min_y, self.min_z)
    }

    pub fn max_pos(&self) -> BlockPos {
        BlockPos::new(self.max_x, self.max_y, self.max_z)
    }

    pub fn size_x(&self) -> i32 {
        self.max_x - self.min_x + 1
    }

    pub fn size_y(&self) -> i32 {
        self.max_y - self.min_y + 1
    }

    pub fn size_z(&self) -> i32 {
        self.max_z - self.min_z + 1
    }

    pub fn volume(&self) -> u64 {
        (self.size_x() as u64) * (self.size_y() as u64) * (self.size_z() as u64)
    }

    pub fn contains(&self, pos: BlockPos) -> bool {
        pos.x >= self.min_x && pos.x <= self.max_x
            && pos.y >= self.min_y && pos.y <= self.max_y
            && pos.z >= self.min_z && pos.z <= self.max_z
    }

    pub fn intersects(&self, other: &IntBoundingBox) -> bool {
        self.max_x >= other.min_x && self.min_x <= other.max_x
            && self.max_y >= other.min_y && self.min_y <= other.max_y
            && self.max_z >= other.min_z && self.min_z <= other.max_z
    }

    pub fn intersection(&self, other: &IntBoundingBox) -> Option<IntBoundingBox> {
        let min_x = self.min_x.max(other.min_x);
        let min_y = self.min_y.max(other.min_y);
        let min_z = self.min_z.max(other.min_z);
        let max_x = self.max_x.min(other.max_x);
        let max_y = self.max_y.min(other.max_y);
        let max_z = self.max_z.min(other.max_z);

        if min_x <= max_x && min_y <= max_y && min_z <= max_z {
            Some(IntBoundingBox {
                min_x,
                min_y,
                min_z,
                max_x,
                max_y,
                max_z,
            })
        } else {
            None
        }
    }

    pub fn offset(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            min_z: self.min_z + dz,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
            max_z: self.max_z + dz,
        }
    }

    pub fn iter_blocks(&self) -> BlockPosIterator {
        BlockPosIterator {
            box_: *self,
            curr_x: self.min_x,
            curr_y: self.min_y,
            curr_z: self.min_z,
            done: false,
        }
    }
}

pub struct BlockPosIterator {
    box_: IntBoundingBox,
    curr_x: i32,
    curr_y: i32,
    curr_z: i32,
    done: bool,
}

impl Iterator for BlockPosIterator {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let result = BlockPos::new(self.curr_x, self.curr_y, self.curr_z);

        self.curr_x += 1;
        if self.curr_x > self.box_.max_x {
            self.curr_x = self.box_.min_x;
            self.curr_y += 1;
            if self.curr_y > self.box_.max_y {
                self.curr_y = self.box_.min_y;
                self.curr_z += 1;
                if self.curr_z > self.box_.max_z {
                    self.done = true;
                }
            }
        }

        Some(result)
    }
}
