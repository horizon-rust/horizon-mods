//! 1:1 of the Continuity client.resource.ModelWrappingHandler, by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity).
//!
//! Upstream wraps every baked block/item model after bake: the CTM wrapper
//! swaps in CtmBakedModel for block-state models and the emissive wrapper swaps
//! in EmissiveBakedModel, both driven by the mod's config booleans. Engine
//! surfaces (the model loader, the block registry walk, ModelIdentifier) sit
//! behind `seams`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::emissive_baked_model::EmissiveBakedModel;
use crate::model::{BakedModel, CtmBakedModel};
use crate::seams::{self, BlockState, Identifier, ModelIdentifier};

/// java: client.resource.ModelWrappingHandler.
pub struct ModelWrappingHandler {
    wrap_ctm: bool,
    wrap_emissive: bool,
    /// java: ImmutableMap<ModelIdentifier, BlockState> blockStateModelIds.
    block_state_model_ids: HashMap<ModelIdentifier, BlockState>,
}

impl ModelWrappingHandler {
    /// java: private ModelWrappingHandler(boolean wrapCtm, boolean wrapEmissive).
    fn new(wrap_ctm: bool, wrap_emissive: bool) -> ModelWrappingHandler {
        ModelWrappingHandler {
            wrap_ctm,
            wrap_emissive,
            block_state_model_ids: create_block_state_model_id_map(),
        }
    }

    /// java: @Nullable public static ModelWrappingHandler create(boolean wrapCtm,
    /// boolean wrapEmissive).
    pub fn create(wrap_ctm: bool, wrap_emissive: bool) -> Option<ModelWrappingHandler> {
        if !wrap_ctm && !wrap_emissive {
            return None;
        }
        Some(ModelWrappingHandler::new(wrap_ctm, wrap_emissive))
    }

    /// java: public BakedModel wrap(@Nullable BakedModel model, Identifier modelId).
    pub fn wrap(
        &self,
        model: Option<Rc<dyn BakedModel>>,
        model_id: &Identifier,
    ) -> Option<Rc<dyn BakedModel>> {
        let mut model = model?;
        if !model.is_builtin() && *model_id != seams::model_loader_missing_id() {
            if self.wrap_ctm {
                if seams::identifier_is_model_identifier(model_id) {
                    let lookup = ModelIdentifier {
                        id: model_id.clone(),
                        variant: seams::identifier_model_variant(model_id),
                    };
                    if let Some(state) = self.block_state_model_ids.get(&lookup) {
                        model = Rc::new(CtmBakedModel::new(model, state.clone()));
                    }
                }
            }
            if self.wrap_emissive {
                model = Rc::new(EmissiveBakedModel::new(model));
            }
        }
        Some(model)
    }
}

/// java: private static ImmutableMap<ModelIdentifier, BlockState>
/// createBlockStateModelIdMap().
fn create_block_state_model_id_map() -> HashMap<ModelIdentifier, BlockState> {
    let mut builder = HashMap::new();
    // Match code of BakedModelManager#bake
    for (block_id, state) in seams::registry_block_states() {
        let model_id = seams::block_models_get_model_id(&block_id, &state);
        builder.insert(model_id, state);
    }
    builder
}

// --- java: me.pepperbell.continuity.client.mixinterface.ModelLoaderExtension ---

thread_local! {
    /// java: continuity$getModelWrappingHandler() on the loader, set during model
    /// loading.
    static MODEL_WRAPPING_HANDLER: std::cell::RefCell<Option<Rc<ModelWrappingHandler>>> =
        std::cell::RefCell::new(None);
}

/// java: ModelLoaderExtension.continuity$setModelWrappingHandler(handler).
pub fn continuity_set_model_wrapping_handler(handler: Option<Rc<ModelWrappingHandler>>) {
    MODEL_WRAPPING_HANDLER.with(|h| *h.borrow_mut() = handler);
}

/// java: ((ModelLoaderExtension) modelLoader).continuity$getModelWrappingHandler().
pub fn continuity_get_model_wrapping_handler() -> Option<Rc<ModelWrappingHandler>> {
    MODEL_WRAPPING_HANDLER.with(|h| h.borrow().clone())
}

/// java: the WRAP_LAST_PHASE modifier body, as a stored registration the engine
/// invokes after each bake.
pub type WrapAfterBakeFn = fn(
    Option<Rc<dyn BakedModel>>,
    &Identifier,
) -> Option<Rc<dyn BakedModel>>;

thread_local! {
    static WRAP_LAST_PHASE: std::cell::RefCell<Option<WrapAfterBakeFn>> =
        std::cell::RefCell::new(None);
}

/// java: the registered lambda: reads the wrapping handler off the loader
/// extension and wraps the baked model with it.
fn wrap_model_after_bake_last_phase(
    model: Option<Rc<dyn BakedModel>>,
    model_id: &Identifier,
) -> Option<Rc<dyn BakedModel>> {
    match continuity_get_model_wrapping_handler() {
        Some(handler) => handler.wrap(model, model_id),
        None => model,
    }
}

/// java: public static void init() - ModelLoadingPlugin.register(pluginCtx ->
/// pluginCtx.modifyModelAfterBake().register(ModelModifier.WRAP_LAST_PHASE, ...)).
pub fn init() {
    seams::model_loading_plugin_register();
    WRAP_LAST_PHASE.with(|r| *r.borrow_mut() = Some(wrap_model_after_bake_last_phase));
}

/// Engine hook the launcher calls after each baked model (the WRAP_LAST_PHASE
/// phase), running the registered modifier.
pub fn wrap_model_after_bake(
    model: Option<Rc<dyn BakedModel>>,
    model_id: &Identifier,
) -> Option<Rc<dyn BakedModel>> {
    let modifier = WRAP_LAST_PHASE.with(|r| *r.borrow());
    match modifier {
        Some(modifier) => modifier(model, model_id),
        None => model,
    }
}

#[cfg(test)]
mod model_wrapping_tests {
    use super::*;
    use crate::processor_base::Random;
    use crate::render_api::RenderContextHandle;
    use crate::seams::{BlockPos, BlockRenderView};

    struct TestModel {
        builtin: bool,
    }

    impl BakedModel for TestModel {
        fn emit_block_quads(
            &self,
            _block_view: &BlockRenderView,
            _state: &BlockState,
            _pos: &BlockPos,
            _random_supplier: &dyn Fn() -> Random,
            _context: &RenderContextHandle,
        ) {
        }

        fn is_builtin(&self) -> bool {
            self.builtin
        }
    }

    fn block_id(name: &str) -> Identifier {
        Identifier::new("minecraft", name).unwrap()
    }

    fn state(id: u64) -> BlockState {
        BlockState { block_id: id, state_id: id, sprites: Vec::new() }
    }

    #[test]
    fn create_none_when_both_off() {
        assert!(ModelWrappingHandler::create(false, false).is_none());
        assert!(ModelWrappingHandler::create(true, true).is_some());
        assert!(ModelWrappingHandler::create(true, false).is_some());
        assert!(ModelWrappingHandler::create(false, true).is_some());
    }

    // rust carrying: the engine block-registry seam is one process-global, so the tests
    // that set and read it hold this mutex for their whole set-read-wrap sequence.
    fn block_registry_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn wrap_null_model_stays_null() {
        let handler = ModelWrappingHandler::create(true, true).unwrap();
        assert!(handler.wrap(None, &block_id("stone")).is_none());
    }

    #[test]
    fn wrap_builtin_untouched() {
        let handler = ModelWrappingHandler::create(true, true).unwrap();
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: true }) as Rc<dyn BakedModel>), &block_id("stone"))
            .unwrap();
        assert!(wrapped.is_builtin());
    }

    #[test]
    fn wrap_missing_id_untouched() {
        let handler = ModelWrappingHandler::create(true, true).unwrap();
        let missing = seams::model_loader_missing_id();
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &missing)
            .unwrap();
        assert!(wrapped.is_vanilla_adapter());
    }

    #[test]
    fn wrap_ctm_wraps_block_state_model_ids() {
        let _block_registry = block_registry_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = block_id("glass");
        seams::set_engine_block_registry(vec![(id.clone(), state(11))]);
        let handler = ModelWrappingHandler::create(true, false).unwrap();
        let model_id = seams::block_models_get_model_id(&id, &state(11)).id;
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &model_id)
            .unwrap();
        assert!(!wrapped.is_vanilla_adapter());
        // A plain (non-block-model) identifier is not a ModelIdentifier: no wrap.
        let plain = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &id)
            .unwrap();
        assert!(plain.is_vanilla_adapter());
    }

    #[test]
    fn wrap_ctm_skips_unmapped_states() {
        let _block_registry = block_registry_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = block_id("oak_planks");
        seams::set_engine_block_registry(vec![(id.clone(), state(7))]);
        let handler = ModelWrappingHandler::create(true, false).unwrap();
        // A ModelIdentifier the map does not hold (different state id).
        seams::set_engine_block_registry(Vec::new());
        let other = seams::block_models_get_model_id(&id, &state(99)).id;
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &other)
            .unwrap();
        assert!(wrapped.is_vanilla_adapter());
    }

    #[test]
    fn wrap_emissive_wraps_everything_reachable() {
        let handler = ModelWrappingHandler::create(false, true).unwrap();
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &block_id("stone"))
            .unwrap();
        // EmissiveBakedModel.isVanillaAdapter(): false when the emissive option is on.
        assert!(!wrapped.is_vanilla_adapter());
    }

    #[test]
    fn ctm_then_emissive_layering() {
        let _block_registry = block_registry_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = block_id("bricks");
        seams::set_engine_block_registry(vec![(id.clone(), state(3))]);
        let handler = ModelWrappingHandler::create(true, true).unwrap();
        let model_id = seams::block_models_get_model_id(&id, &state(3)).id;
        let wrapped = handler
            .wrap(Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>), &model_id)
            .unwrap();
        assert!(!wrapped.is_vanilla_adapter());
    }

    #[test]
    fn init_registers_plugin_and_modifier() {
        let before = seams::model_loading_plugin_register();
        init();
        assert!(seams::model_loading_plugin_register() > before);
        // No handler on the loader: the modifier is a pass-through.
        continuity_set_model_wrapping_handler(None);
        let model = Some(Rc::new(TestModel { builtin: false }) as Rc<dyn BakedModel>);
        let passed = wrap_model_after_bake(model, &block_id("stone")).unwrap();
        assert!(passed.is_vanilla_adapter());
    }

    #[test]
    fn handler_roundtrip_on_loader_extension() {
        let handler = Rc::new(ModelWrappingHandler::create(true, false).unwrap());
        continuity_set_model_wrapping_handler(Some(handler));
        assert!(continuity_get_model_wrapping_handler().is_some());
        continuity_set_model_wrapping_handler(None);
        assert!(continuity_get_model_wrapping_handler().is_none());
    }
}
