/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `RainbowUtils.java`. Upstream reads
//! `Util.getNanos()` for time and packs ARGB ints; here time is injectable
//! and the color is returned as floats plus the packed ARGB, with the ARGB
//! packing itself translated (`Mth.sin` is plain `f32::sin`).

/// `RainbowUtils.makeChannel`
fn make_channel(speed: f32, shift: f32, offset: i32) -> f32 {
    (1.0 + (speed * shift + offset as f32).sin()) * 0.5
}

/// `RainbowUtils.rainbowColor(float alpha)`, with nanos injected instead of
/// `Util.getNanos()`. Returns the packed ARGB int, same layout as
/// `ARGB.colorFromFloat`.
pub fn rainbow_color(alpha: f32, nanos: u64) -> u32 {
    let time = nanos as f32 * 1e-9;
    let speed = 0.5; // TODO config (upstream comment kept)
    color_from_float(
        alpha,
        make_channel(speed, time, 0),
        make_channel(speed, time, 2),
        make_channel(speed, time, 4),
    )
}

/// `ARGB.colorFromFloat`: r << 16 | g << 8 | b | alpha << 24, float channels
/// scaled by 255.
fn color_from_float(a: f32, r: f32, g: f32, b: f32) -> u32 {
    let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u32;
    (f(a) << 24) | (f(r) << 16) | (f(g) << 8) | f(b)
}

/// `ARGB.from8BitChannel`
pub fn from_8bit_channel(value: i32) -> f32 {
    value as f32 / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn make_channel_matches_java() {
        // (1 + sin(0.5*t + off)) / 2
        assert!(close(make_channel(0.5, 0.0, 0), 0.5));
        assert!(close(make_channel(0.5, 0.0, 2), (1.0 + 2.0f32.sin()) * 0.5), "offset adds inside sin");
    }

    #[test]
    fn rainbow_channels_differ_at_a_given_time() {
        let c = rainbow_color(1.0, 1_000_000_000); // t = 1s
        // 8-bit packing quantizes, so compare channel bytes, not floats.
        let chan = |v: f32| (v * 255.0) as u32;
        let r = (c >> 16) & 0xFF;
        let g = (c >> 8) & 0xFF;
        let b = c & 0xFF;
        assert_eq!(r, chan((1.0 + 0.5f32.sin()) * 0.5), "speed*shift = 0.5*1");
        assert_eq!(g, chan((1.0 + 2.5f32.sin()) * 0.5), "offset 2");
        assert_eq!(b, chan((1.0 + 4.5f32.sin()) * 0.5), "offset 4");
    }

    #[test]
    fn packing_layout_matches_argb() {
        let c = color_from_float(1.0, 1.0, 0.0, 0.5);
        assert_eq!(c >> 24, 0xFF);
        assert_eq!((c >> 16) & 0xFF, 0xFF);
        assert_eq!((c >> 8) & 0xFF, 0x00);
        assert_eq!(c & 0xFF, 127, "0.5 * 255 truncates, matching ARGB.as8bitChannel");
    }

    #[test]
    fn from_8bit_channel_scales() {
        assert!(close(from_8bit_channel(255), 1.0));
        assert!(close(from_8bit_channel(0), 0.0));
    }
}
