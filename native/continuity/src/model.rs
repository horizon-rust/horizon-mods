//! 1:1 of the Continuity client.model layer: ModelObjectsContainer,
//! CtmBakedModel (+ CtmQuadTransform), the BooleanState/feature-state plumbing it
//! reads, and the BakedModel engine surface, by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity). Upstream files:
//! src/main/java/me/pepperbell/continuity/client/model/{ModelObjectsContainer,
//! CtmBakedModel}.java, client/util/BooleanState.java,
//! impl/client/ContinuityFeatureStatesImpl.java,
//! api/client/ContinuityFeatureStates.java.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread_local;

use crate::config::{config_instance, ConfigOption};
use crate::processor_base::{
    Random, Sprite, BlockPos, BlockState, BlockRenderView, MutableQuadView, QuadEmitter,
};
use crate::processing_context_impl::ProcessingContextImpl;
use crate::quad_processors::{get_cache, Slice};
use crate::processor_enums::Direction;
use crate::render_api::{render_util_sprite_finder_find, RenderContextHandle};
use crate::seams::block_state_get_appearance;

/// java: net.minecraft.item.ItemStack (engine surface; a handle only).
pub struct ItemStack;

/// java: net.minecraft.client.render.model.BakedModel (engine surface; the three
/// members the model layer touches).
pub trait BakedModel {
    /// java: emitBlockQuads(BlockRenderView, BlockState, BlockPos, Supplier<Random>, RenderContext).
    fn emit_block_quads(
        &self,
        block_view: &BlockRenderView,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        context: &RenderContextHandle,
    );

    /// java: emitItemQuads(ItemStack, Supplier<Random>, RenderContext).
    fn emit_item_quads(&self, _stack: &ItemStack, _random_supplier: &dyn Fn() -> Random, _context: &RenderContextHandle) {}

    /// java: isVanillaAdapter().
    fn is_vanilla_adapter(&self) -> bool {
        true
    }

    /// java: BakedModel.isBuiltin() (vanilla builtin models override to true).
    fn is_builtin(&self) -> bool {
        false
    }
}

/// java: client.util.BooleanState.
pub struct BooleanState {
    times_enabled: Cell<i32>,
}

impl BooleanState {
    pub fn new() -> BooleanState {
        BooleanState { times_enabled: Cell::new(0) }
    }

    /// java: isEnabled() = timesEnabled > 0.
    pub fn is_enabled(&self) -> bool {
        self.times_enabled.get() > 0
    }

    /// java: enable().
    pub fn enable(&self) {
        self.times_enabled.set(self.times_enabled.get() + 1);
    }

    /// java: disable().
    pub fn disable(&self) {
        self.times_enabled.set(self.times_enabled.get() - 1);
    }
}

impl Default for BooleanState {
    fn default() -> Self {
        BooleanState::new()
    }
}

/// java: api.client.ContinuityFeatureStates.FeatureState (nested in
/// ContinuityFeatureStates).
pub trait FeatureState {
    fn is_enabled_state(&self) -> bool;

    fn enable(&self);

    fn disable(&self);
}

/// java: impl.client.ContinuityFeatureStatesImpl.FeatureStateImpl extends
/// BooleanState implements ContinuityFeatureStates.FeatureState.
pub struct FeatureStateImpl {
    pub state: BooleanState,
}

impl FeatureState for FeatureStateImpl {
    fn is_enabled_state(&self) -> bool {
        self.state.is_enabled()
    }

    fn enable(&self) {
        self.state.enable();
    }

    fn disable(&self) {
        self.state.disable();
    }
}

/// java: api.client.ContinuityFeatureStates.
pub trait ContinuityFeatureStates {
    fn get_connected_textures_state(&self) -> Rc<dyn FeatureState>;

    fn get_emissive_textures_state(&self) -> Rc<dyn FeatureState>;
}

/// java: impl.client.ContinuityFeatureStatesImpl.
pub struct ContinuityFeatureStatesImpl {
    connected_textures_state: Rc<FeatureStateImpl>,
    emissive_textures_state: Rc<FeatureStateImpl>,
}

impl ContinuityFeatureStatesImpl {
    pub fn new() -> ContinuityFeatureStatesImpl {
        // java: instance initializer { connectedTexturesState.enable();
        // emissiveTexturesState.enable(); }.
        let connected = Rc::new(FeatureStateImpl { state: BooleanState::new() });
        let emissive = Rc::new(FeatureStateImpl { state: BooleanState::new() });
        connected.state.enable();
        emissive.state.enable();
        ContinuityFeatureStatesImpl {
            connected_textures_state: connected,
            emissive_textures_state: emissive,
        }
    }

    /// java: static ContinuityFeatureStatesImpl get() =
    /// ModelObjectsContainer.get().featureStates.
    pub fn get() -> Rc<ContinuityFeatureStatesImpl> {
        Rc::clone(&model_objects_container().feature_states)
    }
}

impl Default for ContinuityFeatureStatesImpl {
    fn default() -> Self {
        ContinuityFeatureStatesImpl::new()
    }
}

impl ContinuityFeatureStates for ContinuityFeatureStatesImpl {
    fn get_connected_textures_state(&self) -> Rc<dyn FeatureState> {
        Rc::clone(&self.connected_textures_state) as Rc<dyn FeatureState>
    }

    fn get_emissive_textures_state(&self) -> Rc<dyn FeatureState> {
        Rc::clone(&self.emissive_textures_state) as Rc<dyn FeatureState>
    }
}

/// java: client.model.ModelObjectsContainer. Java holds one per render thread
/// (ThreadLocal); the port carries the same per-thread instance with shared handles.
pub struct ModelObjectsContainer {
    pub ctm_quad_transform: Rc<RefCell<CtmQuadTransform>>,
    pub emissive_block_quad_transform: Rc<RefCell<crate::emissive_baked_model::EmissiveBlockQuadTransform>>,
    pub emissive_item_quad_transform: Rc<RefCell<crate::emissive_baked_model::EmissiveItemQuadTransform>>,
    pub feature_states: Rc<ContinuityFeatureStatesImpl>,
    /// java: MeshBuilder meshBuilder (engine surface; the port carries the emitter).
    pub mesh_builder: Rc<RefCell<QuadEmitter>>,
}

thread_local! {
    static CONTAINER: Rc<ModelObjectsContainer> = Rc::new(build_container());
}

fn build_container() -> ModelObjectsContainer {
    ModelObjectsContainer {
        ctm_quad_transform: Rc::new(RefCell::new(CtmQuadTransform::new())),
        emissive_block_quad_transform: Rc::new(RefCell::new(
            crate::emissive_baked_model::EmissiveBlockQuadTransform::new(),
        )),
        emissive_item_quad_transform: Rc::new(RefCell::new(
            crate::emissive_baked_model::EmissiveItemQuadTransform::new(),
        )),
        feature_states: Rc::new(ContinuityFeatureStatesImpl::new()),
        mesh_builder: Rc::new(RefCell::new(QuadEmitter::default())),
    }
}

/// java: ModelObjectsContainer.get() (the thread-local accessor).
pub fn model_objects_container() -> Rc<ModelObjectsContainer> {
    CONTAINER.with(|container| container.clone())
}

/// java: client.model.CtmBakedModel.
pub struct CtmBakedModel {
    /// java: ForwardingBakedModel.wrapped.
    pub wrapped: Rc<dyn BakedModel>,
    pub default_state: BlockState,
    /// java: volatile Function<Sprite, QuadProcessors.Slice> defaultSliceFunc.
    default_slice_func: RefCell<Option<Rc<dyn Fn(&Sprite) -> Slice>>>,
}

/// java: public static final int PASSES = 4.
pub const PASSES: i32 = 4;

impl CtmBakedModel {
    /// java: CtmBakedModel(BakedModel wrapped, BlockState defaultState).
    pub fn new(wrapped: Rc<dyn BakedModel>, default_state: BlockState) -> CtmBakedModel {
        CtmBakedModel {
            wrapped,
            default_state,
            default_slice_func: RefCell::new(None),
        }
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
        if !config_instance().connected_textures.get() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        let container = model_objects_container();
        if !container.feature_states.get_connected_textures_state().is_enabled_state() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        let quad_transform = Rc::clone(&container.ctm_quad_transform);
        if quad_transform.borrow().is_active() {
            self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
            return;
        }

        // The correct way to get the appearance of the origin state from within a
        // block model is to (1) call getAppearance on the result of
        // blockView.getBlockState(pos) instead of the passed state and (2) pass the
        // pos and world state of the adjacent block as the source pos and source
        // state. (1) is not followed here because at this point in execution, within
        // this call to CtmBakedModel#emitBlockQuads, the state parameter must already
        // contain the world state. Even if this CtmBakedModel is wrapped, then the
        // wrapper must pass the same state as it received because not doing so can
        // cause crashes when the wrapped model is a vanilla multipart model or
        // delegates to one. Thus, getting the world state again is inefficient and
        // unnecessary. (2) is not possible here because the appearance state is
        // necessary to get the slice and only the processors within the slice
        // actually perform checks on adjacent blocks. Likewise, the processors
        // themselves cannot retrieve the appearance state since the correct
        // processors can only be chosen with the initially correct appearance state.
        // Additionally, the side is chosen to always be the first constant of the
        // enum (DOWN) for simplicity. Querying the appearance for all six sides would
        // be more correct, but less efficient. This may be fixed in the future,
        // especially if there is an actual use case for it.
        let appearance_state = block_state_get_appearance(state, block_view, pos, Direction::Down, state, pos);

        let random_seed = random_supplier().next_long();
        quad_transform.borrow_mut().prepare(
            block_view,
            appearance_state.clone(),
            state.clone(),
            pos.clone(),
            random_seed,
            Rc::clone(context),
            config_instance().use_manual_culling.get(),
            self.get_slice_func(&appearance_state),
        );

        context.borrow_mut().push_transform();
        self.wrapped.emit_block_quads(block_view, state, pos, random_supplier, context);
        context.borrow_mut().pop_transform();

        {
            let mut ctx_borrow = context.borrow_mut();
            let emitter = ctx_borrow.get_emitter();
            quad_transform.borrow_mut().processing_context.borrow_mut().output_to(emitter);
        }
        quad_transform.borrow_mut().reset();
    }

    /// java: isVanillaAdapter().
    pub fn is_vanilla_adapter(&self) -> bool {
        if !config_instance().connected_textures.get() {
            return true;
        }
        false
    }

    /// java: getSliceFunc(BlockState state): the default state caches its function,
    /// everything else asks the QuadProcessors cache each time.
    fn get_slice_func(&self, state: &BlockState) -> Rc<dyn Fn(&Sprite) -> Slice> {
        // java: state == defaultState is reference identity; the port compares state
        // identity via state_id.
        if state.is_same_state(&self.default_state) {
            if self.default_slice_func.borrow().is_none() {
                let slice_func = get_cache(state);
                *self.default_slice_func.borrow_mut() = Some(slice_func);
            }
            return self.default_slice_func.borrow().as_ref().unwrap().clone();
        }
        get_cache(state)
    }
}

impl BakedModel for CtmBakedModel {
    fn emit_block_quads(
        &self,
        block_view: &BlockRenderView,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        context: &RenderContextHandle,
    ) {
        CtmBakedModel::emit_block_quads(self, block_view, state, pos, random_supplier, context)
    }

    fn is_vanilla_adapter(&self) -> bool {
        CtmBakedModel::is_vanilla_adapter(self)
    }
}

/// java: protected static class CtmQuadTransform implements RenderContext.QuadTransform.
pub struct CtmQuadTransform {
    pub processing_context: RefCell<ProcessingContextImpl>,
    /// java: final Supplier<Random> randomSupplier with final Random random =
    /// Random.createLocal(), re-seeded per get(); the port carries the seed.
    local_random_seed: Cell<u64>,
    block_view: RefCell<Option<Rc<BlockRenderView>>>,
    appearance_state: RefCell<Option<BlockState>>,
    state: RefCell<Option<BlockState>>,
    pos: RefCell<Option<BlockPos>>,
    random_seed: Cell<u64>,
    render_context: RefCell<Option<RenderContextHandle>>,
    use_manual_culling: Cell<bool>,
    slice_func: RefCell<Option<Rc<dyn Fn(&Sprite) -> Slice>>>,
    active: Cell<bool>,
}

impl CtmQuadTransform {
    pub fn new() -> CtmQuadTransform {
        CtmQuadTransform {
            processing_context: RefCell::new(ProcessingContextImpl::new()),
            local_random_seed: Cell::new(0),
            block_view: RefCell::new(None),
            appearance_state: RefCell::new(None),
            state: RefCell::new(None),
            pos: RefCell::new(None),
            random_seed: Cell::new(0),
            render_context: RefCell::new(None),
            use_manual_culling: Cell::new(false),
            slice_func: RefCell::new(None),
            active: Cell::new(false),
        }
    }

    /// java: transform(MutableQuadView quad).
    pub fn transform(&self, quad: &MutableQuadView) -> bool {
        if self.use_manual_culling.get()
            && self
                .render_context
                .borrow()
                .as_ref()
                .map(|context| context.borrow().is_face_culled(quad.cull_face()))
                .unwrap_or(false)
        {
            return false;
        }

        for pass in 0..PASSES {
            if let Some(result) = self.transform_once(quad, pass) {
                return result;
            }
        }

        true
    }

    /// java: transformOnce(MutableQuadView quad, int pass); Some(bool) is the Java
    /// return, None the null "next pass".
    fn transform_once(&self, quad: &MutableQuadView, pass: i32) -> Option<bool> {
        let sprite = render_util_sprite_finder_find(quad);
        let slice_func = self.slice_func.borrow().as_ref().unwrap().clone();
        let slice = slice_func(&sprite);
        let processors = if pass == 0 { &slice.processors } else { &slice.multipass_processors };
        let block_view = self.block_view.borrow().clone().unwrap_or_else(|| Rc::new(BlockRenderView));
        let appearance_state = self.appearance_state.borrow().clone().unwrap();
        let state = self.state.borrow().clone().unwrap();
        let pos = self.pos.borrow().clone().unwrap();
        let random_seed = self.random_seed.get();
        let random_supplier = move || Random { seed: random_seed };
        for processor in processors {
            let result = processor.process_quad(
                quad,
                &sprite,
                &block_view,
                &appearance_state,
                &state,
                &pos,
                &random_supplier,
                pass,
                &mut *self.processing_context.borrow_mut(),
            );
            if result == crate::processor_base::ProcessingResult::NextProcessor {
                continue;
            }
            if result == crate::processor_base::ProcessingResult::NextPass {
                return None;
            }
            if result == crate::processor_base::ProcessingResult::Stop {
                return Some(true);
            }
            if result == crate::processor_base::ProcessingResult::Discard {
                return Some(false);
            }
        }
        Some(true)
    }

    /// java: isActive().
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// java: prepare(...).
    pub fn prepare(
        &mut self,
        block_view: &BlockRenderView,
        appearance_state: BlockState,
        state: BlockState,
        pos: BlockPos,
        random_seed: i64,
        render_context: RenderContextHandle,
        use_manual_culling: bool,
        slice_func: Rc<dyn Fn(&Sprite) -> Slice>,
    ) {
        // java: this.blockView = blockView (a borrowed engine surface; the port
        // carries the unit handle).
        self.block_view = RefCell::new(Some(Rc::new(BlockRenderView)));
        let _ = block_view;
        self.appearance_state = RefCell::new(Some(appearance_state));
        self.state = RefCell::new(Some(state));
        self.pos = RefCell::new(Some(pos));
        self.random_seed = Cell::new(random_seed as u64);
        self.render_context = RefCell::new(Some(render_context));
        self.use_manual_culling = Cell::new(use_manual_culling);
        self.slice_func = RefCell::new(Some(slice_func));

        self.active = Cell::new(true);

        self.processing_context.borrow_mut().prepare();
    }

    /// java: reset().
    pub fn reset(&mut self) {
        self.block_view = RefCell::new(None);
        self.appearance_state = RefCell::new(None);
        self.state = RefCell::new(None);
        self.pos = RefCell::new(None);
        self.render_context = RefCell::new(None);
        self.use_manual_culling = Cell::new(false);
        self.slice_func = RefCell::new(None);

        self.active = Cell::new(false);

        self.processing_context.borrow_mut().reset();
    }
}

impl Default for CtmQuadTransform {
    fn default() -> Self {
        CtmQuadTransform::new()
    }
}
