//! Engine-surface seams for the Fabric renderer API and RenderUtil, as read by the
//! Continuity model layer (LGPL-3.0-only, PepperCode1,
//! https://github.com/PepperCode1/Continuity). Upstream: client/util/RenderUtil.java
//! plus the net.fabricmc.fabric.api.renderer.v1 surfaces the model files call.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::processor_base::{BlendMode, QuadEmitter, QuadView, RenderMaterial};
use crate::processor_enums::Direction;
use crate::seams::{missing_sprite_id, sprite_for_id, BlockPos, BlockRenderView, BlockState, Sprite, SpriteId};

/// java: RenderUtil.getSpriteFinder().find(quad). Engine surface: the SpriteFinder
/// maps the quad's UVs to the atlas sprite; the port carries the sprite id on the
/// quad and falls back to the missing sprite.
pub fn render_util_sprite_finder_find(quad: &QuadView) -> Sprite {
    let id: SpriteId = quad.sprite_id.clone().unwrap_or_else(missing_sprite_id);
    sprite_for_id(id)
}

/// java: RenderUtil's ThreadLocal MaterialFinder.
pub struct MaterialFinder {
    blend_mode: BlendMode,
    emissive: bool,
    disable_diffuse: bool,
    ambient_occlusion: bool,
}

/// java: RenderUtil.getMaterialFinder().
pub fn render_util_get_material_finder() -> MaterialFinder {
    MaterialFinder {
        blend_mode: BlendMode::Default,
        emissive: false,
        disable_diffuse: false,
        ambient_occlusion: true,
    }
}

impl MaterialFinder {
    /// java: emissive(boolean).
    pub fn emissive(mut self, emissive: bool) -> MaterialFinder {
        self.emissive = emissive;
        self
    }

    /// java: disableDiffuse(boolean).
    pub fn disable_diffuse(mut self, disable_diffuse: bool) -> MaterialFinder {
        self.disable_diffuse = disable_diffuse;
        self
    }

    /// java: ambientOcclusion(TriState) (carried as boolean: TriState.FALSE = false).
    pub fn ambient_occlusion(mut self, ambient_occlusion: bool) -> MaterialFinder {
        self.ambient_occlusion = ambient_occlusion;
        self
    }

    /// java: blendMode(BlendMode).
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> MaterialFinder {
        self.blend_mode = blend_mode;
        self
    }

    /// java: find().
    pub fn find(self) -> RenderMaterial {
        RenderMaterial {
            blend_mode: self.blend_mode,
            emissive: self.emissive,
            disable_diffuse: self.disable_diffuse,
            ambient_occlusion: self.ambient_occlusion,
        }
    }
}

/// java: EmissiveBakedModel's static block building EMISSIVE_MATERIALS over
/// BlendMode.values(); built on first access in Rust (Java builds at class load).
fn build_emissive_materials() -> Vec<RenderMaterial> {
    BlendMode::values()
        .iter()
        .map(|&blend_mode| {
            render_util_get_material_finder()
                .emissive(true)
                .disable_diffuse(true)
                .ambient_occlusion(false)
                .blend_mode(blend_mode)
                .find()
        })
        .collect()
}

pub fn emissive_materials() -> &'static [RenderMaterial] {
    static MATERIALS: OnceLock<Vec<RenderMaterial>> = OnceLock::new();
    MATERIALS.get_or_init(build_emissive_materials)
}

/// java: DEFAULT_EMISSIVE_MATERIAL (the DEFAULT blend mode entry).
pub fn default_emissive_material() -> &'static RenderMaterial {
    &emissive_materials()[BlendMode::Default.ordinal()]
}

/// java: CUTOUT_MIPPED_EMISSIVE_MATERIAL (the CUTOUT_MIPPED blend mode entry).
pub fn cutout_mipped_emissive_material() -> &'static RenderMaterial {
    &emissive_materials()[BlendMode::CutoutMipped.ordinal()]
}

/// java: RenderLayers.getBlockLayer(state) == RenderLayer.getSolid() (engine seam;
/// the engine answers per block, the port defaults to solid).
pub fn render_layers_get_block_layer_is_solid(_state: &BlockState) -> bool {
    true
}

/// java: net.fabricmc.fabric.api.renderer.v1.render.RenderContext (engine surface;
/// the port carries the transform-stack depth and the emitter hand-off).
#[derive(Default)]
pub struct RenderContext {
    transform_depth: usize,
    emitter: QuadEmitter,
}

pub type RenderContextHandle = Rc<RefCell<RenderContext>>;

/// rust carrying: builds a handle the way the engine hands a context around.
pub fn render_context() -> RenderContextHandle {
    Rc::new(RefCell::new(RenderContext::default()))
}

impl RenderContext {
    /// java: isFaceCulled(Direction cullFace) (engine seam; defaults to false).
    pub fn is_face_culled(&self, _cull_face: Option<Direction>) -> bool {
        false
    }

    /// java: pushTransform(QuadTransform).
    pub fn push_transform(&mut self) {
        self.transform_depth += 1;
    }

    /// java: popTransform().
    pub fn pop_transform(&mut self) {
        self.transform_depth = self.transform_depth.saturating_sub(1);
    }

    /// java: getEmitter().
    pub fn get_emitter(&mut self) -> &mut QuadEmitter {
        &mut self.emitter
    }
}

/// java: RenderUtil.getTintColor(@Nullable BlockState state, BlockRenderView blockView,
/// BlockPos pos, int tintIndex).
pub fn render_util_get_tint_color(
    state: Option<&BlockState>,
    block_view: &BlockRenderView,
    pos: &BlockPos,
    tint_index: i32,
) -> i32 {
    if state.is_none() || tint_index == -1 {
        return -1;
    }
    // java: 0xFF000000 | BlockColors.INSTANCE.getColor(state, blockView, pos, tintIndex).
    // rust carrying: java's int 0xFF000000 is the packed negative alpha constant.
    (0xFF000000u32 as i32) | crate::seams::block_colors_get_color(state.unwrap(), block_view, pos, tint_index)
}

/// java: RenderUtil.canHaveAO(BlockState state).
pub fn render_util_can_have_ao(state: &BlockState) -> bool {
    // java: state.getLuminance() == 0.
    crate::seams::block_state_get_luminance(state) == 0
}

/// java: RenderUtil.findOverlayMaterial(BlendMode blendMode, @Nullable BlockState tintBlock).
pub fn render_util_find_overlay_material(blend_mode: BlendMode, tint_block: Option<&BlockState>) -> RenderMaterial {
    // java: MaterialFinder finder = getMaterialFinder().blendMode(blendMode);
    let finder = render_util_get_material_finder().blend_mode(blend_mode);
    // java: if (tintBlock != null) { finder.ambientOcclusion(canHaveAO(tintBlock)); }
    // else { finder.ambientOcclusion(true); }
    let finder = match tint_block {
        Some(state) => finder.ambient_occlusion(render_util_can_have_ao(state)),
        None => finder.ambient_occlusion(true),
    };
    finder.find()
}


/// java: RenderUtil.ReloadListener's static blockAtlasSpriteFinder state: the
/// reload sets it to SpriteFinder.get(MODEL_MANAGER.getAtlas(BLOCK_ATLAS_TEXTURE)).
thread_local! {
    static BLOCK_ATLAS_SPRITE_FINDER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// java: the ReloadListener.reload(manager) step; the engine seam rebuilds the
/// atlas SpriteFinder after a model reload.
pub fn render_util_reload_listener_reload(_resource_manager: &crate::seams::ResourceManager) {
    BLOCK_ATLAS_SPRITE_FINDER.with(|finder| finder.set(true));
}

/// java: RenderUtil.getSpriteFinder() != null once the reload listener ran.
pub fn render_util_sprite_finder_ready() -> bool {
    BLOCK_ATLAS_SPRITE_FINDER.with(|finder| finder.get())
}

/// java: RenderUtil.ReloadListener.init() - registers INSTANCE with the client
/// resource reload listeners, depending on MODELS.
pub fn render_util_reload_listener_init() {
    crate::seams::register_client_reload_listener(
        crate::seams::Identifier::new("continuity", "render_util").unwrap(),
        std::rc::Rc::new(
            |manager: &crate::seams::ResourceManager| render_util_reload_listener_reload(manager),
        ),
    );
}
use crate::model::ContinuityFeatureStates;

// --- java: client.mixin.FallingBlockEntityRendererMixin ---
// Injection points: before/after BlockModelRenderer#render inside
// FallingBlockEntityRenderer#render. Both feature states are disabled for the
// falling block's model render so connected textures and emissive overlays do not
// apply to the entity render.

/// java: continuity$beforeRenderModel.
pub fn falling_block_entity_renderer_before_render_model() {
    let states = crate::model::ContinuityFeatureStatesImpl::get();
    states.get_connected_textures_state().disable();
    states.get_emissive_textures_state().disable();
}

/// java: continuity$afterRenderModel.
pub fn falling_block_entity_renderer_after_render_model() {
    let states = crate::model::ContinuityFeatureStatesImpl::get();
    states.get_connected_textures_state().enable();
    states.get_emissive_textures_state().enable();
}

// --- java: client.mixin.PistonBlockEntityRendererMixin ---
// Injection points: before/after BlockModelRenderer#enableBrightnessCache /
// #disableBrightnessCache inside PistonBlockEntityRenderer#render.

/// java: continuity$beforeRenderModels.
pub fn piston_block_entity_renderer_before_render_models() {
    let states = crate::model::ContinuityFeatureStatesImpl::get();
    states.get_connected_textures_state().disable();
    states.get_emissive_textures_state().disable();
}

/// java: continuity$afterRenderModels.
pub fn piston_block_entity_renderer_after_render_models() {
    let states = crate::model::ContinuityFeatureStatesImpl::get();
    states.get_connected_textures_state().enable();
    states.get_emissive_textures_state().enable();
}

#[cfg(test)]
mod entity_renderer_mixin_tests {
    use crate::model::{ContinuityFeatureStates, ContinuityFeatureStatesImpl, FeatureState};

    #[test]
    fn falling_block_renderer_toggles_both_states() {
        let states = ContinuityFeatureStatesImpl::get();
        // java: instance initializer enables both states; they start enabled.
        assert!(states.get_connected_textures_state().is_enabled_state());
        assert!(states.get_emissive_textures_state().is_enabled_state());
        super::falling_block_entity_renderer_before_render_model();
        assert!(!states.get_connected_textures_state().is_enabled_state());
        assert!(!states.get_emissive_textures_state().is_enabled_state());
        super::falling_block_entity_renderer_after_render_model();
        assert!(states.get_connected_textures_state().is_enabled_state());
        assert!(states.get_emissive_textures_state().is_enabled_state());
    }

    #[test]
    fn piston_renderer_toggles_both_states() {
        let states = ContinuityFeatureStatesImpl::get();
        super::piston_block_entity_renderer_before_render_models();
        assert!(!states.get_connected_textures_state().is_enabled_state());
        assert!(!states.get_emissive_textures_state().is_enabled_state());
        super::piston_block_entity_renderer_after_render_models();
        assert!(states.get_connected_textures_state().is_enabled_state());
        assert!(states.get_emissive_textures_state().is_enabled_state());
    }
}
