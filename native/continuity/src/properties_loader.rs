//! 1:1 of me.pepperbell.continuity.client.resource.CtmPropertiesLoader, plus the registry
//! and property surfaces it drives: api/client/CtmLoader, api/client/CtmProperties,
//! api/client/CtmLoaderRegistry, impl/client/CtmLoaderRegistryImpl, and
//! client/resource/InvalidIdentifierStateHolder. Upstream by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity).

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::thread_local;

use crate::model::BooleanState;
use crate::parsing::Properties;
use crate::processor_base::{
    TextureGetter,
    CachingPredicates, ProcessingContext, ProcessingResult, QuadProcessor, Random, BlockPos,
    BlockRenderView, BlockState, MutableQuadView,
};
use crate::quad_processors::ProcessorHolder;
use crate::properties::BaseCtmProperties;
use crate::seams::{
    biome_holder_manager_clear_cache, biome_holder_manager_refresh_holders,
    identifier_compare_to, parse_properties_stream, sprite_identifier_atlas_id,
    sprite_identifier_texture_id, Identifier, ResourcePack, ResourceManager, Sprite, SpriteId,
};

/// java: api.client.CtmProperties.
pub trait CtmProperties {
    /// java: getTextureDependencies() - lazily resolves the tiles on first call.
    fn get_texture_dependencies(&mut self) -> HashSet<SpriteId>;
    /// java: compareTo(CtmProperties o) on BaseCtmProperties.
    fn compare_to(&self, other: &dyn CtmProperties) -> Ordering;
    /// java: the compareTo instanceof BaseCtmProperties check.
    fn as_base(&self) -> Option<&BaseCtmProperties>;

    /// rust carrying: the downcast seam the erased CtmLoaderObj factories need (Rust dyn
    /// erasure takes the place of Java's generic factories, so the concrete loader
    /// downcasts the shared properties back to its type here).
    fn as_any(&self) -> &dyn std::any::Any;
}

impl CtmProperties for BaseCtmProperties {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_texture_dependencies(&mut self) -> HashSet<SpriteId> {
        if self.texture_dependencies.is_none() {
            self.resolve_tiles();
        }
        self.texture_dependencies.clone().unwrap()
    }

    fn compare_to(&self, other: &dyn CtmProperties) -> Ordering {
        // java: if (o instanceof BaseCtmProperties o1) { ... } return 0;
        let Some(o1) = other.as_base() else {
            return Ordering::Equal;
        };
        if self.prioritized && !o1.prioritized {
            return Ordering::Greater;
        }
        if !self.prioritized && o1.prioritized {
            return Ordering::Less;
        }
        let c = (self.pack_priority - o1.pack_priority).signum();
        if c > 0 {
            return Ordering::Greater;
        }
        if c < 0 {
            return Ordering::Less;
        }
        // java: o1.getResourceId().compareTo(getResourceId()) - reversed operands.
        identifier_compare_to(&o1.resource_id, &self.resource_id)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(self)
    }
}

/// java: api.client.CtmLoader<T> with its three factories. The generic T collapses under
/// dyn erasure: the loader receives/returns erased CtmProperties and downcasts in each
/// concrete factory, which keeps the 1:1 factory methods per loader.
// rust carrying: the port keeps the loader registry on the loading thread (thread_local
// LOADER_MAP), so loaders are shared as Rc and the Send+Sync bound the upstream loading
// worker satisfies does not apply here.
pub trait CtmLoaderObj {
    /// java: getPropertiesFactory().createProperties(properties, resourceId, pack,
    /// packPriority, resourceManager, method).
    fn create_properties(
        &self,
        properties: &Properties,
        resource_id: &Identifier,
        pack: &ResourcePack,
        pack_priority: i32,
        resource_manager: &ResourceManager,
        method: &str,
    ) -> Option<Box<dyn CtmProperties>>;
    /// java: getProcessorFactory().createProcessor(properties, textureGetter).
    fn create_processor(
        &self,
        properties: &dyn CtmProperties,
        texture_getter: &TextureGetter,
    ) -> Rc<dyn QuadProcessor>;
    /// java: getPredicatesFactory().createPredicates(properties, textureGetter).
    fn create_predicates(
        &self,
        properties: &dyn CtmProperties,
        texture_getter: &TextureGetter,
    ) -> Rc<dyn CachingPredicates>;
}

/// java: api.client.CtmLoaderRegistry.get() -> impl.client.CtmLoaderRegistryImpl.
pub struct CtmLoaderRegistry;

thread_local! {
    /// java: loaderMap. Rust carry: the loader objects are Rc, so the registry map lives on
    /// the loading thread (resource loads are single-threaded on the client thread).
    static LOADER_MAP: RefCell<HashMap<String, Rc<dyn CtmLoaderObj>>> =
        RefCell::new(HashMap::new());
}

impl CtmLoaderRegistry {
    /// java: CtmLoaderRegistry.get().
    pub fn get() -> &'static CtmLoaderRegistry {
        static INSTANCE: CtmLoaderRegistry = CtmLoaderRegistry;
        &INSTANCE
    }

    /// java: registerLoader(method, loader).
    pub fn register_loader(&self, method: &str, loader: Rc<dyn CtmLoaderObj>) {
        LOADER_MAP.with(|m| m.borrow_mut().insert(method.to_string(), loader));
    }

    /// java: getLoader(method) - @Nullable.
    pub fn get_loader(&self, method: &str) -> Option<Rc<dyn CtmLoaderObj>> {
        LOADER_MAP.with(|m| m.borrow().get(method).cloned())
    }
}

/// java: client.resource.InvalidIdentifierStateHolder's thread-local BooleanState.
thread_local! {
    static STATES: RefCell<BooleanState> = RefCell::new(BooleanState::new());
}

/// java: InvalidIdentifierStateHolder.get() + the enable/disable calls on it; the seam keeps
/// the state on the calling thread and routes enable/disable through it.
pub fn invalid_identifier_state_enable() {
    STATES.with(|s| s.borrow().enable());
}

pub fn invalid_identifier_state_disable() {
    STATES.with(|s| s.borrow().disable());
}

/// java: the identifier validation mixin only rejects invalid identifiers when the
/// shared state is not enabled (InvalidIdentifierStateHolder.isInvalidIdentifierState()).
pub fn invalid_identifier_state_is_enabled() -> bool {
    STATES.with(|s| s.borrow().is_enabled())
}

/// java: private record LoadingContainer<T extends CtmProperties>(CtmLoader<T> loader,
/// T properties) implements Comparable<LoadingContainer<?>>.
pub struct LoadingContainer {
    pub loader: Rc<dyn CtmLoaderObj>,
    pub properties: Box<dyn CtmProperties>,
}

impl LoadingContainer {
    /// java: toProcessorHolder(textureGetter).
    pub fn to_processor_holder(
        &self,
        texture_getter: &TextureGetter,
    ) -> ProcessorHolder {
        let processor = self.loader.create_processor(self.properties.as_ref(), texture_getter);
        let predicates = self.loader.create_predicates(self.properties.as_ref(), texture_getter);
        ProcessorHolder { processor, predicates }
    }

    /// java: compareTo(o) = properties.compareTo(o.properties).
    pub fn compare_to(&self, other: &LoadingContainer) -> Ordering {
        self.properties.compare_to(other.properties.as_ref())
    }
}

/// java: public static class LoadingResult.
pub struct LoadingResult {
    containers: Vec<LoadingContainer>,
    /// java: Map<Identifier, Set<Identifier>> textureDependencies (atlas id -> texture ids).
    texture_dependencies: HashMap<Identifier, HashSet<Identifier>>,
}

impl LoadingResult {
    /// java: createProcessorHolders(textureGetter).
    pub fn create_processor_holders(
        &self,
        texture_getter: &TextureGetter,
    ) -> Vec<ProcessorHolder> {
        let mut processor_holders = Vec::new();
        for container in &self.containers {
            processor_holders.push(container.to_processor_holder(texture_getter));
        }
        processor_holders
    }

    /// java: getTextureDependencies().
    pub fn get_texture_dependencies(&self) -> &HashMap<Identifier, HashSet<Identifier>> {
        &self.texture_dependencies
    }
}

/// java: public class CtmPropertiesLoader.
pub struct CtmPropertiesLoader<'a> {
    resource_manager: &'a ResourceManager,
    containers: Vec<LoadingContainer>,
    texture_dependencies: HashMap<Identifier, HashSet<Identifier>>,
}

impl<'a> CtmPropertiesLoader<'a> {
    fn new(resource_manager: &'a ResourceManager) -> CtmPropertiesLoader<'a> {
        CtmPropertiesLoader {
            resource_manager,
            containers: Vec::new(),
            texture_dependencies: HashMap::new(),
        }
    }

    /// java: public static LoadingResult loadAllWithState(ResourceManager resourceManager).
    pub fn load_all_with_state(resource_manager: &'a ResourceManager) -> LoadingResult {
        // java: TODO: move these to the very beginning of resource reload
        biome_holder_manager_clear_cache();

        let result = Self::load_all(resource_manager);

        // java: TODO: move these to the very end of resource reload
        biome_holder_manager_refresh_holders();

        result
    }

    /// java: public static LoadingResult loadAll(ResourceManager resourceManager).
    pub fn load_all(resource_manager: &'a ResourceManager) -> LoadingResult {
        Self::new(resource_manager).load_all_impl()
    }

    /// java: private LoadingResult loadAll().
    fn load_all_impl(mut self) -> LoadingResult {
        let mut pack_priority = 0;
        // java: Iterator<ResourcePack> iterator = resourceManager.streamResourcePacks().iterator();
        // carry: the pack list is snapshotted so the loop owns its iteration order.
        let packs: Vec<&ResourcePack> = self.resource_manager.stream_resource_packs().collect();
        invalid_identifier_state_enable();
        for pack in packs {
            self.load_all_pack(pack, pack_priority);
            pack_priority += 1;
        }
        invalid_identifier_state_disable();

        // java: containers.sort(Comparator.reverseOrder());
        self.containers.sort_by(|a, b| b.compare_to(a));

        LoadingResult {
            containers: self.containers,
            texture_dependencies: self.texture_dependencies,
        }
    }

    /// java: private void loadAll(ResourcePack pack, int packPriority).
    fn load_all_pack(&mut self, pack: &ResourcePack, pack_priority: i32) {
        for namespace in pack.get_namespaces(crate::seams::ResourceType::ClientResources) {
            // java: pack.findResources(type, namespace, "optifine/ctm", (resourceId,
            // inputSupplier) -> { ... }); the streams are read inside the callback.
            let mut found: Vec<(Identifier, String)> = Vec::new();
            pack.find_resources(
                crate::seams::ResourceType::ClientResources,
                &namespace,
                "optifine/ctm",
                &mut |resource_id, contents| {
                    found.push((resource_id.clone(), contents.to_string()));
                },
            );
            for (resource_id, stream) in found {
                if resource_id.path.ends_with(".properties") {
                    // java: try (InputStream stream = inputSupplier.get()) { Properties
                    // properties = new Properties(); properties.load(stream); load(...); }
                    // catch (Exception e) { LOGGER.error(...) }
                    let properties = parse_properties_stream(&stream);
                    self.load(properties, &resource_id, pack, pack_priority);
                }
            }
        }
    }

    /// java: private void load(Properties properties, Identifier resourceId, ResourcePack pack,
    /// int packPriority).
    fn load(
        &mut self,
        properties: Properties,
        resource_id: &Identifier,
        pack: &ResourcePack,
        pack_priority: i32,
    ) {
        // java: String method = properties.getProperty("method", "ctm").trim();
        let method = properties
            .get("method")
            .cloned()
            .unwrap_or_else(|| "ctm".to_string())
            .trim()
            .to_string();
        // java: CtmLoader<?> loader = CtmLoaderRegistry.get().getLoader(method);
        let loader = CtmLoaderRegistry::get().get_loader(&method);
        match loader {
            Some(loader) => {
                self.load_with(loader, properties, resource_id, pack, pack_priority, &method);
            }
            None => {
                // java: LOGGER.error("Unknown 'method' value '" + method + "' in file '" +
                // resourceId + "' in pack '" + pack.getName() + "'");
                eprintln!(
                    "[continuity] Unknown 'method' value '{}' in file '{}' in pack '{}'",
                    method, resource_id, pack.get_name()
                );
            }
        }
    }

    /// java: private <T extends CtmProperties> void load(CtmLoader<T> loader, Properties
    /// properties, Identifier resourceId, ResourcePack pack, int packPriority, String method).
    fn load_with(
        &mut self,
        loader: Rc<dyn CtmLoaderObj>,
        properties: Properties,
        resource_id: &Identifier,
        pack: &ResourcePack,
        pack_priority: i32,
        method: &str,
    ) {
        // java: T ctmProperties = loader.getPropertiesFactory().createProperties(properties,
        // resourceId, pack, packPriority, resourceManager, method);
        let Some(mut ctm_properties) = loader.create_properties(
            &properties,
            resource_id,
            pack,
            pack_priority,
            self.resource_manager,
            method,
        ) else {
            return;
        };
        // java: LoadingContainer<T> container = new LoadingContainer<>(loader, ctmProperties);
        // containers.add(container);
        let mut container = LoadingContainer {
            loader,
            properties: ctm_properties,
        };
        // java: for (SpriteIdentifier spriteId : ctmProperties.getTextureDependencies()) {
        // Set<Identifier> atlasTextureDependencies =
        // textureDependencies.computeIfAbsent(spriteId.getAtlasId(), id -> new ...);
        // atlasTextureDependencies.add(spriteId.getTextureId()); }
        let sprite_dependencies = container.properties.get_texture_dependencies();
        for sprite_id in sprite_dependencies {
            let atlas_texture_dependencies = self
                .texture_dependencies
                .entry(sprite_identifier_atlas_id(&sprite_id))
                .or_default();
            atlas_texture_dependencies.insert(sprite_identifier_texture_id(&sprite_id));
        }
        self.containers.push(container);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeProperties {
        base: BaseCtmProperties,
    }

    impl CtmProperties for FakeProperties {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn get_texture_dependencies(&mut self) -> HashSet<SpriteId> {
            self.base.get_texture_dependencies()
        }

        fn compare_to(&self, other: &dyn CtmProperties) -> Ordering {
            self.base.compare_to(other)
        }

        fn as_base(&self) -> Option<&BaseCtmProperties> {
            Some(&self.base)
        }
    }

    struct FakeLoader {
        create_count: std::sync::atomic::AtomicI32,
        null_properties: bool,
    }

    impl CtmLoaderObj for FakeLoader {
        fn create_properties(
            &self,
            _properties: &Properties,
            resource_id: &Identifier,
            _pack: &ResourcePack,
            _pack_priority: i32,
            _resource_manager: &ResourceManager,
            _method: &str,
        ) -> Option<Box<dyn CtmProperties>> {
            self.create_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.null_properties {
                return None;
            }
            let mut base = BaseCtmProperties::new(
                Properties::new(),
                resource_id.clone(),
                "testpack".into(),
                0,
                None,
                "ctm".into(),
            );
            base.tiles = vec![Identifier::new("minecraft", "textures/block/stone.png").unwrap()];
            Some(Box::new(FakeProperties { base }))
        }

        fn create_processor(
            &self,
            _properties: &dyn CtmProperties,
            _texture_getter: &TextureGetter,
        ) -> Rc<dyn QuadProcessor> {
            Rc::new(CountingProcessor)
        }

        fn create_predicates(
            &self,
            _properties: &dyn CtmProperties,
            _texture_getter: &TextureGetter,
        ) -> Rc<dyn CachingPredicates> {
            Rc::new(CountingPredicates)
        }
    }

    struct CountingProcessor;

    impl QuadProcessor for CountingProcessor {
        fn process_quad(
            &self,
            _quad: &MutableQuadView,
            _sprite: &Sprite,
            _block_view: &BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &BlockPos,
            _random_supplier: &dyn Fn() -> Random,
            _pass: i32,
            _context: &mut dyn ProcessingContext,
        ) -> ProcessingResult {
            ProcessingResult::NextProcessor
        }
    }

    struct CountingPredicates;

    impl CachingPredicates for CountingPredicates {
        fn affects_sprites(&self) -> bool { false }
        fn affects_sprite(&self, _sprite: &Sprite) -> bool { false }
        fn affects_block_states(&self) -> bool { false }
        fn affects_block_state(&self, _state: &BlockState) -> bool { false }
        fn is_valid_for_multipass(&self) -> bool { false }
    }

    fn test_pack(method: &str) -> ResourcePack {
        let mut pack = ResourcePack::default();
        pack.name = "testpack".into();
        pack.namespaces = vec!["minecraft".into()];
        let properties_text = format!("method={method}\nmatchTiles=stone\n");
        pack.resources.insert(
            "minecraft".into(),
            vec![(
                Identifier::new("minecraft", "optifine/ctm/stone.properties").unwrap(),
                properties_text,
            )],
        );
        pack
    }

    fn reset_registry() {
        // java: the registry singleton keeps entries for the process; the test registers
        // under a fresh method name each time instead of clearing the global map.
    }

    #[test]
    fn load_all_reads_properties_files_and_registers_containers() {
        reset_registry();
        let loader = Rc::new(FakeLoader {
            create_count: std::sync::atomic::AtomicI32::new(0),
            null_properties: false,
        });
        let method = "testctm_loadall";
        CtmLoaderRegistry::get().register_loader(method, loader.clone());

        let pack = test_pack("testctm_loadall");
        let manager = ResourceManager { packs: vec![pack], redirect_handler: None };
        let result = CtmPropertiesLoader::load_all(&manager);

        assert_eq!(
            loader.create_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(result.containers.len(), 1);
    }

    #[test]
    fn unknown_method_logs_and_skips() {
        reset_registry();
        let mut text_pack = test_pack("doesnotexist");
        text_pack.resources.get_mut("minecraft").unwrap()[0].1 = "method=doesnotexist\n".into();
        let manager = ResourceManager { packs: vec![text_pack], redirect_handler: None };
        // java: logs and adds nothing.
        let result = CtmPropertiesLoader::load_all(&manager);
        assert_eq!(result.containers.len(), 0);
    }

    #[test]
    fn null_properties_are_skipped() {
        reset_registry();
        let loader = Rc::new(FakeLoader {
            create_count: std::sync::atomic::AtomicI32::new(0),
            null_properties: true,
        });
        let method = "testctm_null";
        CtmLoaderRegistry::get().register_loader(method, loader.clone());

        let manager = ResourceManager { packs: vec![test_pack(method)], redirect_handler: None };
        let result = CtmPropertiesLoader::load_all(&manager);
        assert_eq!(
            loader.create_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(result.containers.len(), 0);
    }

    #[test]
    fn texture_dependencies_group_by_atlas() {
        reset_registry();
        let loader = Rc::new(FakeLoader {
            create_count: std::sync::atomic::AtomicI32::new(0),
            null_properties: false,
        });
        let method = "testctm_deps";
        CtmLoaderRegistry::get().register_loader(method, loader);

        let manager = ResourceManager { packs: vec![test_pack(method)], redirect_handler: None };
        let result = CtmPropertiesLoader::load_all(&manager);
        let deps = result.get_texture_dependencies();
        // java: the atlas id is the block atlas constant; the texture id is the resolved sprite id.
        assert_eq!(deps.len(), 1);
        let atlas = sprite_identifier_atlas_id(&Identifier::new("minecraft", "block/stone").unwrap());
        let textures = deps.get(&atlas).unwrap();
        assert_eq!(
            textures,
            &HashSet::from([Identifier::new("minecraft", "block/stone").unwrap()])
        );
    }

    #[test]
    fn containers_sort_by_properties_compare_to_descending() {
        reset_registry();
        // java: containers.sort(Comparator.reverseOrder()) - prioritized properties sort first.
        let mut props_high = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "a.properties").unwrap(),
            "p".into(),
            0,
            None,
            "ctm".into(),
        );
        props_high.prioritized = true;
        let props_low = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "b.properties").unwrap(),
            "p".into(),
            0,
            None,
            "ctm".into(),
        );
        assert_eq!(props_high.compare_to(&props_low), Ordering::Greater);
        assert_eq!(props_low.compare_to(&props_high), Ordering::Less);
        // java: packPriority signum then reversed resource id compare.
        let props_older = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "x.properties").unwrap(),
            "p".into(),
            0,
            None,
            "ctm".into(),
        );
        let props_newer = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "y.properties").unwrap(),
            "p".into(),
            1,
            None,
            "ctm".into(),
        );
        assert_eq!(props_newer.compare_to(&props_older), Ordering::Greater);
        // java: same priority: larger resource id first after reverseOrder.
        let props_a = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "a.properties").unwrap(),
            "p".into(),
            0,
            None,
            "ctm".into(),
        );
        let props_b = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "b.properties").unwrap(),
            "p".into(),
            0,
            None,
            "ctm".into(),
        );
        // java: compareTo returns o1.resourceId.compareTo(self.resourceId) - reversed.
        assert_eq!(props_b.compare_to(&props_a), Ordering::Less);
    }

    #[test]
    fn create_processor_holders_built_from_containers() {
        reset_registry();
        let loader = Rc::new(FakeLoader {
            create_count: std::sync::atomic::AtomicI32::new(0),
            null_properties: false,
        });
        let method = "testctm_holders";
        CtmLoaderRegistry::get().register_loader(method, loader);

        let manager = ResourceManager { packs: vec![test_pack(method)], redirect_handler: None };
        let result = CtmPropertiesLoader::load_all(&manager);
        let texture_getter: TextureGetter = Rc::new(|sprite_id: &SpriteId| {
            Some(Sprite {
                id: sprite_id.clone(),
                min_u: 0.0,
                min_v: 0.0,
                max_u: 1.0,
                max_v: 1.0,
                animation_frame_delta: 0.0,
                emissive_sprite: None,
            })
        });
        let holders = result.create_processor_holders(&texture_getter);
        assert_eq!(holders.len(), 1);
    }

    #[test]
    fn properties_stream_parses_java_format() {
        // java: Properties.load - comments, =/: separators, whitespace, escapes, continuations.
        let text = "# comment\n! another\nmethod = ctm\nmatchTiles:stone glass\nbroken=lines\\\nplit\nunicode=\\u0041";
        let parsed = parse_properties_stream(text);
        assert_eq!(parsed.get("method").unwrap(), "ctm");
        assert_eq!(parsed.get("matchTiles").unwrap(), "stone glass");
        assert_eq!(parsed.get("broken").unwrap(), "linesplit");
        assert_eq!(parsed.get("unicode").unwrap(), "A");
        // java: a key with no separator gets an empty value.
        let bare = parse_properties_stream("flag\n");
        assert_eq!(bare.get("flag").unwrap(), "");
    }

    #[test]
    fn invalid_identifier_state_nests_enable_disable() {
        // java: BooleanState counts enable/disable depth.
        invalid_identifier_state_enable();
        invalid_identifier_state_enable();
        invalid_identifier_state_disable();
        invalid_identifier_state_disable();
        // java: balanced calls leave the state disabled.
        let disabled = STATES.with(|s| !s.borrow().is_enabled());
        assert!(disabled);
    }
}
