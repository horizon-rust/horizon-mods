/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Easing.java` (easings from easings.net + constant +
//! linear). Every formula is transcribed literally, including its float
//! precision.

/// `Easing.sine`
pub fn sine(n: f32) -> f32 {
    1.0 - (n * std::f32::consts::PI / 2.0).cos()
}

/// `Easing.cubic`
pub fn cubic(n: f32) -> f32 {
    n * n * n
}

/// `Easing.quadratic`
pub fn quadratic(n: f32) -> f32 {
    n * n
}

/// `Easing.pow(n)` as a closure
pub fn pow(n: f32) -> impl Fn(f32) -> f32 {
    move |t: f32| t.powf(n)
}

/// `Easing.exp`
pub fn exp(n: f32) -> f32 {
    2f32.powf(10.0 * (n - 1.0))
}

/// `Easing.circle`
pub fn circle(n: f32) -> f32 {
    1.0 - (1.0 - n * n).sqrt()
}

/// `Easing.back(n)`: `null` -> 1.70158, else `n * 1.70158`.
pub fn back(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = match n {
        None => 1.70158,
        Some(n) => n * 1.70158,
    };
    move |t: f32| t * t * ((n2 + 1.0) * t - n2)
}

/// `Easing.elastic(n)`: `null` -> 1.
pub fn elastic(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(1.0);
    move |t: f32| 1.0 - ((t * std::f32::consts::PI / 2.0).cos()).powi(3) * (t * n2 * std::f32::consts::PI).cos()
}

/// `Easing.bounce(n)`: `null` -> 0.5; min of the four parabolas.
pub fn bounce(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(0.5);
    move |t: f32| {
        let one = |x: f32| 121.0f32 / 16.0 * x * x;
        let two = |x: f32| 121.0f32 / 4.0 * n2 * (x - 6.0f32 / 11.0).powi(2) + 1.0 - n2;
        let three = |x: f32| 121.0 * n2 * n2 * (x - 9.0f32 / 11.0).powi(2) + 1.0 - n2 * n2;
        let four =
            |x: f32| 484.0 * n2 * n2 * n2 * (x - 10.5f32 / 11.0).powi(2) + 1.0 - n2 * n2 * n2;
        one(t).min(two(t)).min(three(t).min(four(t)))
    }
}

/// `Easing.step(n)`: `null` -> 2; panics below 2 steps, then binary-searches
/// the step index, exactly like the Java loop.
pub fn step(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(2.0);
    assert!(n2 >= 2.0, "Steps must be >= 2, got: {}", n2);
    let steps = n2 as i32;
    move |t: f32| {
        let mut result = 0.0f32;
        if t < 0.0 {
            return result;
        }
        let step_length = 1.0 / steps as f32;
        result = (steps - 1) as f32 * step_length;
        if t > result {
            return result;
        }
        let mut left_border_index = 0i32;
        let mut right_border_index = steps - 1;
        while right_border_index - left_border_index != 1 {
            let test_index = left_border_index + (right_border_index - left_border_index) / 2;
            if t >= test_index as f32 * step_length {
                left_border_index = test_index;
            } else {
                right_border_index = test_index;
            }
        }
        left_border_index as f32 * step_length
    }
}

/// `Easing.catmullRom`
pub fn catmull_rom(n: f32) -> f32 {
    0.5f32
        * (2.0 * (n + 1.0)
            + (((n + 2.0) - n) * 1.0)
            + (2.0 * n - 5.0 * (n + 1.0) + 4.0 * (n + 2.0) - (n + 3.0)) * 1.0
            + (3.0 * (n + 1.0) - n - 3.0 * (n + 2.0) + (n + 3.0)) * 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn endpoints_match_easings_net() {
        assert!(close(sine(0.0), 0.0));
        assert!(close(sine(1.0), 1.0));
        assert!(close(cubic(0.5), 0.125));
        assert!(close(quadratic(0.5), 0.25));
        assert!(close(exp(1.0), 1.0));
        assert!(close(circle(0.0), 0.0));
        assert!(close(circle(1.0), 1.0));
    }

    #[test]
    fn back_matches_java_formula() {
        let f = back(None);
        // n2 = 1.70158: t=0.5 -> 0.25 * (2.70158*0.5 - 1.70158)
        assert!(close(f(0.5), 0.25 * (2.70158 * 0.5 - 1.70158)));
        let g = back(Some(2.0));
        assert!(close(g(0.5), 0.25 * ((2.0 * 1.70158 + 1.0) * 0.5 - 2.0 * 1.70158)));
    }

    #[test]
    fn elastic_at_zero_and_one() {
        let f = elastic(None);
        assert!(close(f(0.0), 0.0), "cos(0)=1 * cos(0)=1 -> 1-1");
        assert!(close(f(1.0), 1.0), "t=1: cos(pi/2)^3 = 0, so 1 - 0*cos(pi) = 1");
    }

    #[test]
    fn bounce_takes_min_of_four() {
        let f = bounce(None);
        assert!(close(f(0.0), 0.0));
        assert!(close(f(1.0), 1.0), "all four hit 1 - n2^n at x=1");
    }

    #[test]
    #[should_panic(expected = "Steps must be >= 2")]
    fn step_panics_below_two() {
        step(Some(1.5));
    }

    #[test]
    fn step_binary_search_matches_java() {
        let f = step(Some(4.0));
        assert!(close(f(-0.5), 0.0), "t<0 -> 0");
        assert!(close(f(1.0), 0.75), "past the last border -> (steps-1)/steps");
        assert!(close(f(0.1), 0.0));
        assert!(close(f(0.9), 0.75));
        assert!(close(step(None)(0.9), 0.5), "default 2 steps");
    }

    #[test]
    fn catmull_rom_transcribed() {
        // Literal transcription: n=0 -> 0.5*(2 + 2 + 0 + 0) = 2
        assert!(close(catmull_rom(0.0), 2.0));
    }
}
