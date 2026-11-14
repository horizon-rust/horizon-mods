//! continuity runtime splice (1:1 port of the LGPL-3.0 Continuity mod by
//! PepperCode1).
//!
//! This file is the Horizon native-mod wiring the launcher compiles into its
//! client runtime. Upstream reaches the game through ContinuityClient's
//! ClientModInitializer and fabric model-loading hooks; this splice is only
//! the host binding and adds no policy of its own: it runs the upstream init
//! order and the resource-reload flow through the exact 1:1 modules.

use std::collections::HashMap;
use std::rc::Rc;

use crate::baked_model_manager_reload;
use crate::model_wrapping::{self, ModelWrappingHandler};
use crate::seams::{self, AtlasPreparation, Identifier, ResourceManager};

/// One client session's mod state: the model wrapping handler the loader
/// extension mixin holds, as upstream keeps it on the model loader.
pub struct ContinuityRuntime {
    model_wrapping_handler: Option<Rc<ModelWrappingHandler>>,
}

impl ContinuityRuntime {
    /// java: ContinuityClient.onInitializeClient(), in upstream order. The
    /// RenderUtil.ReloadListener, CustomBlockLayers.ReloadListener, builtin
    /// resource-pack registration and the CtmLoaderRegistry loader
    /// registrations land with the remaining resource-loader stage.
    pub fn init(&mut self) {
        // java: ProcessingDataKeyRegistryImpl.INSTANCE.init().
        let _registry = crate::processor_base::processing_data_key_registry();
        // java: BiomeHolderManager.init().
        seams::biome_holder_manager_clear_cache();
        seams::biome_holder_manager_refresh_holders();
        // java: ProcessingDataKeys.init().
        let _keys = crate::processor_base::processing_data_keys();
        // java: ModelWrappingHandler.init().
        model_wrapping::init();
        // java: RenderUtil.ReloadListener.init().
        crate::render_api::render_util_reload_listener_init();
        // java: CustomBlockLayers.ReloadListener.init().
        crate::custom_block_layers::ReloadListener::init();

        // java: the two built-in resource packs and the CtmLoaderRegistry loader
        // registrations follow the init calls in ContinuityClient.onInitializeClient.
        crate::continuity_client::on_initialize_client();
    }

    /// java: the resource-reload path through BakedModelManagerMixin:
    /// continuity$onHeadReload creates the BakedModelManagerReloadExtension (the
    /// properties load, the loader-load context, the emissive suffix load) and sets
    /// its context for the sprite-loading phase the engine drives through
    /// SpriteLoader/AtlasLoader; continuity$onReturnReload clears that context when
    /// the reload call returns. The extension itself stays alive across the sprite
    /// phase and the bake (upstream drops it only when the whole reload future
    /// completes), so the splice keeps it until upload().
    pub fn reload(&mut self, resource_manager: &ResourceManager) {
        baked_model_manager_reload::baked_model_manager_on_head_reload(resource_manager);
        baked_model_manager_reload::baked_model_manager_on_return_reload();
    }

    /// java: BakedModelManagerMixin.continuity$onHeadBake - the joined loading
    /// result builds the processor holders from the atlas preparations and swaps
    /// the model wrapping handler in (wrapCtm = holders non-empty, wrapEmissive =
    /// the emissive mark from the stitch).
    pub fn on_bake(&mut self, preparations: &HashMap<Identifier, AtlasPreparation>) {
        baked_model_manager_reload::baked_model_manager_on_head_bake(preparations);
        self.model_wrapping_handler = model_wrapping::continuity_get_model_wrapping_handler();
    }

    /// java: BakedModelManagerMixin.continuity$onReturnUpload swaps the processor
    /// holders in, and continuity$modifyReturnReload drops the extension when the
    /// reload future completes.
    pub fn on_upload(&mut self) {
        baked_model_manager_reload::baked_model_manager_on_return_upload();
        baked_model_manager_reload::baked_model_manager_modify_return_reload();
    }

    /// java: ((ModelLoaderExtension) modelLoader).continuity$getModelWrappingHandler().
    pub fn model_wrapping_handler(&self) -> Option<Rc<ModelWrappingHandler>> {
        self.model_wrapping_handler.clone()
    }
}
