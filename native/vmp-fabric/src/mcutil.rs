// io.papermc.paper.util.MCUtil.getCoordinateKey/getCoordinateX/Z and the
// ChunkPos.getPackedX/getPackedZ equivalents VMP builds its maps on.

pub fn get_coordinate_key(x: i32, z: i32) -> i64 {
    ((x as i64) << 32) | ((z as i64) & 0xFFFF_FFFF)
}

pub fn get_coordinate_x(key: i64) -> i32 {
    (key >> 32) as i32
}

pub fn get_coordinate_z(key: i64) -> i32 {
    ((key << 32) >> 32) as i32
}

pub fn chunk_pos_packed_x(pos: i64) -> i32 {
    get_coordinate_x(pos)
}

pub fn chunk_pos_packed_z(pos: i64) -> i32 {
    get_coordinate_z(pos)
}

pub fn chunk_pos_as_long(x: i32, z: i32) -> i64 {
    get_coordinate_key(x, z)
}
