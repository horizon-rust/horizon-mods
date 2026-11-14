//! 1:1 of the Continuity client.model.QuadProcessors, by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity). Upstream file:
//! src/main/java/me/pepperbell/continuity/client/model/QuadProcessors.java.

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::thread_local;

use crate::processor_base::{
    block_state_get_axis, processing_data_keys, AbstractProcessorProps, AbstractQuadProcessor,
    AbstractQuadProcessorFactory, BaseProcessingPredicate, BlockPos, BlockRenderView,
    CachingPredicates, MutableQuadView, ProcessingContext, ProcessingPredicate, ProcessingResult,
    ProcessingDataProvider, QuadProcessor, QuadView, Random,
};
use crate::properties::BaseCtmProperties;
use crate::processor_enums::{Axis, AxisDirection, Direction};
use crate::simple_sprite_provider::{SpriteProvider, SpriteProviderFactory};
use crate::connecting_properties::{
    ConnectionPredicate, ConnectingCtmProperties, OrientedConnectingCtmProperties,
};
use crate::seams::{is_missing_sprite, BlockState, Sprite, SpriteId};

/// java: record ProcessorHolder(QuadProcessor processor, CachingPredicates predicates).
#[derive(Clone)]
pub struct ProcessorHolder {
    pub processor: Rc<dyn QuadProcessor>,
    pub predicates: Rc<dyn CachingPredicates>,
}

/// java: record Slice(QuadProcessor[] processors, QuadProcessor[] multipassProcessors).
#[derive(Clone)]
pub struct Slice {
    pub processors: Vec<Rc<dyn QuadProcessor>>,
    pub multipass_processors: Vec<Rc<dyn QuadProcessor>>,
}

thread_local! {
    /// java: private static ProcessorHolder[] processorHolders.
    static PROCESSOR_HOLDERS: RefCell<Vec<ProcessorHolder>> = RefCell::new(Vec::new());
    /// java: private static final BlockStateKeyCache CACHE.
    static CACHE: BlockStateKeyCache = BlockStateKeyCache::new();
}

/// java: private static Slice computeSlice(BlockState state, Sprite sprite).
fn compute_slice(state: &BlockState, sprite: &Sprite) -> Slice {
    let mut processor_list: Vec<Rc<dyn QuadProcessor>> = Vec::new();
    let mut multipass_processor_list: Vec<Rc<dyn QuadProcessor>> = Vec::new();

    PROCESSOR_HOLDERS.with(|holders| {
        for holder in holders.borrow().iter() {
            let processor = Rc::clone(&holder.processor);
            let predicates = &holder.predicates;
            if !predicates.affects_block_states() || predicates.affects_block_state(state) {
                if predicates.affects_sprites() {
                    if predicates.affects_sprite(sprite) {
                        processor_list.push(Rc::clone(&processor));
                        if predicates.is_valid_for_multipass() {
                            multipass_processor_list.push(processor);
                        }
                    }
                } else {
                    processor_list.push(Rc::clone(&processor));
                }
            }
        }
    });

    Slice {
        processors: processor_list,
        multipass_processors: multipass_processor_list,
    }
}

/// java: public static Function<Sprite, Slice> getCache(BlockState state) =
/// CACHE.apply(state). The returned function is the per-state sprite cache.
pub fn get_cache(state: &BlockState) -> Rc<dyn Fn(&Sprite) -> Slice> {
    CACHE.with(|cache| cache.apply(state))
}

/// java: public static void reload(List<ProcessorHolder> processorHolders).
pub fn reload(processor_holders: Vec<ProcessorHolder>) {
    PROCESSOR_HOLDERS.with(|holders| *holders.borrow_mut() = processor_holders);
    CACHE.with(|cache| cache.clear());
}

/// java: private static class SpriteKeyCache implements Function<Sprite, Slice>.
/// Java keys the map by reference identity; the port keys it by sprite id.
struct SpriteKeyCache {
    map: RefCell<HashMap<SpriteId, Slice>>,
}

impl SpriteKeyCache {
    fn new() -> SpriteKeyCache {
        SpriteKeyCache { map: RefCell::new(HashMap::new()) }
    }

    /// java: apply(Sprite sprite): optimistic-read-then-lock; the port runs the
    /// model layer single-threaded, so the StampedLock collapses to the cache check.
    fn apply(&self, state: &BlockState, sprite: &Sprite) -> Slice {
        if let Some(slice) = self.map.borrow().get(&sprite.id) {
            return slice.clone();
        }
        let mut map = self.map.borrow_mut();
        if let Some(slice) = map.get(&sprite.id) {
            return slice.clone();
        }
        let slice = compute_slice(state, sprite);
        map.insert(sprite.id.clone(), slice.clone());
        slice
    }

    /// java: clear().
    fn clear(&self) {
        self.map.borrow_mut().clear();
    }
}

/// java: private static class BlockStateKeyCache. Java keys the map by reference
/// identity; the port keys it by state id.
struct BlockStateKeyCache {
    map: RefCell<HashMap<u64, Rc<SpriteKeyCache>>>,
}

impl BlockStateKeyCache {
    fn new() -> BlockStateKeyCache {
        BlockStateKeyCache { map: RefCell::new(HashMap::new()) }
    }

    /// java: apply(BlockState state), the same optimistic-read-then-lock shape.
    fn apply(&self, state: &BlockState) -> Rc<dyn Fn(&Sprite) -> Slice> {
        let inner = {
            let mut map = self.map.borrow_mut();
            map.entry(state.state_id)
                .or_insert_with(|| Rc::new(SpriteKeyCache::new()))
                .clone()
        };
        let state = state.clone();
        Rc::new(move |sprite: &Sprite| inner.apply(&state, sprite))
    }

    /// java: clear(): clears each inner sprite cache (the outer map keeps its
    /// entries, matching upstream).
    fn clear(&self) {
        let inners: Vec<Rc<SpriteKeyCache>> = self.map.borrow().values().cloned().collect();
        for inner in inners {
            inner.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor_base::{ProcessingContext, ProcessingResult};
    use crate::processor_enums::Direction;
    use crate::seams::Identifier;

    struct CountingProcessor {
        id: u64,
    }

    impl QuadProcessor for CountingProcessor {
        fn process_quad(
            &self,
            _quad: &crate::processor_base::MutableQuadView,
            _sprite: &Sprite,
            _block_view: &crate::seams::BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &crate::seams::BlockPos,
            _random_supplier: &dyn Fn() -> crate::processor_base::Random,
            _pass: i32,
            _context: &mut dyn ProcessingContext,
        ) -> ProcessingResult {
            ProcessingResult::Stop
        }
    }

    struct AlwaysPredicates {
        affects_states: bool,
        affects_sprites: bool,
        multipass: bool,
    }

    impl CachingPredicates for AlwaysPredicates {
        fn affects_sprites(&self) -> bool {
            self.affects_sprites
        }

        fn affects_sprite(&self, _sprite: &Sprite) -> bool {
            self.affects_sprites
        }

        fn affects_block_states(&self) -> bool {
            self.affects_states
        }

        fn affects_block_state(&self, _state: &BlockState) -> bool {
            self.affects_states
        }

        fn is_valid_for_multipass(&self) -> bool {
            self.multipass
        }
    }

    fn state(id: u64) -> BlockState {
        BlockState { block_id: id, state_id: id, sprites: Vec::new() }
    }

    struct NoopContext;

    impl crate::processor_base::ProcessingDataProvider for NoopContext {
        fn get_data(&self, _key: &crate::processor_base::ProcessingDataKey) -> std::rc::Rc<std::cell::RefCell<Box<dyn std::any::Any>>> {
            unreachable!()
        }
    }

    impl ProcessingContext for NoopContext {
        fn add_emitter_consumer(&mut self, _consumer: Box<dyn FnOnce(&mut crate::processor_base::QuadEmitter)>) {}
        fn add_mesh(&mut self, _mesh: crate::processor_base::Mesh) {}
        fn get_extra_quad_emitter(&mut self) -> &mut crate::processor_base::QuadEmitter {
            unreachable!()
        }
        fn mark_has_extra_quads(&mut self) {}
        fn as_data_provider(&self) -> &dyn crate::processor_base::ProcessingDataProvider {
            self
        }
    }

    fn sprite(id: &str) -> Sprite {
        crate::seams::sprite_for_id(Identifier::new("minecraft", id).unwrap())
    }

    #[test]
    fn slice_selects_by_state_and_sprite_gates() {
        reload(vec![
            ProcessorHolder {
                processor: Rc::new(CountingProcessor { id: 1 }),
                predicates: Rc::new(AlwaysPredicates { affects_states: false, affects_sprites: true, multipass: true }),
            },
            ProcessorHolder {
                processor: Rc::new(CountingProcessor { id: 2 }),
                predicates: Rc::new(AlwaysPredicates { affects_states: true, affects_sprites: false, multipass: false }),
            },
            ProcessorHolder {
                processor: Rc::new(CountingProcessor { id: 3 }),
                predicates: Rc::new(AlwaysPredicates { affects_states: true, affects_sprites: true, multipass: false }),
            },
        ]);
        let cache = get_cache(&state(7));
        // affectsBlockStates=false -> skipped entirely
        let slice = cache(&sprite("stone"));
        // affectsBlockStates=false -> included; affectsSprites=false -> included;
        // all three pass the gates, only the multipass-valid one joins pass 1+.
        assert_eq!(slice.processors.len(), 3);
        assert_eq!(slice.multipass_processors.len(), 1);
    }

    #[test]
    fn cache_is_stable_per_state_and_sprite_and_clears() {
        reload(vec![ProcessorHolder {
            processor: Rc::new(CountingProcessor { id: 1 }),
            predicates: Rc::new(AlwaysPredicates { affects_states: true, affects_sprites: true, multipass: true }),
        }]);
        let cache_a = get_cache(&state(11));
        let cache_b = get_cache(&state(11));
        let slice_a = cache_a(&sprite("oak"));
        let slice_b = cache_b(&sprite("oak"));
        assert_eq!(slice_a.processors.len(), slice_b.processors.len());
        let other_sprite = cache_a(&sprite("spruce"));
        assert_eq!(other_sprite.processors.len(), 1);
        reload(vec![]);
        let cleared = get_cache(&state(11));
        assert_eq!(cleared(&sprite("oak")).processors.len(), 0);
    }

    #[test]
    fn multipass_only_valid_predicates_join() {
        reload(vec![ProcessorHolder {
            processor: Rc::new(CountingProcessor { id: 5 }),
            predicates: Rc::new(AlwaysPredicates { affects_states: true, affects_sprites: true, multipass: true }),
        }]);
        let cache = get_cache(&state(3));
        let slice = cache(&sprite("glass"));
        assert_eq!(slice.processors.len(), 1);
        assert_eq!(slice.multipass_processors.len(), 1);
        assert_eq!(slice.multipass_processors[0].process_quad(
            &crate::processor_base::QuadView::bare(Direction::North),
            &sprite("glass"),
            &crate::seams::BlockRenderView,
            &state(3),
            &state(3),
            &crate::seams::BlockPos { x: 0, y: 0, z: 0 },
            &|| crate::processor_base::Random { seed: 0 },
            1,
            &mut NoopContext,
        ), ProcessingResult::Stop);
    }
}

/// java: client.processor.simple.SimpleQuadProcessor implements QuadProcessor.
pub struct SimpleQuadProcessor {
    sprite_provider: Box<dyn SpriteProvider>,
    processing_predicate: Rc<dyn ProcessingPredicate>,
}

impl SimpleQuadProcessor {
    /// java: SimpleQuadProcessor(SpriteProvider spriteProvider, ProcessingPredicate processingPredicate).
    pub fn new(
        sprite_provider: Box<dyn SpriteProvider>,
        processing_predicate: Rc<dyn ProcessingPredicate>,
    ) -> SimpleQuadProcessor {
        SimpleQuadProcessor { sprite_provider, processing_predicate }
    }

    /// java: public static ProcessingResult process(MutableQuadView quad, Sprite oldSprite,
    /// @Nullable Sprite newSprite).
    pub fn process(quad: &QuadView, old_sprite: &Sprite, new_sprite: Option<Sprite>) -> ProcessingResult {
        let new_sprite = match new_sprite {
            Some(new_sprite) => new_sprite,
            None => return ProcessingResult::Stop,
        };
        if crate::seams::is_missing_sprite(&new_sprite) {
            return ProcessingResult::NextProcessor;
        }
        crate::quad_util::interpolate(quad, old_sprite, &new_sprite);
        ProcessingResult::NextPass
    }
}

impl QuadProcessor for SimpleQuadProcessor {
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
        _pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        if !self.processing_predicate.should_process_quad(
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
        let new_sprite = self.sprite_provider.get_sprite(
            quad,
            sprite,
            block_view,
            appearance_state,
            state,
            pos,
            random_supplier,
            context.as_data_provider(),
        );
        Self::process(quad, sprite, new_sprite)
    }
}

/// java: SimpleQuadProcessor.Factory<T extends BaseCtmProperties> extends
/// AbstractQuadProcessorFactory<T>, generic over the sprite provider factory's
/// properties type.
pub struct SimpleQuadProcessorFactory<T, F: SpriteProviderFactory<T>> {
    /// java: protected SpriteProvider.Factory<? super T> spriteProviderFactory.
    pub sprite_provider_factory: F,
    _phantom: PhantomData<T>,
}

impl<T, F: SpriteProviderFactory<T>> SimpleQuadProcessorFactory<T, F> {
    /// java: Factory(SpriteProvider.Factory<? super T> spriteProviderFactory).
    pub fn new(sprite_provider_factory: F) -> SimpleQuadProcessorFactory<T, F> {
        SimpleQuadProcessorFactory { sprite_provider_factory, _phantom: PhantomData }
    }
}

impl<T, F> AbstractQuadProcessorFactory<T> for SimpleQuadProcessorFactory<T, F>
where
    T: AbstractProcessorProps,
    F: SpriteProviderFactory<T>,
{
    /// java: createProcessor(T properties, Sprite[] sprites).
    fn create_processor(
        &self,
        properties: &T,
        sprites: Vec<Option<Sprite>>,
    ) -> Box<dyn QuadProcessor> {
        Box::new(SimpleQuadProcessor::new(
            self.sprite_provider_factory.create_sprite_provider(sprites, properties),
            Rc::new(BaseProcessingPredicate::from_properties(properties.base())),
        ))
    }

    /// java: getTextureAmount(T properties).
    fn get_texture_amount(&self, properties: &T) -> usize {
        self.sprite_provider_factory.get_texture_amount(properties)
    }
}

/// java: net.minecraft.util.math.Direction.from(Direction.Axis axis,
/// Direction.AxisDirection direction).
pub fn direction_from_axis_direction(axis: Axis, axis_direction: AxisDirection) -> Direction {
    match (axis, axis_direction) {
        (Axis::X, AxisDirection::Positive) => Direction::East,
        (Axis::X, AxisDirection::Negative) => Direction::West,
        (Axis::Y, AxisDirection::Positive) => Direction::Up,
        (Axis::Y, AxisDirection::Negative) => Direction::Down,
        (Axis::Z, AxisDirection::Positive) => Direction::South,
        (Axis::Z, AxisDirection::Negative) => Direction::North,
    }
}

/// java: client.processor.TopQuadProcessor extends AbstractQuadProcessor.
pub struct TopQuadProcessor {
    sprites: Vec<Option<Sprite>>,
    processing_predicate: Rc<dyn ProcessingPredicate>,
    connection_predicate: Rc<dyn ConnectionPredicate>,
    inner_seams: bool,
}

impl TopQuadProcessor {
    /// java: TopQuadProcessor(Sprite[] sprites, ProcessingPredicate processingPredicate,
    /// ConnectionPredicate connectionPredicate, boolean innerSeams).
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        processing_predicate: Rc<dyn ProcessingPredicate>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
    ) -> TopQuadProcessor {
        TopQuadProcessor { sprites, processing_predicate, connection_predicate, inner_seams }
    }
}

impl AbstractQuadProcessor for TopQuadProcessor {
    /// java: protected Sprite[] sprites.
    fn sprites(&self) -> &[Option<Sprite>] {
        &self.sprites
    }

    /// java: protected ProcessingPredicate processingPredicate.
    fn processing_predicate(&self) -> &dyn ProcessingPredicate {
        self.processing_predicate.as_ref()
    }

    /// java: processQuadInner(quad, sprite, blockView, appearanceState, state, pos,
    /// randomSupplier, pass, context).
    fn process_quad_inner(
        &self,
        quad: &MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        _pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        let light_face = quad.light_face();
        // java: appearanceState.contains(Properties.AXIS) ? appearanceState.get(Properties.AXIS)
        // : Direction.Axis.Y.
        let axis = crate::processor_base::block_state_get_axis(appearance_state).unwrap_or(Axis::Y);
        if light_face.axis() != axis {
            let up = direction_from_axis_direction(axis, AxisDirection::Positive);
            let keys = crate::processor_base::processing_data_keys();
            let mutable_pos = context.as_data_provider().get_data(&keys.mutable_pos);
            let connects = {
                let mut guard = mutable_pos.borrow_mut();
                let mutable_pos = guard.downcast_mut::<BlockPos>().unwrap();
                // java: context.getData(ProcessingDataKeys.MUTABLE_POS).set(pos, up);
                mutable_pos.set(pos, up);
                // java: connectionPredicate.shouldConnect(blockView, appearanceState, state,
                // pos, mutablePos, lightFace, sprite, innerSeams).
                self.connection_predicate.should_connect(
                    block_view,
                    appearance_state,
                    state,
                    pos,
                    mutable_pos,
                    light_face,
                    sprite,
                )
            };
            if connects {
                // java: return SimpleQuadProcessor.process(quad, sprite, sprites[0]);
                return SimpleQuadProcessor::process(quad, sprite, self.sprites[0].clone());
            }
        }
        ProcessingResult::NextProcessor
    }
}

impl QuadProcessor for TopQuadProcessor {
    /// java: the AbstractQuadProcessor.processQuad dispatch.
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
    ) -> ProcessingResult {
        Self::abstract_process_quad(
            self, quad, sprite, block_view, appearance_state, state, pos, random_supplier, pass,
            context,
        )
    }
}

/// java: TopQuadProcessor.Factory extends AbstractQuadProcessorFactory
/// <ConnectingCtmProperties>.
pub struct TopQuadProcessorFactory;

impl AbstractQuadProcessorFactory<ConnectingCtmProperties> for TopQuadProcessorFactory {
    /// java: createProcessor(properties, sprites).
    fn create_processor(
        &self,
        properties: &ConnectingCtmProperties,
        sprites: Vec<Option<Sprite>>,
    ) -> Box<dyn QuadProcessor> {
        Box::new(TopQuadProcessor::new(
            sprites,
            Rc::new(BaseProcessingPredicate::from_properties(&properties.base.base)),
            Rc::new(
                properties
                    .base
                    .get_connection_predicate()
                    .expect("connection predicate missing"),
            ),
            properties.get_inner_seams(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &ConnectingCtmProperties) -> usize {
        1
    }
}

impl AbstractProcessorProps for ConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

impl AbstractProcessorProps for OrientedConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base
    }
}

#[cfg(test)]
mod simple_processor_tests {
    use super::*;
    use std::any::Any;
    use crate::parsing::Properties;
    use crate::processor_base::ProcessingDataKey;
    use crate::processor_enums::Axis;
    use crate::seams::Identifier;

    fn sprite(id: &str) -> Sprite {
        Sprite {
            id: Identifier::new("continuity", id).unwrap(),
            min_u: 0.0,
            min_v: 0.0,
            max_u: 1.0,
            max_v: 1.0,
            animation_frame_delta: 0.0,
            emissive_sprite: None,
        }
    }

    fn missing_sprite() -> Sprite {
        Sprite {
            id: crate::seams::missing_sprite_id(),
            min_u: 0.0,
            min_v: 0.0,
            max_u: 1.0,
            max_v: 1.0,
            animation_frame_delta: 0.0,
            emissive_sprite: None,
        }
    }

    fn state() -> BlockState {
        BlockState { block_id: 0, state_id: 0, sprites: vec![] }
    }

    struct TestContext {
        map: RefCell<HashMap<usize, Rc<RefCell<Box<dyn Any>>>>>,
    }

    impl TestContext {
        fn new() -> TestContext {
            let _ = processing_data_keys();
            TestContext { map: RefCell::new(HashMap::new()) }
        }
    }

    impl ProcessingDataProvider for TestContext {
        fn get_data(&self, key: &ProcessingDataKey) -> Rc<RefCell<Box<dyn Any>>> {
            if let Some(value) = self.map.borrow().get(&key.raw_id) {
                return value.clone();
            }
            let value = Rc::new(RefCell::new((key.value_supplier)()));
            self.map.borrow_mut().insert(key.raw_id, value.clone());
            value
        }
    }

    impl ProcessingContext for TestContext {
        fn add_emitter_consumer(&mut self, _consumer: Box<dyn FnOnce(&mut crate::processor_base::QuadEmitter)>) {}
        fn add_mesh(&mut self, _mesh: crate::processor_base::Mesh) {}
        fn get_extra_quad_emitter(&mut self) -> &mut crate::processor_base::QuadEmitter {
            unreachable!()
        }
        fn mark_has_extra_quads(&mut self) {}
        fn as_data_provider(&self) -> &dyn ProcessingDataProvider {
            self
        }
    }

    struct FixedPredicate(bool);

    impl ProcessingPredicate for FixedPredicate {
        fn should_process_quad(
            &self,
            _quad: &QuadView,
            _sprite: &Sprite,
            _block_view: &BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &BlockPos,
            _data_provider: &dyn ProcessingDataProvider,
        ) -> bool {
            self.0
        }
    }

    struct FixedConnect(bool);

    impl ConnectionPredicate for FixedConnect {
        fn should_connect_full(
            &self,
            _block_view: &BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &BlockPos,
            _other_appearance_state: &BlockState,
            _other_state: &BlockState,
            _other_pos: &BlockPos,
            _face: Direction,
            _quad_sprite: &Sprite,
        ) -> bool {
            self.0
        }
    }

    /// java: a SpriteProvider stub answering getSprite with a constant sprite.
    struct ConstSpriteProvider(Option<Sprite>);

    impl SpriteProvider for ConstSpriteProvider {
        fn get_sprite(
            &self,
            _quad: &QuadView,
            _sprite: &Sprite,
            _block_view: &BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &BlockPos,
            _random_supplier: &dyn Fn() -> Random,
            _data_provider: &dyn ProcessingDataProvider,
        ) -> Option<Sprite> {
            self.0.clone()
        }
    }

    #[test]
    fn process_null_new_sprite_returns_stop() {
        let quad = QuadView::bare(Direction::North);
        assert_eq!(SimpleQuadProcessor::process(&quad, &sprite("old"), None), ProcessingResult::Stop);
    }

    #[test]
    fn process_missing_new_sprite_returns_next_processor() {
        let quad = QuadView::bare(Direction::North);
        assert_eq!(
            SimpleQuadProcessor::process(&quad, &sprite("old"), Some(missing_sprite())),
            ProcessingResult::NextProcessor
        );
    }

    #[test]
    fn process_normal_new_sprite_interpolates_and_next_pass() {
        // java: the real path carries a MutableQuadView with vertex data, which
        // interpolate writes UVs into.
        let quad = QuadView::with_data(Direction::North, crate::processor_base::QuadVertexData::default());
        assert_eq!(
            SimpleQuadProcessor::process(&quad, &sprite("old"), Some(sprite("new"))),
            ProcessingResult::NextPass
        );
    }

    #[test]
    fn simple_processor_predicate_false_returns_next_processor() {
        let processor = SimpleQuadProcessor::new(
            Box::new(ConstSpriteProvider(Some(sprite("new")))),
            Rc::new(FixedPredicate(false)),
        );
        let quad = QuadView::bare(Direction::North);
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        assert_eq!(
            processor.process_quad(&quad, &sprite("old"), &BlockRenderView, &st, &st, &pos, &|| Random { seed: 0 }, 0, &mut TestContext::new()),
            ProcessingResult::NextProcessor
        );
    }

    #[test]
    fn simple_processor_end_to_end_next_pass() {
        let processor = SimpleQuadProcessor::new(
            Box::new(ConstSpriteProvider(Some(sprite("new")))),
            Rc::new(FixedPredicate(true)),
        );
        let quad = QuadView::with_data(Direction::North, crate::processor_base::QuadVertexData::default());
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        assert_eq!(
            processor.process_quad(&quad, &sprite("old"), &BlockRenderView, &st, &st, &pos, &|| Random { seed: 0 }, 0, &mut TestContext::new()),
            ProcessingResult::NextPass
        );
    }

    #[test]
    fn top_connects_on_light_face_off_axis() {
        // lightFace North (axis Z) vs axis Y -> connect -> sprites[0] is a normal sprite -> NextPass.
        let processor = TopQuadProcessor::new(
            vec![Some(sprite("top"))],
            Rc::new(FixedPredicate(true)),
            Rc::new(FixedConnect(true)),
            false,
        );
        let quad = QuadView::with_data(Direction::North, crate::processor_base::QuadVertexData::default());
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        assert_eq!(
            processor.process_quad(&quad, &sprite("old"), &BlockRenderView, &st, &st, &pos, &|| Random { seed: 0 }, 0, &mut TestContext::new()),
            ProcessingResult::NextPass
        );
    }

    #[test]
    fn top_skips_when_light_face_matches_axis() {
        // lightFace Up (axis Y) matches the default Y axis -> no connection check -> NextProcessor.
        let processor = TopQuadProcessor::new(
            vec![Some(sprite("top"))],
            Rc::new(FixedPredicate(true)),
            Rc::new(FixedConnect(true)),
            false,
        );
        let quad = QuadView::bare(Direction::Up);
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        assert_eq!(
            processor.process_quad(&quad, &sprite("old"), &BlockRenderView, &st, &st, &pos, &|| Random { seed: 0 }, 0, &mut TestContext::new()),
            ProcessingResult::NextProcessor
        );
    }

    #[test]
    fn top_skips_when_predicate_declines() {
        let processor = TopQuadProcessor::new(
            vec![Some(sprite("top"))],
            Rc::new(FixedPredicate(true)),
            Rc::new(FixedConnect(false)),
            false,
        );
        let quad = QuadView::bare(Direction::North);
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        assert_eq!(
            processor.process_quad(&quad, &sprite("old"), &BlockRenderView, &st, &st, &pos, &|| Random { seed: 0 }, 0, &mut TestContext::new()),
            ProcessingResult::NextProcessor
        );
    }

    #[test]
    fn top_factory_texture_amount_is_1() {
        let properties = ConnectingCtmProperties::new(
            Properties::new(),
            Identifier::new("minecraft", "optifine/ctm/top.properties").unwrap(),
            "pack".into(),
            0,
            None,
            "top".into(),
        );
        assert_eq!(TopQuadProcessorFactory.get_texture_amount(&properties), 1);
    }

    #[test]
    fn direction_from_axis_direction_maps_all_six() {
        assert_eq!(direction_from_axis_direction(Axis::X, AxisDirection::Positive), Direction::East);
        assert_eq!(direction_from_axis_direction(Axis::X, AxisDirection::Negative), Direction::West);
        assert_eq!(direction_from_axis_direction(Axis::Y, AxisDirection::Positive), Direction::Up);
        assert_eq!(direction_from_axis_direction(Axis::Y, AxisDirection::Negative), Direction::Down);
        assert_eq!(direction_from_axis_direction(Axis::Z, AxisDirection::Positive), Direction::South);
        assert_eq!(direction_from_axis_direction(Axis::Z, AxisDirection::Negative), Direction::North);
    }
}
