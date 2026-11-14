//! 1:1 of the Continuity client.processor base classes plus the api.client
//! processor interfaces they implement, by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity). Upstream files: src/main/java/me/
//! pepperbell/continuity/client/processor/{AbstractQuadProcessor,
//! AbstractQuadProcessorFactory,ProcessingDataKeys,DirectionMaps,
//! ProcessingPredicate,BaseProcessingPredicate,BaseCachingPredicates}.java and
//! api/client/{QuadProcessor,ProcessingDataProvider,ProcessingDataKey,
//! ProcessingDataKeyRegistry,CachingPredicates}.java.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::parsing::BlockStateMatcher;
use crate::processor_enums::{Axis, AxisDirection, Direction, OrientationMode};
use crate::properties::{BaseCtmProperties, HeightPredicate, NamePredicate};
use crate::seams::{
    block_state_get_block_id, block_view_get_biome,
    block_view_get_block_entity_custom_name, missing_sprite_id,
};
pub use crate::seams::{block_state_get_axis, BlockPos, BlockRenderView, BlockState, Biome, Identifier, Sprite, SpriteId};

/// java: net.fabricmc.fabric.api.renderer.v1.mesh.QuadView (engine surface; carried
/// by its light face, which is all the processor logic reads).
#[derive(Clone, Debug)]
pub struct QuadView {
    pub light_face: Direction,
    /// rust carrying: the per-vertex data the engine keeps on the quad; None for
    /// predicate-only quads (the predicates read only the light face).
    pub data: Option<Rc<RefCell<QuadVertexData>>>,
    /// java: MutableQuadView.cullFace() (engine surface; None when unset).
    pub cull_face: Option<Direction>,
    /// java: MutableQuadView.material() (engine surface).
    pub material: Option<RenderMaterial>,
    /// rust carrying: the sprite the quad's UVs resolve to, which the engine's
    /// SpriteFinder recovers (None until a quad builder records it).
    pub sprite_id: Option<SpriteId>,
}

/// java: the QuadView vertex attributes the processors read and write, laid out
/// per-vertex (Java QuadView holds the same fields per vertex index).
#[derive(Clone, Debug)]
pub struct QuadVertexData {
    pub x: [f32; 4],
    pub y: [f32; 4],
    pub z: [f32; 4],
    pub color: [i32; 4],
    pub light: [i32; 4],
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub has_normal: [bool; 4],
    pub normal_x: [f32; 4],
    pub normal_y: [f32; 4],
    pub normal_z: [f32; 4],
}

impl Default for QuadVertexData {
    fn default() -> Self {
        QuadVertexData {
            x: [0.0; 4],
            y: [0.0; 4],
            z: [0.0; 4],
            color: [-1; 4],
            light: [0; 4],
            u: [0.0; 4],
            v: [0.0; 4],
            has_normal: [false; 4],
            normal_x: [0.0; 4],
            normal_y: [0.0; 4],
            normal_z: [0.0; 4],
        }
    }
}

impl QuadView {
    /// java: lightFace().
    pub fn light_face(&self) -> Direction {
        self.light_face
    }

    /// rust carrying: a bare quad with only its light face (predicate path).
    pub fn bare(light_face: Direction) -> QuadView {
        QuadView { light_face, data: None, cull_face: None, material: None, sprite_id: None }
    }

    /// rust carrying: a quad carrying full per-vertex data (processor path).
    pub fn with_data(light_face: Direction, data: QuadVertexData) -> QuadView {
        QuadView { light_face, data: Some(Rc::new(RefCell::new(data))), cull_face: None, material: None, sprite_id: None }
    }

    /// java: cullFace().
    pub fn cull_face(&self) -> Option<Direction> {
        self.cull_face
    }

    /// java: material().
    pub fn material(&self) -> Option<&RenderMaterial> {
        self.material.as_ref()
    }

    /// rust carrying: records the sprite id the engine's SpriteFinder would recover.
    pub fn set_sprite_id(&mut self, sprite_id: SpriteId) {
        self.sprite_id = Some(sprite_id);
    }

    fn vertex_data(&self) -> std::cell::Ref<'_, QuadVertexData> {
        std::cell::Ref::map(
            self.data.as_ref().expect("quad vertex data unavailable (engine seam)").borrow(),
            |d| d,
        )
    }

    fn vertex_data_mut(&self) -> std::cell::RefMut<'_, QuadVertexData> {
        self.data.as_ref().expect("quad vertex data unavailable (engine seam)").borrow_mut()
    }

    /// java: x(int) / y(int) / z(int).
    pub fn x(&self, i: usize) -> f32 { self.vertex_data().x[i] }
    pub fn y(&self, i: usize) -> f32 { self.vertex_data().y[i] }
    pub fn z(&self, i: usize) -> f32 { self.vertex_data().z[i] }
    /// java: color(int).
    pub fn color(&self, i: usize) -> i32 { self.vertex_data().color[i] }
    /// java: lightmap(int).
    pub fn lightmap(&self, i: usize) -> i32 { self.vertex_data().light[i] }
    /// java: u(int) / v(int).
    pub fn u(&self, i: usize) -> f32 { self.vertex_data().u[i] }
    pub fn v(&self, i: usize) -> f32 { self.vertex_data().v[i] }
    /// java: hasNormal(int).
    pub fn has_normal(&self, i: usize) -> bool { self.vertex_data().has_normal[i] }
    /// java: normalX(int) / normalY(int) / normalZ(int).
    pub fn normal_x(&self, i: usize) -> f32 { self.vertex_data().normal_x[i] }
    pub fn normal_y(&self, i: usize) -> f32 { self.vertex_data().normal_y[i] }
    pub fn normal_z(&self, i: usize) -> f32 { self.vertex_data().normal_z[i] }
    /// java: posByIndex(int index, int axis) (axis 0 = x, 1 = y, 2 = z).
    pub fn pos_by_index(&self, i: usize, axis: usize) -> f32 {
        match axis {
            0 => self.x(i),
            1 => self.y(i),
            _ => self.z(i),
        }
    }
    /// java: pos(int index, float x, float y, float z).
    pub fn set_pos(&self, i: usize, x: f32, y: f32, z: f32) {
        let mut d = self.vertex_data_mut();
        d.x[i] = x;
        d.y[i] = y;
        d.z[i] = z;
    }
    /// java: color(int index, int color).
    pub fn set_color(&self, i: usize, color: i32) {
        self.vertex_data_mut().color[i] = color;
    }
    /// java: lightmap(int index, int lightmap).
    pub fn set_lightmap(&self, i: usize, light: i32) {
        self.vertex_data_mut().light[i] = light;
    }
    /// java: uv(int index, float u, float v).
    pub fn set_uv(&self, i: usize, u: f32, v: f32) {
        let mut d = self.vertex_data_mut();
        d.u[i] = u;
        d.v[i] = v;
    }
    /// java: normal(int index, float x, float y, float z).
    pub fn set_normal(&self, i: usize, x: f32, y: f32, z: f32) {
        let mut d = self.vertex_data_mut();
        d.has_normal[i] = true;
        d.normal_x[i] = x;
        d.normal_y[i] = y;
        d.normal_z[i] = z;
    }
}

/// java: MutableQuadView extends QuadView (the mutable surface emitters write).
pub type MutableQuadView = QuadView;

/// java: net.minecraft.util.math.random.Random (engine surface; carried as a seed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Random {
    pub seed: u64,
}

impl Random {
    /// java: Random.nextLong() (engine surface; carried as a seed-derived value).
    pub fn next_long(&self) -> i64 {
        let mut z = self.seed.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        (z ^ (z >> 31)) as i64
    }
}

/// java: net.fabricmc.fabric.api.renderer.v1.mesh.QuadEmitter (engine surface; lands
/// with the emitter work).
/// java: net.fabricmc.fabric.api.renderer.v1.material.BlendMode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Default,
    Solid,
    CutoutMipped,
    Cutout,
    Translucent,
    TranslucentCutout,
}

impl BlendMode {
    /// java: BlendMode.values().
    pub fn values() -> [BlendMode; 6] {
        [BlendMode::Default, BlendMode::Solid, BlendMode::CutoutMipped, BlendMode::Cutout, BlendMode::Translucent, BlendMode::TranslucentCutout]
    }

    /// java: ordinal().
    pub fn ordinal(self) -> usize {
        self as usize
    }
}

/// java: net.fabricmc.fabric.api.renderer.v1.material.RenderMaterial (engine surface;
/// the renderer reads the flags; the port carries them).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderMaterial {
    pub blend_mode: BlendMode,
    pub emissive: bool,
    pub disable_diffuse: bool,
    /// java: TriState ambientOcclusion; FALSE=false, TRUE/DEFAULT=true.
    pub ambient_occlusion: bool,
}

pub struct QuadEmitter {
    /// rust carrying: the quad the emitter is building (Java builds into an
    /// internal buffer); emitted quads collect in order.
    pub current: QuadView,
    pub emitted: Vec<QuadView>,
}

impl Default for QuadEmitter {
    fn default() -> Self {
        QuadEmitter { current: QuadView::bare(Direction::North), emitted: Vec::new() }
    }
}

impl QuadEmitter {
    /// java: copyFrom(QuadView quad).
    pub fn copy_from(&mut self, quad: &QuadView) {
        self.current = quad.clone();
    }

    /// java: emit().
    pub fn emit(&mut self) {
        let face = self.current.light_face;
        let quad = std::mem::replace(&mut self.current, QuadView::bare(face));
        self.emitted.push(quad);
    }

    /// java: pos(int index, float x, float y, float z).
    pub fn pos(&mut self, i: usize, x: f32, y: f32, z: f32) {
        self.current.set_pos(i, x, y, z);
    }
    /// java: color(int index, int color).
    pub fn color(&mut self, i: usize, color: i32) {
        self.current.set_color(i, color);
    }
    /// java: lightmap(int index, int lightmap).
    pub fn lightmap(&mut self, i: usize, light: i32) {
        self.current.set_lightmap(i, light);
    }
    /// java: uv(int index, float u, float v).
    pub fn uv(&mut self, i: usize, u: f32, v: f32) {
        self.current.set_uv(i, u, v);
    }
    /// java: normal(int index, float x, float y, float z).
    pub fn normal(&mut self, i: usize, x: f32, y: f32, z: f32) {
        self.current.set_normal(i, x, y, z);
    }
    /// java: square(Direction face, float x1, float y1, float x2, float y2, float z).
    /// Engine surface: face-local corner layout is the renderer's; the port records
    /// the face and nominal corner positions the callers read back.
    pub fn square(&mut self, face: Direction, x1: f32, y1: f32, x2: f32, y2: f32, z: f32) {
        self.current = QuadView::with_data(face, QuadVertexData::default());
        self.current.set_pos(0, x1, y1, z);
        self.current.set_pos(1, x2, y1, z);
        self.current.set_pos(2, x2, y2, z);
        self.current.set_pos(3, x1, y2, z);
    }
    /// java: material(RenderMaterial material). Engine surface: the material rides
    /// to the renderer with the emitted quad.
    pub fn material(&mut self, material: &RenderMaterial) {
        self.current.material = Some(material.clone());
    }

    /// java: cullFace(Direction).
    pub fn cull_face(&mut self, cull_face: Direction) {
        self.current.cull_face = Some(cull_face);
    }

    /// rust carrying: records the sprite id the engine's SpriteFinder would recover.
    pub fn sprite_id(&mut self, sprite_id: SpriteId) {
        self.current.sprite_id = Some(sprite_id);
    }

    /// java: MeshBuilder.build() (returns the built mesh and resets the builder).
    pub fn build(&mut self) -> Mesh {
        let quads = std::mem::take(&mut self.emitted);
        Mesh { quads }
    }
}

/// java: net.fabricmc.fabric.api.renderer.v1.mesh.Mesh (engine surface; the port
/// carries the quads so outputTo can replay them).
#[derive(Default)]
pub struct Mesh {
    pub quads: Vec<QuadView>,
}

impl Mesh {
    /// java: Mesh.outputTo(QuadEmitter).
    pub fn output_to(&mut self, emitter: &mut QuadEmitter) {
        emitter.emitted.append(&mut self.quads);
    }
}

/// java: QuadProcessor.ProcessingResult { NEXT_PROCESSOR, NEXT_PASS, STOP, DISCARD }.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessingResult {
    NextProcessor,
    NextPass,
    Stop,
    Discard,
}

/// java: api.client.ProcessingDataProvider; values are type-erased over Box<dyn Any>.
pub trait ProcessingDataProvider {
    /// java: <T> T getData(ProcessingDataKey<T> key).
    fn get_data(&self, key: &ProcessingDataKey) -> Rc<RefCell<Box<dyn Any>>>;
}

/// java: api.client.ProcessingDataKey<T> as returned by the registry.
#[derive(Clone)]
pub struct ProcessingDataKey {
    pub id: Identifier,
    pub raw_id: usize,
    pub value_supplier: Rc<dyn Fn() -> Box<dyn Any>>,
    pub value_reset_action: Option<Rc<dyn Fn(&mut dyn Any)>>,
}

/// java: api.client.ProcessingDataKeyRegistry / impl.client.ProcessingDataKeyRegistryImpl.
pub struct ProcessingDataKeyRegistry {
    registered_amount: Cell<usize>,
    /// java: ProcessingDataKeyRegistryImpl.getAllResettable() (keys registered with
    /// a reset action, in registration order).
    resettable_keys: RefCell<Vec<ProcessingDataKey>>,
}

thread_local! {
    static REGISTRY: Rc<ProcessingDataKeyRegistry> = Rc::new(ProcessingDataKeyRegistry {
        registered_amount: Cell::new(0),
        resettable_keys: RefCell::new(Vec::new()),
    });
}

/// java: ProcessingDataKeyRegistry.get() singleton (thread-local carrying of the
/// upstream static).
pub fn processing_data_key_registry() -> Rc<ProcessingDataKeyRegistry> {
    REGISTRY.with(|registry| registry.clone())
}

impl ProcessingDataKeyRegistry {
    /// java: registerKey(id, valueSupplier) / registerKey(id, valueSupplier, valueResetAction).
    pub fn register_key(
        &self,
        id: Identifier,
        value_supplier: Rc<dyn Fn() -> Box<dyn Any>>,
        value_reset_action: Option<Rc<dyn Fn(&mut dyn Any)>>,
    ) -> ProcessingDataKey {
        let raw_id = self.registered_amount.get();
        self.registered_amount.set(raw_id + 1);
        let key = ProcessingDataKey {
            id,
            raw_id,
            value_supplier,
            value_reset_action,
        };
        if key.value_reset_action.is_some() {
            self.resettable_keys.borrow_mut().push(key.clone());
        }
        key
    }

    /// java: ProcessingDataKeyRegistryImpl.getAllResettable().
    pub fn get_all_resettable(&self) -> Vec<ProcessingDataKey> {
        self.resettable_keys.borrow().clone()
    }

    /// java: getRegisteredAmount().
    pub fn get_registered_amount(&self) -> usize {
        self.registered_amount.get()
    }
}

/// java: ContinuityClient.asId(id).
fn as_id(id: &str) -> Identifier {
    Identifier::new("continuity", id).unwrap()
}

/// java: client.processor.ProcessingDataKeys, the full init set from upstream's
/// static block (vertex container and the two overlay emitter pools included).
pub struct ProcessingDataKeys {
    pub mutable_pos: ProcessingDataKey,
    pub biome_cache: ProcessingDataKey,
    pub block_entity_name_cache: ProcessingDataKey,
    pub vertex_container: ProcessingDataKey,
    pub simple_overlay_emitter_pool: ProcessingDataKey,
    pub standard_overlay_emitter_pool: ProcessingDataKey,
}

thread_local! {
    static KEYS: Rc<ProcessingDataKeys> = Rc::new(build_processing_data_keys());
}

/// java: ProcessingDataKeys static finals (thread-local carrying of the upstream statics).
pub fn processing_data_keys() -> Rc<ProcessingDataKeys> {
    KEYS.with(|keys| keys.clone())
}

fn build_processing_data_keys() -> ProcessingDataKeys {
    {
        let registry = processing_data_key_registry();
        // java: create("mutable_pos", BlockPos.Mutable::new).
        let mutable_pos = registry.register_key(
            as_id("mutable_pos"),
            Rc::new(|| Box::new(BlockPos { x: 0, y: 0, z: 0 })),
            None,
        );
        // java: create("biome_cache", BiomeCache::new, BiomeCache::reset).
        let biome_cache = registry.register_key(
            as_id("biome_cache"),
            Rc::new(|| Box::new(BiomeCache::new())),
            Some(Rc::new(|v| v.downcast_mut::<BiomeCache>().unwrap().reset())),
        );
        // java: create("block_entity_name_cache", BlockEntityNameCache::new,
        // BlockEntityNameCache::reset).
        let block_entity_name_cache = registry.register_key(
            as_id("block_entity_name_cache"),
            Rc::new(|| Box::new(BlockEntityNameCache::new())),
            Some(Rc::new(|v| {
                v.downcast_mut::<BlockEntityNameCache>().unwrap().reset();
            })),
        );
        // java: create("vertex_container", CompactCtmQuadProcessor.VertexContainer::new).
        let vertex_container = registry.register_key(
            as_id("vertex_container"),
            Rc::new(|| Box::new(crate::processor_compact::VertexContainer::new())),
            None,
        );
        // java: create("simple_overlay_emitter_pool",
        // SimpleOverlayQuadProcessor.OverlayEmitterPool::new, OverlayEmitterPool::reset).
        let simple_overlay_emitter_pool = registry.register_key(
            as_id("simple_overlay_emitter_pool"),
            Rc::new(|| Box::new(crate::overlay_processors::SimpleOverlayEmitterPool::new())),
            Some(Rc::new(|v| {
                v.downcast_mut::<crate::overlay_processors::SimpleOverlayEmitterPool>()
                    .unwrap()
                    .reset();
            })),
        );
        // java: create("standard_overlay_emitter_pool",
        // StandardOverlayQuadProcessor.OverlayEmitterPool::new, OverlayEmitterPool::reset).
        let standard_overlay_emitter_pool = registry.register_key(
            as_id("standard_overlay_emitter_pool"),
            Rc::new(|| Box::new(crate::overlay_processors::StandardOverlayEmitterPool::new())),
            Some(Rc::new(|v| {
                v.downcast_mut::<crate::overlay_processors::StandardOverlayEmitterPool>()
                    .unwrap()
                    .reset();
            })),
        );
        ProcessingDataKeys {
            mutable_pos,
            biome_cache,
            block_entity_name_cache,
            vertex_container,
            simple_overlay_emitter_pool,
            standard_overlay_emitter_pool,
        }
    }
}

/// java: client.processor.ProcessingPredicate.
pub trait ProcessingPredicate {
    /// java: shouldProcessQuad(quad, sprite, blockView, appearanceState, state, pos, dataProvider).
    fn should_process_quad(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        data_provider: &dyn ProcessingDataProvider,
    ) -> bool;
}

/// java: api.client.QuadProcessor.
pub trait QuadProcessor {
    /// java: processQuad(quad, sprite, blockView, appearanceState, state, pos, randomSupplier, pass, context).
    fn process_quad(
        &self,
        quad: &MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult;
}

/// java: QuadProcessor.ProcessingContext extends ProcessingDataProvider.
pub trait ProcessingContext: ProcessingDataProvider {
    /// java: addEmitterConsumer(Consumer<QuadEmitter>).
    fn add_emitter_consumer(&mut self, consumer: Box<dyn FnOnce(&mut QuadEmitter)>);

    /// java: addMesh(Mesh).
    fn add_mesh(&mut self, mesh: Mesh);

    /// java: getExtraQuadEmitter().
    fn get_extra_quad_emitter(&mut self) -> &mut QuadEmitter;

    /// java: markHasExtraQuads().
    fn mark_has_extra_quads(&mut self);

    /// rust carrying: ProcessingContext extends ProcessingDataProvider.
    fn as_data_provider(&self) -> &dyn ProcessingDataProvider;
}

/// java: AbstractQuadProcessor implements QuadProcessor.
pub trait AbstractQuadProcessor {
    /// java: protected Sprite[] sprites.
    fn sprites(&self) -> &[Option<Sprite>];

    /// java: protected ProcessingPredicate processingPredicate.
    fn processing_predicate(&self) -> &dyn ProcessingPredicate;

    /// java: processQuad(quad, sprite, blockView, appearanceState, state, pos, randomSupplier, pass, context).
    fn abstract_process_quad(
        &self,
        quad: &MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        if !self.processing_predicate().should_process_quad(
            quad,
            sprite,
            block_view,
            appearance_state,
            state,
            pos,
            context.as_data_provider(),
        ) {
            return ProcessingResult::NextProcessor;
        }
        self.process_quad_inner(
            quad,
            sprite,
            block_view,
            appearance_state,
            state,
            pos,
            random_supplier,
            pass,
            context,
        )
    }

    /// java: processQuadInner(quad, sprite, blockView, appearanceState, state, pos, randomSupplier, pass, context).
    fn process_quad_inner(
        &self,
        quad: &MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult;
}

/// java: Function<SpriteIdentifier, Sprite>.
pub type TextureGetter = Rc<dyn Fn(&SpriteId) -> Option<Sprite>>;

/// java: the BaseCtmProperties surface AbstractQuadProcessorFactory touches
/// (getSpriteIds/getMethod/getResourceId/getPackId); BaseCtmProperties implements it.
pub trait AbstractProcessorProps {
    fn base(&self) -> &BaseCtmProperties;
}

impl AbstractProcessorProps for BaseCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        self
    }
}

/// java: AbstractQuadProcessorFactory<T extends BaseCtmProperties> implements
/// QuadProcessor.Factory<T>.
pub trait AbstractQuadProcessorFactory<T: AbstractProcessorProps> {
    /// java: QuadProcessor.Factory<T>::createProcessor(properties, textureGetter).
    fn create_processor_with_textures(
        &self,
        properties: &T,
        texture_getter: &TextureGetter,
    ) -> Box<dyn QuadProcessor> {
        let base = properties.base();
        let texture_amount = self.get_texture_amount(properties);
        let sprite_ids: Vec<SpriteId> = base.sprite_ids.clone().unwrap_or_default();
        let provided = sprite_ids.len();
        let mut max = provided;

        if provided > texture_amount {
            eprintln!(
                "[continuity] Method '{}' requires {} tiles but {} were provided in file '{}' in pack '{}'",
                base.method, texture_amount, provided, base.resource_id, base.pack_id
            );
            max = texture_amount;
        }

        let mut sprites: Vec<Option<Sprite>> = vec![None; texture_amount];
        let missing_sprite = texture_getter(&missing_sprite_id());
        let supports_null_sprites = self.supports_null_sprites(properties);
        for i in 0..max {
            let sprite;
            let sprite_id = &sprite_ids[i];
            if *sprite_id == BaseCtmProperties::special_skip_sprite_id() {
                // java: spriteId.equals(BaseCtmProperties.SPECIAL_SKIP_SPRITE_ID).
                sprite = missing_sprite.clone();
            } else if *sprite_id == BaseCtmProperties::special_default_sprite_id() {
                // java: spriteId.equals(BaseCtmProperties.SPECIAL_DEFAULT_SPRITE_ID),
                // null when null sprites are supported.
                sprite = if supports_null_sprites {
                    None
                } else {
                    missing_sprite.clone()
                };
            } else {
                sprite = texture_getter(sprite_id);
            }
            sprites[i] = sprite;
        }

        if provided < texture_amount {
            eprintln!(
                "[continuity] Method '{}' requires {} tiles but only {} were provided in file '{}' in pack '{}'",
                base.method, texture_amount, provided, base.resource_id, base.pack_id
            );
            for i in provided..texture_amount {
                sprites[i] = missing_sprite.clone();
            }
        }

        self.create_processor(properties, sprites)
    }

    /// java: createProcessor(T properties, Sprite[] sprites).
    fn create_processor(&self, properties: &T, sprites: Vec<Option<Sprite>>) -> Box<dyn QuadProcessor>;

    /// java: getTextureAmount(T properties).
    fn get_texture_amount(&self, properties: &T) -> usize;

    /// java: supportsNullSprites(T properties).
    fn supports_null_sprites(&self, _properties: &T) -> bool {
        true
    }
}

/// java: api.client.CachingPredicates.
pub trait CachingPredicates {
    /// java: affectsSprites().
    fn affects_sprites(&self) -> bool;

    /// java: affectsSprite(Sprite sprite).
    fn affects_sprite(&self, sprite: &Sprite) -> bool;

    /// java: affectsBlockStates().
    fn affects_block_states(&self) -> bool;

    /// java: affectsBlockState(BlockState state).
    fn affects_block_state(&self, state: &BlockState) -> bool;

    /// java: isValidForMultipass().
    fn is_valid_for_multipass(&self) -> bool;
}

/// java: BaseCachingPredicates implements CachingPredicates.
pub struct BaseCachingPredicates {
    /// java: @Nullable Set<Identifier> spriteIdSet.
    pub sprite_id_set: Option<HashSet<Identifier>>,
    /// java: @Nullable Predicate<BlockState> blockStatePredicate, carried as the
    /// compiled parseBlockStates matcher.
    pub block_state_predicate: Option<BlockStateMatcher>,
    pub is_valid_for_multipass: bool,
}

impl CachingPredicates for BaseCachingPredicates {
    fn affects_sprites(&self) -> bool {
        self.sprite_id_set.is_some()
    }

    /// java: spriteIdSet.contains(sprite.getContents().getId()); the sprite handle
    /// carries its texture id, so identity is direct.
    fn affects_sprite(&self, sprite: &Sprite) -> bool {
        match &self.sprite_id_set {
            Some(set) => set.contains(&sprite.id),
            None => false,
        }
    }

    fn affects_block_states(&self) -> bool {
        self.block_state_predicate.is_some()
    }

    /// java: blockStatePredicate.test(state).
    fn affects_block_state(&self, state: &BlockState) -> bool {
        match &self.block_state_predicate {
            Some(predicate) => block_state_matcher_test(predicate, state),
            None => false,
        }
    }

    fn is_valid_for_multipass(&self) -> bool {
        self.is_valid_for_multipass
    }
}

/// java: BaseCachingPredicates.Factory<T> implements CachingPredicates.Factory<T>.
pub struct BaseCachingPredicatesFactory {
    pub is_valid_for_multipass: bool,
}

impl BaseCachingPredicatesFactory {
    /// java: createPredicates(properties, textureGetter) - textureGetter only reaches
    /// sprites the predicates do not need, so it is unused here as upstream.
    pub fn create_predicates<P: AbstractProcessorProps>(&self, properties: &P) -> BaseCachingPredicates {
        let base = properties.base();
        BaseCachingPredicates {
            sprite_id_set: base.match_tiles_set.clone(),
            block_state_predicate: base.match_blocks_predicate.clone(),
            is_valid_for_multipass: self.is_valid_for_multipass,
        }
    }
}

/// java: the parseBlockStates matcher's Predicate<BlockState> test. The full engine
/// surface (block property values on a BlockState) lands with the quad processors'
/// world access; for now the test matches the block's registry key outright or as a
/// propertyMap target, exactly the shapes the parse step stores.
pub fn block_state_matcher_test(matcher: &BlockStateMatcher, state: &BlockState) -> bool {
    let block = block_state_get_block_id(state);
    matcher.blocks.contains(&block) || matcher.property_maps.contains_key(&block)
}

/// java: BaseProcessingPredicate implements ProcessingPredicate.
pub struct BaseProcessingPredicate {
    /// java: @Nullable EnumSet<Direction> faces.
    pub faces: Option<HashSet<Direction>>,
    /// java: @Nullable Predicate<Biome> biomePredicate.
    pub biome_predicate: Option<crate::properties::BiomePredicate>,
    /// java: @Nullable IntPredicate heightPredicate (parseHeights anyOf).
    pub height_predicate: Option<Vec<HeightPredicate>>,
    /// java: @Nullable Predicate<String> blockEntityNamePredicate.
    pub block_entity_name_predicate: Option<NamePredicate>,
}

impl BaseProcessingPredicate {
    /// java: fromProperties(properties).
    pub fn from_properties(properties: &BaseCtmProperties) -> BaseProcessingPredicate {
        BaseProcessingPredicate {
            faces: properties.faces.clone(),
            biome_predicate: properties.biome_predicate.clone(),
            height_predicate: properties.height_predicate.clone(),
            block_entity_name_predicate: properties.block_entity_name_predicate.clone(),
        }
    }
}

impl ProcessingPredicate for BaseProcessingPredicate {
    fn should_process_quad(
        &self,
        quad: &QuadView,
        _sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        _state: &BlockState,
        pos: &BlockPos,
        data_provider: &dyn ProcessingDataProvider,
    ) -> bool {
        if let Some(height_predicate) = &self.height_predicate {
            // java: heightPredicate.test(pos.getY()).
            if !height_predicate.iter().any(|p| p.test(pos.y)) {
                return false;
            }
        }
        if let Some(faces) = &self.faces {
            let mut face = quad.light_face();
            // java: appearanceState.contains(Properties.AXIS) remap.
            if let Some(axis) = block_state_get_axis(appearance_state) {
                if axis == Axis::X {
                    face = face.rotate_clockwise(Axis::Z);
                } else if axis == Axis::Z {
                    face = face.rotate_counterclockwise(Axis::X);
                }
            }
            if !faces.contains(&face) {
                return false;
            }
        }
        if let Some(biome_predicate) = &self.biome_predicate {
            let biome = data_provider
                .get_data(&processing_data_keys().biome_cache)
                .borrow()
                .downcast_ref::<BiomeCache>()
                .unwrap()
                .get(block_view, pos);
            // java: biome == null || !biomePredicate.test(biome).
            if biome.map_or(true, |b| !biome_predicate.test(&b)) {
                return false;
            }
        }
        if let Some(block_entity_name_predicate) = &self.block_entity_name_predicate {
            let name = data_provider
                .get_data(&processing_data_keys().block_entity_name_cache)
                .borrow()
                .downcast_ref::<BlockEntityNameCache>()
                .unwrap()
                .get(block_view, pos);
            // java: blockEntityName == null || !blockEntityNamePredicate.test(blockEntityName).
            if name.map_or(true, |n| !block_entity_name_predicate.test(&n)) {
                return false;
            }
        }
        true
    }
}

/// java: BaseProcessingPredicate.BiomeCache.
pub struct BiomeCache {
    /// java: @Nullable Biome biome.
    biome: RefCell<Option<Biome>>,
    invalid: Cell<bool>,
}

impl BiomeCache {
    pub fn new() -> BiomeCache {
        BiomeCache {
            biome: RefCell::new(None),
            invalid: Cell::new(true),
        }
    }

    /// java: get(blockView, pos).
    pub fn get(&self, block_view: &BlockRenderView, pos: &BlockPos) -> Option<Biome> {
        if self.invalid.get() {
            *self.biome.borrow_mut() = block_view_get_biome(block_view, pos);
            self.invalid.set(false);
        }
        self.biome.borrow().clone()
    }

    /// java: reset().
    pub fn reset(&self) {
        self.invalid.set(true);
    }
}

/// java: BaseProcessingPredicate.BlockEntityNameCache.
pub struct BlockEntityNameCache {
    /// java: @Nullable String blockEntityName.
    block_entity_name: RefCell<Option<String>>,
    invalid: Cell<bool>,
}

impl BlockEntityNameCache {
    pub fn new() -> BlockEntityNameCache {
        BlockEntityNameCache {
            block_entity_name: RefCell::new(None),
            invalid: Cell::new(true),
        }
    }

    /// java: get(blockView, pos).
    pub fn get(&self, block_view: &BlockRenderView, pos: &BlockPos) -> Option<String> {
        if self.invalid.get() {
            *self.block_entity_name.borrow_mut() =
                block_view_get_block_entity_custom_name(block_view, pos);
            self.invalid.set(false);
        }
        self.block_entity_name.borrow().clone()
    }

    /// java: reset().
    pub fn reset(&self) {
        self.invalid.set(true);
    }
}

/// java: DirectionMaps.DIRECTION_MAPS as Direction[6][8][] - each face has eight
/// slots of four directions.
pub type DirectionMapsTable = [[[Direction; 4]; 8]; 6];

static DIRECTION_MAPS: OnceLock<DirectionMapsTable> = OnceLock::new();

/// java: DirectionMaps static init.
pub fn direction_maps() -> &'static DirectionMapsTable {
    DIRECTION_MAPS.get_or_init(|| {
        let mut maps = [[[Direction::Down; 4]; 8]; 6];
        // java: for (Direction face : Direction.values()) - vanilla order
        // DOWN, UP, NORTH, SOUTH, WEST, EAST.
        for face in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            // java: textureUp - UP faces NORTH, DOWN faces SOUTH, everything else faces UP.
            let texture_up = match face {
                Direction::Up => Direction::North,
                Direction::Down => Direction::South,
                _ => Direction::Up,
            };

            // java: textureLeft - negative faces rotate textureUp clockwise, positive
            // counterclockwise.
            let texture_left = if face.axis_direction() == AxisDirection::Negative {
                texture_up.rotate_clockwise(face.axis())
            } else {
                texture_up.rotate_counterclockwise(face.axis())
            };

            let map = &mut maps[face.ordinal()];

            // java: map[0] = { textureLeft, textureUp.getOpposite(), textureLeft.getOpposite(), textureUp } // l d r u
            map[0] = [
                texture_left,
                texture_up.opposite(),
                texture_left.opposite(),
                texture_up,
            ];
            // java: map[1..3] = successive left shifts of map[0] (ArrayUtils.shift(-1)).
            for i in 1..4 {
                map[i] = map[i - 1];
                map[i].rotate_left(1);
            }
            // java: map[4..7] = clones of map[0..3] with entries 0 and 2 swapped.
            for i in 4..8 {
                map[i] = map[i - 4];
                map[i].swap(0, 2);
            }
        }
        maps
    })
}

/// java: DirectionMaps.getMap(direction).
pub fn get_map(direction: Direction) -> &'static [[Direction; 4]; 8] {
    &direction_maps()[direction.ordinal()]
}

/// java: DirectionMaps.getDirections(orientationMode, quad, state).
pub fn get_directions(orientation_mode: OrientationMode, quad: &QuadView, state: &BlockState) -> [Direction; 4] {
    // java: getMap(quad.lightFace())[orientationMode.getOrientation(quad, state)].
    let face = quad.light_face();
    let orientation = orientation_mode
        .get_orientation(face, block_state_get_axis(state).map(|axis| axis.ordinal()));
    direction_maps()[face.ordinal()][orientation as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::Properties;
    use crate::properties::BiomePredicate;
    use crate::seams;

    fn test_sprite(i: &Identifier) -> Sprite {
        Sprite {
            id: i.clone(),
            min_u: 0.0,
            min_v: 0.0,
            max_u: 1.0,
            max_v: 1.0,
            animation_frame_delta: 0.0,
            emissive_sprite: None,
        }
    }

    fn id(s: &str) -> Identifier {
        Identifier::parse(s).unwrap()
    }

    #[test]
    fn direction_rotations_match_java() {
        assert_eq!(Direction::North.rotate_clockwise(Axis::X), Direction::Down);
        assert_eq!(Direction::Down.rotate_clockwise(Axis::X), Direction::South);
        assert_eq!(Direction::South.rotate_clockwise(Axis::X), Direction::Up);
        assert_eq!(Direction::Up.rotate_clockwise(Axis::X), Direction::North);
        assert_eq!(Direction::Down.rotate_clockwise(Axis::Z), Direction::West);
        assert_eq!(Direction::West.rotate_clockwise(Axis::Z), Direction::Up);
        assert_eq!(Direction::Up.rotate_clockwise(Axis::Z), Direction::East);
        assert_eq!(Direction::East.rotate_clockwise(Axis::Z), Direction::Down);
        assert_eq!(Direction::North.rotate_clockwise(Axis::Y), Direction::East);
        assert_eq!(Direction::East.rotate_clockwise(Axis::Y), Direction::South);
        assert_eq!(Direction::South.rotate_clockwise(Axis::Y), Direction::West);
        assert_eq!(Direction::West.rotate_clockwise(Axis::Y), Direction::North);
        assert_eq!(Direction::North.rotate_counterclockwise(Axis::X), Direction::Up);
        assert_eq!(Direction::Up.rotate_counterclockwise(Axis::Z), Direction::West);
        assert_eq!(Direction::North.rotate_counterclockwise(Axis::Y), Direction::West);
        assert_eq!(Direction::North.axis(), Axis::Z);
        assert_eq!(Direction::East.axis(), Axis::X);
        assert_eq!(Direction::Down.axis(), Axis::Y);
    }

    #[test]
    fn direction_maps_up_and_north_rows() {
        let maps = direction_maps();
        // UP face: textureUp NORTH, textureLeft WEST (UP is positive on Y).
        assert_eq!(
            maps[Direction::Up.ordinal()],
            [
                [Direction::West, Direction::South, Direction::East, Direction::North],
                [Direction::South, Direction::East, Direction::North, Direction::West],
                [Direction::East, Direction::North, Direction::West, Direction::South],
                [Direction::North, Direction::West, Direction::South, Direction::East],
                [Direction::East, Direction::South, Direction::West, Direction::North],
                [Direction::North, Direction::East, Direction::South, Direction::West],
                [Direction::West, Direction::North, Direction::East, Direction::South],
                [Direction::South, Direction::West, Direction::North, Direction::East],
            ]
        );
        // NORTH face: textureUp UP, textureLeft EAST (NORTH is negative on Z).
        assert_eq!(
            maps[Direction::North.ordinal()],
            [
                [Direction::East, Direction::Down, Direction::West, Direction::Up],
                [Direction::Down, Direction::West, Direction::Up, Direction::East],
                [Direction::West, Direction::Up, Direction::East, Direction::Down],
                [Direction::Up, Direction::East, Direction::Down, Direction::West],
                [Direction::West, Direction::Down, Direction::East, Direction::Up],
                [Direction::Up, Direction::West, Direction::Down, Direction::East],
                [Direction::East, Direction::Up, Direction::West, Direction::Down],
                [Direction::Down, Direction::East, Direction::Up, Direction::West],
            ]
        );
    }

    #[test]
    fn direction_maps_get_directions_state_axis() {
        let quad = QuadView::bare(Direction::North);
        let state = BlockState { block_id: 0, state_id: 0, sprites: vec![] };
        seams::set_engine_state_axis_hook(Some(Axis::Y));
        // java: AXIS_ORIENTATIONS[1][2] for the Y axis on the NORTH light face.
        let y = get_directions(OrientationMode::StateAxis, &quad, &state);
        seams::set_engine_state_axis_hook(None);
        assert_eq!(y, get_map(Direction::North)[OrientationMode::StateAxis.get_orientation(Direction::North, Some(Axis::Y.ordinal())) as usize]);
    }

    fn base_props(sprite_ids: Option<Vec<SpriteId>>) -> BaseCtmProperties {
        let mut props = BaseCtmProperties::new(
            Properties::default(),
            id("minecraft:test"),
            "test_pack".to_string(),
            0,
            None,
            "ctm".to_string(),
        );
        props.sprite_ids = sprite_ids;
        props
    }

    struct TestFactory {
        amount: usize,
        supports_null: bool,
        captured: RefCell<Vec<Option<Sprite>>>,
    }

    impl AbstractQuadProcessorFactory<BaseCtmProperties> for TestFactory {
        fn create_processor(
            &self,
            _properties: &BaseCtmProperties,
            sprites: Vec<Option<Sprite>>,
        ) -> Box<dyn QuadProcessor> {
            *self.captured.borrow_mut() = sprites;
            Box::new(TestProcessor)
        }

        fn get_texture_amount(&self, _properties: &BaseCtmProperties) -> usize {
            self.amount
        }

        fn supports_null_sprites(&self, _properties: &BaseCtmProperties) -> bool {
            self.supports_null
        }
    }

    struct TestProcessor;

    impl QuadProcessor for TestProcessor {
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
            ProcessingResult::Discard
        }
    }

    #[test]
    fn factory_expands_sprites_like_java() {
        let texture_getter: TextureGetter =
            Rc::new(|id: &SpriteId| Some(test_sprite(id)));
        let missing = missing_sprite_id();

        // java: more sprites provided than the method needs - warn and truncate.
        let factory = TestFactory {
            amount: 2,
            supports_null: true,
            captured: RefCell::new(vec![]),
        };
        factory.create_processor_with_textures(&base_props(Some(vec![id("a"), id("b"), id("c")])), &texture_getter);
        let sprites = factory.captured.borrow();
        assert_eq!(sprites.len(), 2);
        assert_eq!(sprites[0], Some(test_sprite(&id("a"))));
        assert_eq!(sprites[1], Some(test_sprite(&id("b"))));

        // java: fewer provided - error and pad with the missing sprite.
        let factory = TestFactory {
            amount: 3,
            supports_null: true,
            captured: RefCell::new(vec![]),
        };
        factory.create_processor_with_textures(&base_props(Some(vec![id("a")])), &texture_getter);
        let sprites = factory.captured.borrow();
        assert_eq!(sprites.len(), 3);
        assert_eq!(sprites[0], Some(test_sprite(&id("a"))));
        assert_eq!(sprites[1], Some(test_sprite(&missing)));
        assert_eq!(sprites[2], Some(test_sprite(&missing)));

        // java: special/skip resolves to the missing sprite.
        let factory = TestFactory {
            amount: 1,
            supports_null: true,
            captured: RefCell::new(vec![]),
        };
        factory.create_processor_with_textures(&base_props(Some(vec![BaseCtmProperties::special_skip_sprite_id()])), &texture_getter);
        assert_eq!(*factory.captured.borrow(), vec![Some(test_sprite(&missing))]);

        // java: special/default resolves to null when null sprites are supported.
        let factory = TestFactory {
            amount: 1,
            supports_null: true,
            captured: RefCell::new(vec![]),
        };
        factory.create_processor_with_textures(&base_props(Some(vec![BaseCtmProperties::special_default_sprite_id()])), &texture_getter);
        assert_eq!(*factory.captured.borrow(), vec![None]);

        // java: special/default falls back to the missing sprite otherwise.
        let factory = TestFactory {
            amount: 1,
            supports_null: false,
            captured: RefCell::new(vec![]),
        };
        factory.create_processor_with_textures(&base_props(Some(vec![BaseCtmProperties::special_default_sprite_id()])), &texture_getter);
        assert_eq!(*factory.captured.borrow(), vec![Some(test_sprite(&missing))]);
    }

    struct TestContext {
        map: RefCell<HashMap<usize, Rc<RefCell<Box<dyn Any>>>>>,
        supplier_calls: Cell<usize>,
        extra_quads_marked: Cell<bool>,
    }

    impl TestContext {
        fn new() -> TestContext {
            TestContext {
                map: RefCell::new(HashMap::new()),
                supplier_calls: Cell::new(0),
                extra_quads_marked: Cell::new(false),
            }
        }
    }

    impl ProcessingDataProvider for TestContext {
        fn get_data(&self, key: &ProcessingDataKey) -> Rc<RefCell<Box<dyn Any>>> {
            let mut map = self.map.borrow_mut();
            if let Some(value) = map.get(&key.raw_id) {
                return value.clone();
            }
            self.supplier_calls.set(self.supplier_calls.get() + 1);
            let value = Rc::new(RefCell::new((key.value_supplier)()));
            map.insert(key.raw_id, value.clone());
            value
        }
    }

    impl ProcessingContext for TestContext {
        fn add_emitter_consumer(&mut self, _consumer: Box<dyn FnOnce(&mut QuadEmitter)>) {}

        fn add_mesh(&mut self, _mesh: Mesh) {}

        fn get_extra_quad_emitter(&mut self) -> &mut QuadEmitter {
            unreachable!()
        }

        fn mark_has_extra_quads(&mut self) {
            self.extra_quads_marked.set(true);
        }

        fn as_data_provider(&self) -> &dyn ProcessingDataProvider {
            self
        }
    }

    #[test]
    fn processing_data_keys_register_with_continuity_ids() {
        let keys = processing_data_keys();
        assert_eq!(keys.mutable_pos.id, Identifier::new("continuity", "mutable_pos").unwrap());
        assert_eq!(keys.biome_cache.id, Identifier::new("continuity", "biome_cache").unwrap());
        assert!(processing_data_key_registry().get_registered_amount() >= 3);
    }

    #[test]
    fn base_predicate_height_and_faces() {
        let quad = QuadView::bare(Direction::North);
        let state = BlockState { block_id: 0, state_id: 0, sprites: vec![] };
        let pos = BlockPos { x: 0, y: 64, z: 0 };
        let context = TestContext::new();

        let mut predicate = BaseProcessingPredicate {
            faces: None,
            biome_predicate: None,
            height_predicate: Some(vec![HeightPredicate::Exact(64)]),
            block_entity_name_predicate: None,
        };
        assert!(predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));

        predicate.height_predicate = Some(vec![HeightPredicate::Exact(65)]);
        assert!(!predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));

        // java: faces filter with the Properties.AXIS remap.
        let mut faces = HashSet::new();
        faces.insert(Direction::North);
        predicate.faces = Some(faces);
        predicate.height_predicate = None;
        seams::set_engine_state_axis_hook(None);
        assert!(predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
        seams::set_engine_state_axis_hook(Some(Axis::X));
        // java: axis X remaps the UP light face through rotateClockwise(Z) to EAST,
        // which is not in the faces set. (A NORTH light face throws upstream, vanilla
        // rotateClockwise(Z) rejects NORTH/SOUTH.)
        let up_quad = QuadView::bare(Direction::Up);
        assert!(!predicate.should_process_quad(&up_quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
        let down_quad = QuadView::bare(Direction::Down);
        let mut faces2 = HashSet::new();
        faces2.insert(Direction::West);
        predicate.faces = Some(faces2);
        assert!(predicate.should_process_quad(&down_quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
        seams::set_engine_state_axis_hook(Some(Axis::Z));
        let mut faces3 = HashSet::new();
        faces3.insert(Direction::North);
        predicate.faces = Some(faces3);
        // java: axis Z remaps NORTH through rotateCounterclockwise(X) to UP.
        assert!(!predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
        seams::set_engine_state_axis_hook(None);
        assert!(predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
    }

    #[test]
    fn base_predicate_biome_and_block_entity_caches() {
        let quad = QuadView::bare(Direction::Up);
        let state = BlockState { block_id: 0, state_id: 0, sprites: vec![] };
        let pos = BlockPos { x: 1, y: 2, z: 3 };
        seams::set_engine_biome_hook(Some(Biome { id: id("minecraft:plains") }));
        seams::set_engine_block_entity_name_hook(Some("Chest".to_string()));

        let context = TestContext::new();
        let predicate = BaseProcessingPredicate {
            faces: None,
            biome_predicate: Some(BiomePredicate {
                holders: HashSet::from([id("minecraft:plains")]),
                negate: false,
            }),
            height_predicate: None,
            block_entity_name_predicate: Some(crate::properties::NamePredicate::Equals("Chest".to_string())),
        };
        assert!(predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));
        // java: the caches are per-context instances, so each key supplies once.
        assert_eq!(context.supplier_calls.get(), 2);

        // java: a biome outside the set fails the predicate.
        seams::set_engine_biome_hook(Some(Biome { id: id("minecraft:badlands") }));
        let context = TestContext::new();
        assert!(!predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));

        // java: a missing custom name fails the block entity name predicate.
        seams::set_engine_biome_hook(Some(Biome { id: id("minecraft:plains") }));
        seams::set_engine_block_entity_name_hook(None);
        let context = TestContext::new();
        assert!(!predicate.should_process_quad(&quad, &test_sprite(&id("sprite")), &BlockRenderView, &state, &state, &pos, &context));

        seams::set_engine_biome_hook(None);
        seams::set_engine_block_entity_name_hook(None);
    }

    #[test]
    fn cache_reset_refetches_through_the_supplier() {
        let block_view = BlockRenderView;
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let cache = BiomeCache::new();
        seams::set_engine_biome_hook(Some(Biome { id: id("minecraft:plains") }));
        assert_eq!(cache.get(&block_view, &pos).unwrap().id, id("minecraft:plains"));
        cache.reset();
        seams::set_engine_biome_hook(Some(Biome { id: id("minecraft:badlands") }));
        assert_eq!(cache.get(&block_view, &pos).unwrap().id, id("minecraft:badlands"));
        seams::set_engine_biome_hook(None);
    }

    #[test]
    fn base_caching_predicates_matches_java() {
        let state = BlockState { block_id: 0, state_id: 0, sprites: vec![] };
        seams::set_engine_block_id_hook(Some(id("minecraft:stone")));

        let predicates = BaseCachingPredicates {
            sprite_id_set: Some(HashSet::from([id("minecraft:block/a"), id("minecraft:block/b")])),
            block_state_predicate: Some(BlockStateMatcher {
                blocks: HashSet::from([id("minecraft:stone")]),
                property_maps: HashMap::new(),
            }),
            is_valid_for_multipass: true,
        };
        assert!(predicates.affects_sprites());
        assert!(predicates.affects_sprite(&test_sprite(&id("minecraft:block/a"))));
        assert!(!predicates.affects_sprite(&test_sprite(&id("minecraft:block/c"))));
        assert!(predicates.affects_block_states());
        assert!(predicates.affects_block_state(&state));
        assert!(predicates.is_valid_for_multipass());

        // java: null set and null predicate mean no affect at all.
        let empty = BaseCachingPredicates {
            sprite_id_set: None,
            block_state_predicate: None,
            is_valid_for_multipass: false,
        };
        assert!(!empty.affects_sprites());
        assert!(!empty.affects_sprite(&test_sprite(&id("minecraft:block/a"))));
        assert!(!empty.affects_block_states());
        assert!(!empty.affects_block_state(&state));
        assert!(!empty.is_valid_for_multipass());

        seams::set_engine_block_id_hook(None);
    }

    #[test]
    fn factory_defaults_support_null_sprites() {
        struct DefaultFactory;
        impl AbstractQuadProcessorFactory<BaseCtmProperties> for DefaultFactory {
            fn create_processor(
                &self,
                _properties: &BaseCtmProperties,
                _sprites: Vec<Option<Sprite>>,
            ) -> Box<dyn QuadProcessor> {
                Box::new(TestProcessor)
            }

            fn get_texture_amount(&self, _properties: &BaseCtmProperties) -> usize {
                1
            }
        }
        assert!(DefaultFactory.supports_null_sprites(&base_props(None)));
    }
}
