//! sound-physics-remastered runtime for Horizon (slice 1): the pure sound
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//! physics, per-block materials, direct-path occlusion raymarching and
//! raytraced reverb estimation. Ported behaviorally from Sound Physics
//! Remastered (GPL-3.0-only). The client audio-path glue lands with the
//! Horizon sound surface; world access rides issue #10152's snapshots.

pub mod materials {
    //! Per-block sound behavior. SPR derives occlusion, absorption and sound
    //! diffusivity from each block's material; this port keeps the same shape:
    //! every block gets an absorption in 0.0 (air) .. 1.0 (fully solid) plus a
    //! reverb-reflectivity.

    /// Sound behavior of one block type.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Material {
        /// How much direct sound the block absorbs (0 = air, 1 = fully absorbing).
        pub absorption: f32,
        /// Fraction of reflected energy the block returns (stone echoes, wool does not).
        pub reflectivity: f32,
    }

    pub const AIR: Material = Material {
        absorption: 0.0,
        reflectivity: 0.0,
    };
    pub const STONE: Material = Material {
        absorption: 0.2,
        reflectivity: 0.8,
    };
    pub const WOOL: Material = Material {
        absorption: 1.0,
        reflectivity: 0.1,
    };
    pub const WOOD: Material = Material {
        absorption: 0.5,
        reflectivity: 0.4,
    };
    pub const GLASS: Material = Material {
        absorption: 0.1,
        reflectivity: 0.7,
    };
    pub const DIRT: Material = Material {
        absorption: 0.7,
        reflectivity: 0.3,
    };
    pub const WATER: Material = Material {
        absorption: 0.3,
        reflectivity: 0.5,
    };

    /// Map a block name to its material, the lookup SPR does from the block's
    /// sound type. Names are the `minecraft:` ids the client reports.
    pub fn material_for(block: &str) -> Material {
        match block {
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => AIR,
            "minecraft:stone"
            | "minecraft:cobblestone"
            | "minecraft:obsidian"
            | "minecraft:bricks" => STONE,
            "minecraft:white_wool"
            | "minecraft:orange_wool"
            | "minecraft:black_wool"
            | "minecraft:carpet" => WOOL,
            "minecraft:oak_planks" | "minecraft:spruce_planks" | "minecraft:oak_log" => WOOD,
            "minecraft:glass" | "minecraft:glass_pane" => GLASS,
            "minecraft:dirt" | "minecraft:grass_block" | "minecraft:sand" => DIRT,
            "minecraft:water" => WATER,
            // Unknown solid blocks default to stone-like, matching SPR's fallback.
            _ => STONE,
        }
    }
}
pub mod occlusion {
    //! Direct-path sound occlusion: raymarch from listener to source the way SPR
    //! does, sampling every half block and accumulating the absorption of the
    //! blocks the path passes through.

    use crate::materials::AIR;

    /// A voxel world the physics math can query. Values are the block's
    /// absorption; the horizon side supplies this from the live world.
    pub trait VoxelWorld {
        fn block_at(&self, x: i32, y: i32, z: i32) -> f32;
    }

    /// A test/table world; the client-side implementation wraps Horizon's world
    /// snapshot (issue #10152 covers thread-safe world snapshots).
    pub struct TableWorld(pub std::collections::HashMap<(i32, i32, i32), f32>);

    impl VoxelWorld for TableWorld {
        fn block_at(&self, x: i32, y: i32, z: i32) -> f32 {
            *self.0.get(&(x, y, z)).unwrap_or(&0.0)
        }
    }

    /// The accumulated occlusion of the direct path (sum of each distinct
    /// non-air voxel's absorption, at most one hit per voxel like SPR's raymarch).
    pub fn direct_occlusion(
        world: &dyn VoxelWorld,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
    ) -> f32 {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let dz = to.2 - from.2;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist == 0.0 {
            return 0.0;
        }
        // One sample per half block, not counting the endpoints' own voxels.
        let steps = (dist / 0.5).ceil() as i32;
        let mut occlusion = 0.0f32;
        let mut last: Option<(i32, i32, i32)> = None;
        for s in 1..steps {
            let t = s as f64 / steps as f64;
            let p = (from.0 + dx * t, from.1 + dy * t, from.2 + dz * t);
            let voxel = (p.0.floor() as i32, p.1.floor() as i32, p.2.floor() as i32);
            if Some(voxel) != last {
                let a = world.block_at(voxel.0, voxel.1, voxel.2);
                if a > 0.0 {
                    occlusion += a;
                }
                last = Some(voxel);
            }
        }
        occlusion.min(1.0)
    }

    /// The final direct-sound factor: distance falloff times what survives the
    /// blocks. Falloff is Minecraft-style linear clamp at the audible range.
    pub fn direct_sound_factor(distance: f64, audible_range: f64, occlusion: f32) -> f32 {
        if distance >= audible_range {
            return 0.0;
        }
        let distance_factor = (1.0 - distance / audible_range) as f32;
        distance_factor * (1.0 - occlusion)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::collections::HashMap;

        fn world(walls: &[(i32, i32, i32, f32)]) -> TableWorld {
            let mut m = HashMap::new();
            for (x, y, z, a) in walls {
                m.insert((*x, *y, *z), *a);
            }
            TableWorld(m)
        }

        #[test]
        fn clear_path_has_no_occlusion() {
            let w = world(&[]);
            assert_eq!(
                direct_occlusion(&w, (0.5, 64.0, 0.5), (10.5, 64.0, 0.5)),
                0.0
            );
        }

        #[test]
        fn one_wall_between_listener_and_source() {
            // Stone wall at x=5 on the straight path y=64, z=0.
            let w = world(&[(5, 64, 0, 1.0)]);
            let occ = direct_occlusion(&w, (0.5, 64.0, 0.5), (10.5, 64.0, 0.5));
            assert_eq!(occ, 1.0);
            // A wool wall absorbs everything already at 1.0 per voxel; a thinner
            // absorber accumulates once per voxel, not per sample.
            let w2 = world(&[(5, 64, 0, 0.3)]);
            assert!(
                (direct_occlusion(&w2, (0.5, 64.0, 0.5), (10.5, 64.0, 0.5)) - 0.3).abs() < 1e-6
            );
        }

        #[test]
        fn path_beside_the_wall_is_clear() {
            let w = world(&[(5, 64, 0, 1.0)]);
            let occ = direct_occlusion(&w, (0.5, 64.0, 2.5), (10.5, 64.0, 2.5));
            assert_eq!(occ, 0.0);
        }

        #[test]
        fn factor_falls_with_distance_and_occlusion() {
            assert!((direct_sound_factor(0.0, 32.0, 0.0) - 1.0).abs() < 1e-6);
            assert!((direct_sound_factor(16.0, 32.0, 0.0) - 0.5).abs() < 1e-6);
            assert_eq!(direct_sound_factor(33.0, 32.0, 0.0), 0.0);
            assert!((direct_sound_factor(16.0, 32.0, 0.5) - 0.25).abs() < 1e-6);
        }

        #[test]
        fn same_point_is_zero() {
            let w = world(&[]);
            assert_eq!(direct_occlusion(&w, (1.0, 1.0, 1.0), (1.0, 1.0, 1.0)), 0.0);
        }
    }
}
pub mod reverb {
    //! Raytraced reverb estimation: SPR casts rays from the listener into the
    //! world, bounces them off walls up to a bounce budget, and derives the room's
    //! reverb decay from how far the energy travels before dying. This port keeps
    //! that algorithm with a deterministic LCG instead of a global RNG so tests
    //! and replays are reproducible.

    use crate::materials::material_for;
    use crate::materials::{AIR, Material};
    use crate::occlusion::VoxelWorld;

    pub struct ReverbSettings {
        /// Rays cast from the listener (SPR's ray count).
        pub ray_count: u32,
        /// Bounces per ray before the energy is spent.
        pub max_bounces: u32,
        /// How far a ray may fly per leg, in blocks.
        pub max_ray_distance: f64,
        /// Fraction of energy kept per wall bounce (roughly the room's mean reflectivity).
        pub bounce_energy: f32,
    }

    impl Default for ReverbSettings {
        fn default() -> Self {
            Self {
                ray_count: 64,
                max_bounces: 6,
                max_ray_distance: 256.0,
                bounce_energy: 0.55,
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Vec3 {
        x: f64,
        y: f64,
        z: f64,
    }

    impl Vec3 {
        fn normalized(self) -> Vec3 {
            let l = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
            if l == 0.0 {
                Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                }
            } else {
                Vec3 {
                    x: self.x / l,
                    y: self.y / l,
                    z: self.z / l,
                }
            }
        }
    }

    /// One cast ray's travel summary.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RayResult {
        /// Total distance traveled before the ray's energy ran out.
        pub total_distance: f64,
        /// How many walls it bounced off.
        pub bounces: u32,
    }

    /// Deterministic LCG in [0, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next_f64(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
        }
        /// A uniformly random direction on the sphere.
        fn next_dir(&mut self) -> Vec3 {
            // Marsaglia's method on [0,1] cube, rejected inside the sphere.
            loop {
                let (a, b) = (self.next_f64() * 2.0 - 1.0, self.next_f64() * 2.0 - 1.0);
                let r2 = a * a + b * b;
                if r2 < 1.0 {
                    let f = (1.0 - r2).sqrt();
                    return Vec3 {
                        x: 2.0 * a * f,
                        y: 2.0 * b * f,
                        z: 1.0 - 2.0 * r2,
                    }
                    .normalized();
                }
            }
        }
    }

    /// The first solid block a ray hits within `max` blocks, with hit position and
    /// the surface material.
    pub fn cast_ray(
        world: &dyn VoxelWorld,
        blocks: &dyn Fn(i32, i32, i32) -> Material,
        origin: (f64, f64, f64),
        dir: Vec3,
        max: f64,
    ) -> Option<((f64, f64, f64), f64, Material)> {
        let step = 0.5_f64;
        let mut t = step;
        let mut last: Option<(i32, i32, i32)> = None;
        while t <= max {
            let p = (
                origin.0 + dir.x * t,
                origin.1 + dir.y * t,
                origin.2 + dir.z * t,
            );
            let voxel = (p.0.floor() as i32, p.1.floor() as i32, p.2.floor() as i32);
            if Some(voxel) != last {
                let m = blocks(voxel.0, voxel.1, voxel.2);
                if m.absorption > AIR.absorption {
                    return Some((p, t, m));
                }
                last = Some(voxel);
            }
            t += step;
        }
        None
    }

    /// Reflect a ray off an axis-aligned wall: the dominant component of the hit
    /// direction flips. SPR reflects off the face actually hit; the AABB face the
    /// ray entered through is the axis it traveled fastest along.
    fn reflect(dir: Vec3, hit_dir: Vec3) -> Vec3 {
        let ax = hit_dir.x.abs();
        let ay = hit_dir.y.abs();
        let az = hit_dir.z.abs();
        let mut d = dir;
        if ax >= ay && ax >= az {
            d.x = -d.x;
        } else if ay >= az {
            d.y = -d.y;
        } else {
            d.z = -d.z;
        }
        d
    }

    /// Cast `settings.ray_count` rays and summarize what the room does to sound:
    /// the mean free path (average ray leg) drives the decay time estimate.
    pub fn reverb_estimate(
        world: &dyn VoxelWorld,
        blocks: &dyn Fn(i32, i32, i32) -> Material,
        listener: (f64, f64, f64),
        settings: &ReverbSettings,
    ) -> (f64, u32) {
        let mut rng = Lcg(0x5EED_C0FFEE);
        let mut total_distance = 0.0f64;
        let mut total_bounces = 0u32;
        let mut rays_that_hit = 0u32;
        for _ in 0..settings.ray_count {
            let mut dir = rng.next_dir();
            let mut pos = listener;
            let mut dist = 0.0f64;
            let mut bounces = 0u32;
            let mut energy = 1.0f32;
            for _ in 0..settings.max_bounces {
                if let Some((hit, leg, m)) =
                    cast_ray(world, blocks, pos, dir, settings.max_ray_distance - dist)
                {
                    dist += leg;
                    bounces += 1;
                    energy *= m.reflectivity * settings.bounce_energy;
                    if energy < 0.05 {
                        break;
                    }
                    pos = hit;
                    dir = reflect(dir, dir);
                } else {
                    dist += settings.max_ray_distance - dist;
                    break;
                }
            }
            total_distance += dist;
            total_bounces += bounces;
            if bounces > 0 {
                rays_that_hit += 1;
            }
        }
        if settings.ray_count == 0 {
            return (0.0, 0);
        }
        (total_distance / settings.ray_count as f64, rays_that_hit)
    }

    /// The block-material lookup wired to a table world.
    pub fn table_blocks(
        table: &std::collections::HashMap<(i32, i32, i32), String>,
    ) -> impl Fn(i32, i32, i32) -> Material + '_ {
        move |x, y, z| {
            material_for(
                table
                    .get(&(x, y, z))
                    .map(String::as_str)
                    .unwrap_or("minecraft:air"),
            )
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::materials::{AIR, STONE, WOOL};
        use crate::occlusion::TableWorld;
        use std::collections::HashMap;

        fn blocks_of(w: &TableWorld) -> impl Fn(i32, i32, i32) -> Material + '_ {
            move |x, y, z| {
                if w.0.contains_key(&(x, y, z)) {
                    STONE
                } else {
                    AIR
                }
            }
        }

        #[test]
        fn ray_hits_a_wall_at_the_right_distance() {
            let w = TableWorld(HashMap::new());
            let hit = cast_ray(
                &w,
                &blocks_of(&w),
                (0.5, 64.5, 0.5),
                Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                100.0,
            );
            assert!(hit.is_none(), "empty world, nothing to hit");

            let mut m = HashMap::new();
            m.insert((10, 64, 0), 1.0);
            let w2 = TableWorld(m);
            let (_, dist, _) = cast_ray(
                &w2,
                &blocks_of(&w2),
                (0.5, 64.5, 0.5),
                Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                100.0,
            )
            .unwrap();
            assert!((dist - 9.5).abs() < 1.0, "hit ~10 blocks out, got {dist}");
        }

        #[test]
        fn closed_room_bounces_open_world_does_not() {
            // A 5x3x5 stone box with the listener inside.
            let mut m = HashMap::new();
            for x in -2..=2 {
                for z in -2..=2 {
                    for y in 63..=66 {
                        let on_wall = x == -2 || x == 2 || z == -2 || z == 2 || y == 63 || y == 66;
                        if on_wall {
                            m.insert((x, y, z), 1.0);
                        }
                    }
                }
            }
            let room = TableWorld(m);
            let (_, room_bounces) = reverb_estimate(
                &room,
                &blocks_of(&room),
                (0.5, 64.5, 0.5),
                &ReverbSettings::default(),
            );
            assert!(room_bounces > 0, "rays in a closed room must bounce, got 0");

            let empty = TableWorld(HashMap::new());
            let (_, open_bounces) = reverb_estimate(
                &empty,
                &blocks_of(&empty),
                (0.5, 64.5, 0.5),
                &ReverbSettings::default(),
            );
            assert_eq!(open_bounces, 0, "rays in open air must not bounce");
        }

        #[test]
        fn small_room_has_a_shorter_mean_free_path_than_a_big_one() {
            let build = |half: i32| {
                let mut m = HashMap::new();
                for x in -half..=half {
                    for z in -half..=half {
                        for y in 63..=67 {
                            if x == -half
                                || x == half
                                || z == -half
                                || z == half
                                || y == 63
                                || y == 67
                            {
                                m.insert((x, y, z), 1.0);
                            }
                        }
                    }
                }
                TableWorld(m)
            };
            let s = ReverbSettings {
                ray_count: 128,
                ..Default::default()
            };
            let small = reverb_estimate(&build(2), &blocks_of(&build(2)), (0.5, 64.5, 0.5), &s).0;
            let big = reverb_estimate(&build(12), &blocks_of(&build(12)), (0.5, 64.5, 0.5), &s).0;
            assert!(
                small < big,
                "small room {small} should reverb shorter than big room {big}"
            );
        }

        #[test]
        fn estimate_is_deterministic() {
            let empty = TableWorld(HashMap::new());
            let a = reverb_estimate(
                &empty,
                &blocks_of(&empty),
                (1.0, 1.0, 1.0),
                &ReverbSettings::default(),
            );
            let b = reverb_estimate(
                &empty,
                &blocks_of(&empty),
                (1.0, 1.0, 1.0),
                &ReverbSettings::default(),
            );
            assert_eq!(a, b);
        }

        #[test]
        fn wool_walls_kill_reverb_faster_than_stone() {
            let build = |mat: Material| {
                let mut m: HashMap<(i32, i32, i32), f32> = HashMap::new();
                for x in -2..=2 {
                    for z in -2..=2 {
                        for y in 63..=66 {
                            if x == -2 || x == 2 || z == -2 || z == 2 || y == 63 || y == 66 {
                                m.insert((x, y, z), 1.0);
                            }
                        }
                    }
                }
                let blocks = move |_: i32, _: i32, _: i32| mat;
                (TableWorld(m), blocks)
            };
            let s = ReverbSettings {
                ray_count: 128,
                ..Default::default()
            };
            let (d_stone, _) = {
                let (w, b) = build(STONE);
                reverb_estimate(&w, &b, (0.5, 64.5, 0.5), &s)
            };
            let (d_wool, _) = {
                let (w, b) = build(WOOL);
                reverb_estimate(&w, &b, (0.5, 64.5, 0.5), &s)
            };
            assert!(
                d_wool < d_stone,
                "wool room {d_wool} must decay before stone room {d_stone}"
            );
        }
    }
}
