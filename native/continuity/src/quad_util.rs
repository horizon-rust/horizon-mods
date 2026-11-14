//! 1:1 of me.pepperbell.continuity.client.util.QuadUtil, by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity).
//! Upstream: src/main/java/me/pepperbell/continuity/client/util/QuadUtil.java.

use crate::processor_base::{QuadEmitter, QuadView, RenderMaterial};
use crate::processor_enums::Direction;
use crate::seams::Sprite;

use crate::math_util::lerp_f32;


/// java: QuadUtil.interpolate(MutableQuadView quad, Sprite oldSprite, Sprite newSprite).
pub fn interpolate(quad: &QuadView, old_sprite: &Sprite, new_sprite: &Sprite) {
    let old_min_u = old_sprite.min_u();
    let old_min_v = old_sprite.min_v();
    let new_min_u = new_sprite.min_u();
    let new_min_v = new_sprite.min_v();
    let u_factor = (new_sprite.max_u() - new_min_u) / (old_sprite.max_u() - old_min_u);
    let v_factor = (new_sprite.max_v() - new_min_v) / (old_sprite.max_v() - old_min_v);
    for i in 0..4 {
        quad.set_uv(
            i,
            new_min_u + (quad.u(i) - old_min_u) * u_factor,
            new_min_v + (quad.v(i) - old_min_v) * v_factor,
        );
    }
}

/// java: QuadUtil.assignLerpedUvs(MutableQuadView quad, Sprite sprite).
pub fn assign_lerped_uvs(quad: &QuadView, sprite: &Sprite) {
    let delta = sprite.get_animation_frame_delta();
    let center_u = (sprite.min_u() + sprite.max_u()) * 0.5;
    let center_v = (sprite.min_v() + sprite.max_v()) * 0.5;
    let lerped_min_u = lerp_f32(delta, sprite.min_u(), center_u);
    let lerped_max_u = lerp_f32(delta, sprite.max_u(), center_u);
    let lerped_min_v = lerp_f32(delta, sprite.min_v(), center_v);
    let lerped_max_v = lerp_f32(delta, sprite.max_v(), center_v);
    quad.set_uv(0, lerped_min_u, lerped_min_v);
    quad.set_uv(1, lerped_min_u, lerped_max_v);
    quad.set_uv(2, lerped_max_u, lerped_max_v);
    quad.set_uv(3, lerped_max_u, lerped_min_v);
}

/// java: QuadUtil.emitOverlayQuad(QuadEmitter emitter, Direction face, Sprite sprite,
/// int color, RenderMaterial material).
pub fn emit_overlay_quad(
    emitter: &mut QuadEmitter,
    face: Direction,
    sprite: &Sprite,
    color: i32,
    material: &RenderMaterial,
) {
    emitter.square(face, 0.0, 0.0, 1.0, 1.0, 0.0);
    emitter.color(0, color);
    emitter.color(1, color);
    emitter.color(2, color);
    emitter.color(3, color);
    assign_lerped_uvs(&mut emitter.current, sprite);
    emitter.material(material);
    emitter.emit();
}

/// java: QuadUtil.isQuadUnitSquare(QuadView quad).
pub fn is_quad_unit_square(quad: &QuadView) -> bool {
    let (index_a, index_b): (usize, usize);
    match quad.light_face().get_axis() {
        crate::processor_enums::Axis::X => {
            index_a = 1;
            index_b = 2;
        }
        crate::processor_enums::Axis::Y => {
            index_a = 0;
            index_b = 2;
        }
        crate::processor_enums::Axis::Z => {
            index_a = 1;
            index_b = 0;
        }
    }

    for i in 0..4 {
        let a = quad.pos_by_index(i, index_a);
        if (a >= 0.0001 || a <= -0.0001) && (a >= 1.0001 || a <= 0.9999) {
            return false;
        }
        let b = quad.pos_by_index(i, index_b);
        if (b >= 0.0001 || b <= -0.0001) && (b >= 1.0001 || b <= 0.9999) {
            return false;
        }
    }
    true
}

/// Returns an int in range [0, 7] representing the texture orientation of the given
/// quad relative to the world.
///
/// 0 - 0 degree counterclockwise rotation, counterclockwise UV winding order
/// 1 - 90 degree counterclockwise rotation, counterclockwise UV winding order
/// 2 - 180 degree counterclockwise rotation, counterclockwise UV winding order
/// 3 - 270 degree counterclockwise rotation, counterclockwise UV winding order
/// 4 - 0 degree counterclockwise rotation, clockwise UV winding order
/// 5 - 90 degree counterclockwise rotation, clockwise UV winding order
/// 6 - 180 degree counterclockwise rotation, clockwise UV winding order
/// 7 - 270 degree counterclockwise rotation, clockwise UV winding order
///
/// java: QuadUtil.getTextureOrientation(QuadView quad).
pub fn get_texture_orientation(quad: &QuadView) -> i32 {
    // Texture matrix
    let tm00 = quad.u(3) - quad.u(1);
    let tm01 = quad.v(3) - quad.v(1);
    let tm10 = quad.u(2) - quad.u(0);
    let tm11 = quad.v(2) - quad.v(0);
    // Determinant of texture matrix; also cross product of its column vectors
    let determinant = tm00 * tm11 - tm10 * tm01;
    if determinant == 0.0 {
        return 0;
    }
    let s = 1.0 / determinant;
    // Second column of inverse texture matrix
    let itm10 = -tm10 * s;
    let itm11 = tm00 * s;

    let (x_axis, x_axis_sign, y_axis, y_axis_sign): (usize, f32, usize, f32);
    match quad.light_face() {
        Direction::Down => {
            x_axis = 0; // +X
            x_axis_sign = 1.0;
            y_axis = 2; // +Z
            y_axis_sign = 1.0;
        }
        Direction::Up => {
            x_axis = 0; // +X
            x_axis_sign = 1.0;
            y_axis = 2; // -Z
            y_axis_sign = -1.0;
        }
        Direction::North => {
            x_axis = 0; // -X
            x_axis_sign = -1.0;
            y_axis = 1; // +Y
            y_axis_sign = 1.0;
        }
        Direction::South => {
            x_axis = 0; // +X
            x_axis_sign = 1.0;
            y_axis = 1; // +Y
            y_axis_sign = 1.0;
        }
        Direction::West => {
            x_axis = 2; // +Z
            x_axis_sign = 1.0;
            y_axis = 1; // +Y
            y_axis_sign = 1.0;
        }
        Direction::East => {
            x_axis = 2; // -Z
            x_axis_sign = -1.0;
            y_axis = 1; // +Y
            y_axis_sign = 1.0;
        }
    }
    // Position matrix
    let pm00 = quad.pos_by_index(3, x_axis) - quad.pos_by_index(1, x_axis);
    let pm01 = quad.pos_by_index(3, y_axis) - quad.pos_by_index(1, y_axis);
    let pm10 = quad.pos_by_index(2, x_axis) - quad.pos_by_index(0, x_axis);
    let pm11 = quad.pos_by_index(2, y_axis) - quad.pos_by_index(0, y_axis);

    // Texture up vector in projected world space
    // Computed as (position matrix * inverse texture matrix * [0; -1]); [0; -1] is the
    // texture up vector in texture space
    // Axis signs should be multiplied into position matrix values, but multiplying here
    // instead saves 2 multiplications
    let x = -(pm00 * itm10 + pm10 * itm11) * x_axis_sign;
    let y = -(pm01 * itm10 + pm11 * itm11) * y_axis_sign;

    // Clamp vector to nearest axis-aligned direction
    // up/+y -> 0, left/-x -> 1, down/-y -> 2, right/+x -> 3
    // Add 4 if the UV winding order is clockwise
    (if y.abs() >= x.abs() { if y > 0.0 { 0 } else { 2 } } else if x > 0.0 { 3 } else { 1 })
        + if determinant < 0.0 { 4 } else { 0 }
}
