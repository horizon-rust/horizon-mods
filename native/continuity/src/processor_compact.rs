//! 1:1 of me.pepperbell.continuity.client.processor.CompactCtmQuadProcessor,
//! by PepperCode1 (LGPL-3.0-only, https://github.com/PepperCode1/Continuity).
//! Upstream: src/main/java/me/pepperbell/continuity/client/processor/CompactCtmQuadProcessor.java.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::math_util::{get_lerp_progress_f32, java_signum_f32, lerp_color, lerp_f32, lerp_light};
use crate::processor_base::{
    get_map, processing_data_keys, AbstractQuadProcessor, BlockRenderView, BlockState, BlockPos,
    ProcessingContext, ProcessingPredicate, ProcessingResult, QuadEmitter, QuadProcessor, QuadView,
    Random, TextureGetter,
};
use crate::processor_enums::OrientationMode;
use crate::connecting_properties::CtmProperties;
use crate::properties::BaseCtmProperties;
use crate::simple_sprite_provider::{CtmSpriteProvider, SPRITE_INDEX_MAP};
use crate::seams::{is_missing_sprite, missing_sprite_id, sprite_for_id, Sprite};

/// java: private static final int[][] QUADRANT_INDEX_MAPS with the clone/shift/reverse
/// construction (ArrayUtils.shift rotates right by the offset).
fn quadrant_index_maps() -> &'static [[i32; 4]; 8] {
    static MAPS: OnceLock<[[i32; 4]; 8]> = OnceLock::new();
    MAPS.get_or_init(|| {
        let mut map = [[0i32; 4]; 8];
        // 0 1 2 3
        map[0] = [0, 1, 2, 3];
        // 3 0 1 2
        map[1] = { let mut m = map[0]; m.rotate_right(1); m };
        // 2 3 0 1
        map[2] = { let mut m = map[1]; m.rotate_right(1); m };
        // 1 2 3 0
        map[3] = { let mut m = map[2]; m.rotate_right(1); m };
        // 3 2 1 0
        map[4] = { let mut m = map[0]; m.reverse(); m };
        // 0 3 2 1
        map[5] = { let mut m = map[4]; m.rotate_right(1); m };
        // 1 0 3 2
        map[6] = { let mut m = map[5]; m.rotate_right(1); m };
        // 2 1 0 3
        map[7] = { let mut m = map[6]; m.rotate_right(1); m };
        map
    })
}

/// java: CompactCtmQuadProcessor implements QuadProcessor.
pub struct CompactCtmQuadProcessor {
    sprites: Vec<Option<Sprite>>,
    processing_predicate: Rc<dyn ProcessingPredicate>,
    connection_predicate: Rc<dyn ConnectionPredicate>,
    inner_seams: bool,
    orientation_mode: OrientationMode,
    /// java: @Nullable Sprite[] replacementSprites.
    replacement_sprites: Option<Vec<Option<Sprite>>>,
}

use crate::connecting_properties::ConnectionPredicate;

impl CompactCtmQuadProcessor {
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        processing_predicate: Rc<dyn ProcessingPredicate>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
        replacement_sprites: Option<Vec<Option<Sprite>>>,
    ) -> CompactCtmQuadProcessor {
        CompactCtmQuadProcessor {
            sprites,
            processing_predicate,
            connection_predicate,
            inner_seams,
            orientation_mode,
            replacement_sprites,
        }
    }
}

impl QuadProcessor for CompactCtmQuadProcessor {
    fn process_quad(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        Self::abstract_process_quad(
            self, quad, sprite, block_view, appearance_state, state, pos, random_supplier, pass,
            context,
        )
    }
}

impl AbstractQuadProcessor for CompactCtmQuadProcessor {
    fn sprites(&self) -> &[Option<Sprite>] {
        &self.sprites
    }

    fn processing_predicate(&self) -> &dyn ProcessingPredicate {
        self.processing_predicate.as_ref()
    }

    /// java: private ProcessingResult processQuadInner(quad, sprite, blockView,
    /// appearanceState, state, pos, randomSupplier, pass, context).
    fn process_quad_inner(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        _pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        let orientation = self.orientation_mode.get_orientation(
            quad.light_face(),
            block_state_axis_index(appearance_state),
        );
        let directions = get_map(quad.light_face())[orientation as usize];
        let keys = processing_data_keys();
        let mutable_pos = context.as_data_provider().get_data(&keys.mutable_pos);
        let connections = CtmSpriteProvider::get_connections(
            &directions,
            self.connection_predicate.as_ref(),
            self.inner_seams,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );

        // Replacement sprite
        if let Some(replacement_sprites) = &self.replacement_sprites {
            let ctm_index = SPRITE_INDEX_MAP[connections as usize];
            let replacement_sprite = &replacement_sprites[ctm_index as usize];
            if let Some(replacement_sprite) = replacement_sprite {
                if !is_missing_sprite(replacement_sprite) {
                    crate::quad_util::interpolate(quad, sprite, replacement_sprite);
                }
                return ProcessingResult::NextPass;
            }
        }

        let un0 = get_lerp_progress_f32(quad.u(0), sprite.min_u(), sprite.max_u()) - 0.5;
        let un1 = get_lerp_progress_f32(quad.u(1), sprite.min_u(), sprite.max_u()) - 0.5;
        let un2 = get_lerp_progress_f32(quad.u(2), sprite.min_u(), sprite.max_u()) - 0.5;
        let un3 = get_lerp_progress_f32(quad.u(3), sprite.min_u(), sprite.max_u()) - 0.5;
        let vn0 = get_lerp_progress_f32(quad.v(0), sprite.min_v(), sprite.max_v()) - 0.5;
        let vn1 = get_lerp_progress_f32(quad.v(1), sprite.min_v(), sprite.max_v()) - 0.5;
        let vn2 = get_lerp_progress_f32(quad.v(2), sprite.min_v(), sprite.max_v()) - 0.5;
        let vn3 = get_lerp_progress_f32(quad.v(3), sprite.min_v(), sprite.max_v()) - 0.5;

        let u_signum0 = java_signum_f32(un0);
        let u_signum1 = java_signum_f32(un1);
        let u_signum2 = java_signum_f32(un2);
        let u_signum3 = java_signum_f32(un3);
        let v_signum0 = java_signum_f32(vn0);
        let v_signum1 = java_signum_f32(vn1);
        let v_signum2 = java_signum_f32(vn2);
        let v_signum3 = java_signum_f32(vn3);

        let u_split01 = Self::should_split_uv(u_signum0, u_signum1);
        let u_split12 = Self::should_split_uv(u_signum1, u_signum2);
        let u_split23 = Self::should_split_uv(u_signum2, u_signum3);
        let u_split30 = Self::should_split_uv(u_signum3, u_signum0);
        let v_split01 = Self::should_split_uv(v_signum0, v_signum1);
        let v_split12 = Self::should_split_uv(v_signum1, v_signum2);
        let v_split23 = Self::should_split_uv(v_signum2, v_signum3);
        let v_split30 = Self::should_split_uv(v_signum3, v_signum0);

        // Center split
        if (u_split01 & v_split01) | (u_split12 & v_split12) | (u_split23 & v_split23) | (u_split30 & v_split30) {
            // Midpoint of the positions of all 4 vertices
            let x = (quad.x(0) + quad.x(1) + quad.x(2) + quad.x(3)) * 0.25;
            let y = (quad.y(0) + quad.y(1) + quad.y(2) + quad.y(3)) * 0.25;
            let z = (quad.z(0) + quad.z(1) + quad.z(2) + quad.z(3)) * 0.25;

            let keys = processing_data_keys();
            let vertex_container = context.as_data_provider().get_data(&keys.vertex_container);
            let mut vc_borrow = vertex_container.borrow_mut();
            let vc: &mut VertexContainer = vc_borrow.downcast_mut::<VertexContainer>().unwrap();
            vc.fill_base_vertices(quad);

            let mut quad_emitter = context.get_extra_quad_emitter();

            let u_split = (u_split01 & u_split23) | (u_split12 & u_split30);
            let v_split = (v_split01 & v_split23) | (v_split12 & v_split30);

            if u_split & v_split {
                let (delta01, delta23, delta12, delta30, delta4);
                if u_split01 {
                    delta01 = get_lerp_progress_f32(0.0, un0, un1);
                    delta23 = get_lerp_progress_f32(0.0, un2, un3);
                    delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                    delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                    delta4 = get_lerp_progress_f32(
                        0.0,
                        lerp_f32(delta01, vn0, vn1),
                        lerp_f32(delta23, vn2, vn3),
                    );
                } else {
                    delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                    delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                    delta12 = get_lerp_progress_f32(0.0, un1, un2);
                    delta30 = get_lerp_progress_f32(0.0, un3, un0);
                    delta4 = get_lerp_progress_f32(
                        0.0,
                        lerp_f32(delta01, un0, un1),
                        lerp_f32(delta23, un2, un3),
                    );
                }
                vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                vc.vertex4.set_lerped(delta4, &vc.vertex01, &vc.vertex23);

                let quadrant_index_map = &quadrant_index_maps()[orientation as usize];
                self.split_quadrant(
                    quad,
                    sprite,
                    quad_emitter,
                    vc,
                    quadrant_index_map[0],
                    Self::get_sprite_index(quadrant_index_map[0], connections),
                );
                self.split_quadrant(
                    quad,
                    sprite,
                    quad_emitter,
                    vc,
                    quadrant_index_map[1],
                    Self::get_sprite_index(quadrant_index_map[1], connections),
                );
                self.split_quadrant(
                    quad,
                    sprite,
                    quad_emitter,
                    vc,
                    quadrant_index_map[2],
                    Self::get_sprite_index(quadrant_index_map[2], connections),
                );
                self.split_quadrant(
                    quad,
                    sprite,
                    quad_emitter,
                    vc,
                    quadrant_index_map[3],
                    Self::get_sprite_index(quadrant_index_map[3], connections),
                );
            } else {
                // Java: if (!(split01 | split12)) ... else if ... (fold with ||
                // short-circuit; each arm runs only when earlier ones did not).
                let u_split01 = u_split01;
                let mut split01 = u_split01;
                let mut split12 = u_split12;
                let mut split23 = u_split23;
                let mut split30 = u_split30;
                if !(split01 || split12) {
                    split12 = true;
                } else if !(split12 || split23) {
                    split23 = true;
                } else if !(split23 || split30) {
                    split30 = true;
                } else if !(split30 || split01) {
                    split01 = true;
                }

                let splits =
                    (split01 as i32) + (split12 as i32) + (split23 as i32) + (split30 as i32);

                if splits == 2 {
                    if split01 {
                        let (delta01, delta23);
                        if u_split01 {
                            delta01 = get_lerp_progress_f32(0.0, un0, un1);
                            delta23 = get_lerp_progress_f32(0.0, un2, un3);
                        } else {
                            delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                            delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                        }
                        vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                        vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                        self.split_half(quad, sprite, quad_emitter, vc, 1, Self::get_sprite_index(1, connections));
                        self.split_half(quad, sprite, quad_emitter, vc, 3, Self::get_sprite_index(3, connections));
                    } else {
                        let (delta12, delta30);
                        if u_split01 {
                            delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                            delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                        } else {
                            delta12 = get_lerp_progress_f32(0.0, un1, un2);
                            delta30 = get_lerp_progress_f32(0.0, un3, un0);
                        }
                        vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                        vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                        self.split_half(quad, sprite, quad_emitter, vc, 0, Self::get_sprite_index(0, connections));
                        self.split_half(quad, sprite, quad_emitter, vc, 2, Self::get_sprite_index(2, connections));
                    }
                } else if !split01 {
                    let (delta23, delta12, delta30, delta4);
                    if u_split01 {
                        delta23 = get_lerp_progress_f32(0.0, un2, un3);
                        delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                        delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta12, un1, un2),
                            lerp_f32(delta30, un3, un0),
                        );
                    } else {
                        delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                        delta12 = get_lerp_progress_f32(0.0, un1, un2);
                        delta30 = get_lerp_progress_f32(0.0, un3, un0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta12, vn1, vn2),
                            lerp_f32(delta30, vn3, vn0),
                        );
                    }
                    vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                    vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                    vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                    vc.vertex4.set_lerped(delta4, &vc.vertex12, &vc.vertex30);
                    self.split_half(quad, sprite, quad_emitter, vc, 0, Self::get_sprite_index(0, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 2, Self::get_sprite_index(2, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 3, Self::get_sprite_index(3, connections));
                } else if !split12 {
                    let (delta01, delta23, delta30, delta4);
                    if u_split01 {
                        delta01 = get_lerp_progress_f32(0.0, un0, un1);
                        delta23 = get_lerp_progress_f32(0.0, un2, un3);
                        delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta01, vn0, vn1),
                            lerp_f32(delta23, vn2, vn3),
                        );
                    } else {
                        delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                        delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                        delta30 = get_lerp_progress_f32(0.0, un3, un0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta01, un0, un1),
                            lerp_f32(delta23, un2, un3),
                        );
                    }
                    vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                    vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                    vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                    vc.vertex4.set_lerped(delta4, &vc.vertex01, &vc.vertex23);
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 0, Self::get_sprite_index(0, connections));
                    self.split_half(quad, sprite, quad_emitter, vc, 1, Self::get_sprite_index(1, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 3, Self::get_sprite_index(3, connections));
                } else if !split23 {
                    let (delta01, delta12, delta30, delta4);
                    if u_split01 {
                        delta01 = get_lerp_progress_f32(0.0, un0, un1);
                        delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                        delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta12, un1, un2),
                            lerp_f32(delta30, un3, un0),
                        );
                    } else {
                        delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                        delta12 = get_lerp_progress_f32(0.0, un1, un2);
                        delta30 = get_lerp_progress_f32(0.0, un3, un0);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta12, vn1, vn2),
                            lerp_f32(delta30, vn3, vn0),
                        );
                    }
                    vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                    vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                    vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                    vc.vertex4.set_lerped(delta4, &vc.vertex12, &vc.vertex30);
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 0, Self::get_sprite_index(0, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 1, Self::get_sprite_index(1, connections));
                    self.split_half(quad, sprite, quad_emitter, vc, 2, Self::get_sprite_index(2, connections));
                } else {
                    // !split30
                    let (delta01, delta23, delta12, delta4);
                    if u_split01 {
                        delta01 = get_lerp_progress_f32(0.0, un0, un1);
                        delta23 = get_lerp_progress_f32(0.0, un2, un3);
                        delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta01, vn0, vn1),
                            lerp_f32(delta23, vn2, vn3),
                        );
                    } else {
                        delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                        delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                        delta12 = get_lerp_progress_f32(0.0, un1, un2);
                        delta4 = get_lerp_progress_f32(
                            0.0,
                            lerp_f32(delta01, un0, un1),
                            lerp_f32(delta23, un2, un3),
                        );
                    }
                    vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                    vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                    vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                    vc.vertex4.set_lerped(delta4, &vc.vertex01, &vc.vertex23);
                    self.split_half(quad, sprite, quad_emitter, vc, 3, Self::get_sprite_index(3, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 1, Self::get_sprite_index(1, connections));
                    self.split_quadrant(quad, sprite, quad_emitter, vc, 2, Self::get_sprite_index(2, connections));
                }
            }

            context.mark_has_extra_quads();
            return ProcessingResult::Discard;
        }

        let u_split = (u_split01 & u_split23) | (u_split12 & u_split30);
        let v_split = (v_split01 & v_split23) | (v_split12 & v_split30);

        if u_split & v_split {
            // Quadrant
            let u_sum = u_signum0 + u_signum1 + u_signum2 + u_signum3;
            let v_sum = v_signum0 + v_signum1 + v_signum2 + v_signum3;
            let quadrant: i32;
            if u_sum <= 0 {
                if v_sum <= 0 {
                    quadrant = 0;
                } else {
                    quadrant = 1;
                }
            } else if v_sum <= 0 {
                quadrant = 3;
            } else {
                quadrant = 2;
            }
            let sprite_index = Self::get_sprite_index(quadrant, connections);
            Self::try_interpolate(quad, &self.sprites, sprite, sprite_index);
            return ProcessingResult::Stop;
        }

        if u_split | v_split {
            // Split in half
            let first_split: bool;
            let mut swap_ab: bool;
            let mut sprite_index_a: i32;
            let mut sprite_index_b: i32;
            if u_split {
                first_split = u_split01;
                swap_ab = orientation == 2 || orientation == 3 || orientation == 4 || orientation == 5;
                if u_signum0 + u_signum1 + u_signum2 + u_signum3 <= 0 {
                    sprite_index_a = Self::get_sprite_index(0, connections);
                    sprite_index_b = Self::get_sprite_index(3, connections);
                } else {
                    sprite_index_a = Self::get_sprite_index(1, connections);
                    sprite_index_b = Self::get_sprite_index(2, connections);
                }
            } else {
                first_split = v_split01;
                swap_ab = orientation == 1 || orientation == 2 || orientation == 5 || orientation == 6;
                if v_signum0 + v_signum1 + v_signum2 + v_signum3 <= 0 {
                    sprite_index_a = Self::get_sprite_index(1, connections);
                    sprite_index_b = Self::get_sprite_index(0, connections);
                } else {
                    sprite_index_a = Self::get_sprite_index(2, connections);
                    sprite_index_b = Self::get_sprite_index(3, connections);
                }
            }

            if sprite_index_a == sprite_index_b {
                Self::try_interpolate(quad, &self.sprites, sprite, sprite_index_a);
                return ProcessingResult::Stop;
            }
            if swap_ab {
                std::mem::swap(&mut sprite_index_a, &mut sprite_index_b);
            }
            let _ = swap_ab;
            swap_ab = false;
            let _ = swap_ab;

            let keys = processing_data_keys();
            let vertex_container = context.as_data_provider().get_data(&keys.vertex_container);
            let mut vc_borrow = vertex_container.borrow_mut();
            let vc: &mut VertexContainer = vc_borrow.downcast_mut::<VertexContainer>().unwrap();
            vc.fill_base_vertices(quad);

            let mut quad_emitter = context.get_extra_quad_emitter();

            if first_split {
                let (delta01, delta23);
                if u_split {
                    delta01 = get_lerp_progress_f32(0.0, un0, un1);
                    delta23 = get_lerp_progress_f32(0.0, un2, un3);
                } else {
                    delta01 = get_lerp_progress_f32(0.0, vn0, vn1);
                    delta23 = get_lerp_progress_f32(0.0, vn2, vn3);
                }
                vc.vertex01.set_lerped(delta01, &vc.v0, &vc.v1);
                vc.vertex23.set_lerped(delta23, &vc.v2, &vc.v3);
                self.split_half(quad, sprite, quad_emitter, vc, 1, sprite_index_a);
                self.split_half(quad, sprite, quad_emitter, vc, 3, sprite_index_b);
            } else {
                let (delta12, delta30);
                if u_split {
                    delta12 = get_lerp_progress_f32(0.0, un1, un2);
                    delta30 = get_lerp_progress_f32(0.0, un3, un0);
                } else {
                    delta12 = get_lerp_progress_f32(0.0, vn1, vn2);
                    delta30 = get_lerp_progress_f32(0.0, vn3, vn0);
                }
                vc.vertex12.set_lerped(delta12, &vc.v1, &vc.v2);
                vc.vertex30.set_lerped(delta30, &vc.v3, &vc.v0);
                self.split_half(quad, sprite, quad_emitter, vc, 0, sprite_index_a);
                self.split_half(quad, sprite, quad_emitter, vc, 2, sprite_index_b);
            }

            context.mark_has_extra_quads();
            return ProcessingResult::Discard;
        }

        let u_sum = u_signum0 + u_signum1 + u_signum2 + u_signum3;
        let v_sum = v_signum0 + v_signum1 + v_signum2 + v_signum3;
        let quadrant: i32;
        if u_sum <= 0 {
            if v_sum <= 0 {
                quadrant = 0;
            } else {
                quadrant = 1;
            }
        } else if v_sum <= 0 {
            quadrant = 3;
        } else {
            quadrant = 2;
        }
        let sprite_index = Self::get_sprite_index(quadrant, connections);
        Self::try_interpolate(quad, &self.sprites, sprite, sprite_index);
        ProcessingResult::Stop
    }
}

impl CompactCtmQuadProcessor {
    /// java: private static boolean shouldSplitUV(int signumA, int signumB).
    fn should_split_uv(signum_a: i32, signum_b: i32) -> bool {
        (signum_a ^ signum_b) == -2
    }

    /// java: private int getSpriteIndex(int quadrantIndex, int connections).
    fn get_sprite_index(quadrant_index: i32, connections: i32) -> i32 {
        let index1 = quadrant_index;
        let index2 = (quadrant_index + 3) % 4;
        let connected1 = ((connections >> (index1 * 2)) & 1) == 1;
        let connected2 = ((connections >> (index2 * 2)) & 1) == 1;
        if connected1 && connected2 {
            if ((connections >> (index2 * 2 + 1)) & 1) == 1 {
                return 1;
            }
            return 4;
        }
        if connected1 {
            return 3 - (quadrant_index % 2);
        }
        if connected2 {
            return 2 + (quadrant_index % 2);
        }
        0
    }

    /// java: private static void tryInterpolate(quad, sprites, oldSprite, spriteIndex).
    fn try_interpolate(
        quad: &QuadView,
        sprites: &[Option<Sprite>],
        old_sprite: &Sprite,
        sprite_index: i32,
    ) {
        let new_sprite = &sprites[sprite_index as usize];
        if let Some(new_sprite) = new_sprite {
            if !is_missing_sprite(new_sprite) {
                crate::quad_util::interpolate(quad, old_sprite, new_sprite);
            }
        }
    }

    /// java: private void splitHalf(quad, sprite, quadEmitter, vertexContainer, id,
    /// spriteIndex).
    fn split_half(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        quad_emitter: &mut QuadEmitter,
        vertex_container: &VertexContainer,
        id: i32,
        sprite_index: i32,
    ) {
        quad_emitter.copy_from(quad);
        vertex_container.lerped_vertices[((id + 1) % 4) as usize]
            .write_to_quad(quad_emitter, ((id + 2) % 4) as usize);
        let id3 = (id + 3) % 4;
        vertex_container.lerped_vertices[id3 as usize].write_to_quad(quad_emitter, id3 as usize);
        Self::try_interpolate(&mut quad_emitter.current, &self.sprites, sprite, sprite_index);
        quad_emitter.emit();
    }

    /// java: private void splitQuadrant(quad, sprite, quadEmitter, vertexContainer, id,
    /// spriteIndex).
    fn split_quadrant(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        quad_emitter: &mut QuadEmitter,
        vertex_container: &VertexContainer,
        id: i32,
        sprite_index: i32,
    ) {
        quad_emitter.copy_from(quad);
        vertex_container.lerped_vertices[id as usize]
            .write_to_quad(quad_emitter, ((id + 1) % 4) as usize);
        vertex_container.vertex4.write_to_quad(quad_emitter, ((id + 2) % 4) as usize);
        let id3 = (id + 3) % 4;
        vertex_container.lerped_vertices[id3 as usize].write_to_quad(quad_emitter, id3 as usize);
        Self::try_interpolate(&mut quad_emitter.current, &self.sprites, sprite, sprite_index);
        quad_emitter.emit();
    }
}

/// java: CompactCtmQuadProcessor.Vertex.
#[derive(Clone, Debug)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: i32,
    pub lightmap: i32,
    pub u: f32,
    pub v: f32,
    pub has_normal: bool,
    pub normal_x: f32,
    pub normal_y: f32,
    pub normal_z: f32,
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            color: -1,
            lightmap: 0,
            u: 0.0,
            v: 0.0,
            has_normal: false,
            normal_x: 0.0,
            normal_y: 0.0,
            normal_z: 0.0,
        }
    }
}

impl Vertex {
    /// java: void setLerped(float delta, Vertex a, Vertex b).
    pub fn set_lerped(&mut self, delta: f32, a: &Vertex, b: &Vertex) {
        self.x = lerp_f32(delta, a.x, b.x);
        self.y = lerp_f32(delta, a.y, b.y);
        self.z = lerp_f32(delta, a.z, b.z);
        self.color = lerp_color(delta, a.color, b.color);
        self.lightmap = lerp_light(delta, a.lightmap, b.lightmap);
        self.u = lerp_f32(delta, a.u, b.u);
        self.v = lerp_f32(delta, a.v, b.v);
        self.has_normal = a.has_normal && b.has_normal;
        if self.has_normal {
            self.normal_x = lerp_f32(delta, a.normal_x, b.normal_x);
            self.normal_y = lerp_f32(delta, a.normal_y, b.normal_y);
            self.normal_z = lerp_f32(delta, a.normal_z, b.normal_z);
        } else {
            self.normal_x = 0.0;
            self.normal_y = 0.0;
            self.normal_z = 0.0;
        }
    }

    /// java: void writeToQuad(MutableQuadView quad, int vertexIndex). The emitter is
    /// the MutableQuadView in the port.
    pub fn write_to_quad(&self, quad: &mut QuadEmitter, vertex_index: usize) {
        quad.pos(vertex_index, self.x, self.y, self.z);
        quad.color(vertex_index, self.color);
        quad.lightmap(vertex_index, self.lightmap);
        quad.uv(vertex_index, self.u, self.v);
        if self.has_normal {
            quad.normal(vertex_index, self.normal_x, self.normal_y, self.normal_z);
        }
    }
}

/// java: CompactCtmQuadProcessor.VertexContainer implements ProcessingData.
#[derive(Clone, Debug, Default)]
pub struct VertexContainer {
    v0: Vertex,
    v1: Vertex,
    v2: Vertex,
    v3: Vertex,
    vertex01: Vertex,
    vertex12: Vertex,
    vertex23: Vertex,
    vertex30: Vertex,
    vertex4: Vertex,
    lerped_vertices: [Vertex; 4],
}

impl VertexContainer {
    pub fn new() -> VertexContainer {
        VertexContainer {
            v0: Vertex::default(),
            v1: Vertex::default(),
            v2: Vertex::default(),
            v3: Vertex::default(),
            vertex01: Vertex::default(),
            vertex12: Vertex::default(),
            vertex23: Vertex::default(),
            vertex30: Vertex::default(),
            vertex4: Vertex::default(),
            lerped_vertices: [Vertex::default(), Vertex::default(), Vertex::default(), Vertex::default()],
        }
    }

    /// java: void fillBaseVertices(QuadView quad).
    pub fn fill_base_vertices(&mut self, quad: &QuadView) {
        self.v0 = Vertex {
            x: quad.x(0),
            y: quad.y(0),
            z: quad.z(0),
            color: quad.color(0),
            lightmap: quad.lightmap(0),
            u: quad.u(0),
            v: quad.v(0),
            has_normal: quad.has_normal(0),
            normal_x: quad.normal_x(0),
            normal_y: quad.normal_y(0),
            normal_z: quad.normal_z(0),
        };
        self.v1 = Vertex {
            x: quad.x(1),
            y: quad.y(1),
            z: quad.z(1),
            color: quad.color(1),
            lightmap: quad.lightmap(1),
            u: quad.u(1),
            v: quad.v(1),
            has_normal: quad.has_normal(1),
            normal_x: quad.normal_x(1),
            normal_y: quad.normal_y(1),
            normal_z: quad.normal_z(1),
        };
        self.v2 = Vertex {
            x: quad.x(2),
            y: quad.y(2),
            z: quad.z(2),
            color: quad.color(2),
            lightmap: quad.lightmap(2),
            u: quad.u(2),
            v: quad.v(2),
            has_normal: quad.has_normal(2),
            normal_x: quad.normal_x(2),
            normal_y: quad.normal_y(2),
            normal_z: quad.normal_z(2),
        };
        self.v3 = Vertex {
            x: quad.x(3),
            y: quad.y(3),
            z: quad.z(3),
            color: quad.color(3),
            lightmap: quad.lightmap(3),
            u: quad.u(3),
            v: quad.v(3),
            has_normal: quad.has_normal(3),
            normal_x: quad.normal_x(3),
            normal_y: quad.normal_y(3),
            normal_z: quad.normal_z(3),
        };
    }
}

/// java: CompactCtmQuadProcessor.Factory implements QuadProcessor.Factory
/// <CompactConnectingCtmProperties>.
pub struct CompactCtmQuadProcessorFactory;

impl CompactCtmQuadProcessorFactory {
    /// java: QuadProcessor createProcessor(CompactConnectingCtmProperties properties,
    /// Sprite[] sprites, Sprite[] replacementSprites).
    pub fn create_processor(
        &self,
        properties: &crate::connecting_properties::CompactConnectingCtmProperties,
        sprites: Vec<Option<Sprite>>,
        replacement_sprites: Option<Vec<Option<Sprite>>>,
    ) -> Box<dyn QuadProcessor> {
        // java: BaseProcessingPredicate.fromProperties(properties).
        let processing_predicate: Rc<dyn ProcessingPredicate> = Rc::new(
            crate::processor_base::BaseProcessingPredicate::from_properties(properties.base()),
        );
        // java: getConnectionPredicate() (the loader rejects invalid properties before
        // this runs, so the predicate is always present).
        let connection_predicate: Rc<dyn ConnectionPredicate> = Rc::new(
            properties.base.base.base.get_connection_predicate()
                .expect("connection predicate missing"),
        );
        let inner_seams = properties.base.base.get_inner_seams();
        let orientation_mode = properties.get_orientation_mode();
        Box::new(CompactCtmQuadProcessor::new(
            sprites,
            processing_predicate,
            connection_predicate,
            inner_seams,
            orientation_mode,
            replacement_sprites,
        ))
    }

    /// java: createProcessorWithTextures(properties, textureGetter).
    pub fn create_processor_with_textures(
        &self,
        properties: &crate::connecting_properties::CompactConnectingCtmProperties,
        texture_getter: &crate::processor_base::TextureGetter,
    ) -> Box<dyn QuadProcessor> {
        let texture_amount = self.get_texture_amount(properties);
        let sprite_ids: Vec<crate::seams::SpriteId> =
            properties.base().sprite_ids.clone().unwrap_or_default();
        let provided_amount = sprite_ids.len();
        let mut max_amount = provided_amount;

        let replacement_map: Option<&HashMap<i32, i32>> = properties.get_tile_replacement_map();
        let mut replacement_sprites: Option<Vec<Option<Sprite>>> = None;
        if let Some(replacement_map) = replacement_map {
            let replacement_texture_amount = self.get_replacement_texture_amount(properties);
            let mut map = vec![None; replacement_texture_amount];
            let mut keys: Vec<i32> = replacement_map.keys().copied().collect();
            keys.sort_unstable();
            for key in keys {
                let value = replacement_map[&key];
                if (key as usize) < replacement_texture_amount {
                    if (value as usize) < provided_amount {
                        map[key as usize] = texture_getter(&sprite_ids[value as usize]);
                    } else {
                        // java: LOGGER.warn("Cannot replace tile %d with tile %d as only %d tiles were provided in file '%s' in pack '%s'").
                        let base = properties.base();
                        eprintln!("[continuity] WARN: Cannot replace tile {key} with tile {value} as only {provided_amount} tiles were provided in file '{}' in pack '{}'", base.resource_id, base.pack_id);
                    }
                } else {
                    // java: LOGGER.warn("Cannot replace tile %d as method '%s' only supports %d replacement tiles in file '%s' in pack '%s'").
                    let base = properties.base();
                    eprintln!("[continuity] WARN: Cannot replace tile {key} as method '{}' only supports {replacement_texture_amount} replacement tiles in file '{}' in pack '{}'", base.method, base.resource_id, base.pack_id);
                }
            }
            replacement_sprites = Some(map);
        }

        if provided_amount > texture_amount {
            if replacement_sprites.is_none() {
                // java: LOGGER.warn("Method '%s' requires %d tiles but %d were provided in file '%s' in pack '%s'").
                let base = properties.base();
                eprintln!("[continuity] WARN: Method '{}' requires {texture_amount} tiles but {provided_amount} were provided in file '{}' in pack '{}'", base.method, base.resource_id, base.pack_id);
            }
            max_amount = texture_amount;
        }

        let mut sprites: Vec<Option<Sprite>> = vec![None; texture_amount];
        let missing_sprite: Sprite = texture_getter(&missing_sprite_id())
            .unwrap_or_else(|| sprite_for_id(missing_sprite_id()));
        let supports_null_sprites = self.supports_null_sprites(properties);
        for i in 0..max_amount {
            let sprite_id = &sprite_ids[i];
            let sprite: Option<Sprite> = if *sprite_id == BaseCtmProperties::special_skip_sprite_id() {
                // java: special skip.
                None
            } else if *sprite_id == BaseCtmProperties::special_default_sprite_id() {
                // java: special default.
                if supports_null_sprites {
                    None
                } else {
                    Some(missing_sprite.clone())
                }
            } else {
                texture_getter(sprite_id)
            };
            sprites[i] = sprite;
        }

        if provided_amount < texture_amount {
            // java: LOGGER.error("Method '%s' requires at least %d tiles but only %d were provided in file '%s' in pack '%s'").
            let base = properties.base();
            eprintln!("[continuity] ERROR: Method '{}' requires at least {texture_amount} tiles but only {provided_amount} were provided in file '{}' in pack '{}'", base.method, base.resource_id, base.pack_id);
            for i in provided_amount..texture_amount {
                sprites[i] = Some(missing_sprite.clone());
            }
        }

        self.create_processor(properties, sprites, replacement_sprites)
    }

    /// java: getTextureAmount(properties).
    pub fn get_texture_amount(&self, _properties: &crate::connecting_properties::CompactConnectingCtmProperties) -> usize {
        5
    }

    /// java: getReplacementTextureAmount(properties).
    pub fn get_replacement_texture_amount(
        &self,
        _properties: &crate::connecting_properties::CompactConnectingCtmProperties,
    ) -> usize {
        47
    }

    /// java: supportsNullSprites(properties).
    pub fn supports_null_sprites(
        &self,
        _properties: &crate::connecting_properties::CompactConnectingCtmProperties,
    ) -> bool {
        false
    }
}

/// java: Properties.AXIS on the appearance state, carried as the axis's enum ordinal.
fn block_state_axis_index(state: &BlockState) -> Option<usize> {
    crate::processor_base::block_state_get_axis(state).map(|axis| match axis {
        crate::processor_enums::Axis::X => 0,
        crate::processor_enums::Axis::Y => 1,
        crate::processor_enums::Axis::Z => 2,
    })
}

impl crate::processor_base::AbstractQuadProcessorFactory<
    crate::connecting_properties::CompactConnectingCtmProperties,
> for CompactCtmQuadProcessorFactory
{
    /// java: createProcessor(properties, sprites) - the plain path carries no replacement
    /// sprites.
    fn create_processor(
        &self,
        properties: &crate::connecting_properties::CompactConnectingCtmProperties,
        sprites: Vec<Option<Sprite>>,
    ) -> Box<dyn crate::processor_base::QuadProcessor> {
        CompactCtmQuadProcessorFactory::create_processor(self, properties, sprites, None)
    }

    /// java: createProcessor(properties, textureGetter); the inherent method carries the
    /// real body and the fully-qualified call resolves to it (inherent wins over the
    /// trait method), so this does not recurse.
    fn create_processor_with_textures(
        &self,
        properties: &crate::connecting_properties::CompactConnectingCtmProperties,
        texture_getter: &crate::processor_base::TextureGetter,
    ) -> Box<dyn crate::processor_base::QuadProcessor> {
        CompactCtmQuadProcessorFactory::create_processor_with_textures(self, properties, texture_getter)
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(
        &self,
        properties: &crate::connecting_properties::CompactConnectingCtmProperties,
    ) -> usize {
        CompactCtmQuadProcessorFactory::get_texture_amount(self, properties)
    }

    /// java: supportsNullSprites(properties).
    fn supports_null_sprites(
        &self,
        _properties: &crate::connecting_properties::CompactConnectingCtmProperties,
    ) -> bool {
        false
    }
}
