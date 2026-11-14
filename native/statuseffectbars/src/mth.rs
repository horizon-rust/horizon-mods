//! Stand-in for `net.minecraft.util.Mth` (host surface).

/// `Mth.lerpInt(delta, start, end)`: `start + floor(delta * (end - start))`.
pub fn lerp_int(delta: f32, start: i32, end: i32) -> i32 {
    start + (delta * (end - start) as f32).floor() as i32
}
