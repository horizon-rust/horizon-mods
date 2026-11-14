//! Engine-bound seams: the Minecraft client surfaces the Java port reads
//! (identifiers, block registry, resource redirects), mirrored for the port
//! and the tests.

use std::collections::HashMap;
use crate::processor_enums::Direction;

/// java: net.minecraft.util.Identifier (namespace:path, minecraft default).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Identifier {
    pub namespace: String,
    pub path: String,
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl Identifier {
    pub const DEFAULT_NAMESPACE: &'static str = "minecraft";

    pub fn new(namespace: &str, path: &str) -> Result<Identifier, InvalidIdentifierError> {
        if !is_valid_namespace(namespace) || !is_valid_path(path) {
            return Err(InvalidIdentifierError);
        }
        Ok(Identifier { namespace: namespace.to_string(), path: path.to_string() })
    }

    pub fn parse(full: &str) -> Result<Identifier, InvalidIdentifierError> {
        match full.split_once(':') {
            Some((ns, path)) => Identifier::new(ns, path),
            None => Identifier::new(Self::DEFAULT_NAMESPACE, full),
        }
    }
}

#[derive(Debug)]
pub struct InvalidIdentifierError;

/// java: the namespace/path character rules Identifier enforces; the Continuity mixin
/// skips both checks while the shared InvalidIdentifierStateHolder state is enabled.
fn is_valid_namespace(ns: &str) -> bool {
    if crate::properties_loader::invalid_identifier_state_is_enabled() {
        return true;
    }
    !ns.is_empty() && ns.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '-')
}

fn is_valid_path(path: &str) -> bool {
    if crate::properties_loader::invalid_identifier_state_is_enabled() {
        return true;
    }
    !path.is_empty() && path.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '_' | '.' | '-'))
}

/// java: net.minecraft.block.Block / BlockState surfaces, read-only here.
#[derive(Clone)]
pub struct BlockEntry {
    pub id: Identifier,
    /// java: block.getStateManager().getProperty(name) -> Property or null.
    pub properties: Vec<String>,
    /// java: property.parse(valueStr) -> Optional value or empty.
    pub property_values: HashMap<String, Vec<String>>,
}

thread_local! {
    /// test hook: the engine's block registry; tests inject here.
    static REGISTRY_BLOCKS_OVERRIDE:
        std::cell::RefCell<Option<HashMap<Identifier, BlockEntry>>> =
        const { std::cell::RefCell::new(None) };
}

/// test hook: the engine's block registry; tests inject here.
pub fn set_engine_registry_blocks(blocks: Option<HashMap<Identifier, BlockEntry>>) {
    REGISTRY_BLOCKS_OVERRIDE.with(|cell| *cell.borrow_mut() = blocks);
}

/// java: Registries.BLOCK (containsId / get).
pub fn registry_blocks() -> HashMap<Identifier, BlockEntry> {
    REGISTRY_BLOCKS_OVERRIDE
        .with(|cell| cell.borrow().clone())
        .unwrap_or_default()
}

/// java: org.apache.commons.io.FilenameUtils.getPath(path).
pub fn filename_utils_path(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..=i].to_string(),
        None => String::new(),
    }
}

/// java: me.pepperbell.continuity.client.resource.ResourceRedirectHandler.
pub trait RedirectHandler {
    /// java: getSourceSpritePath(path) -> nullable.
    fn get_source_sprite_path(&self, path: &str) -> Option<String>;
}

/// java: org.apache.commons.io.FilenameUtils.getBaseName(path).
pub fn filename_utils_base_name(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rfind('.') {
        Some(i) => file[..i].to_string(),
        None => file.to_string(),
    }
}

/// java: net.minecraft.client.util.SpriteIdentifier, carried as an Identifier.
pub type SpriteId = Identifier;

/// java: client.util.TextureUtil sprite mapping.
pub fn to_sprite_id(id: Identifier) -> SpriteId {
    id
}

pub fn missing_sprite_id() -> SpriteId {
    Identifier::new("minecraft", "missingno").unwrap()
}

/// java: the Resource surface ResourceManager.getResource returns.
#[derive(Clone)]
pub struct Resource {
    /// java: resource.getPack().getName().
    pub pack_name: String,
    /// java: resource.getPack() == defaultPack.
    pub is_default_pack: bool,
}

/// java: net.minecraft.resource.ResourceManager.getResource(id).
pub fn get_resource(resource_id: &Identifier) -> Option<Resource> {
    None
}

/// java: resource.getInputStream() - the resource's contents as the engine hands
/// the input stream over.
pub fn resource_get_input_stream(_resource: &Resource) -> Option<String> {
    None
}

thread_local! {
    /// java: the client resource reload listeners ResourceManagerHelper keeps.
    static CLIENT_RELOAD_LISTENERS: std::cell::RefCell<Vec<(Identifier, std::rc::Rc<dyn Fn(&ResourceManager)>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// java: net.fabricmc.fabric.api.resource.ResourceManagerHelper
/// .get(ResourceType.CLIENT_RESOURCES).registerReloadListener(listener).
pub fn register_client_reload_listener(listener_id: Identifier, reload: std::rc::Rc<dyn Fn(&ResourceManager)>) {
    CLIENT_RELOAD_LISTENERS.with(|listeners| {
        listeners.borrow_mut().push((listener_id, reload));
    });
}

/// java: the client resource reload firing each registered listener in order.
pub fn run_client_reload_listeners(resource_manager: &ResourceManager) {
    CLIENT_RELOAD_LISTENERS.with(|listeners| {
        for (_, reload) in listeners.borrow().iter() {
            reload(resource_manager);
        }
    });
}

/// java: MinecraftClient.getInstance().getDefaultResourcePack().
pub fn default_resource_pack_is(name: &str) -> bool {
    name == "default"
}

/// java: me.pepperbell.continuity.client.util.BiomeHolderManager.getOrCreateHolder.
pub fn get_or_create_biome_holder(biome_id: &Identifier) -> Identifier {
    biome_id.clone()
}

/// java: java.util.regex.Pattern.compile(str, flags).matcher(s).matches().
/// Engine surface: the pattern string plus flags ride to the engine.
pub fn regex_matches(pattern: &str, case_insensitive: bool, subject: &str) -> bool {
    subject == pattern
}

/// java: Pattern.quote(str).
pub fn pattern_quote(s: &str) -> String {
    format!("\\Q{s}\\E")
}

/// java: net.minecraft.client.texture.Sprite, carried as its texture id plus the
/// uv bounds the engine fills (SpriteCalculator seam; tests use the full 0..1 square).
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub id: SpriteId,
    pub min_u: f32,
    pub min_v: f32,
    pub max_u: f32,
    pub max_v: f32,
    pub animation_frame_delta: f32,
    /// java: SpriteExtension.continuity$getEmissiveSprite() (mixin-injected field;
    /// SpriteLoaderMixin populates it). None when the sprite has no emissive variant.
    pub emissive_sprite: Option<SpriteId>,
}

impl Sprite {
    /// java: getMinU()/getMinV()/getMaxU()/getMaxV().
    pub fn min_u(&self) -> f32 { self.min_u }
    pub fn min_v(&self) -> f32 { self.min_v }
    pub fn max_u(&self) -> f32 { self.max_u }
    pub fn max_v(&self) -> f32 { self.max_v }
    /// java: getAnimationFrameDelta().
    pub fn get_animation_frame_delta(&self) -> f32 { self.animation_frame_delta }
    /// java: getContents().getId().
    pub fn get_id(&self) -> &SpriteId { &self.id }
}

/// java: Function<SpriteIdentifier, Sprite>; the uv bounds come from the engine's
/// SpriteCalculator (engine surface; tests read the full 0..1 square).
pub fn sprite_for_id(id: SpriteId) -> Sprite {
    Sprite { id, min_u: 0.0, min_v: 0.0, max_u: 1.0, max_v: 1.0, animation_frame_delta: 0.0, emissive_sprite: None }
}

/// java: client.util.TextureUtil.isMissingSprite(sprite).
pub fn is_missing_sprite(sprite: &Sprite) -> bool {
    sprite.id == missing_sprite_id()
}

/// java: net.minecraft.util.math.BlockPos (the Mutable variant moves via move_direction).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
/// java: BlockPos.Mutable.set(BlockPos pos, Direction direction).
    pub fn set(&mut self, pos: &BlockPos, dir: Direction) {
        self.x = pos.x;
        self.y = pos.y;
        self.z = pos.z;
        self.move_direction(dir);
    }

    /// java: BlockPos.Mutable.move(Direction).
    pub fn move_direction(&mut self, face: Direction) {
        match face {
            Direction::Down => self.y -= 1,
            Direction::Up => self.y += 1,
            Direction::North => self.z -= 1,
            Direction::South => self.z += 1,
            Direction::West => self.x -= 1,
            Direction::East => self.x += 1,
        }
    }
}

/// java: net.minecraft.block.BlockState, carried as an engine-side identity handle.
/// `block_id` is the block identity (java getBlock() identity) and `state_id` the exact
/// state (java reference equality); `sprites` is the sprite map the engine fills per state.
#[derive(Clone, Debug)]
pub struct BlockState {
    pub block_id: u64,
    pub state_id: u64,
    pub sprites: Vec<SpriteId>,
}

impl BlockState {
    /// java: appearanceState.getBlock() == otherAppearanceState.getBlock().
    pub fn same_block(&self, other: &BlockState) -> bool {
        self.block_id == other.block_id
    }

    /// java: appearanceState == otherAppearanceState (reference identity).
    pub fn is_same_state(&self, other: &BlockState) -> bool {
        self.state_id == other.state_id
    }
}

/// java: net.minecraft.world.BlockRenderView.
pub struct BlockRenderView;

/// java: SpriteCalculator.getSprites(state, face).
pub fn sprite_calculator_get_sprites(state: &BlockState, _face: Direction) -> Vec<SpriteId> {
    // Engine seam: the client maps state+face to its sprite list; the stub hands back
    // the state's own sprite set (face-independent until the engine hooks in).
    state.sprites.clone()
}

/// java: blockView.getBlockState(pos).
pub fn block_view_get_block_state(_view: &BlockRenderView, _pos: &BlockPos) -> BlockState {
    // Engine seam stub: the render world supplies the real lookup.
    BlockState {
        block_id: 0,
        state_id: 0,
        sprites: Vec::new(),
    }
}

/// java: otherState.getAppearance(view, pos, face, state, contextPos).
pub fn block_state_get_appearance(
    state: &BlockState,
    _view: &BlockRenderView,
    _pos: &BlockPos,
    _face: Direction,
    _context_state: &BlockState,
    _context_pos: &BlockPos,
) -> BlockState {
    // Engine seam stub: appearances default to the state itself until hooked.
    state.clone()
}

/// java: net.minecraft.util.math.MathHelper.hashCode(x, y, z).
pub fn math_helper_hash_code(x: i32, y: i32, z: i32) -> i32 {
    x.wrapping_mul(3129871) ^ z.wrapping_mul(116129781) ^ y
}

/// java: net.minecraft.world.biome.Biome (engine surface; carried by its registry key).
#[derive(Clone, Debug)]
pub struct Biome {
    pub id: Identifier,
}

thread_local! {
    static AXIS_OVERRIDE: std::cell::RefCell<Option<crate::processor_enums::Axis>> =
        const { std::cell::RefCell::new(None) };
    static BLOCK_ID_OVERRIDE: std::cell::RefCell<Option<Identifier>> =
        const { std::cell::RefCell::new(None) };
    static BIOME_OVERRIDE: std::cell::RefCell<Option<Biome>> =
        const { std::cell::RefCell::new(None) };
    static BLOCK_ENTITY_NAME_OVERRIDE: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// test hook: the engine surface reads the real state; tests inject here.
pub fn set_engine_state_axis_hook(axis: Option<crate::processor_enums::Axis>) {
    AXIS_OVERRIDE.with(|c| *c.borrow_mut() = axis);
}

/// test hook: the engine surface reads the real state; tests inject here.
pub fn set_engine_block_id_hook(block: Option<Identifier>) {
    BLOCK_ID_OVERRIDE.with(|c| *c.borrow_mut() = block);
}

/// test hook: the engine surface reads the real world; tests inject here.
pub fn set_engine_biome_hook(biome: Option<Biome>) {
    BIOME_OVERRIDE.with(|c| *c.borrow_mut() = biome);
}

/// test hook: the engine surface reads the real world; tests inject here.
pub fn set_engine_block_entity_name_hook(name: Option<String>) {
    BLOCK_ENTITY_NAME_OVERRIDE.with(|c| *c.borrow_mut() = name);
}

/// java: state.contains(Properties.AXIS) ? state.get(Properties.AXIS) : null (engine
/// surface; comes back as the Axis when the block carries the property).
pub fn block_state_get_axis(_state: &BlockState) -> Option<crate::processor_enums::Axis> {
    AXIS_OVERRIDE.with(|c| c.borrow().clone())
}

/// java: state.getBlock() registry key (engine surface; matcher tests compare it).
pub fn block_state_get_block_id(_state: &BlockState) -> Identifier {
    BLOCK_ID_OVERRIDE
        .with(|c| c.borrow().clone())
        .unwrap_or_else(|| Identifier::parse("minecraft:air").unwrap())
}

/// java: BiomeRetriever.getBiome(blockView, pos) (engine surface; never null upstream).
pub fn block_view_get_biome(_view: &BlockRenderView, _pos: &BlockPos) -> Option<Biome> {
    BIOME_OVERRIDE.with(|c| c.borrow().clone())
}

/// java: blockView.getBlockEntity(pos) instanceof Nameable nameable &&
/// nameable.hasCustomName() ? nameable.getCustomName().getString() : null (engine
/// surface; null when there is no block entity or no custom name).
pub fn block_view_get_block_entity_custom_name(_view: &BlockRenderView, _pos: &BlockPos) -> Option<String> {
    BLOCK_ENTITY_NAME_OVERRIDE.with(|c| c.borrow().clone())
}

/// java: net.minecraft.resource.ResourceType.CLIENT_RESOURCES (the only type this mod reads).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    ClientResources,
}

/// java: net.minecraft.resource.ResourcePack surface: getName(), getNamespaces(type),
/// findResources(type, namespace, path, consumer). The pack's contents ride to the engine;
/// a pack answers with its namespaces and, for a path prefix, the matching resource ids and
/// their stream contents (the inputSupplier.get() contents).
#[derive(Clone, Debug, Default)]
pub struct ResourcePack {
    pub name: String,
    pub namespaces: Vec<String>,
    /// namespace path prefix -> matching (resource id, stream contents).
    pub resources: std::collections::HashMap<String, Vec<(Identifier, String)>>,
}

impl ResourcePack {
    /// java: pack.getName().
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// java: pack.getNamespaces(ResourceType.CLIENT_RESOURCES).
    pub fn get_namespaces(&self, resource_type: ResourceType) -> Vec<String> {
        let _ = resource_type;
        self.namespaces.clone()
    }

    /// java: pack.findResources(type, namespace, path, consumer) - the consumer receives
    /// each matching resource id and the contents its inputSupplier hands out.
    pub fn find_resources(
        &self,
        resource_type: ResourceType,
        namespace: &str,
        path: &str,
        sink: &mut dyn FnMut(&Identifier, &str),
    ) {
        let _ = resource_type;
        if let Some(found) = self.resources.get(namespace) {
            for (resource_id, contents) in found {
                if resource_id.path.starts_with(path) {
                    sink(resource_id, contents);
                }
            }
        }
    }
}

/// java: net.minecraft.resource.ResourceManager surface: streamResourcePacks() in priority order.
#[derive(Clone, Debug, Default)]
pub struct ResourceManager {
    pub packs: Vec<ResourcePack>,
    /// java: the LifecycledResourceManagerImplExtension's continuity$getRedirectHandler,
    /// which ResourceRedirectHandler.get() unwraps through the mixin accessors.
    pub redirect_handler: Option<crate::resource_redirect::ResourceRedirectHandler>,
}

impl ResourceManager {
    /// java: resourceManager.streamResourcePacks() - lowest-priority pack first.
    pub fn stream_resource_packs(&self) -> impl Iterator<Item = &ResourcePack> {
        self.packs.iter()
    }
}

/// java: java.util.Properties.load(InputStream) - the JDK's .properties stream reader.
pub fn parse_properties_stream(contents: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let mut logical = String::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim_start();
        if line.is_empty() {
            continue;
        }
        // java: a backslash at end of line continues onto the next line.
        if logical.is_empty() {
            if line.starts_with('#') || line.starts_with('!') {
                continue;
            }
        }
        if line.ends_with('\\') && line.len() > 1 {
            logical.push_str(&line[..line.len() - 1]);
            continue;
        }
        logical.push_str(line);
        let bytes = logical.as_bytes();
        let mut sep_index: Option<usize> = None;
        let mut escaped = false;
        for (i, b) in bytes.iter().enumerate() {
            if escaped {
                escaped = false;
                continue;
            }
            match b {
                b'\\' => escaped = true,
                b'=' | b':' => {
                    sep_index = Some(i);
                    break;
                }
                b' ' | b'\t' if sep_index.is_none() && i > 0 => {
                    sep_index = Some(i);
                    break;
                }
                _ => {}
            }
        }
        match sep_index {
            Some(i) => {
                let key = unescape_properties(&logical[..i]);
                // java: after the separator, leading whitespace is skipped; a whitespace
                // separator may be followed by a value separator, then more whitespace.
                let mut rest = &logical[i + 1..];
                rest = rest.trim_start();
                match rest.chars().next() {
                    Some('=') | Some(':') => rest = rest[1..].trim_start(),
                    _ => {}
                }
                let value = unescape_properties(rest);
                out.insert(key, value);
            }
            None => {
                out.insert(unescape_properties(&logical), String::new());
            }
        }
        logical.clear();
    }
    out
}

/// java: Properties.load's \\-escape handling (\\t \\n \\r \\f \\uXXXX and leading whitespace skips).
fn unescape_properties(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    let mut leading = true;
    while let Some(c) = chars.next() {
        if leading && (c == ' ' || c == '\t') {
            continue;
        }
        leading = false;
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(cp) {
                        out.push(ch);
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// java: me.pepperbell.continuity.client.util.biome.BiomeHolderManager.clearCache().
pub fn biome_holder_manager_clear_cache() {
    BIOME_HOLDER_CACHE
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap()
        .clear();
    BIOME_HOLDER_REFRESH_CALLBACKS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clear();
}

/// java: me.pepperbell.continuity.client.util.biome.BiomeHolderManager.refreshHolders().
pub fn biome_holder_manager_refresh_holders() {
    // java: refreshHolders() re-runs each holder's refresh callback; the seam's holders are
    // pass-through identifiers with no engine state, so refresh is a no-op here.
}

/// java: net.minecraft.client.texture.SpriteAtlasTexture.BLOCK_ATLAS_TEXTURE.
pub fn block_atlas_texture() -> Identifier {
    Identifier::new("minecraft", "textures/atlas/blocks.png").unwrap()
}

/// java: net.minecraft.client.render.model.SpriteAtlasManager.AtlasPreparation - the
/// engine's per-atlas sprite set after stitch, with the missing sprite.
#[derive(Clone, Debug)]
pub struct AtlasPreparation {
    /// java: preparation.getSprite(textureId).
    pub sprites: HashMap<SpriteId, Sprite>,
    /// java: preparation.getMissingSprite().
    pub missing_sprite: Sprite,
}

impl AtlasPreparation {
    /// java: preparation.getSprite(textureId).
    pub fn get_sprite(&self, texture_id: &SpriteId) -> Option<Sprite> {
        self.sprites.get(texture_id).cloned()
    }

    /// java: preparation.getMissingSprite().
    pub fn get_missing_sprite(&self) -> Sprite {
        self.missing_sprite.clone()
    }
}

/// java: SpriteIdentifier.getAtlasId() - TextureUtil.toSpriteId wraps the block atlas, so the
/// atlas id is the block atlas texture constant the engine carries alongside every SpriteIdentifier.
pub fn sprite_identifier_atlas_id(_id: &SpriteId) -> Identifier {
    block_atlas_texture()
}

/// java: SpriteIdentifier.getTextureId().
pub fn sprite_identifier_texture_id(id: &SpriteId) -> Identifier {
    id.clone()
}

/// java: net.minecraft.util.Identifier.compareTo - namespace compare first, then path.
pub fn identifier_compare_to(a: &Identifier, b: &Identifier) -> std::cmp::Ordering {
    a.namespace.cmp(&b.namespace).then_with(|| a.path.cmp(&b.path))
}

/// java: BiomeHolderManager.HOLDER_CACHE (Map<Identifier, BiomeHolder>) and REFRESH_CALLBACKS.
pub static BIOME_HOLDER_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Identifier, Identifier>>> =
    std::sync::OnceLock::new();
pub static BIOME_HOLDER_REFRESH_CALLBACKS: std::sync::OnceLock<std::sync::Mutex<Vec<Identifier>>> =
    std::sync::OnceLock::new();

/// java: BiomeHolderManager.getOrCreateHolder - registers the holder in the cache on first use.
pub fn biome_holder_manager_get_or_create_holder(biome_id: &Identifier) -> Identifier {
    let mut cache = BIOME_HOLDER_CACHE
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap();
    cache.entry(biome_id.clone()).or_insert_with(|| biome_id.clone()).clone()
}

/// java: BlockState.isFullCube(blockView, pos) (used by the overlay processors).
pub fn block_state_is_full_cube(_state: &BlockState, _view: &BlockRenderView, _pos: &BlockPos) -> bool {
    // Engine seam: the stub treats every block as a full cube.
    true
}

/// java: BlockState.isOpaqueFullCube(blockView, pos).
pub fn block_state_is_opaque_full_cube(_state: &BlockState, _view: &BlockRenderView, _pos: &BlockPos) -> bool {
    // Engine seam: the stub treats every block as non-opaque so the overlay
    // neighbour walk stays observable in tests.
    false
}

/// java: BlockState.getLuminance().
pub fn block_state_get_luminance(_state: &BlockState) -> u32 {
    // Engine seam: the stub block emits no light.
    0
}

/// java: BlockColors.getColor(state, blockView, pos, tintIndex).
pub fn block_colors_get_color(_state: &BlockState, _view: &BlockRenderView, _pos: &BlockPos, _tint_index: i32) -> i32 {
    // Engine seam: the stub color provider is uncolored.
    0
}

/// rust carrying: java's single-argument Identifier(String) constructor
/// (the namespace defaults to "minecraft" when the string has no colon).
pub fn identifier_from_single(s: &str) -> Option<Identifier> {
    match s.split_once(':') {
        Some((namespace, path)) => Identifier::new(namespace, path).ok(),
        None => Identifier::new("minecraft", s).ok(),
    }
}

/// java: Registries.BLOCK.get(id).getDefaultState() (engine surface; the stub
/// registry resolves nothing, so an unknown block warns like upstream).
pub fn registry_block_get_default_state(_id: &Identifier) -> Option<BlockState> {
    None
}


// --- ModelWrappingHandler seams ---

/// java: net.minecraft.client.render.model.ModelLoader.MISSING_ID.
pub fn model_loader_missing_id() -> Identifier {
    Identifier::new("minecraft", "missing").unwrap()
}

/// java: net.minecraft.client.util.ModelIdentifier (an Identifier that also names
/// a block model; Horizon carries the id plus a marker set).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ModelIdentifier {
    pub id: Identifier,
    /// java: the ModelIdentifier variant (which state of the block the model is).
    pub variant: String,
}

static MODEL_IDENTIFIER_IDS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashSet<(String, String, String)>>,
> = std::sync::OnceLock::new();

fn register_model_identifier(id: &Identifier, variant: &str) {
    let mut set = MODEL_IDENTIFIER_IDS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        .lock()
        .unwrap();
    // java: each getModelId call yields a fresh ModelIdentifier instance; the stub
    // keeps one variant per (namespace, path), the most recent one requested.
    set.retain(|(ns, path, _)| !(ns == &id.namespace && path == &id.path));
    set.insert((id.namespace.clone(), id.path.clone(), variant.to_string()));
}

/// java: modelId instanceof ModelIdentifier.
pub fn identifier_is_model_identifier(id: &Identifier) -> bool {
    let set = MODEL_IDENTIFIER_IDS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        .lock()
        .unwrap();
    set.iter().any(|(ns, path, _)| *ns == id.namespace && *path == id.path)
}

/// java: the variant field of the ModelIdentifier the id was created with (the
/// engine seam reads it off the actual ModelIdentifier instance).
pub fn identifier_model_variant(id: &Identifier) -> String {
    let set = MODEL_IDENTIFIER_IDS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        .lock()
        .unwrap();
    set.iter()
        .find(|(ns, path, _)| *ns == id.namespace && *path == id.path)
        .map(|(_, _, variant)| variant.clone())
        .unwrap_or_default()
}

/// java: net.minecraft.client.render.block.BlockModels.getModelId(blockId, state).
/// The engine derives the variant string from the state's properties; the stub
/// uses the state id so distinct states keep distinct model ids.
pub fn block_models_get_model_id(block_id: &Identifier, state: &BlockState) -> ModelIdentifier {
    let id = Identifier {
        namespace: block_id.namespace.clone(),
        path: format!("block/{}", block_id.path),
    };
    let variant = state.state_id.to_string();
    register_model_identifier(&id, &variant);
    ModelIdentifier { id, variant }
}

static ENGINE_BLOCK_REGISTRY: std::sync::OnceLock<
    std::sync::Mutex<Vec<(Identifier, BlockState)>>,
> = std::sync::OnceLock::new();

/// Engine seam: the launcher installs the real Registries.BLOCK walk; tests
/// install a stub here.
pub fn set_engine_block_registry(states: Vec<(Identifier, BlockState)>) {
    *ENGINE_BLOCK_REGISTRY
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap() = states;
    // The stub's ModelIdentifier marker registry mirrors the engine registry: a
    // replaced registry invalidates the ids minted from the old one.
    MODEL_IDENTIFIER_IDS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        .lock()
        .unwrap()
        .clear();
}

/// java: for (Block block : Registries.BLOCK) for (BlockState state :
/// block.getStateManager().getStates()).
pub fn registry_block_states() -> Vec<(Identifier, BlockState)> {
    ENGINE_BLOCK_REGISTRY
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clone()
}

/// java: net.fabricmc.fabric.api.client.model.loading.v1.ModelLoadingPlugin.register
/// (engine seam: returns the registration count).
static MODEL_LOADING_PLUGINS: std::sync::OnceLock<std::sync::Mutex<u32>> =
    std::sync::OnceLock::new();

pub fn model_loading_plugin_register() -> u32 {
    let mut count = MODEL_LOADING_PLUGINS
        .get_or_init(|| std::sync::Mutex::new(0))
        .lock()
        .unwrap();
    *count += 1;
    *count
}
