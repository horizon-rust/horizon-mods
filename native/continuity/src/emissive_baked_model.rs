//! 1:1 of the Continuity client.model.EmissiveBakedModel (+ its two quad
//! transforms) and the EmissiveSpriteApi plumbing it reads, by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity). Upstream files:
//! src/main/java/me/pepperbell/continuity/client/model/EmissiveBakedModel.java,
//! impl/client/EmissiveSpriteApiImpl.java, api/client/EmissiveSpriteApi.java.

use std::cell::RefCell;
use std::rc::Rc;

use crate::config::config_instance;
use crate::model::{BakedModel, ContinuityFeatureStates, ItemStack, model_objects_container};
use crate::processor_base::{
    Random, Sprite, BlockState, BlockRenderView, BlockPos, MutableQuadView, QuadEmitter,
};
use crate::quad_util::interpolate;
use crate::render_api::{
    cutout_mipped_emissive_material, default_emissive_material, emissive_materials,
    render_layers_get_block_layer_is_solid, render_util_sprite_finder_find, RenderContextHandle,
};
use crate::seams::sprite_for_id;
use crate::processor_base::BlendMode;

/// java: api.client.EmissiveSpriteApi.
pub trait EmissiveSpriteApi {
    /// java: getEmissiveSprite(Sprite sprite).
    fn get_emissive_sprite(&self, sprite: &Sprite) -> Option<Sprite>;
}

/// java: impl.client.EmissiveSpriteApiImpl.
pub struct EmissiveSpriteApiImpl;

/// java: EmissiveSpriteApiImpl.INSTANCE singleton.
pub fn emissive_sprite_api() -> Rc<dyn EmissiveSpriteApi> {
    // java: a single public instance; the port hands the stateless impl out per call.
    Rc::new(EmissiveSpriteApiImpl)
}

impl EmissiveSpriteApi for EmissiveSpriteApiImpl {
    /// java: ((SpriteExtension) sprite).continuity$getEmissiveSprite() (the
    /// mixin-injected field, carried on the Sprite).
    fn get_emissive_sprite(&self, sprite: &Sprite) -> Option<Sprite> {
        sprite.emissive_sprite.clone().map(sprite_for_id)
    }
}

/// java: client.model.EmissiveBakedModel.
pub struct EmissiveBakedModel {
    /// java: ForwardingBakedModel.wrapped.
    pub wrapped: Rc<dyn BakedModel>,
}

impl EmissiveBakedModel {
    /// java: EmissiveBakedModel(BakedModel wrapped).
    pub fn new(wrapped: Rc<dyn BakedModel>) -> EmissiveBakedModel {
        EmissiveBakedModel { wrapped }
    }

    /// java: emitBlockQuads(...).
    pub fn emit_block_quads(
        &self,
        block_view: &BlockRenderView,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        context: &RenderContextHandle,
    ) {
        if !config_instance().emissive_textures.get() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        let container = model_objects_container();
        if !container.feature_states.get_emissive_textures_state().is_enabled_state() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        let quad_transform = Rc::clone(&container.emissive_block_quad_transform);
        if quad_transform.borrow().is_active() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        let mesh_builder = Rc::clone(&container.mesh_builder);
        let emitter = mesh_builder.clone();
        quad_transform.borrow_mut().prepare(emitter, state.clone(), Rc::clone(context), config_instance().use_manual_culling.get());

        context.borrow_mut().push_transform();
        self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
        context.borrow_mut().pop_transform();

        if quad_transform.borrow().did_emit() {
            let mesh = mesh_builder.borrow_mut().build();
            let mut ctx_borrow = context.borrow_mut();
            let target = ctx_borrow.get_emitter();
            let mut mesh = mesh;
            mesh.output_to(target);
        }
        quad_transform.borrow_mut().reset();
    }

    /// java: emitItemQuads(ItemStack stack, Supplier<Random> randomSupplier, RenderContext context).
    pub fn emit_item_quads(&self, stack: &ItemStack, random_supplier: &dyn Fn() -> Random, context: &RenderContextHandle) {
        if !config_instance().emissive_textures.get() {
            self.wrapped.emit_item_quads(stack, random_supplier, context);
            return;
        }

        let container = model_objects_container();
        if !container.feature_states.get_emissive_textures_state().is_enabled_state() {
            self.wrapped.emit_item_quads(stack, random_supplier, context);
            return;
        }

        let quad_transform = Rc::clone(&container.emissive_item_quad_transform);
        if quad_transform.borrow().is_active() {
            self.wrapped.emit_item_quads(stack, random_supplier, context);
            return;
        }

        let mesh_builder = Rc::clone(&container.mesh_builder);
        let emitter = mesh_builder.clone();
        quad_transform.borrow_mut().prepare(emitter);

        context.borrow_mut().push_transform();
        self.wrapped.emit_item_quads(stack, random_supplier, context);
        context.borrow_mut().pop_transform();

        if quad_transform.borrow().did_emit() {
            let mesh = mesh_builder.borrow_mut().build();
            let mut ctx_borrow = context.borrow_mut();
            let target = ctx_borrow.get_emitter();
            let mut mesh = mesh;
            mesh.output_to(target);
        }
        quad_transform.borrow_mut().reset();
    }

    /// java: isVanillaAdapter().
    pub fn is_vanilla_adapter(&self) -> bool {
        if !config_instance().emissive_textures.get() {
            return true;
        }
        false
    }
}

impl BakedModel for EmissiveBakedModel {
    fn emit_block_quads(
        &self,
        block_view: &BlockRenderView,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        context: &RenderContextHandle,
    ) {
        EmissiveBakedModel::emit_block_quads(self, block_view, state, pos, random_supplier, context)
    }

    fn emit_item_quads(&self, stack: &ItemStack, random_supplier: &dyn Fn() -> Random, context: &RenderContextHandle) {
        EmissiveBakedModel::emit_item_quads(self, stack, random_supplier, context)
    }

    fn is_vanilla_adapter(&self) -> bool {
        EmissiveBakedModel::is_vanilla_adapter(self)
    }
}

/// java: protected static class EmissiveBlockQuadTransform implements
/// RenderContext.QuadTransform.
pub struct EmissiveBlockQuadTransform {
    /// java: QuadEmitter emitter (the container mesh builder's emitter; the port
    /// carries the shared handle).
    emitter: RefCell<Option<Rc<RefCell<QuadEmitter>>>>,
    state: RefCell<Option<BlockState>>,
    render_context: RefCell<Option<RenderContextHandle>>,
    use_manual_culling: RefCell<bool>,

    active: RefCell<bool>,
    did_emit_flag: RefCell<bool>,
    calculate_default_layer: RefCell<bool>,
    is_default_layer_solid: RefCell<bool>,
}

impl EmissiveBlockQuadTransform {
    pub fn new() -> EmissiveBlockQuadTransform {
        EmissiveBlockQuadTransform {
            emitter: RefCell::new(None),
            state: RefCell::new(None),
            render_context: RefCell::new(None),
            use_manual_culling: RefCell::new(false),
            active: RefCell::new(false),
            did_emit_flag: RefCell::new(false),
            calculate_default_layer: RefCell::new(false),
            is_default_layer_solid: RefCell::new(false),
        }
    }

    /// java: transform(MutableQuadView quad).
    pub fn transform(&self, quad: &MutableQuadView) -> bool {
        if *self.use_manual_culling.borrow()
            && self
                .render_context
                .borrow()
                .as_ref()
                .map(|context| context.borrow().is_face_culled(quad.cull_face()))
                .unwrap_or(false)
        {
            return false;
        }

        let sprite = render_util_sprite_finder_find(quad);
        let emissive_sprite = emissive_sprite_api().get_emissive_sprite(&sprite);
        if let Some(emissive_sprite) = emissive_sprite {
            let emitter = self.emitter.borrow().clone().unwrap();
            {
                let mut emitter_guard = emitter.borrow_mut();
                emitter_guard.copy_from(quad);
            }

            // java: quad.material().blendMode() (the material rides the quad; the port
            // defaults to the DEFAULT blend mode until the engine records it).
            let blend_mode = quad
                .material()
                .map(|material| material.blend_mode)
                .unwrap_or(BlendMode::Default);
            let emissive_material: &'static crate::processor_base::RenderMaterial = if blend_mode == BlendMode::Default {
                if *self.calculate_default_layer.borrow() {
                    *self.is_default_layer_solid.borrow_mut() =
                        render_layers_get_block_layer_is_solid(self.state.borrow().as_ref().unwrap());
                    *self.calculate_default_layer.borrow_mut() = false;
                }

                if *self.is_default_layer_solid.borrow() {
                    cutout_mipped_emissive_material()
                } else {
                    default_emissive_material()
                }
            } else if blend_mode == BlendMode::Solid {
                cutout_mipped_emissive_material()
            } else {
                &emissive_materials()[blend_mode.ordinal()]
            };

            {
                let mut emitter_guard = emitter.borrow_mut();
                emitter_guard.material(emissive_material);
                interpolate(&mut emitter_guard.current, &sprite, &emissive_sprite);
                emitter_guard.emit();
            }
            *self.did_emit_flag.borrow_mut() = true;
        }
        true
    }

    /// java: isActive().
    pub fn is_active(&self) -> bool {
        *self.active.borrow()
    }

    /// java: didEmit().
    pub fn did_emit(&self) -> bool {
        *self.did_emit_flag.borrow()
    }

    /// java: prepare(QuadEmitter emitter, BlockState state, RenderContext
    /// renderContext, boolean useManualCulling).
    pub fn prepare(&mut self, emitter: Rc<RefCell<QuadEmitter>>, state: BlockState, render_context: RenderContextHandle, use_manual_culling: bool) {
        self.emitter = RefCell::new(Some(emitter));
        self.state = RefCell::new(Some(state));
        self.render_context = RefCell::new(Some(render_context));
        self.use_manual_culling = RefCell::new(use_manual_culling);

        *self.active.borrow_mut() = true;
        *self.did_emit_flag.borrow_mut() = false;
        *self.calculate_default_layer.borrow_mut() = true;
        *self.is_default_layer_solid.borrow_mut() = false;
    }

    /// java: reset().
    pub fn reset(&mut self) {
        self.emitter = RefCell::new(None);
        self.state = RefCell::new(None);
        self.render_context = RefCell::new(None);
        self.use_manual_culling = RefCell::new(false);

        *self.active.borrow_mut() = false;
    }
}

impl Default for EmissiveBlockQuadTransform {
    fn default() -> Self {
        EmissiveBlockQuadTransform::new()
    }
}

/// java: protected static class EmissiveItemQuadTransform implements
/// RenderContext.QuadTransform.
pub struct EmissiveItemQuadTransform {
    emitter: RefCell<Option<Rc<RefCell<QuadEmitter>>>>,

    active: RefCell<bool>,
    did_emit_flag: RefCell<bool>,
}

impl EmissiveItemQuadTransform {
    pub fn new() -> EmissiveItemQuadTransform {
        EmissiveItemQuadTransform {
            emitter: RefCell::new(None),
            active: RefCell::new(false),
            did_emit_flag: RefCell::new(false),
        }
    }

    /// java: transform(MutableQuadView quad).
    pub fn transform(&self, quad: &MutableQuadView) -> bool {
        let sprite = render_util_sprite_finder_find(quad);
        let emissive_sprite = emissive_sprite_api().get_emissive_sprite(&sprite);
        if let Some(emissive_sprite) = emissive_sprite {
            let emitter = self.emitter.borrow().clone().unwrap();
            {
                let mut emitter_guard = emitter.borrow_mut();
                emitter_guard.copy_from(quad);
                emitter_guard.material(default_emissive_material());
                interpolate(&mut emitter_guard.current, &sprite, &emissive_sprite);
                emitter_guard.emit();
            }
            *self.did_emit_flag.borrow_mut() = true;
        }
        true
    }

    /// java: isActive().
    pub fn is_active(&self) -> bool {
        *self.active.borrow()
    }

    /// java: didEmit().
    pub fn did_emit(&self) -> bool {
        *self.did_emit_flag.borrow()
    }

    /// java: prepare(QuadEmitter emitter).
    pub fn prepare(&mut self, emitter: Rc<RefCell<QuadEmitter>>) {
        self.emitter = RefCell::new(Some(emitter));

        *self.active.borrow_mut() = true;
        *self.did_emit_flag.borrow_mut() = false;
    }

    /// java: reset().
    pub fn reset(&mut self) {
        *self.active.borrow_mut() = false;
        self.emitter = RefCell::new(None);
    }
}

impl Default for EmissiveItemQuadTransform {
    fn default() -> Self {
        EmissiveItemQuadTransform::new()
    }
}

