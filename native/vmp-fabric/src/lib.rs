// vmp-fabric slice 1 (port of RelativityMC/VMP-fabric, MIT, ishland;
// upstream read at ver/26.1.1, commit 7e89476).
//
// Logic-only core: the full vmp.properties config model, the chunk
// coordinate-key helpers the watching maps run on, the pooled-object
// utility, the AreaMap spatial index behind player watching, and the
// player-chunk-watching manager's bookkeeping. The server-coupled
// surfaces (Netty event loops, async chunks, portals, async logging,
// entity tracking, ticket propagator, biome access, collections, spawn
// density cap) are seams noted in the README.

pub mod area_map;
pub mod chunk_watching;
pub mod config;
pub mod mcutil;
pub mod pool;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_key_round_trip() {
        for (x, z) in [(0, 0), (1, -1), (-3, 7), (i32::MAX, i32::MIN), (-1, -1)] {
            let key = mcutil::get_coordinate_key(x, z);
            assert_eq!(mcutil::get_coordinate_x(key), x);
            assert_eq!(mcutil::get_coordinate_z(key), z);
        }
    }

    #[test]
    fn packed_pos_matches_chunk_pos_helpers() {
        let pos = mcutil::get_coordinate_key(-57, 1203);
        assert_eq!(mcutil::chunk_pos_packed_x(pos), -57);
        assert_eq!(mcutil::chunk_pos_packed_z(pos), 1203);
        assert_eq!(
            mcutil::get_coordinate_key(-57, 1203),
            mcutil::chunk_pos_as_long(-57, 1203)
        );
    }
}
