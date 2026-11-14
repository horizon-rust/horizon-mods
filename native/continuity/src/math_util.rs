//! 1:1 of me.pepperbell.continuity.client.util.MathUtil.

use crate::seams::math_helper_hash_code;

/// java: GOLDEN_GAMMA - Borrowed from SplittableRandom (the Java long literal is
/// two's-complement; the u64 value cast keeps the exact bits).
pub const GOLDEN_GAMMA: i64 = 0x9e3779b97f4a7c15u64 as i64;

/// java: signum(int).
pub fn signum(value: i32) -> i32 {
    if value > 0 {
        1
    } else if value < 0 {
        -1
    } else {
        0
    }
}

/// java: lerp(float, int, int) - the (int) cast truncates toward zero, as `as i32` does.
pub fn lerp(delta: f32, start: i32, end: i32) -> i32 {
    start + (delta * (end - start) as f32) as i32
}

/// java: lerpColor(float, int, int).
pub fn lerp_color(delta: f32, color_a: i32, color_b: i32) -> i32 {
    let a = color_a as u32;
    let b = color_b as u32;
    let channel = |shift: u32| lerp(delta, (a >> shift & 0xFF) as i32, (b >> shift & 0xFF) as i32);
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// java: lerpLight(float, int, int).
pub fn lerp_light(delta: f32, light_a: i32, light_b: i32) -> i32 {
    let a = light_a as u32;
    let b = light_b as u32;
    let high = lerp(delta, (a >> 20 & 0xF) as i32, (b >> 20 & 0xF) as i32);
    let low = lerp(delta, (a >> 4 & 0xF) as i32, (b >> 4 & 0xF) as i32);
    (high << 20) | (low << 4)
}

/// java: mix64(long) - Borrowed from SplittableRandom.
pub fn mix64(z: i64) -> i64 {
    let mut z = z as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    (z ^ (z >> 31)) as i64
}

/// java: mix32(long) - Borrowed from SplittableRandom.
pub fn mix32(z: i64) -> i32 {
    let mut z = z as u64;
    z = (z ^ (z >> 33)).wrapping_mul(0x62a9d9ed799705f5);
    let z = (z ^ (z >> 28)).wrapping_mul(0xcb24d0a5c88c35b3);
    ((z >> 32) as u32) as i32
}

/// java: mix(int, int, int, int, int).
pub fn mix(x: i32, y: i32, z: i32, face: i32, loops: i32) -> i32 {
    mix32(
        (math_helper_hash_code(x, y, z) as i64 ^ mix64(GOLDEN_GAMMA.wrapping_mul(1 + face as i64)))
            + GOLDEN_GAMMA.wrapping_mul(1 + loops as i64),
    )
}

/// java: removeSignBit(int).
pub fn remove_sign_bit(value: i32) -> i32 {
    value & 0x7FFFFFFF
}

/// java: MathHelper.getLerpProgress(value, start, end) (float form, unclamped).
pub fn get_lerp_progress_f32(value: f32, start: f32, end: f32) -> f32 {
    (value - start) / (end - start)
}

/// java: MathHelper.lerp(delta, start, end) (float form).
pub fn lerp_f32(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

/// java: Math.signum(float) -> int cast: 0.0 maps to 0 (Rust's f32::signum maps
/// +0.0 to 1.0, so the Java zero behavior is kept here).
pub fn java_signum_f32(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}
