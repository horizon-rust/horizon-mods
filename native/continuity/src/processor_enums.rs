//! 1:1 of me.pepperbell.continuity.client.processor.{Symmetry,OrientationMode}.

/// java: enum Symmetry { NONE, OPPOSITE, ALL } with apply(Direction).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Symmetry {
    None,
    Opposite,
    All,
}

impl Symmetry {
    /// java: valueOf(str.trim().toUpperCase(Locale.ROOT)).
    pub fn parse(s: &str) -> Option<Symmetry> {
        match s.trim().to_ascii_uppercase().as_str() {
            "NONE" => Some(Symmetry::None),
            "OPPOSITE" => Some(Symmetry::Opposite),
            "ALL" => Some(Symmetry::All),
            _ => None,
        }
    }

    /// java: apply(face); axis_direction positive flips to opposite.
    pub fn apply(self, face: Direction) -> Direction {
        match self {
            Symmetry::None => face,
            Symmetry::Opposite => {
                if face.axis_direction() == AxisDirection::Positive {
                    face.opposite()
                } else {
                    face
                }
            }
            Symmetry::All => Direction::Down,
        }
    }
}

/// java: enum OrientationMode { NONE, STATE_AXIS, TEXTURE }.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrientationMode {
    None,
    StateAxis,
    Texture,
}

impl OrientationMode {
    /// java: AXIS_ORIENTATIONS[axis.ordinal()][quad.lightFace().ordinal()].
    pub const AXIS_ORIENTATIONS: [[i32; 6]; 3] = [
        [3, 3, 1, 3, 0, 2],
        [0, 0, 0, 0, 0, 0],
        [2, 0, 2, 0, 1, 3],
    ];

    pub fn get_orientation(self, light_face: Direction, has_axis: Option<usize>) -> i32 {
        match self {
            OrientationMode::None => 0,
            OrientationMode::StateAxis => match has_axis {
                Some(axis) => Self::AXIS_ORIENTATIONS[axis][light_face.ordinal()],
                None => 0,
            },
            // java: QuadUtil.getTextureOrientation(quad) - engine surface.
            OrientationMode::Texture => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AxisDirection {
    Positive,
    Negative,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn ordinal(self) -> usize {
        self as usize
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    pub fn axis_direction(self) -> AxisDirection {
        match self {
            Direction::Down | Direction::North | Direction::West => AxisDirection::Negative,
            Direction::Up | Direction::South | Direction::East => AxisDirection::Positive,
        }
    }
    /// java: Direction.getAxis().
    pub fn get_axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

}

/// java: net.minecraft.util.math.Direction.Axis { X, Y, Z }.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// java: ordinal().
    pub fn ordinal(self) -> usize {
        self as usize
    }
}

impl Direction {
    /// java: getAxis().
    pub fn axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    /// java: rotateClockwise(axis) - X: NORTH->DOWN->SOUTH->UP->NORTH;
    /// Z: DOWN->WEST->UP->EAST->DOWN; Y: NORTH->EAST->SOUTH->WEST->NORTH.
    pub fn rotate_clockwise(self, axis: Axis) -> Direction {
        match axis {
            Axis::X => match self {
                Direction::North => Direction::Down,
                Direction::Down => Direction::South,
                Direction::South => Direction::Up,
                Direction::Up => Direction::North,
                other => panic!("Unable to get X-rotated form of facing {:?}", other),
            },
            Axis::Z => match self {
                Direction::Down => Direction::West,
                Direction::West => Direction::Up,
                Direction::Up => Direction::East,
                Direction::East => Direction::Down,
                other => panic!("Unable to get Z-rotated form of facing {:?}", other),
            },
            Axis::Y => match self {
                Direction::North => Direction::East,
                Direction::East => Direction::South,
                Direction::South => Direction::West,
                Direction::West => Direction::North,
                other => panic!("Unable to get Y-rotated form of facing {:?}", other),
            },
        }
    }

    /// java: rotateCounterclockwise(axis) - the inverse cycles.
    pub fn rotate_counterclockwise(self, axis: Axis) -> Direction {
        match axis {
            Axis::X => match self {
                Direction::North => Direction::Up,
                Direction::Up => Direction::South,
                Direction::South => Direction::Down,
                Direction::Down => Direction::North,
                other => panic!("Unable to get X-rotated form of facing {:?}", other),
            },
            Axis::Z => match self {
                Direction::Down => Direction::East,
                Direction::East => Direction::Up,
                Direction::Up => Direction::West,
                Direction::West => Direction::Down,
                other => panic!("Unable to get Z-rotated form of facing {:?}", other),
            },
            Axis::Y => match self {
                Direction::North => Direction::West,
                Direction::West => Direction::South,
                Direction::South => Direction::East,
                Direction::East => Direction::North,
                other => panic!("Unable to get Y-rotated form of facing {:?}", other),
            },
        }
    }
}
