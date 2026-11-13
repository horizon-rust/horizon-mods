use std::fmt;
use std::ops::{Add, Sub};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ORIGIN: BlockPos = BlockPos { x: 0, y: 0, z: 0 };

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn offset(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }

    pub fn up(&self) -> Self {
        self.offset(0, 1, 0)
    }

    pub fn down(&self) -> Self {
        self.offset(0, -1, 0)
    }

    pub fn north(&self) -> Self {
        self.offset(0, 0, -1)
    }

    pub fn south(&self) -> Self {
        self.offset(0, 0, 1)
    }

    pub fn west(&self) -> Self {
        self.offset(-1, 0, 0)
    }

    pub fn east(&self) -> Self {
        self.offset(1, 0, 0)
    }

    pub fn manhattan_distance(&self, other: &BlockPos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }

    pub fn distance_sq(&self, other: &BlockPos) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        let dz = (self.z - other.z) as f64;
        dx * dx + dy * dy + dz * dz
    }

    pub fn distance(&self, other: &BlockPos) -> f64 {
        self.distance_sq(other).sqrt()
    }
}

impl Add for BlockPos {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for BlockPos {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl fmt::Display for BlockPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}
