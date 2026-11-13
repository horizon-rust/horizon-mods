/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Ease.java` (easings from easings.net + constant +
//! linear). Enum ids match upstream exactly, gaps included.

use super::easing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,          // 0
    Constant,        // 1
    InSine,          // 6
    OutSine,         // 7
    InOutSine,       // 8
    InCubic,         // 9
    OutCubic,        // 10
    InOutCubic,      // 11
    InQuad,          // 12
    OutQuad,         // 13
    InOutQuad,       // 14
    InQuart,         // 15
    OutQuart,        // 16
    InOutQuart,      // 17
    InQuint,         // 18
    OutQuint,        // 19
    InOutQuint,      // 20
    InExpo,          // 21
    OutExpo,         // 22
    InOutExpo,       // 23
    InCirc,          // 24
    OutCirc,         // 25
    InOutCirc,       // 26
    InBack,          // 27
    OutBack,         // 28
    InOutBack,       // 29
    InElastic,       // 30
    OutElastic,      // 31
    InOutElastic,    // 32
    InBounce,        // 33
    OutBounce,       // 34
    InOutBounce,     // 35
    CatmullRom,      // 36
    Step,            // 37
}

impl Ease {
    /// The upstream enum id, gaps (2..=5) preserved.
    pub fn id(&self) -> u8 {
        match self {
            Ease::Linear => 0,
            Ease::Constant => 1,
            Ease::InSine => 6,
            Ease::OutSine => 7,
            Ease::InOutSine => 8,
            Ease::InCubic => 9,
            Ease::OutCubic => 10,
            Ease::InOutCubic => 11,
            Ease::InQuad => 12,
            Ease::OutQuad => 13,
            Ease::InOutQuad => 14,
            Ease::InQuart => 15,
            Ease::OutQuart => 16,
            Ease::InOutQuart => 17,
            Ease::InQuint => 18,
            Ease::OutQuint => 19,
            Ease::InOutQuint => 20,
            Ease::InExpo => 21,
            Ease::OutExpo => 22,
            Ease::InOutExpo => 23,
            Ease::InCirc => 24,
            Ease::OutCirc => 25,
            Ease::InOutCirc => 26,
            Ease::InBack => 27,
            Ease::OutBack => 28,
            Ease::InOutBack => 29,
            Ease::InElastic => 30,
            Ease::OutElastic => 31,
            Ease::InOutElastic => 32,
            Ease::InBounce => 33,
            Ease::OutBounce => 34,
            Ease::InOutBounce => 35,
            Ease::CatmullRom => 36,
            Ease::Step => 37,
        }
    }

    /// `invoke(t, n)`: run the easing with `t` in 0..1 and its argument.
    pub fn invoke(&self, t: f32, n: Option<f32>) -> f32 {
        let base: Box<dyn Fn(f32) -> f32> = match self {
            Ease::Linear => Box::new(|f| f),
            Ease::Constant => Box::new(|_| 0.0),
            Ease::InSine => Box::new(easing::sine),
            Ease::OutSine => Box::new(easing::sine),
            Ease::InOutSine => Box::new(easing::sine),
            Ease::InCubic => Box::new(easing::cubic),
            Ease::OutCubic => Box::new(easing::cubic),
            Ease::InOutCubic => Box::new(easing::cubic),
            Ease::InQuad => Box::new(easing::quadratic),
            Ease::OutQuad => Box::new(easing::quadratic),
            Ease::InOutQuad => Box::new(easing::quadratic),
            Ease::InQuart => Box::new(easing::pow(4.0)),
            Ease::OutQuart => Box::new(easing::pow(4.0)),
            Ease::InOutQuart => Box::new(easing::pow(4.0)),
            Ease::InQuint => Box::new(easing::pow(5.0)),
            Ease::OutQuint => Box::new(easing::pow(5.0)),
            Ease::InOutQuint => Box::new(easing::pow(5.0)),
            Ease::InExpo => Box::new(easing::exp),
            Ease::OutExpo => Box::new(easing::exp),
            Ease::InOutExpo => Box::new(easing::exp),
            Ease::InCirc => Box::new(easing::circle),
            Ease::OutCirc => Box::new(easing::circle),
            Ease::InOutCirc => Box::new(easing::circle),
            Ease::InBack => Box::new(easing::back(n)),
            Ease::OutBack => Box::new(easing::back(n)),
            Ease::InOutBack => Box::new(easing::back(n)),
            Ease::InElastic => Box::new(easing::elastic(n)),
            Ease::OutElastic => Box::new(easing::elastic(n)),
            Ease::InOutElastic => Box::new(easing::elastic(n)),
            Ease::InBounce => Box::new(easing::bounce(n)),
            Ease::OutBounce => Box::new(easing::bounce(n)),
            Ease::InOutBounce => Box::new(easing::bounce(n)),
            Ease::CatmullRom => Box::new(easing::catmull_rom),
            Ease::Step => Box::new(easing::step(n)),
        };
        match self {
            Ease::OutSine
            | Ease::OutCubic
            | Ease::OutQuad
            | Ease::OutQuart
            | Ease::OutQuint
            | Ease::OutExpo
            | Ease::OutCirc
            | Ease::OutBack
            | Ease::OutElastic
            | Ease::OutBounce => ease_out(base)(t),
            Ease::InOutSine
            | Ease::InOutCubic
            | Ease::InOutQuad
            | Ease::InOutQuart
            | Ease::InOutQuint
            | Ease::InOutExpo
            | Ease::InOutCirc
            | Ease::InOutBack
            | Ease::InOutElastic
            | Ease::InOutBounce
            | Ease::CatmullRom => ease_in_out(base)(t),
            _ => base(t),
        }
    }
}

/// `Ease.easeIn`
pub fn ease_in(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    function
}

/// `Ease.easeOut`: `time -> 1 - f(1 - time)`
pub fn ease_out(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |time: f32| 1.0 - function(1.0 - time)
}

/// `Ease.easeInOut`
pub fn ease_in_out(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |time: f32| {
        if time < 0.5 {
            function(time * 2.0) / 2.0
        } else {
            1.0 - function((1.0 - time) * 2.0) / 2.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn ids_match_upstream_enum() {
        assert_eq!(Ease::Linear.id(), 0);
        assert_eq!(Ease::Constant.id(), 1);
        assert_eq!(Ease::InSine.id(), 6);
        assert_eq!(Ease::Step.id(), 37);
    }

    #[test]
    fn ease_out_shape() {
        let out = ease_out(easing::cubic);
        assert!(close(out(0.0), 0.0), "1 - cubic(1)");
        assert!(close(out(1.0), 1.0), "1 - cubic(0)");
        assert!(close(out(0.5), 0.875), "1 - 0.125");
    }

    #[test]
    fn ease_in_out_shape() {
        let io = ease_in_out(easing::cubic);
        assert!(close(io(0.25), 0.0625), "cubic(0.5)/2 = 0.125/2");
        assert!(close(io(0.75), 0.9375), "1 - cubic(0.5)/2");
        assert!(close(io(0.5), 0.5), "branch boundary: (1-t)*2 = 1 -> cubic(1)/2 = 0.5");
    }

    #[test]
    fn invoke_routes_modes() {
        assert!(close(Ease::InCubic.invoke(0.5, None), 0.125));
        assert!(close(Ease::OutCubic.invoke(0.5, None), 0.875));
        assert!(close(Ease::InOutCubic.invoke(0.75, None), 0.9375));
        assert!(close(Ease::Constant.invoke(0.7, None), 0.0));
        assert!(close(Ease::Linear.invoke(0.7, None), 0.7));
    }

    #[test]
    fn step_passes_arg_through() {
        let a = Ease::Step.invoke(0.9, Some(4.0));
        let b = Ease::Step.invoke(0.9, None);
        assert!(close(a, 0.75), "4 steps -> last border 0.75");
        assert!(close(b, 0.5), "default 2 steps");
    }
}
