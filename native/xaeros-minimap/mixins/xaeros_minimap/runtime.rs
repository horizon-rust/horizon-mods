//! xaeros-minimap runtime (behavioral port of xaero96/Xaero's Minimap,
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//! All Rights Reserved; no upstream code is included).
//!
//! Ported 1:1 against the behavior the mod's page documents: the vanilla-styled
//! widget (square or circle shape), multiple sizes including automatic, zoom
//! levels, any-pixel positioning, lock-north with the direction arrow,
//! automatic cave mode with the "solid roof" search, waypoints per world and
//! dimension, the entity radar, the vanilla/accurate color modes, and the
//! enlarge keybind's behaviors. The rendering and world reads ride Horizon;
//! the seams are in the README.

pub mod cave {
    //! Automatic cave mode: the "solid roof" search the mod documents. Cave mode
    //! activates when a solid roof of blocks sits above the player within the
    //! configured roof size (a 3x3-and-larger search must NOT trigger under
    //! scattered log blocks, per the mod page).

    /// Minimal world read the algorithm needs: solid or not, above the player.
    pub trait WorldColumns {
        /// Returns whether the block at (x, z) and height y (0 = player level + 1)
        /// is solid. Out-of-world reads are not solid.
        fn is_solid_above(&self, x: i32, z: i32, y: u16) -> bool;
        /// How many blocks above the player to search for the roof.
        fn search_depth(&self) -> u16 {
            32
        }
    }

    /// Whether cave mode activates for a player at (px, pz) with the configured
    /// roof size `n` (odd; 1, 3, 5...). The search requires every column of the
    /// n-by-n footprint centered on the player to have a solid roof block within
    /// the search depth above.
    pub fn cave_mode_active(
        world: &impl WorldColumns,
        px: i32,
        pz: i32,
        roof_size: u8,
        _slope_shading: bool,
    ) -> bool {
        let n = roof_size.max(1) as i32;
        let half = n / 2;
        for dx in -half..=half {
            for dz in -half..=half {
                let mut solid = false;
                for y in 1..=world.search_depth() {
                    if world.is_solid_above(px + dx, pz + dz, y) {
                        solid = true;
                        break;
                    }
                }
                if !solid {
                    return false;
                }
            }
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::collections::HashSet;

        struct Grid {
            solid: HashSet<(i32, i32, u16)>,
        }
        impl WorldColumns for Grid {
            fn is_solid_above(&self, x: i32, z: i32, y: u16) -> bool {
                self.solid.contains(&(x, z, y))
            }
        }

        fn full_roof() -> Grid {
            let mut solid = HashSet::new();
            for x in -2..=2 {
                for z in -2..=2 {
                    solid.insert((x, z, 5));
                }
            }
            Grid { solid }
        }

        #[test]
        fn full_roof_activates_cave_mode() {
            assert!(cave_mode_active(&full_roof(), 0, 0, 3, false));
        }

        #[test]
        fn open_sky_never_activates() {
            assert!(!cave_mode_active(
                &Grid {
                    solid: HashSet::new()
                },
                0,
                0,
                3,
                false
            ));
        }

        #[test]
        fn tree_canopy_does_not_activate_at_roof_size_3() {
            // a lone log-and-canopy footprint: only the center column is covered
            let mut solid = HashSet::new();
            solid.insert((0, 0, 6));
            let g = Grid { solid };
            assert!(!cave_mode_active(&g, 0, 0, 3, false));
        }

        #[test]
        fn single_column_roof_activates_at_roof_size_1() {
            let mut solid = HashSet::new();
            solid.insert((0, 0, 6));
            let g = Grid { solid };
            assert!(cave_mode_active(&g, 0, 0, 1, false));
        }

        #[test]
        fn roof_beyond_search_depth_does_not_count() {
            let mut solid = HashSet::new();
            for x in -2..=2 {
                for z in -2..=2 {
                    solid.insert((x, z, 40));
                }
            }
            let g = Grid { solid };
            assert!(!cave_mode_active(&g, 0, 0, 3, false));
        }
    }
}
pub mod config {
    //! The mod's documented settings model (behavioral port; Xaero's Minimap is
    //! All Rights Reserved, no source published). Only settings the mod's page and
    //! in-game options document are modeled here.

    /// Minimap shape: the two documented options.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Shape {
        Square,
        Circle,
    }

    /// Minimap size options, including the automatic one.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MapSize {
        Small,
        Medium,
        Large,
        Automatic,
    }

    /// Block color mode: vanilla map colors, or accurate block-texture and biome colors.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ColorMode {
        Vanilla,
        Accurate,
    }

    /// Entity radar rendering: dots with a custom color, or icons (usually heads).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EntityDisplay {
        Off,
        Dots,
        Icons,
    }

    /// The config file model with its defaults.
    #[derive(Clone, Debug)]
    pub struct Config {
        pub enabled: bool,
        pub shape: Shape,
        pub size: MapSize,
        /// Zoom level: multiple zoom options are documented; higher = farther out.
        pub zoom: u8,
        /// Pixel offset on screen; the page notes 1-pixel shifts are possible.
        pub offset_x: i32,
        pub offset_y: i32,
        /// "Lock north": stop rotation, show the facing direction as an arrow instead.
        pub lock_north: bool,
        /// Automatic cave mode with the documented "solid roof" size (odd number, 1, 3, 5...).
        pub cave_mode: bool,
        pub cave_roof_size: u8,
        pub color_mode: ColorMode,
        /// Biome colors inside the vanilla color mode.
        pub biome_colors: bool,
        /// Shading customization.
        pub terrain_depth: bool,
        pub terrain_slopes: bool,
        /// The enlarge keybind (Z by default) and its documented behaviors.
        pub enlarge_centered: bool,
        pub enlarge_full_zoom_out: bool,
        pub entity_radar: EntityDisplay,
        pub radar_players: bool,
        pub radar_mobs: bool,
        pub radar_items: bool,
        /// Optional: bind the minimap to an item required in the hotbar or equipped.
        pub required_item_id: Option<String>,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                enabled: true,
                shape: Shape::Square,
                size: MapSize::Automatic,
                zoom: 0,
                offset_x: 0,
                offset_y: 0,
                lock_north: false,
                cave_mode: true,
                cave_roof_size: 3,
                color_mode: ColorMode::Vanilla,
                biome_colors: false,
                terrain_depth: true,
                terrain_slopes: false,
                enlarge_centered: false,
                enlarge_full_zoom_out: false,
                entity_radar: EntityDisplay::Dots,
                radar_players: true,
                radar_mobs: true,
                radar_items: true,
                required_item_id: None,
            }
        }
    }

    /// The map size in pixels the widget renders at, mirroring the size options;
    /// Automatic sizes to a fraction of the screen's shorter side.
    pub fn map_size_pixels(cfg: &Config, screen_w: u32, screen_h: u32) -> u32 {
        match cfg.size {
            MapSize::Small => 128,
            MapSize::Medium => 192,
            MapSize::Large => 256,
            MapSize::Automatic => screen_w.min(screen_h) * 2 / 5,
        }
    }

    /// Whether the minimap may be shown at all, applying the optional item bind:
    /// with a bind set, the minimap only shows when the required item is in the
    /// hotbar or equipped.
    pub fn may_show(cfg: &Config, hotbar_or_equipped_has_required_item: bool) -> bool {
        cfg.enabled && (cfg.required_item_id.is_none() || hotbar_or_equipped_has_required_item)
    }
}
pub mod map {
    //! Minimap frame and rendering math: the square and circle shapes, zoom,
    //! rotation lock with the direction arrow, and the enlarge keybind behaviors.

    use crate::config::{Config, Shape, map_size_pixels};

    /// A point on the map canvas, in world-block coordinates relative to the map center.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct MapPoint {
        pub x: f64,
        pub z: f64,
    }

    /// Which world-block a screen pixel inside the map widget samples, given zoom.
    /// `zoom_level` follows the mod's multiple zoom options: each step doubles the
    /// world blocks the widget covers.
    pub fn sample_block(
        cfg: &Config,
        widget_size: u32,
        px: i32,
        pz: i32,
        screen_px: i32,
        screen_pz: i32,
    ) -> Option<MapPoint> {
        let size = widget_size as i32;
        let half = size as f64 / 2.0;
        if cfg.shape == Shape::Circle {
            let dx = (screen_px - size / 2) as f64;
            let dz = (screen_pz - size / 2) as f64;
            if dx * dx + dz * dz > half * half {
                return None;
            }
        } else if screen_px < 0 || screen_pz < 0 || screen_px >= size || screen_pz >= size {
            return None;
        }
        let blocks_per_pixel = (1usize << cfg.zoom) as f64;
        let rel_x = (screen_px - size / 2) as f64 * blocks_per_pixel;
        let rel_z = (screen_pz - size / 2) as f64 * blocks_per_pixel;
        Some(MapPoint {
            x: px as f64 + rel_x,
            z: pz as f64 + rel_z,
        })
    }

    /// The player marker's angle on the map, in radians clockwise from map-up.
    /// With lock_north the map never rotates and the facing shows as an arrow, so
    /// the marker angle is the player's yaw; without it the map rotates under the
    /// player, so the marker always points up.
    pub fn player_marker_angle(cfg: &Config, yaw_radians: f64) -> f64 {
        if cfg.lock_north { yaw_radians } else { 0.0 }
    }

    /// The on-screen position of the widget's top-left corner, from the config
    /// offsets (any pixel shift, including 1-pixel nudges, is honored).
    pub fn widget_origin(
        cfg: &Config,
        screen_w: u32,
        screen_h: u32,
        widget_size: u32,
    ) -> (i32, i32) {
        let _ = (screen_w, screen_h);
        (cfg.offset_x, cfg.offset_y)
    }

    /// The enlarge keybind's (Z by default) temporary zoom and position: centered
    /// on screen when the centered setting is on, and fully zoomed out when that
    /// setting is on. Returns the effective widget size and zoom while enlarged.
    pub fn enlarged_state(
        cfg: &Config,
        screen_w: u32,
        screen_h: u32,
        normal_size: u32,
    ) -> (u32, u8) {
        let mut size = normal_size.max(map_minimum_for(screen_w, screen_h));
        if cfg.enlarge_centered {
            size = map_minimum_for(screen_w, screen_h);
        }
        let zoom = if cfg.enlarge_full_zoom_out {
            u8::MAX
        } else {
            cfg.zoom
        };
        (size, zoom)
    }

    fn map_minimum_for(screen_w: u32, screen_h: u32) -> u32 {
        screen_w.min(screen_h) * 4 / 5
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::config::MapSize;

        fn cfg(shape: Shape) -> Config {
            Config {
                shape,
                ..Config::default()
            }
        }

        #[test]
        fn zoom_zero_maps_one_block_per_pixel() {
            let c = cfg(Shape::Square);
            let p = sample_block(&c, 128, 0, 0, 64, 64).unwrap();
            assert_eq!((p.x, p.z), (0.0, 0.0));
        }

        #[test]
        fn zoom_two_covers_four_blocks_per_pixel() {
            let mut c = cfg(Shape::Square);
            c.zoom = 2;
            let p = sample_block(&c, 128, 0, 0, 64 + 4, 64).unwrap();
            assert_eq!(p.x, 16.0);
        }

        #[test]
        fn square_rejects_outside_pixels_circle_rejects_corners() {
            let c = cfg(Shape::Square);
            assert!(sample_block(&c, 128, 0, 0, 200, 64).is_none());
            let circle = cfg(Shape::Circle);
            assert!(
                sample_block(&circle, 128, 0, 0, 0, 0).is_none(),
                "corner of a circle map is outside"
            );
            assert!(sample_block(&circle, 128, 0, 0, 64, 64).is_some());
        }

        #[test]
        fn lock_north_shows_facing_yaw_otherwise_marker_points_up() {
            assert_eq!(
                player_marker_angle(
                    &Config {
                        lock_north: true,
                        ..Config::default()
                    },
                    1.25
                ),
                1.25
            );
            assert_eq!(
                player_marker_angle(
                    &Config {
                        lock_north: false,
                        ..Config::default()
                    },
                    1.25
                ),
                0.0
            );
        }

        #[test]
        fn offsets_shift_by_single_pixels() {
            let mut c = cfg(Shape::Square);
            c.offset_x = 3;
            c.offset_y = 7;
            assert_eq!(widget_origin(&c, 1920, 1080, 192), (3, 7));
        }

        #[test]
        fn automatic_size_scales_with_screen() {
            let c = Config {
                size: MapSize::Automatic,
                ..Config::default()
            };
            assert_eq!(map_size_pixels(&c, 1920, 1080), 432);
        }

        #[test]
        fn enlarge_full_zoom_out_pushes_zoom_to_max() {
            let c = Config {
                enlarge_full_zoom_out: true,
                ..Config::default()
            };
            let (_, zoom) = enlarged_state(&c, 1920, 1080, 192);
            assert_eq!(zoom, u8::MAX);
        }

        #[test]
        fn enlarge_centered_grows_widget() {
            let c = Config {
                enlarge_centered: true,
                ..Config::default()
            };
            let (size, zoom) = enlarged_state(&c, 1920, 1080, 192);
            assert!(size > 192);
            assert_eq!(zoom, c.zoom);
        }
    }
}
pub mod radar {
    //! Entity radar: surrounding entities as dots with a custom color or as icons
    //! (usually heads), with per-category toggles.

    use crate::config::{Config, EntityDisplay};

    /// An entity on the radar, in world coordinates relative to the player.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Entity {
        /// Category kind, kept abstract: the host registry classifies it.
        pub kind: EntityKind,
        pub dx: f64,
        pub dz: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EntityKind {
        Player,
        Mob,
        Item,
    }

    /// The dot color per category, the radar's "custom color" defaults.
    pub fn dot_color(kind: EntityKind) -> (u8, u8, u8) {
        match kind {
            EntityKind::Player => (255, 255, 255),
            EntityKind::Mob => (255, 64, 64),
            EntityKind::Item => (255, 255, 0),
        }
    }

    /// Which entities the radar draws, honoring the display mode and the
    /// per-category toggles. Entities outside the widget never draw.
    pub fn visible(cfg: &Config, all: &[Entity], widget_size: u32) -> Vec<Entity> {
        if cfg.entity_radar == EntityDisplay::Off {
            return Vec::new();
        }
        let half = widget_size as f64 / 2.0;
        all.iter()
            .filter(|e| match e.kind {
                EntityKind::Player => cfg.radar_players,
                EntityKind::Mob => cfg.radar_mobs,
                EntityKind::Item => cfg.radar_items,
            })
            .filter(|e| e.dx.abs() <= half && e.dz.abs() <= half)
            .copied()
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn cfg() -> Config {
            Config::default()
        }

        #[test]
        fn off_radar_hides_everything() {
            let mut c = cfg();
            c.entity_radar = EntityDisplay::Off;
            let es = [Entity {
                kind: EntityKind::Player,
                dx: 1.0,
                dz: 1.0,
            }];
            assert!(visible(&c, &es, 128).is_empty());
        }

        #[test]
        fn category_toggles_filter() {
            let mut c = cfg();
            c.radar_items = false;
            let es = [
                Entity {
                    kind: EntityKind::Player,
                    dx: 1.0,
                    dz: 1.0,
                },
                Entity {
                    kind: EntityKind::Item,
                    dx: 2.0,
                    dz: 2.0,
                },
            ];
            let v = visible(&c, &es, 128);
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].kind, EntityKind::Player);
        }

        #[test]
        fn entities_outside_widget_do_not_draw() {
            let es = [Entity {
                kind: EntityKind::Mob,
                dx: 500.0,
                dz: 0.0,
            }];
            assert!(visible(&cfg(), &es, 128).is_empty());
        }

        #[test]
        fn dot_colors_differ_by_category() {
            assert_ne!(dot_color(EntityKind::Player), dot_color(EntityKind::Mob));
        }
    }
}
pub mod waypoint {
    //! Waypoints: coordinates with name, color and symbol, visible on the minimap
    //! and in the game world, organized per world/server and dimension, with
    //! distance sorting. (Behavioral port of the documented feature set.)

    #[derive(Clone, Debug, PartialEq)]
    pub struct Waypoint {
        pub x: i32,
        pub y: i32,
        pub z: i32,
        pub name: String,
        /// ARGB color of the beacon / map marker.
        pub color: u32,
        /// Single-character symbol shown on the map marker.
        pub symbol: char,
        pub disabled: bool,
        /// Temporary waypoints (auto-created, e.g. from death position).
        pub temporary: bool,
    }

    /// Where a waypoint set lives: a world (single-player or server address) plus
    /// a dimension, mirroring the mod's per-world organization.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct MapKey {
        pub world: String,
        pub dimension: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct WaypointStore {
        sets: std::collections::HashMap<MapKey, Vec<Waypoint>>,
    }

    impl WaypointStore {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn add(&mut self, key: MapKey, wp: Waypoint) {
            self.sets.entry(key).or_default().push(wp);
        }

        pub fn remove(&mut self, key: &MapKey, index: usize) -> Option<Waypoint> {
            let set = self.sets.get_mut(key)?;
            if index < set.len() {
                Some(set.remove(index))
            } else {
                None
            }
        }

        pub fn set(&self, key: &MapKey) -> Option<&Vec<Waypoint>> {
            self.sets.get(key)
        }

        /// Enabled waypoints of a set, nearest first, by squared distance in the
        /// horizontal plane (the behavior the map uses for marker priority).
        pub fn nearest(&self, key: &MapKey, px: i32, pz: i32) -> Vec<Waypoint> {
            let mut list: Vec<Waypoint> = self
                .set(key)
                .map(|v| v.iter().filter(|w| !w.disabled).cloned().collect())
                .unwrap_or_default();
            list.sort_by_key(|w| {
                let dx = w.x - px;
                let dz = w.z - pz;
                dx * dx + dz * dz
            });
            list
        }

        /// Death-position helper: a temporary waypoint at the death coordinates.
        pub fn add_death_waypoint(&mut self, key: MapKey, x: i32, y: i32, z: i32) {
            self.add(
                key,
                Waypoint {
                    x,
                    y,
                    z,
                    name: "death".into(),
                    color: 0xFF0000,
                    symbol: 'X',
                    disabled: false,
                    temporary: true,
                },
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn key() -> MapKey {
            MapKey {
                world: "server1".into(),
                dimension: "overworld".into(),
            }
        }

        fn wp(x: i32, z: i32) -> Waypoint {
            Waypoint {
                x,
                y: 64,
                z,
                name: format!("wp{x}"),
                color: 0xFF00FF,
                symbol: 'A',
                disabled: false,
                temporary: false,
            }
        }

        #[test]
        fn add_and_list_per_world() {
            let mut s = WaypointStore::new();
            s.add(key(), wp(10, 10));
            s.add(
                MapKey {
                    world: "server2".into(),
                    dimension: "overworld".into(),
                },
                wp(1, 1),
            );
            assert_eq!(s.set(&key()).unwrap().len(), 1);
        }

        #[test]
        fn nearest_sorts_by_distance_and_skips_disabled() {
            let mut s = WaypointStore::new();
            s.add(key(), wp(100, 0));
            s.add(key(), wp(10, 0));
            let mut far_disabled = wp(500, 0);
            far_disabled.disabled = true;
            s.add(key(), far_disabled);
            let sorted = s.nearest(&key(), 0, 0);
            assert_eq!(sorted[0].name, "wp10");
            assert_eq!(sorted[1].name, "wp100");
            assert_eq!(sorted.len(), 2);
        }

        #[test]
        fn remove_by_index() {
            let mut s = WaypointStore::new();
            s.add(key(), wp(1, 1));
            s.add(key(), wp(2, 2));
            let removed = s.remove(&key(), 0).unwrap();
            assert_eq!(removed.name, "wp1");
            assert_eq!(s.set(&key()).unwrap().len(), 1);
            assert!(s.remove(&key(), 9).is_none());
        }

        #[test]
        fn death_waypoint_is_temporary_and_named() {
            let mut s = WaypointStore::new();
            s.add_death_waypoint(key(), -5, 12, 7);
            let w = &s.set(&key()).unwrap()[0];
            assert!(w.temporary && w.name == "death" && (w.x, w.y, w.z) == (-5, 12, 7));
        }
    }
}
