//! 1:1 of the Continuity client.resource.BakedModelManagerReloadExtension with the
//! atlas/sprite loader contexts its reload drives (SpriteLoaderLoadContext +
//! EmissiveControl, SpriteLoaderStitchContext, AtlasLoaderInitContext,
//! AtlasLoaderLoadContext) and the mixin bodies of BakedModelManagerMixin,
//! SpriteLoaderMixin and AtlasLoaderMixin as engine seam hooks, by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity).
//!
//! Upstream loads sprites on the prepare executor through CompletableFutures and
//! reaches the game through mixins; the single-threaded port resolves the
//! futures synchronously and exposes each mixin injection point as a plain
//! function the launcher calls at the same point.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::emissive_suffix_loader;
use crate::model_wrapping::{self, ModelWrappingHandler};
use crate::properties_loader::{CtmPropertiesLoader, LoadingResult};
use crate::processor_base::TextureGetter;
use crate::quad_processors::{self, ProcessorHolder};
use crate::seams::{self, Identifier, ResourceManager, Sprite, SpriteId};

/// java: id.withPath(newPath) - same namespace, new path.
fn with_path(id: &Identifier, new_path: &str) -> Identifier {
    Identifier::new(&id.namespace, new_path).unwrap()
}

// --- java: client.resource.SpriteLoaderLoadContext.EmissiveControl ---

/// java: interface EmissiveControl.
pub trait EmissiveControl {
    /// java: @Nullable Map<Identifier, Identifier> getEmissiveIdMap().
    fn get_emissive_id_map(&self) -> Option<HashMap<Identifier, Identifier>>;
    /// java: void setEmissiveIdMap(Map<Identifier, Identifier> emissiveIdMap).
    fn set_emissive_id_map(&self, emissive_id_map: HashMap<Identifier, Identifier>);
    /// java: void markHasEmissives().
    fn mark_has_emissives(&self);
}

/// java: private static class EmissiveControlImpl (inside SpriteLoaderLoadContextImpl).
pub struct EmissiveControlImpl {
    /// java: @Nullable private volatile Map<Identifier, Identifier> emissiveIdMap.
    emissive_id_map: RefCell<Option<HashMap<Identifier, Identifier>>>,
    /// java: private final AtomicBoolean hasEmissivesHolder.
    has_emissives_holder: Rc<Cell<bool>>,
}

impl EmissiveControlImpl {
    /// java: EmissiveControlImpl(AtomicBoolean hasEmissivesHolder).
    pub fn new(has_emissives_holder: Rc<Cell<bool>>) -> EmissiveControlImpl {
        EmissiveControlImpl {
            emissive_id_map: RefCell::new(None),
            has_emissives_holder,
        }
    }
}

impl EmissiveControl for EmissiveControlImpl {
    fn get_emissive_id_map(&self) -> Option<HashMap<Identifier, Identifier>> {
        self.emissive_id_map.borrow().clone()
    }

    fn set_emissive_id_map(&self, emissive_id_map: HashMap<Identifier, Identifier>) {
        *self.emissive_id_map.borrow_mut() = Some(emissive_id_map);
    }

    fn mark_has_emissives(&self) {
        self.has_emissives_holder.set(true);
    }
}

/// java: client.resource.SpriteLoaderLoadContextImpl (the private impl inside
/// BakedModelManagerReloadExtension).
pub struct SpriteLoaderLoadContextImpl {
    /// java: CompletableFuture<Map<Identifier, Set<Identifier>>> allExtraIdsFuture;
    /// the single-threaded port carries the resolved map (a texture dependency set
    /// the loader produced, keyed by atlas id).
    all_extra_ids: HashMap<Identifier, Option<HashSet<Identifier>>>,
    /// java: Map<Identifier, CompletableFuture<Set<Identifier>>> extraIdsFutures
    /// (Object2ObjectOpenHashMap), memoized per atlas via computeIfAbsent.
    extra_ids: RefCell<HashMap<Identifier, Option<Rc<HashSet<Identifier>>>>>,
    /// java: private final EmissiveControl blockAtlasEmissiveControl.
    block_atlas_emissive_control: Rc<EmissiveControlImpl>,
}

impl SpriteLoaderLoadContextImpl {
    /// java: SpriteLoaderLoadContextImpl(CompletableFuture<Map<Identifier,
    /// Set<Identifier>>> allExtraIdsFuture, AtomicBoolean blockAtlasHasEmissivesHolder).
    pub fn new(
        all_extra_ids: HashMap<Identifier, Option<HashSet<Identifier>>>,
        block_atlas_has_emissives_holder: Rc<Cell<bool>>,
    ) -> SpriteLoaderLoadContextImpl {
        SpriteLoaderLoadContextImpl {
            all_extra_ids,
            extra_ids: RefCell::new(HashMap::new()),
            block_atlas_emissive_control: Rc::new(EmissiveControlImpl::new(
                block_atlas_has_emissives_holder,
            )),
        }
    }

    /// java: getExtraIdsFuture(Identifier atlasId) - the future collapses to the
    /// synchronous value in the port (allExtraIdsFuture.thenApply(allExtraIds ->
    /// allExtraIds.get(id))), memoized per atlas.
    pub fn get_extra_ids(&self, atlas_id: &Identifier) -> Option<Rc<HashSet<Identifier>>> {
        self.extra_ids
            .borrow_mut()
            .entry(atlas_id.clone())
            .or_insert_with(|| {
                self.all_extra_ids
                    .get(atlas_id)
                    .and_then(|set| set.as_ref())
                    .map(|set| Rc::new(set.clone()))
            })
            .clone()
    }

    /// java: getEmissiveControl(Identifier atlasId) - @Nullable; only the block atlas
    /// carries one.
    pub fn get_emissive_control(&self, atlas_id: &Identifier) -> Option<Rc<dyn EmissiveControl>> {
        if *atlas_id == seams::block_atlas_texture() {
            return Some(self.block_atlas_emissive_control.clone());
        }
        None
    }
}

// --- java: client.resource.SpriteLoaderLoadContext / SpriteLoaderStitchContext /
//         AtlasLoaderInitContext / AtlasLoaderLoadContext ---

thread_local! {
    /// java: SpriteLoaderLoadContext.THREAD_LOCAL.
    static SPRITE_LOADER_LOAD_CONTEXT:
        RefCell<Option<Rc<RefCell<SpriteLoaderLoadContextImpl>>>> = RefCell::new(None);
    /// java: AtlasLoaderInitContext.THREAD_LOCAL.
    static ATLAS_LOADER_INIT_CONTEXT: RefCell<Option<HashSet<Identifier>>> = RefCell::new(None);
    /// java: AtlasLoaderLoadContext.THREAD_LOCAL - the context is the
    /// emissiveControl::setEmissiveIdMap lambda, carried as the control itself.
    static ATLAS_LOADER_LOAD_CONTEXT: RefCell<Option<Rc<dyn EmissiveControl>>> =
        RefCell::new(None);
    /// java: SpriteLoaderStitchContext.THREAD_LOCAL.
    static SPRITE_LOADER_STITCH_CONTEXT: RefCell<Option<Rc<dyn SpriteLoaderStitchContext>>> =
        RefCell::new(None);
}

/// java: interface SpriteLoaderStitchContext.
pub trait SpriteLoaderStitchContext {
    /// java: Map<Identifier, Identifier> getEmissiveIdMap().
    fn get_emissive_id_map(&self) -> HashMap<Identifier, Identifier>;
    /// java: void markHasEmissives().
    fn mark_has_emissives(&self);
}

/// java: the anonymous SpriteLoaderStitchContext built in
/// SpriteLoaderMixin.continuity$modifyFunction - it captures the emissive id map and
/// delegates markHasEmissives to the block atlas control.
struct StitchContextImpl {
    emissive_id_map: HashMap<Identifier, Identifier>,
    emissive_control: Rc<dyn EmissiveControl>,
}

impl SpriteLoaderStitchContext for StitchContextImpl {
    fn get_emissive_id_map(&self) -> HashMap<Identifier, Identifier> {
        self.emissive_id_map.clone()
    }

    fn mark_has_emissives(&self) {
        self.emissive_control.mark_has_emissives();
    }
}

// --- java: client.mixin.AtlasLoaderMixin ---

/// java: net.minecraft.client.texture.atlas.SingleAtlasSource(spriteId,
/// resourceSupplier).
#[derive(Clone, Debug)]
pub struct SingleAtlasSource {
    pub sprite_id: Identifier,
    /// java: Optional<ResourceSupplier>; the port carries the source resource id the
    /// engine's atlas source resolves (None = Optional.empty).
    pub resource_id: Option<Identifier>,
}

/// java: net.minecraft.client.texture.atlas.AtlasSource; the port carries the single
/// source plus a passthrough for engine-owned sources.
#[derive(Clone, Debug)]
pub enum AtlasSource {
    Single(SingleAtlasSource),
    Other,
}

/// java: AtlasLoaderMixin.continuity$modifySources(List<AtlasSource> sources) - the
/// constructor-argument modification that prepends one SingleAtlasSource per extra id.
pub fn atlas_loader_modify_sources(sources: Vec<AtlasSource>) -> Vec<AtlasSource> {
    let extra_ids = ATLAS_LOADER_INIT_CONTEXT.with(|c| c.borrow().clone());
    let Some(extra_ids) = extra_ids else {
        return sources;
    };
    if extra_ids.is_empty() {
        return sources;
    }
    // java: List<AtlasSource> extraSources; for (Identifier extraId : extraIds)
    // extraSources.add(new SingleAtlasSource(extraId, Optional.empty())).
    let extra_sources: Vec<AtlasSource> = extra_ids
        .iter()
        .map(|extra_id| {
            AtlasSource::Single(SingleAtlasSource {
                sprite_id: extra_id.clone(),
                resource_id: None,
            })
        })
        .collect();
    // java: if (sources instanceof ArrayList) sources.addAll(0, extraSources);
    // else copy into a mutable list. The port's Vec is always the mutable case.
    let mut sources = sources;
    let mut combined = extra_sources;
    combined.append(&mut sources);
    combined
}

/// java: AtlasSource.SpriteRegion - the port carries it as the sprite supplier the
/// engine later loads the sprite contents from.
pub type SpriteSupplier = Rc<dyn Fn() -> Sprite>;

/// java: SpriteLoader.load(id, resource) - the engine builds the sprite contents
/// from the resource; the port's sprite surface is the id stub the engine fills.
pub fn sprite_loader_load_from_resource(id: &Identifier) -> Sprite {
    seams::sprite_for_id(id.clone())
}

/// java: AtlasLoaderMixin.continuity$afterLoadSources(resourceManager, suppliers) -
/// the injected locals capture after the suppliers map is built.
pub fn atlas_loader_after_load_sources(
    _resource_manager: &ResourceManager,
    suppliers: &mut HashMap<Identifier, SpriteSupplier>,
) {
    let context = ATLAS_LOADER_LOAD_CONTEXT.with(|c| c.borrow().clone());
    let Some(context) = context else {
        return;
    };
    let Some(emissive_suffix) = emissive_suffix_loader::get_emissive_suffix() else {
        return;
    };
    // java: Map<Identifier, AtlasSource.SpriteRegion> emissiveSuppliers;
    // Map<Identifier, Identifier> emissiveIdMap.
    let mut emissive_suppliers: HashMap<Identifier, SpriteSupplier> = HashMap::new();
    let mut emissive_id_map: HashMap<Identifier, Identifier> = HashMap::new();
    let ids: Vec<Identifier> = suppliers.keys().cloned().collect();
    for id in ids {
        // java: if (!id.getPath().endsWith(emissiveSuffix)).
        if !id.path.ends_with(&emissive_suffix) {
            // java: Identifier emissiveId = id.withPath(id.getPath() + emissiveSuffix).
            let emissive_id = with_path(&id, &format!("{}{}", id.path, emissive_suffix));
            if !suppliers.contains_key(&emissive_id) {
                // java: Identifier emissiveLocation = emissiveId.withPath("textures/"
                // + emissiveId.getPath() + ".png").
                let emissive_location = with_path(
                    &emissive_id,
                    &format!("textures/{}.png", emissive_id.path),
                );
                // java: Optional<Resource> optionalResource =
                // resourceManager.getResource(emissiveLocation).
                if let Some(resource) = seams::get_resource(&emissive_location) {
                    // java: emissiveSuppliers.put(emissiveId, () ->
                    // SpriteLoader.load(emissiveId, resource)).
                    let supplier_id = emissive_id.clone();
                    emissive_suppliers.insert(
                        emissive_id.clone(),
                        Rc::new(move || sprite_loader_load_from_resource(&supplier_id)),
                    );
                    // java: emissiveIdMap.put(id, emissiveId).
                    emissive_id_map.insert(id.clone(), emissive_id);
                }
            } else {
                // java: emissiveIdMap.put(id, emissiveId).
                emissive_id_map.insert(id, emissive_id);
            }
        }
    }
    // java: suppliers.putAll(emissiveSuppliers).
    suppliers.extend(emissive_suppliers);
    // java: if (!emissiveIdMap.isEmpty()) context.setEmissiveIdMap(emissiveIdMap).
    if !emissive_id_map.is_empty() {
        context.set_emissive_id_map(emissive_id_map);
    }
}

/// java: AtlasLoader.loadSources(ResourceManager) - the suppliers map from the
/// (already context-modified) sources, with the injected emissive pass after it.
pub fn atlas_loader_load_sources(
    resource_manager: &ResourceManager,
    sources: Vec<AtlasSource>,
) -> HashMap<Identifier, SpriteSupplier> {
    // java: the constructor arg flows through continuity$modifySources first.
    let sources = atlas_loader_modify_sources(sources);
    let mut suppliers: HashMap<Identifier, SpriteSupplier> = HashMap::new();
    for source in sources {
        if let AtlasSource::Single(single) = source {
            // java: SingleAtlasSource supplies nothing with an empty Optional<ResourceSupplier>.
            if let Some(_resource_id) = single.resource_id {
                let supplier_id = single.sprite_id.clone();
                suppliers.insert(
                    single.sprite_id,
                    Rc::new(move || sprite_loader_load_from_resource(&supplier_id)),
                );
            }
        }
    }
    atlas_loader_after_load_sources(resource_manager, &mut suppliers);
    suppliers
}

// --- java: client.mixin.SpriteLoaderMixin ---

/// java: SpriteLoader.load(ResourceManager, Identifier, int, Executor) with the
/// mixin injection points in place: continuity$modifySupplier wraps the source walk
/// with the loader-load context's init/load contexts, the walk builds the suppliers,
/// continuity$modifyFunction sets the stitch context, and continuity$onReturnStitch
/// carries the emissive sprites. The port runs it synchronously on the client thread
/// and returns the stitch result's regions map.
pub fn sprite_loader_load(
    resource_manager: &ResourceManager,
    atlas_id: &Identifier,
    sources: Vec<AtlasSource>,
    _mip_levels: i32,
) -> HashMap<Identifier, Sprite> {
    // java: SpriteLoaderMixin.continuity$modifySupplier reads the loader-load context
    // set by the reload extension.
    let context = SPRITE_LOADER_LOAD_CONTEXT.with(|c| c.borrow().clone());
    let (extra_ids, emissive_control) = match &context {
        Some(context) => {
            let context = context.borrow();
            (
                context.get_extra_ids(atlas_id),
                context.get_emissive_control(atlas_id),
            )
        }
        None => (None, None),
    };
    // java: the wrapped supplier: AtlasLoaderInitContext.THREAD_LOCAL.set(extraIdsFuture::join);
    // AtlasLoaderLoadContext.THREAD_LOCAL.set(emissiveControl::setEmissiveIdMap);
    // ... run ...; clear both. (When the atlas carries no emissive control the load
    // context is never set.)
    ATLAS_LOADER_INIT_CONTEXT.with(|c| {
        *c.borrow_mut() = extra_ids.map(|set| (*set).clone());
    });
    ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = emissive_control.clone());
    // java: the atlas is built inside the supplier: new AtlasLoader(sources) with
    // continuity$modifySources, then loadSources.
    let suppliers = atlas_loader_load_sources(resource_manager, sources);
    ATLAS_LOADER_INIT_CONTEXT.with(|c| *c.borrow_mut() = None);
    ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = None);

    // java: continuity$modifyFunction - the stitch context is set only when the
    // control carries an emissive id map.
    let emissive_id_map = match &emissive_control {
        Some(control) => control.get_emissive_id_map(),
        None => None,
    };
    let stitch_context: Option<Rc<dyn SpriteLoaderStitchContext>> =
        match (&emissive_id_map, &emissive_control) {
            (Some(emissive_id_map), Some(emissive_control)) => {
                Some(Rc::new(StitchContextImpl {
                    emissive_id_map: emissive_id_map.clone(),
                    emissive_control: emissive_control.clone(),
                }))
            }
            _ => None,
        };
    if let Some(stitch_context) = &stitch_context {
        SPRITE_LOADER_STITCH_CONTEXT.with(|c| *c.borrow_mut() = Some(stitch_context.clone()));
    }

    // java: stitch(...) - the engine loads every supplier's contents and lays the
    // regions out; the port carries the regions as the id-keyed sprite map.
    let mut sprites: HashMap<Identifier, Sprite> = HashMap::new();
    for (id, supplier) in &suppliers {
        sprites.insert(id.clone(), supplier());
    }

    // java: continuity$onReturnStitch, then the stitch context is cleared.
    if let Some(stitch_context) = &stitch_context {
        sprite_loader_on_return_stitch(stitch_context.as_ref(), &mut sprites);
        SPRITE_LOADER_STITCH_CONTEXT.with(|c| *c.borrow_mut() = None);
    }
    sprites
}

/// java: SpriteLoaderMixin.continuity$onReturnStitch(spriteContentsList, mipLevels,
/// executor, cir) - the injected body over the stitch result's regions.
pub fn sprite_loader_on_return_stitch(
    context: &dyn SpriteLoaderStitchContext,
    sprites: &mut HashMap<Identifier, Sprite>,
) {
    let emissive_id_map = context.get_emissive_id_map();
    for (id, emissive_id) in emissive_id_map {
        // java: Sprite sprite = sprites.get(id); if (sprite != null) { Sprite
        // emissiveSprite = sprites.get(emissiveId); if (emissiveSprite != null) {
        // sprite.continuity$setEmissiveSprite(emissiveSprite); context.markHasEmissives(); } }.
        let has_emissive_sprite = sprites.contains_key(&emissive_id);
        if has_emissive_sprite {
            if let Some(sprite) = sprites.get_mut(&id) {
                sprite.emissive_sprite = Some(emissive_id.clone());
            }
            context.mark_has_emissives();
        }
    }
}

// --- java: client.resource.BakedModelManagerReloadExtension ---

/// java: public class BakedModelManagerReloadExtension.
pub struct BakedModelManagerReloadExtension {
    /// java: private final CompletableFuture<CtmPropertiesLoader.LoadingResult>
    /// ctmLoadingResultFuture; the single-threaded port resolves it at construction
    /// (the supplyAsync on the prepare executor).
    ctm_loading_result: LoadingResult,
    /// java: private final AtomicBoolean wrapEmissiveModels.
    wrap_emissive_models: Rc<Cell<bool>>,
    /// java: private final SpriteLoaderLoadContextImpl spriteLoaderLoadContext.
    sprite_loader_load_context: Rc<RefCell<SpriteLoaderLoadContextImpl>>,
    /// java: private volatile List<QuadProcessors.ProcessorHolder> processorHolders.
    processor_holders: RefCell<Option<Vec<ProcessorHolder>>>,
}

impl BakedModelManagerReloadExtension {
    /// java: BakedModelManagerReloadExtension(ResourceManager resourceManager,
    /// Executor prepareExecutor).
    pub fn new(resource_manager: &ResourceManager) -> BakedModelManagerReloadExtension {
        // java: ctmLoadingResultFuture = CompletableFuture.supplyAsync(() ->
        // CtmPropertiesLoader.loadAllWithState(resourceManager), prepareExecutor).
        let ctm_loading_result = CtmPropertiesLoader::load_all_with_state(resource_manager);
        let wrap_emissive_models = Rc::new(Cell::new(false));
        // java: spriteLoaderLoadContext = new SpriteLoaderLoadContextImpl(
        // ctmLoadingResultFuture.thenApply(LoadingResult::getTextureDependencies),
        // wrapEmissiveModels).
        let sprite_loader_load_context = Rc::new(RefCell::new(SpriteLoaderLoadContextImpl::new(
            ctm_loading_result
                .get_texture_dependencies()
                .iter()
                .map(|(atlas_id, set)| (atlas_id.clone(), Some(set.clone())))
                .collect(),
            wrap_emissive_models.clone(),
        )));
        // java: EmissiveSuffixLoader.load(resourceManager).
        emissive_suffix_loader::load(resource_manager);
        BakedModelManagerReloadExtension {
            ctm_loading_result,
            wrap_emissive_models,
            sprite_loader_load_context,
            processor_holders: RefCell::new(None),
        }
    }

    /// java: setContext().
    pub fn set_context(&self) {
        SPRITE_LOADER_LOAD_CONTEXT.with(|c| {
            *c.borrow_mut() = Some(self.sprite_loader_load_context.clone());
        });
    }

    /// java: clearContext().
    pub fn clear_context(&self) {
        SPRITE_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = None);
    }

    /// java: beforeBaking(Map<Identifier, SpriteAtlasManager.AtlasPreparation>
    /// preparations, ModelLoader modelLoader).
    pub fn before_baking(&self, preparations: &HashMap<Identifier, seams::AtlasPreparation>) {
        // java: CtmPropertiesLoader.LoadingResult result = ctmLoadingResultFuture.join().
        // java: processorHolders = result.createProcessorHolders(spriteId -> {
        //     preparation = preparations.get(spriteId.getAtlasId()); sprite =
        //     preparation.getSprite(spriteId.getTextureId()); if (sprite != null) return
        //     sprite; return preparation.getMissingSprite(); }).
        // The getter is 'static, so the preparation map is cloned into it (small).
        let preparations = preparations.clone();
        let texture_getter: TextureGetter = Rc::new(move |sprite_id: &SpriteId| -> Option<Sprite> {
            let atlas_id = seams::sprite_identifier_atlas_id(sprite_id);
            let texture_id = seams::sprite_identifier_texture_id(sprite_id);
            // Upstream would throw on a missing atlas preparation; the port falls
            // through to None.
            let preparation = preparations.get(&atlas_id)?;
            preparation
                .get_sprite(&texture_id)
                .or_else(|| Some(preparation.get_missing_sprite()))
        });
        let processor_holders = self.ctm_loading_result.create_processor_holders(&texture_getter);
        let is_empty = processor_holders.is_empty();
        *self.processor_holders.borrow_mut() = Some(processor_holders);
        // java: ModelWrappingHandler wrappingHandler = ModelWrappingHandler.create(
        // !processorHolders.isEmpty(), wrapEmissiveModels.get()).
        // ((ModelLoaderExtension) modelLoader).continuity$setModelWrappingHandler(
        // wrappingHandler).
        let wrapping_handler = ModelWrappingHandler::create(!is_empty, self.wrap_emissive_models.get());
        model_wrapping::continuity_set_model_wrapping_handler(wrapping_handler.map(Rc::new));
    }

    /// java: apply().
    pub fn apply(&self) {
        // java: List<QuadProcessors.ProcessorHolder> processorHolders = this.
        // processorHolders; if (processorHolders != null) QuadProcessors.reload(...).
        if let Some(processor_holders) = self.processor_holders.borrow().clone() {
            quad_processors::reload(processor_holders);
        }
    }
}

// --- java: client.mixin.BakedModelManagerMixin ---

thread_local! {
    /// java: private volatile BakedModelManagerReloadExtension continuity$reloadExtension
    /// (the mixin field on BakedModelManager).
    static RELOAD_EXTENSION: RefCell<Option<Rc<BakedModelManagerReloadExtension>>> =
        const { RefCell::new(None) };
}

/// java: BakedModelManagerMixin.continuity$onHeadReload - the extension is created and
/// its loader-load context set for the sprite-loading phase.
pub fn baked_model_manager_on_head_reload(resource_manager: &ResourceManager) {
    let reload_extension = Rc::new(BakedModelManagerReloadExtension::new(resource_manager));
    RELOAD_EXTENSION.with(|e| *e.borrow_mut() = Some(reload_extension.clone()));
    reload_extension.set_context();
}

/// java: BakedModelManagerMixin.continuity$onReturnReload.
pub fn baked_model_manager_on_return_reload() {
    if let Some(reload_extension) = RELOAD_EXTENSION.with(|e| e.borrow().clone()) {
        reload_extension.clear_context();
    }
}

/// java: BakedModelManagerMixin.continuity$modifyReturnReload - original.thenRun(()
/// -> reloadExtension = null); the reload future completes after bake and upload, so
/// the splice clears the extension there.
pub fn baked_model_manager_modify_return_reload() {
    RELOAD_EXTENSION.with(|e| *e.borrow_mut() = None);
}

/// java: BakedModelManagerMixin.continuity$onHeadBake.
pub fn baked_model_manager_on_head_bake(
    preparations: &HashMap<Identifier, seams::AtlasPreparation>,
) {
    if let Some(reload_extension) = RELOAD_EXTENSION.with(|e| e.borrow().clone()) {
        reload_extension.before_baking(preparations);
    }
}

/// java: BakedModelManagerMixin.continuity$onReturnUpload.
pub fn baked_model_manager_on_return_upload() {
    if let Some(reload_extension) = RELOAD_EXTENSION.with(|e| e.borrow().clone()) {
        reload_extension.apply();
    }
}

#[cfg(test)]
mod baked_model_manager_reload_tests {
    use super::*;
    use crate::emissive_suffix_loader::set_emissive_suffix_for_test;
    use crate::seams::{AtlasPreparation, block_atlas_texture};

    fn id(namespace: &str, path: &str) -> Identifier {
        Identifier::new(namespace, path).unwrap()
    }

    fn empty_preparation() -> AtlasPreparation {
        AtlasPreparation {
            sprites: HashMap::new(),
            missing_sprite: seams::sprite_for_id(seams::missing_sprite_id()),
        }
    }

    #[test]
    fn extra_ids_resolve_per_atlas_and_memoize() {
        let block_atlas = block_atlas_texture();
        let mut all_extra_ids = HashMap::new();
        let mut block_set = HashSet::new();
        block_set.insert(id("minecraft", "block/stone_e"));
        all_extra_ids.insert(block_atlas.clone(), Some(block_set));
        let context = SpriteLoaderLoadContextImpl::new(all_extra_ids, Rc::new(Cell::new(false)));

        let first = context.get_extra_ids(&block_atlas).unwrap();
        assert!(first.contains(&id("minecraft", "block/stone_e")));
        // java: computeIfAbsent memoizes per atlas.
        let second = context.get_extra_ids(&block_atlas).unwrap();
        assert_eq!(Rc::ptr_eq(&first, &second), true);

        let other = id("minecraft", "textures/atlas/particles.png");
        assert!(context.get_extra_ids(&other).is_none());
    }

    #[test]
    fn emissive_control_only_on_block_atlas() {
        let context = SpriteLoaderLoadContextImpl::new(HashMap::new(), Rc::new(Cell::new(false)));
        let other = id("minecraft", "textures/atlas/particles.png");
        assert!(context.get_emissive_control(&other).is_none());

        let control = context.get_emissive_control(&block_atlas_texture()).unwrap();
        assert!(control.get_emissive_id_map().is_none());
        let mut map = HashMap::new();
        map.insert(id("minecraft", "block/stone"), id("minecraft", "block/stone_e"));
        control.set_emissive_id_map(map.clone());
        assert_eq!(control.get_emissive_id_map(), Some(map));
        // java: markHasEmissives flips the shared holder.
        assert_eq!(control.get_emissive_id_map().is_some(), true);
    }

    #[test]
    fn modify_sources_prepends_extra_ids() {
        let base = vec![
            AtlasSource::Single(SingleAtlasSource {
                sprite_id: id("minecraft", "block/stone"),
                resource_id: Some(id("minecraft", "textures/block/stone.png")),
            }),
            AtlasSource::Other,
        ];

        ATLAS_LOADER_INIT_CONTEXT.with(|c| *c.borrow_mut() = None);
        let unchanged = atlas_loader_modify_sources(base.clone());
        assert_eq!(unchanged.len(), 2);

        let mut extra_ids = HashSet::new();
        extra_ids.insert(id("minecraft", "block/stone_e"));
        ATLAS_LOADER_INIT_CONTEXT.with(|c| *c.borrow_mut() = Some(extra_ids));
        let modified = atlas_loader_modify_sources(base);
        ATLAS_LOADER_INIT_CONTEXT.with(|c| *c.borrow_mut() = None);
        assert_eq!(modified.len(), 3);
        // java: the extra sources land at index 0.
        match &modified[0] {
            AtlasSource::Single(single) => {
                assert_eq!(single.sprite_id, id("minecraft", "block/stone_e"));
                assert_eq!(single.resource_id, None);
            }
            AtlasSource::Other => panic!("expected extra single source first"),
        }
        match &modified[2] {
            AtlasSource::Other => {}
            _ => panic!("expected passthrough last"),
        }
    }

    #[test]
    fn after_load_sources_maps_existing_emissive_supplier() {
        set_emissive_suffix_for_test(Some("_e".to_string()));
        let has_emissives = Rc::new(Cell::new(false));
        let control: Rc<dyn EmissiveControl> =
            Rc::new(EmissiveControlImpl::new(has_emissives.clone()));
        ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = Some(control.clone()));

        let resource_manager = ResourceManager::default();
        let mut suppliers: HashMap<Identifier, SpriteSupplier> = HashMap::new();
        let base = id("minecraft", "block/stone");
        suppliers.insert(base.clone(), Rc::new(|| sprite_loader_load_from_resource(&id("minecraft", "block/stone"))));
        // java: the emissive supplier already present takes only the id-map branch.
        let emissive = id("minecraft", "block/stone_e");
        suppliers.insert(emissive.clone(), Rc::new(|| sprite_loader_load_from_resource(&id("minecraft", "block/stone_e"))));

        atlas_loader_after_load_sources(&resource_manager, &mut suppliers);
        ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = None);
        set_emissive_suffix_for_test(None);

        assert_eq!(suppliers.len(), 2);
        let mut expected = HashMap::new();
        expected.insert(base, emissive);
        assert_eq!(control.get_emissive_id_map(), Some(expected));
    }

    #[test]
    fn after_load_sources_skips_emissive_paths_and_missing_suffix() {
        let resource_manager = ResourceManager::default();
        let mut suppliers: HashMap<Identifier, SpriteSupplier> = HashMap::new();
        suppliers.insert(
            id("minecraft", "block/stone"),
            Rc::new(|| sprite_loader_load_from_resource(&id("minecraft", "block/stone"))),
        );

        // java: no suffix set -> the whole body is skipped.
        set_emissive_suffix_for_test(None);
        let control: Rc<dyn EmissiveControl> = Rc::new(EmissiveControlImpl::new(Rc::new(Cell::new(false))));
        ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = Some(control.clone()));
        atlas_loader_after_load_sources(&resource_manager, &mut suppliers);
        assert!(control.get_emissive_id_map().is_none());

        // java: the id already carrying the suffix is skipped entirely.
        set_emissive_suffix_for_test(Some("_e".to_string()));
        suppliers.clear();
        suppliers.insert(
            id("minecraft", "block/stone_e"),
            Rc::new(|| sprite_loader_load_from_resource(&id("minecraft", "block/stone_e"))),
        );
        atlas_loader_after_load_sources(&resource_manager, &mut suppliers);
        ATLAS_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = None);
        set_emissive_suffix_for_test(None);
        assert!(control.get_emissive_id_map().is_none());
        assert_eq!(suppliers.len(), 1);
    }

    #[test]
    fn sprite_loader_load_carries_emissive_sprites_through_stitch() {
        set_emissive_suffix_for_test(Some("_e".to_string()));
        let has_emissives = Rc::new(Cell::new(false));
        let context = Rc::new(RefCell::new(SpriteLoaderLoadContextImpl::new(
            HashMap::new(),
            has_emissives.clone(),
        )));
        SPRITE_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = Some(context.clone()));

        let resource_manager = ResourceManager::default();
        let atlas_id = block_atlas_texture();
        let sources = vec![
            AtlasSource::Single(SingleAtlasSource {
                sprite_id: id("minecraft", "block/stone"),
                resource_id: Some(id("minecraft", "textures/block/stone.png")),
            }),
            AtlasSource::Single(SingleAtlasSource {
                sprite_id: id("minecraft", "block/stone_e"),
                resource_id: Some(id("minecraft", "textures/block/stone_e.png")),
            }),
        ];
        let sprites = sprite_loader_load(&resource_manager, &atlas_id, sources, 0);
        SPRITE_LOADER_LOAD_CONTEXT.with(|c| *c.borrow_mut() = None);
        set_emissive_suffix_for_test(None);

        // java: afterLoadSources found the emissive supplier already present, so the
        // control carries the id map; stitch then carries the emissive sprite over.
        assert_eq!(sprites.len(), 2);
        assert_eq!(
            sprites[&id("minecraft", "block/stone")].emissive_sprite,
            Some(id("minecraft", "block/stone_e"))
        );
        assert_eq!(has_emissives.get(), true);
        assert_eq!(sprites[&id("minecraft", "block/stone_e")].emissive_sprite, None);
    }

    #[test]
    fn stitch_skips_missing_emissive_sprite() {
        let has_emissives = Rc::new(Cell::new(false));
        let control: Rc<dyn EmissiveControl> = Rc::new(EmissiveControlImpl::new(has_emissives.clone()));
        let mut map = HashMap::new();
        map.insert(id("minecraft", "block/stone"), id("minecraft", "block/gone_e"));
        let stitch_context = StitchContextImpl {
            emissive_id_map: map,
            emissive_control: control.clone(),
        };
        let mut sprites = HashMap::new();
        sprites.insert(id("minecraft", "block/stone"), seams::sprite_for_id(id("minecraft", "block/stone")));

        sprite_loader_on_return_stitch(&stitch_context, &mut sprites);
        assert_eq!(
            sprites[&id("minecraft", "block/stone")].emissive_sprite,
            None
        );
        assert_eq!(has_emissives.get(), false);
    }

    #[test]
    fn extension_lifecycle_wires_bake_and_upload() {
        let resource_manager = ResourceManager::default();
        let extension = BakedModelManagerReloadExtension::new(&resource_manager);
        // java: setContext() then clearContext().
        extension.set_context();
        assert!(SPRITE_LOADER_LOAD_CONTEXT.with(|c| c.borrow().is_some()));
        extension.clear_context();
        assert!(SPRITE_LOADER_LOAD_CONTEXT.with(|c| c.borrow().is_none()));

        // java: beforeBaking with no processor holders and no emissive mark ->
        // ModelWrappingHandler.create(false, false) is null.
        let mut preparations = HashMap::new();
        preparations.insert(block_atlas_texture(), empty_preparation());
        model_wrapping::continuity_set_model_wrapping_handler(None);
        extension.before_baking(&preparations);
        assert!(model_wrapping::continuity_get_model_wrapping_handler().is_none());

        // java: marking emissives flips wrapEmissiveModels, so create(false, true)
        // hands back a handler.
        extension.sprite_loader_load_context.borrow().block_atlas_emissive_control.mark_has_emissives();
        extension.before_baking(&preparations);
        assert!(model_wrapping::continuity_get_model_wrapping_handler().is_some());

        // java: apply() swaps the processor holders in (empty here).
        extension.apply();
    }

    #[test]
    fn mixin_lifecycle_runs_head_reload_bake_upload() {
        baked_model_manager_modify_return_reload();
        let resource_manager = ResourceManager::default();
        baked_model_manager_on_head_reload(&resource_manager);
        assert!(SPRITE_LOADER_LOAD_CONTEXT.with(|c| c.borrow().is_some()));
        baked_model_manager_on_return_reload();
        assert!(SPRITE_LOADER_LOAD_CONTEXT.with(|c| c.borrow().is_none()));

        let mut preparations = HashMap::new();
        preparations.insert(block_atlas_texture(), empty_preparation());
        model_wrapping::continuity_set_model_wrapping_handler(None);
        baked_model_manager_on_head_bake(&preparations);
        baked_model_manager_on_return_upload();
        // java: the extension is dropped once the reload future completes.
        baked_model_manager_modify_return_reload();
        baked_model_manager_on_head_bake(&preparations);
        baked_model_manager_on_return_upload();
    }
}
