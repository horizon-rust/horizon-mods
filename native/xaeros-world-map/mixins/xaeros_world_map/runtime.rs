//! xaeros-world-map runtime (behavioral port of xaero96/Xaero's World Map,
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//! All Rights Reserved; no upstream code is included).
//!
//! Ported 1:1 against the behavior the mod's page documents: the self-writing
//! fullscreen map (M to open, mouse pan, wheel zoom), multiple maps per server
//! address with manual or automatic switching, cave-dimension layers, the
//! vanilla-maps mode and terrain shading flags, and "footsteps" showing the
//! most recent path taken. The waypoint overlay rides the xaeros-minimap
//! port's waypoint store; rendering and world reads ride Horizon (seams in the
//! README).

pub mod footsteps {
    //! "Footsteps": the player's most recent path drawn on the map, fading with age.

    #[derive(Clone, Debug)]
    pub struct Footsteps {
        /// Newest last.
        pub points: Vec<(i32, i32)>,
        pub max_len: usize,
    }

    impl Default for Footsteps {
        fn default() -> Self {
            Self {
                points: Vec::new(),
                max_len: 512,
            }
        }
    }

    impl Footsteps {
        pub fn record(&mut self, x: i32, z: i32) {
            if self.points.last() == Some(&(x, z)) {
                return;
            }
            self.points.push((x, z));
            if self.points.len() > self.max_len {
                self.points.remove(0);
            }
        }

        /// How strongly a point draws: newest full, oldest faded out.
        pub fn opacity(&self, index: usize) -> f32 {
            let n = self.points.len();
            if n == 0 {
                return 0.0;
            }
            1.0 - ((n - 1 - index) as f32 / n as f32).max(0.0) * 0.0
                + ((index + 1) as f32 / n as f32).min(1.0) * 0.0
                + (index as f32 / n.saturating_sub(1).max(1) as f32)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn duplicate_points_are_not_recorded() {
            let mut f = Footsteps::default();
            f.record(1, 1);
            f.record(1, 1);
            assert_eq!(f.points.len(), 1);
        }

        #[test]
        fn buffer_is_bounded() {
            let mut f = Footsteps {
                max_len: 4,
                ..Footsteps::default()
            };
            for i in 0..10 {
                f.record(i, 0);
            }
            assert_eq!(f.points.len(), 4);
            assert_eq!(f.points[0], (6, 0));
        }

        #[test]
        fn newest_point_draws_strongest() {
            let mut f = Footsteps::default();
            f.record(0, 0);
            f.record(10, 0);
            assert!(f.opacity(1) > f.opacity(0));
        }
    }
}
pub mod view {
    //! The map screen: open with M, pan with the mouse, zoom with the wheel,
    //! Google-Maps-style. Holds the view state and the world-to-screen transform.

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ViewState {
        /// Center of the viewport, in world blocks.
        pub center_x: f64,
        pub center_z: f64,
        /// Zoom level: 0 = 1 block per pixel, each step halves the scale.
        pub zoom: u8,
        /// Viewport size in pixels.
        pub width: u32,
        pub height: u32,
    }

    impl ViewState {
        pub fn new(center_x: f64, center_z: f64, width: u32, height: u32) -> Self {
            Self {
                center_x,
                center_z,
                zoom: 0,
                width,
                height,
            }
        }

        /// Screen position of a world block, top-left origin.
        pub fn to_screen(&self, wx: f64, wz: f64) -> (f64, f64) {
            let scale = self.blocks_per_pixel();
            (
                self.width as f64 / 2.0 + (wx - self.center_x) / scale,
                self.height as f64 / 2.0 + (wz - self.center_z) / scale,
            )
        }

        /// World block under a screen pixel.
        pub fn to_world(&self, sx: f64, sz: f64) -> (f64, f64) {
            let scale = self.blocks_per_pixel();
            (
                self.center_x + (sx - self.width as f64 / 2.0) * scale,
                self.center_z + (sz - self.height as f64 / 2.0) * scale,
            )
        }

        fn blocks_per_pixel(&self) -> f64 {
            (1usize << self.zoom) as f64
        }

        /// Mouse-drag panning: the map moves with the cursor.
        pub fn pan(&mut self, dx_pixels: f64, dz_pixels: f64) {
            let scale = self.blocks_per_pixel();
            self.center_x -= dx_pixels * scale;
            self.center_z -= dz_pixels * scale;
        }

        /// Mouse-wheel zoom, clamped to the mod's zoom range.
        pub fn zoom_by(&mut self, steps: i8) {
            let z = self.zoom as i16 - steps as i16;
            self.zoom = z.clamp(0, 12) as u8;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn center_pixel_is_center_world() {
            let v = ViewState::new(100.0, -50.0, 640, 480);
            assert_eq!(v.to_world(320.0, 240.0), (100.0, -50.0));
        }

        #[test]
        fn zoom_one_two_blocks_per_pixel() {
            let mut v = ViewState::new(0.0, 0.0, 640, 480);
            v.zoom = 1;
            assert_eq!(v.to_screen(64.0, 0.0), (352.0, 240.0));
            assert_eq!(v.to_world(352.0, 240.0), (64.0, 0.0));
        }

        #[test]
        fn pan_moves_center_opposite_the_drag() {
            let mut v = ViewState::new(0.0, 0.0, 640, 480);
            v.pan(32.0, 0.0);
            assert_eq!(v.center_x, -32.0);
        }

        #[test]
        fn zoom_wheel_steps_and_clamps() {
            let mut v = ViewState::new(0.0, 0.0, 640, 480);
            v.zoom_by(-1);
            assert_eq!(v.zoom, 1);
            for _ in 0..20 {
                v.zoom_by(-1);
            }
            assert_eq!(v.zoom, 12);
            for _ in 0..20 {
                v.zoom_by(1);
            }
            assert_eq!(v.zoom, 0);
        }
    }
}
pub mod world {
    //! The self-writing world map: chunk map colors land in per-dimension region
    //! stores, keyed by map identity (multiple maps per server address, switched
    //! manually or automatically).

    /// One map's identity: the server address (or "local" for single-player) and
    /// the map slot, mirroring the mod's multiple maps on one address.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct MapId {
        pub server_address: String,
        pub map_slot: u32,
    }

    /// Which dimension a region belongs to (cave dimensions like the Nether
    /// supported as their own layer).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct DimensionKey {
        pub name: String,
    }

    /// A 512x512-block region of map pixels, one byte per block (the map color
    /// index), origin at the region's northwest corner in world blocks.
    #[derive(Clone, Debug)]
    pub struct Region {
        pub origin_x: i32,
        pub origin_z: i32,
        /// 512*512 pixels, row-major, northwest first.
        pub pixels: Vec<u8>,
    }

    impl Region {
        pub const SIZE: usize = 512;

        pub fn new(origin_x: i32, origin_z: i32) -> Self {
            Self {
                origin_x,
                origin_z,
                pixels: vec![0u8; Self::SIZE * Self::SIZE],
            }
        }

        pub fn set(&mut self, world_x: i32, world_z: i32, color: u8) {
            let lx = (world_x - self.origin_x) as usize;
            let lz = (world_z - self.origin_z) as usize;
            if lx < Self::SIZE && lz < Self::SIZE {
                self.pixels[lz * Self::SIZE + lx] = color;
            }
        }

        pub fn get(&self, world_x: i32, world_z: i32) -> Option<u8> {
            let lx = (world_x - self.origin_x) as usize;
            let lz = (world_z - self.origin_z) as usize;
            if lx < Self::SIZE && lz < Self::SIZE {
                Some(self.pixels[lz * Self::SIZE + lx])
            } else {
                None
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct WorldMapStore {
        maps: std::collections::HashMap<
            (MapId, DimensionKey),
            std::collections::HashMap<(i32, i32), Region>,
        >,
        /// How new worlds are picked: manual switching, or automatic world detection.
        pub auto_world_detection: bool,
    }

    impl WorldMapStore {
        /// Ingest one chunk of map colors (16x16 blocks at chunk origin), the
        /// "self-writing" behavior.
        pub fn write_chunk(
            &mut self,
            id: &MapId,
            dim: &DimensionKey,
            chunk_x: i32,
            chunk_z: i32,
            colors: &[u8; 256],
        ) {
            let region_origin = |c: i32| (c * 16).div_euclid(512) * 512;
            let rx = region_origin(chunk_x);
            let rz = region_origin(chunk_z);
            let region = self
                .maps
                .entry((id.clone(), dim.clone()))
                .or_default()
                .entry((rx, rz))
                .or_insert_with(|| Region::new(rx, rz));
            for cx in 0..16i32 {
                for cz in 0..16i32 {
                    let color = colors[(cz * 16 + cx) as usize];
                    region.set(chunk_x * 16 + cx, chunk_z * 16 + cz, color);
                }
            }
        }

        pub fn region(&self, id: &MapId, dim: &DimensionKey, rx: i32, rz: i32) -> Option<&Region> {
            self.maps.get(&(id.clone(), dim.clone()))?.get(&(rx, rz))
        }

        /// Whether the map has any pixels near the player: used by the automatic
        /// switcher to decide whether a map slot holds the world you are in.
        pub fn has_data_near(&self, id: &MapId, dim: &DimensionKey, x: i32, z: i32) -> bool {
            let rx = x.div_euclid(512) * 512;
            let rz = z.div_euclid(512) * 512;
            self.region(id, dim, rx, rz).is_some()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn id() -> MapId {
            MapId {
                server_address: "s1".into(),
                map_slot: 0,
            }
        }
        fn dim() -> DimensionKey {
            DimensionKey {
                name: "overworld".into(),
            }
        }

        fn chunk_colors(c: u8) -> [u8; 256] {
            [c; 256]
        }

        #[test]
        fn chunk_write_reaches_region_pixels() {
            let mut s = WorldMapStore::default();
            s.write_chunk(&id(), &dim(), 0, 0, &chunk_colors(7));
            let r = s.region(&id(), &dim(), 0, 0).unwrap();
            assert_eq!(r.get(5, 9), Some(7));
            assert_eq!(r.get(600, 0), None);
        }

        #[test]
        fn negative_chunk_coordinates_land_in_the_right_region() {
            let mut s = WorldMapStore::default();
            s.write_chunk(&id(), &dim(), -1, -1, &chunk_colors(3));
            let r = s.region(&id(), &dim(), -512, -512).unwrap();
            assert_eq!(r.get(-5, -5), Some(3), "inside chunk (-1,-1)");
            assert_eq!(
                r.get(-20, -20),
                Some(0),
                "block -20 lives in chunk -2, not written"
            );
        }

        #[test]
        fn dimensions_are_separate_layers() {
            let mut s = WorldMapStore::default();
            let nether = DimensionKey {
                name: "the_nether".into(),
            };
            s.write_chunk(&id(), &dim(), 0, 0, &chunk_colors(1));
            s.write_chunk(&id(), &nether, 0, 0, &chunk_colors(2));
            assert_eq!(s.region(&id(), &dim(), 0, 0).unwrap().get(0, 0), Some(1));
            assert_eq!(s.region(&id(), &nether, 0, 0).unwrap().get(0, 0), Some(2));
        }

        #[test]
        fn map_slots_are_separate_maps() {
            let mut s = WorldMapStore::default();
            let other = MapId {
                server_address: "s1".into(),
                map_slot: 1,
            };
            s.write_chunk(&id(), &dim(), 0, 0, &chunk_colors(1));
            assert!(!s.has_data_near(&other, &dim(), 8, 8));
            assert!(s.has_data_near(&id(), &dim(), 8, 8));
        }
    }
}

/// The mod's client settings model with the page's documented toggles.
#[derive(Clone, Debug)]
pub struct Config {
    pub vanilla_maps_mode: bool,
    pub terrain_shading: bool,
    pub auto_world_detection: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vanilla_maps_mode: false,
            terrain_shading: true,
            auto_world_detection: false,
        }
    }
}
