//! 1:1 of the Continuity client.processor.overlay package, by PepperCode1
//! (LGPL-3.0-only upstream; the exact 1:1 port carries the upstream license and credit).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::connecting_properties::{ConnectionPredicate, ConnectionType};
use crate::overlay_properties::{
    OverlayPropertiesSectionProvider, OverlayPropertiesSection, StandardOverlayCtmProperties,
};
use crate::parsing::BlockStateMatcher;
use crate::processor_base::{
    direction_maps, processing_data_keys, AbstractProcessorProps, AbstractQuadProcessorFactory,
    BaseProcessingPredicate, ProcessingContext, ProcessingDataProvider, ProcessingPredicate,
    ProcessingResult, QuadEmitter, QuadProcessor, Random, RenderMaterial, BlendMode,
};
use crate::processor_enums::Direction;
use crate::properties::BaseCtmProperties;
use crate::quad_util::{emit_overlay_quad, is_quad_unit_square};
use crate::render_api::{
    render_util_find_overlay_material, render_util_get_tint_color,
};
use crate::seams::{
    block_state_get_appearance, block_state_is_full_cube, block_state_is_opaque_full_cube,
    block_view_get_block_state, sprite_calculator_get_sprites, is_missing_sprite, BlockPos,
    BlockRenderView, BlockState, Identifier, Sprite, SpriteId,
};
use crate::simple_sprite_provider::{SpriteProvider, SpriteProviderFactory};

/// java: client.processor.overlay.OverlayProcessingPredicate extends
/// BaseProcessingPredicate.
pub struct OverlayProcessingPredicate {
    base: BaseProcessingPredicate,
}

impl OverlayProcessingPredicate {
    /// java: constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        faces: Option<HashSet<Direction>>,
        biome_predicate: Option<crate::properties::BiomePredicate>,
        height_predicate: Option<Vec<crate::properties::HeightPredicate>>,
        block_entity_name_predicate: Option<crate::properties::NamePredicate>,
    ) -> OverlayProcessingPredicate {
        OverlayProcessingPredicate {
            base: BaseProcessingPredicate {
                faces,
                biome_predicate,
                height_predicate,
                block_entity_name_predicate,
            },
        }
    }

    /// java: fromProperties(BaseCtmProperties properties).
    pub fn from_properties(properties: &BaseCtmProperties) -> OverlayProcessingPredicate {
        OverlayProcessingPredicate {
            base: BaseProcessingPredicate::from_properties(properties),
        }
    }
}

impl ProcessingPredicate for OverlayProcessingPredicate {
    fn should_process_quad(
        &self,
        quad: &crate::processor_base::QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        data_provider: &dyn ProcessingDataProvider,
    ) -> bool {
        if !self
            .base
            .should_process_quad(quad, sprite, block_view, appearance_state, state, pos, data_provider)
        {
            return false;
        }
        // java: return QuadUtil.isQuadUnitSquare(quad).
        is_quad_unit_square(quad)
    }
}

/// java: client.processor.overlay.SimpleOverlayQuadProcessor extends SimpleQuadProcessor.
/// The superclass's spriteProvider and processingPredicate fields are carried as
/// plain fields since the port flattens the inheritance.
pub struct SimpleOverlayQuadProcessor {
    /// java: protected SpriteProvider spriteProvider (superclass).
    pub sprite_provider: Box<dyn SpriteProvider>,
    /// java: protected ProcessingPredicate processingPredicate (superclass).
    pub processing_predicate: Rc<dyn ProcessingPredicate>,
    /// java: protected int tintIndex.
    pub tint_index: i32,
    /// java: @Nullable protected BlockState tintBlock.
    pub tint_block: Option<BlockState>,
    /// java: protected RenderMaterial material = RenderUtil.findOverlayMaterial(layer, tintBlock).
    pub material: RenderMaterial,
}

impl SimpleOverlayQuadProcessor {
    /// java: SimpleOverlayQuadProcessor(SpriteProvider spriteProvider, ProcessingPredicate
    /// processingPredicate, int tintIndex, @Nullable BlockState tintBlock, BlendMode layer).
    pub fn new(
        sprite_provider: Box<dyn SpriteProvider>,
        processing_predicate: Rc<dyn ProcessingPredicate>,
        tint_index: i32,
        tint_block: Option<BlockState>,
        layer: BlendMode,
    ) -> SimpleOverlayQuadProcessor {
        let material = render_util_find_overlay_material(layer, tint_block.as_ref());
        SimpleOverlayQuadProcessor {
            sprite_provider,
            processing_predicate,
            tint_index,
            tint_block,
            material,
        }
    }
}

impl QuadProcessor for SimpleOverlayQuadProcessor {
    /// java: processQuad(...) (overrides the superclass and always returns NEXT_PROCESSOR).
    fn process_quad(
        &self,
        quad: &crate::processor_base::MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        _pass: i32,
        context: &mut dyn ProcessingContext,
    ) -> ProcessingResult {
        if self.processing_predicate.should_process_quad(
            quad,
            sprite,
            block_view,
            appearance_state,
            state,
            pos,
            context.as_data_provider(),
        ) {
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
            if let Some(new_sprite) = new_sprite {
                if !is_missing_sprite(&new_sprite) {
                    let keys = processing_data_keys();
                    // java: OverlayEmitter emitter = context.getData(
                    // ProcessingDataKeys.SIMPLE_OVERLAY_EMITTER_POOL).get();
                    let pool = context.get_data(&keys.simple_overlay_emitter_pool);
                    let emitter;
                    {
                        let mut pool_borrow = pool.borrow_mut();
                        emitter = pool_borrow
                            .as_mut()
                            .downcast_mut::<SimpleOverlayEmitterPool>()
                            .unwrap()
                            .get();
                    }
                    emitter.borrow_mut().prepare(
                        quad.light_face(),
                        new_sprite,
                        render_util_get_tint_color(self.tint_block.as_ref(), block_view, pos, self.tint_index),
                        &self.material,
                    );
                    let emitter_for_consumer = Rc::clone(&emitter);
                    context.add_emitter_consumer(Box::new(move |quad_emitter| {
                        emitter_for_consumer.borrow_mut().accept(quad_emitter);
                    }));
                }
            }
        }
        ProcessingResult::NextProcessor
    }
}

/// java: SimpleOverlayQuadProcessor.OverlayEmitter implements Consumer<QuadEmitter>.
pub struct SimpleOverlayEmitter {
    pub face: Option<Direction>,
    pub sprite: Option<Sprite>,
    pub color: i32,
    pub material: Option<RenderMaterial>,
}

impl SimpleOverlayEmitter {
    /// java: OverlayEmitter().
    pub fn new() -> SimpleOverlayEmitter {
        SimpleOverlayEmitter {
            face: None,
            sprite: None,
            color: -1,
            material: None,
        }
    }

    /// java: accept(QuadEmitter emitter).
    pub fn accept(&self, emitter: &mut QuadEmitter) {
        emit_overlay_quad(
            emitter,
            self.face.unwrap(),
            self.sprite.as_ref().unwrap(),
            self.color,
            self.material.as_ref().unwrap(),
        );
    }

    /// java: prepare(Direction face, Sprite sprite, int color, RenderMaterial material).
    pub fn prepare(&mut self, face: Direction, sprite: Sprite, color: i32, material: &RenderMaterial) {
        self.face = Some(face);
        self.sprite = Some(sprite);
        self.color = color;
        self.material = Some(material.clone());
    }
}

/// java: SimpleOverlayQuadProcessor.OverlayEmitterPool.
pub struct SimpleOverlayEmitterPool {
    /// java: protected final List<OverlayEmitter> list.
    pub list: Vec<Rc<RefCell<SimpleOverlayEmitter>>>,
    /// java: protected int nextIndex = 0.
    pub next_index: usize,
}

impl SimpleOverlayEmitterPool {
    /// java: OverlayEmitterPool().
    pub fn new() -> SimpleOverlayEmitterPool {
        SimpleOverlayEmitterPool {
            list: Vec::new(),
            next_index: 0,
        }
    }

    /// java: OverlayEmitter get().
    pub fn get(&mut self) -> Rc<RefCell<SimpleOverlayEmitter>> {
        if self.next_index >= self.list.len() {
            self.list.push(Rc::new(RefCell::new(SimpleOverlayEmitter::new())));
        }
        let emitter = Rc::clone(&self.list[self.next_index]);
        self.next_index += 1;
        emitter
    }

    /// java: reset().
    pub fn reset(&mut self) {
        self.next_index = 0;
    }
}

/// java: SimpleOverlayQuadProcessor.Factory<T extends BaseCtmProperties &
/// OverlayPropertiesSection.Provider> extends SimpleQuadProcessor.Factory<T>.
pub struct SimpleOverlayQuadProcessorFactory<T, F: SpriteProviderFactory<T>> {
    /// java: super(spriteProviderFactory).
    pub sprite_provider_factory: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F: SpriteProviderFactory<T>> SimpleOverlayQuadProcessorFactory<T, F> {
    /// java: Factory(SpriteProviderFactory<T> spriteProviderFactory).
    pub fn new(sprite_provider_factory: F) -> Self {
        Self {
            sprite_provider_factory,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> AbstractQuadProcessorFactory<T> for SimpleOverlayQuadProcessorFactory<T, F>
where
    T: AbstractProcessorProps + OverlayPropertiesSectionProvider,
    F: SpriteProviderFactory<T>,
{
    /// java: createProcessor(T properties, Sprite[] sprites).
    fn create_processor(&self, properties: &T, sprites: Vec<Option<Sprite>>) -> Box<dyn QuadProcessor> {
        // java: OverlayPropertiesSection overlaySection = properties.getOverlayPropertiesSection();
        let overlay_section = properties.get_overlay_properties_section();
        Box::new(SimpleOverlayQuadProcessor::new(
            self.sprite_provider_factory.create_sprite_provider(sprites, properties),
            Rc::new(OverlayProcessingPredicate::from_properties(properties.base())),
            overlay_section.get_tint_index(),
            overlay_section.get_tint_block().cloned(),
            overlay_section.get_layer(),
        ))
    }

    /// java: getTextureAmount inherited from SimpleQuadProcessor.Factory (it delegates
    /// to the sprite provider factory).
    fn get_texture_amount(&self, properties: &T) -> usize {
        self.sprite_provider_factory.get_texture_amount(properties)
    }

    /// java: supportsNullSprites(T properties).
    fn supports_null_sprites(&self, _properties: &T) -> bool {
        false
    }
}

/// java: client.processor.overlay.StandardOverlayQuadProcessor extends AbstractQuadProcessor.
pub struct StandardOverlayQuadProcessor {
    /// java: @Nullable Set<Identifier> matchTilesSet (superclass field).
    pub match_tiles_set: Option<HashSet<Identifier>>,
    /// java: @Nullable Predicate<BlockState> matchBlocksPredicate (superclass field).
    pub match_blocks_predicate: Option<BlockStateMatcher>,
    /// java: @Nullable Set<Identifier> connectTilesSet.
    pub connect_tiles_set: Option<HashSet<Identifier>>,
    /// java: @Nullable Predicate<BlockState> connectBlocksPredicate.
    pub connect_blocks_predicate: Option<BlockStateMatcher>,
    /// java: protected ConnectionPredicate connectionPredicate.
    pub connection_predicate: Rc<dyn ConnectionPredicate>,

    /// java: protected int tintIndex.
    pub tint_index: i32,
    /// java: @Nullable protected BlockState tintBlock.
    pub tint_block: Option<BlockState>,
    /// java: protected RenderMaterial material = RenderUtil.findOverlayMaterial(layer, tintBlock).
    pub material: RenderMaterial,

    /// java: protected Sprite[] sprites (AbstractQuadProcessor's field).
    pub sprites: Vec<Option<Sprite>>,
    /// java: protected ProcessingPredicate processingPredicate (AbstractQuadProcessor's field).
    pub processing_predicate: Rc<dyn ProcessingPredicate>,
}

impl StandardOverlayQuadProcessor {
    /// java: StandardOverlayQuadProcessor(Sprite[] sprites, ProcessingPredicate
    /// processingPredicate, Set<Identifier> matchTilesSet, Predicate<BlockState>
    /// matchBlocksPredicate, Set<Identifier> connectTilesSet, Predicate<BlockState>
    /// connectBlocksPredicate, ConnectionPredicate connectionPredicate, int tintIndex,
    /// @Nullable BlockState tintBlock, BlendMode layer).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mut sprites: Vec<Option<Sprite>>,
        processing_predicate: Rc<dyn ProcessingPredicate>,
        match_tiles_set: Option<HashSet<Identifier>>,
        match_blocks_predicate: Option<BlockStateMatcher>,
        connect_tiles_set: Option<HashSet<Identifier>>,
        connect_blocks_predicate: Option<BlockStateMatcher>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        tint_index: i32,
        tint_block: Option<BlockState>,
        layer: BlendMode,
    ) -> StandardOverlayQuadProcessor {
        let material = render_util_find_overlay_material(layer, tint_block.as_ref());

        // java: Turn all missing sprites into null, since it is more efficient to check for a
        // null sprite than a missing sprite. There is no functional difference between missing
        // and null sprites for this processor.
        for sprite in sprites.iter_mut() {
            if sprite.as_ref().is_some_and(|s| is_missing_sprite(s)) {
                *sprite = None;
            }
        }

        StandardOverlayQuadProcessor {
            sprites,
            processing_predicate,
            match_tiles_set,
            match_blocks_predicate,
            connect_tiles_set,
            connect_blocks_predicate,
            connection_predicate,
            tint_index,
            tint_block,
            material,
        }
    }

    /// java: processQuadInner(...).
    #[allow(clippy::too_many_arguments)]
    fn process_quad_inner(
        &self,
        quad: &crate::processor_base::MutableQuadView,
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
        let emitter = self.get_emitter(
            block_view,
            appearance_state,
            state,
            pos,
            light_face,
            sprite,
            &direction_maps()[light_face.ordinal()][0],
            context.as_data_provider(),
        );
        if let Some(emitter) = emitter {
            let emitter_for_consumer = Rc::clone(&emitter);
            context.add_emitter_consumer(Box::new(move |quad_emitter| {
                emitter_for_consumer.borrow_mut().accept(quad_emitter);
            }));
        }
        ProcessingResult::NextProcessor
    }

    /// java: static boolean matchesAny(Set<Identifier> tiles, Set<Sprite> sprites).
    fn matches_any(tiles: &HashSet<Identifier>, sprites: &[SpriteId]) -> bool {
        sprites.iter().any(|sprite| tiles.contains(sprite))
    }

    /// java: appliesOverlay(BlockState otherAppearanceState, BlockState otherState,
    /// BlockPos otherPos, BlockRenderView blockView, BlockState appearanceState,
    /// BlockState state, BlockPos pos, Direction face, Sprite quadSprite).
    #[allow(clippy::too_many_arguments)]
    fn applies_overlay(
        &self,
        other_appearance_state: &BlockState,
        other_state: &BlockState,
        other_pos: &BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> bool {
        // OptiFine never applies overlays from blocks with dynamic bounds. To improve mod
        // compatibility, call isFullCube with the correct values and do not check for dynamic
        // bounds explicitly. For vanilla blocks, this change only makes it so retracted pistons
        // and shulker boxes can apply overlays.
        if !block_state_is_full_cube(other_state, block_view, other_pos) {
            return false;
        }

        if let Some(connect_blocks_predicate) = &self.connect_blocks_predicate {
            if !connect_blocks_predicate.test(other_appearance_state) {
                return false;
            }
        }

        if let Some(connect_tiles_set) = &self.connect_tiles_set {
            if !Self::matches_any(connect_tiles_set, &sprite_calculator_get_sprites(other_appearance_state, face)) {
                return false;
            }
        }

        !self.connection_predicate.should_connect(
            block_view,
            appearance_state,
            state,
            pos,
            other_pos,
            face,
            quad_sprite,
        )
    }

    /// java: hasSameOverlay(@Nullable BlockState otherAppearanceState, Direction face).
    fn has_same_overlay(&self, other_appearance_state: Option<&BlockState>, face: Direction) -> bool {
        let Some(other_appearance_state) = other_appearance_state else {
            return false;
        };

        if let Some(match_blocks_predicate) = &self.match_blocks_predicate {
            if !match_blocks_predicate.test(other_appearance_state) {
                return false;
            }
        }

        if let Some(match_tiles_set) = &self.match_tiles_set {
            if !Self::matches_any(match_tiles_set, &sprite_calculator_get_sprites(other_appearance_state, face)) {
                return false;
            }
        }

        true
    }

    /// java: appliesOverlayCorner(Direction dir0, Direction dir1, BlockPos.Mutable mutablePos,
    /// BlockRenderView blockView, BlockState appearanceState, BlockState state, BlockPos pos,
    /// Direction lightFace, Sprite quadSprite).
    #[allow(clippy::too_many_arguments)]
    fn applies_overlay_corner(
        &self,
        dir0: Direction,
        dir1: Direction,
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        light_face: Direction,
        quad_sprite: &Sprite,
    ) -> bool {
        mutable_pos.set(pos, dir0);
        mutable_pos.move_direction(dir1);
        let other_state = block_view_get_block_state(block_view, mutable_pos);
        let other_appearance_state =
            block_state_get_appearance(&other_state, block_view, mutable_pos, light_face, state, pos);

        if self.applies_overlay(
            &other_appearance_state,
            &other_state,
            mutable_pos,
            block_view,
            appearance_state,
            state,
            pos,
            light_face,
            quad_sprite,
        ) {
            mutable_pos.move_direction(light_face);
            return !block_state_is_opaque_full_cube(
                &block_view_get_block_state(block_view, mutable_pos),
                block_view,
                mutable_pos,
            );
        }

        false
    }

    /// java: fromTwoSidesAdj(OverlayEmitter emitter, @Nullable BlockState appearanceState0,
    /// @Nullable BlockState appearanceState1, Direction dir0, Direction dir1, int sprite,
    /// int spriteC01, BlockPos.Mutable mutablePos, BlockRenderView blockView, BlockState
    /// appearanceState, BlockState state, BlockPos pos, Direction lightFace, Sprite quadSprite).
    #[allow(clippy::too_many_arguments)]
    fn from_two_sides_adj(
        &self,
        emitter: Rc<RefCell<StandardOverlayEmitter>>,
        appearance_state0: Option<BlockState>,
        appearance_state1: Option<BlockState>,
        dir0: Direction,
        dir1: Direction,
        sprite: usize,
        sprite_c01: usize,
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        light_face: Direction,
        quad_sprite: &Sprite,
    ) -> Rc<RefCell<StandardOverlayEmitter>> {
        self.prepare_emitter(&emitter, light_face, block_view, pos);
        emitter.borrow_mut().add_sprite(self.sprites[sprite].as_ref());

        // OptiFine does not check whether the other two adjacent blocks have the same overlay
        // before trying to apply the corner overlay. I consider this a bug since it is
        // inconsistent with other cases, so it is fixed here by checking those blocks.
        if (self.has_same_overlay(appearance_state0.as_ref(), light_face)
            || self.has_same_overlay(appearance_state1.as_ref(), light_face))
            && self.applies_overlay_corner(
                dir0,
                dir1,
                mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )
        {
            emitter.borrow_mut().add_sprite(self.sprites[sprite_c01].as_ref());
        }

        emitter
    }

    /// java: fromOneSide(OverlayEmitter emitter, @Nullable BlockState appearanceState0,
    /// @Nullable BlockState appearanceState1, @Nullable BlockState appearanceState2,
    /// Direction dir0, Direction dir1, Direction dir2, int sprite, int spriteC01, int
    /// spriteC12, BlockPos.Mutable mutablePos, BlockRenderView blockView, BlockState
    /// appearanceState, BlockState state, BlockPos pos, Direction lightFace, Sprite quadSprite).
    #[allow(clippy::too_many_arguments)]
    fn from_one_side(
        &self,
        emitter: Rc<RefCell<StandardOverlayEmitter>>,
        appearance_state0: Option<BlockState>,
        appearance_state1: Option<BlockState>,
        appearance_state2: Option<BlockState>,
        dir0: Direction,
        dir1: Direction,
        dir2: Direction,
        sprite: usize,
        sprite_c01: usize,
        sprite_c12: usize,
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        light_face: Direction,
        quad_sprite: &Sprite,
    ) -> Rc<RefCell<StandardOverlayEmitter>> {
        let c01;
        let c12;
        if self.has_same_overlay(appearance_state1.as_ref(), light_face) {
            c01 = true;
            c12 = true;
        } else {
            c01 = self.has_same_overlay(appearance_state0.as_ref(), light_face);
            c12 = self.has_same_overlay(appearance_state2.as_ref(), light_face);
        }

        self.prepare_emitter(&emitter, light_face, block_view, pos);
        emitter.borrow_mut().add_sprite(self.sprites[sprite].as_ref());
        if c01
            && self.applies_overlay_corner(
                dir0,
                dir1,
                mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )
        {
            emitter.borrow_mut().add_sprite(self.sprites[sprite_c01].as_ref());
        }
        if c12
            && self.applies_overlay_corner(
                dir1,
                dir2,
                mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )
        {
            emitter.borrow_mut().add_sprite(self.sprites[sprite_c12].as_ref());
        }

        emitter
    }

    /// java: static OverlayEmitter getEmitter(ProcessingDataProvider dataProvider).
    fn get_pooled_emitter(data_provider: &dyn ProcessingDataProvider) -> Rc<RefCell<StandardOverlayEmitter>> {
        let keys = processing_data_keys();
        let pool = data_provider.get_data(&keys.standard_overlay_emitter_pool);
        let emitter;
        {
            let mut pool_borrow = pool.borrow_mut();
            emitter = pool_borrow
                .as_mut()
                .downcast_mut::<StandardOverlayEmitterPool>()
                .unwrap()
                .get();
        }
        emitter
    }

    /// java: prepareEmitter(OverlayEmitter emitter, Direction face, BlockRenderView
    /// blockView, BlockPos pos).
    fn prepare_emitter(
        &self,
        emitter: &Rc<RefCell<StandardOverlayEmitter>>,
        face: Direction,
        block_view: &BlockRenderView,
        pos: &BlockPos,
    ) {
        emitter.borrow_mut().prepare(
            face,
            render_util_get_tint_color(self.tint_block.as_ref(), block_view, pos, self.tint_index),
            &self.material,
        );
    }

    /// java: prepareEmitter(emitter, face, blockView, pos, int sprite0).
    #[allow(clippy::too_many_arguments)]
    fn prepare_emitter_1(
        &self,
        emitter: Rc<RefCell<StandardOverlayEmitter>>,
        face: Direction,
        block_view: &BlockRenderView,
        pos: &BlockPos,
        sprite0: usize,
    ) -> Rc<RefCell<StandardOverlayEmitter>> {
        self.prepare_emitter(&emitter, face, block_view, pos);
        emitter.borrow_mut().add_sprite(self.sprites[sprite0].as_ref());
        emitter
    }

    /// java: prepareEmitter(emitter, face, blockView, pos, int sprite0, int sprite1).
    #[allow(clippy::too_many_arguments)]
    fn prepare_emitter_2(
        &self,
        emitter: Rc<RefCell<StandardOverlayEmitter>>,
        face: Direction,
        block_view: &BlockRenderView,
        pos: &BlockPos,
        sprite0: usize,
        sprite1: usize,
    ) -> Rc<RefCell<StandardOverlayEmitter>> {
        self.prepare_emitter(&emitter, face, block_view, pos);
        emitter.borrow_mut().add_sprite(self.sprites[sprite0].as_ref());
        emitter.borrow_mut().add_sprite(self.sprites[sprite1].as_ref());
        emitter
    }

    /// java: getEmitter(BlockRenderView blockView, BlockState appearanceState, BlockState
    /// state, BlockPos pos, Direction lightFace, Sprite quadSprite, Direction[] directions,
    /// ProcessingDataProvider dataProvider).
    #[allow(clippy::too_many_arguments)]
    fn get_emitter(
        &self,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        light_face: Direction,
        quad_sprite: &Sprite,
        directions: &[Direction; 4],
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Rc<RefCell<StandardOverlayEmitter>>> {
        let keys = processing_data_keys();
        // java: BlockPos.Mutable mutablePos = dataProvider.getData(ProcessingDataKeys.MUTABLE_POS).
        // rust carrying: the shared Mutable is carried as a local copy; every set/read below
        // mirrors the upstream sequence.
        let mut mutable_pos = *data_provider
            .get_data(&keys.mutable_pos)
            .borrow()
            .downcast_ref::<BlockPos>()
            .unwrap();

        // [up] | [right] | [down] | [left]
        //     8
        // 1   *   4
        //     2
        let mut applications = 0u32;

        // OptiFine reuses the same mutable position throughout the whole method and often
        // references appearance states whose positions have long been overwritten. This is
        // bad as it relies on undefined behavior, so the port only keeps the state values.

        mutable_pos.set(pos, directions[0]);
        mutable_pos.move_direction(light_face);
        let appearance_state0: Option<BlockState>;
        if !block_state_is_opaque_full_cube(
            &block_view_get_block_state(block_view, &mutable_pos),
            block_view,
            &mutable_pos,
        ) {
            mutable_pos.set(pos, directions[0]);
            let state0 = block_view_get_block_state(block_view, &mutable_pos);
            let state0_appearance =
                block_state_get_appearance(&state0, block_view, &mutable_pos, light_face, state, pos);
            if self.applies_overlay(
                &state0_appearance,
                &state0,
                &mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            ) {
                applications |= 0b0001;
            }
            appearance_state0 = Some(state0_appearance);
        } else {
            appearance_state0 = None;
        }

        mutable_pos.set(pos, directions[1]);
        mutable_pos.move_direction(light_face);
        let appearance_state1: Option<BlockState>;
        if !block_state_is_opaque_full_cube(
            &block_view_get_block_state(block_view, &mutable_pos),
            block_view,
            &mutable_pos,
        ) {
            mutable_pos.set(pos, directions[1]);
            let state1 = block_view_get_block_state(block_view, &mutable_pos);
            let state1_appearance =
                block_state_get_appearance(&state1, block_view, &mutable_pos, light_face, state, pos);
            if self.applies_overlay(
                &state1_appearance,
                &state1,
                &mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            ) {
                applications |= 0b0010;
            }
            appearance_state1 = Some(state1_appearance);
        } else {
            appearance_state1 = None;
        }

        mutable_pos.set(pos, directions[2]);
        mutable_pos.move_direction(light_face);
        let appearance_state2: Option<BlockState>;
        if !block_state_is_opaque_full_cube(
            &block_view_get_block_state(block_view, &mutable_pos),
            block_view,
            &mutable_pos,
        ) {
            mutable_pos.set(pos, directions[2]);
            let state2 = block_view_get_block_state(block_view, &mutable_pos);
            let state2_appearance =
                block_state_get_appearance(&state2, block_view, &mutable_pos, light_face, state, pos);
            if self.applies_overlay(
                &state2_appearance,
                &state2,
                &mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            ) {
                applications |= 0b0100;
            }
            appearance_state2 = Some(state2_appearance);
        } else {
            appearance_state2 = None;
        }

        mutable_pos.set(pos, directions[3]);
        mutable_pos.move_direction(light_face);
        let appearance_state3: Option<BlockState>;
        if !block_state_is_opaque_full_cube(
            &block_view_get_block_state(block_view, &mutable_pos),
            block_view,
            &mutable_pos,
        ) {
            mutable_pos.set(pos, directions[3]);
            let state3 = block_view_get_block_state(block_view, &mutable_pos);
            let state3_appearance =
                block_state_get_appearance(&state3, block_view, &mutable_pos, light_face, state, pos);
            if self.applies_overlay(
                &state3_appearance,
                &state3,
                &mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            ) {
                applications |= 0b1000;
            }
            appearance_state3 = Some(state3_appearance);
        } else {
            appearance_state3 = None;
        }

        // Match the applications with the lookup table above.
        match applications {
            0b1111 => Some(self.prepare_emitter_1(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                8,
            )),
            0b0111 => Some(self.prepare_emitter_1(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                5,
            )),
            0b1011 => Some(self.prepare_emitter_1(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                6,
            )),
            0b1101 => Some(self.prepare_emitter_1(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                13,
            )),
            0b1110 => Some(self.prepare_emitter_1(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                12,
            )),
            //
            0b0101 => Some(self.prepare_emitter_2(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                9,
                7,
            )),
            0b1010 => Some(self.prepare_emitter_2(
                Self::get_pooled_emitter(data_provider),
                light_face,
                block_view,
                pos,
                1,
                15,
            )),
            //
            0b0011 => Some(self.from_two_sides_adj(
                Self::get_pooled_emitter(data_provider),
                appearance_state2.clone(),
                appearance_state3.clone(),
                directions[2],
                directions[3],
                4,
                14,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b0110 => Some(self.from_two_sides_adj(
                Self::get_pooled_emitter(data_provider),
                appearance_state3.clone(),
                appearance_state0.clone(),
                directions[3],
                directions[0],
                3,
                16,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b1100 => Some(self.from_two_sides_adj(
                Self::get_pooled_emitter(data_provider),
                appearance_state0.clone(),
                appearance_state1.clone(),
                directions[0],
                directions[1],
                10,
                2,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b1001 => Some(self.from_two_sides_adj(
                Self::get_pooled_emitter(data_provider),
                appearance_state1.clone(),
                appearance_state2.clone(),
                directions[1],
                directions[2],
                11,
                0,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            //
            0b0001 => Some(self.from_one_side(
                Self::get_pooled_emitter(data_provider),
                appearance_state1.clone(),
                appearance_state2.clone(),
                appearance_state3.clone(),
                directions[1],
                directions[2],
                directions[3],
                9,
                0,
                14,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b0010 => Some(self.from_one_side(
                Self::get_pooled_emitter(data_provider),
                appearance_state2.clone(),
                appearance_state3.clone(),
                appearance_state0.clone(),
                directions[2],
                directions[3],
                directions[0],
                1,
                14,
                16,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b0100 => Some(self.from_one_side(
                Self::get_pooled_emitter(data_provider),
                appearance_state3.clone(),
                appearance_state0.clone(),
                appearance_state1.clone(),
                directions[3],
                directions[0],
                directions[1],
                7,
                16,
                2,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            0b1000 => Some(self.from_one_side(
                Self::get_pooled_emitter(data_provider),
                appearance_state0.clone(),
                appearance_state1.clone(),
                appearance_state2.clone(),
                directions[0],
                directions[1],
                directions[2],
                15,
                2,
                0,
                &mut mutable_pos,
                block_view,
                appearance_state,
                state,
                pos,
                light_face,
                quad_sprite,
            )),
            //
            0b0000 => {
                // OptiFine never applies overlays from blocks with dynamic bounds. To improve
                // mod compatibility, call isFullCube with the correct values and do not check
                // for dynamic bounds explicitly. For vanilla blocks, this change only makes it
                // so retracted pistons and shulker boxes can apply overlays.

                let s0 = self.has_same_overlay(appearance_state0.as_ref(), light_face);
                let s1 = self.has_same_overlay(appearance_state1.as_ref(), light_face);
                let s2 = self.has_same_overlay(appearance_state2.as_ref(), light_face);
                let s3 = self.has_same_overlay(appearance_state3.as_ref(), light_face);

                // OptiFine does not check whether the other two adjacent blocks have the same
                // overlay before trying to apply the corner overlay. I consider this a bug
                // since it is inconsistent with other cases, so it is fixed here by checking
                // those blocks.

                let c01 = (s0 || s1)
                    && self.applies_overlay_corner(
                        directions[0],
                        directions[1],
                        &mut mutable_pos,
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        light_face,
                        quad_sprite,
                    );
                let c12 = (s1 || s2)
                    && self.applies_overlay_corner(
                        directions[1],
                        directions[2],
                        &mut mutable_pos,
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        light_face,
                        quad_sprite,
                    );
                let c23 = (s2 || s3)
                    && self.applies_overlay_corner(
                        directions[2],
                        directions[3],
                        &mut mutable_pos,
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        light_face,
                        quad_sprite,
                    );
                let c30 = (s3 || s0)
                    && self.applies_overlay_corner(
                        directions[3],
                        directions[0],
                        &mut mutable_pos,
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        light_face,
                        quad_sprite,
                    );

                if c01 || c12 || c23 || c30 {
                    let emitter = Self::get_pooled_emitter(data_provider);
                    self.prepare_emitter(&emitter, light_face, block_view, pos);
                    if c01 {
                        emitter.borrow_mut().add_sprite(self.sprites[2].as_ref());
                    }
                    if c12 {
                        emitter.borrow_mut().add_sprite(self.sprites[0].as_ref());
                    }
                    if c23 {
                        emitter.borrow_mut().add_sprite(self.sprites[14].as_ref());
                    }
                    if c30 {
                        emitter.borrow_mut().add_sprite(self.sprites[16].as_ref());
                    }
                    return Some(emitter);
                }

                None
            }
            //
            _ => panic!("Unexpected value: {}", applications),
        }
    }
}

impl QuadProcessor for StandardOverlayQuadProcessor {
    /// java: AbstractQuadProcessor.processQuad gates on the predicate and then calls
    /// processQuadInner.
    fn process_quad(
        &self,
        quad: &crate::processor_base::MutableQuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        pass: i32,
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
        self.process_quad_inner(quad, sprite, block_view, appearance_state, state, pos, random_supplier, pass, context)
    }
}

/// java: StandardOverlayQuadProcessor.OverlayEmitter implements Consumer<QuadEmitter>.
pub struct StandardOverlayEmitter {
    /// java: protected Sprite[] sprites = new Sprite[4].
    pub sprites: [Option<Sprite>; 4],
    /// java: protected int spriteAmount.
    pub sprite_amount: usize,
    pub face: Direction,
    pub color: i32,
    pub material: Option<RenderMaterial>,
}

impl StandardOverlayEmitter {
    /// java: OverlayEmitter().
    pub fn new() -> StandardOverlayEmitter {
        StandardOverlayEmitter {
            sprites: [None, None, None, None],
            sprite_amount: 0,
            face: Direction::North,
            color: -1,
            material: None,
        }
    }

    /// java: accept(QuadEmitter emitter).
    pub fn accept(&self, emitter: &mut QuadEmitter) {
        for i in 0..self.sprite_amount {
            emit_overlay_quad(
                emitter,
                self.face,
                self.sprites[i].as_ref().unwrap(),
                self.color,
                self.material.as_ref().unwrap(),
            );
        }
    }

    /// java: prepare(Direction face, int color, RenderMaterial material).
    pub fn prepare(&mut self, face: Direction, color: i32, material: &RenderMaterial) {
        // java: System.arraycopy(EMPTY_SPRITES, 0, sprites, 0, EMPTY_SPRITES.length).
        self.sprites = [None, None, None, None];
        self.sprite_amount = 0;
        self.face = face;
        self.color = color;
        self.material = Some(material.clone());
    }

    /// java: addSprite(@Nullable Sprite sprite).
    pub fn add_sprite(&mut self, sprite: Option<&Sprite>) {
        if let Some(sprite) = sprite {
            self.sprites[self.sprite_amount] = Some(sprite.clone());
            self.sprite_amount += 1;
        }
    }
}

/// java: StandardOverlayQuadProcessor.OverlayEmitterPool.
pub struct StandardOverlayEmitterPool {
    /// java: protected final List<OverlayEmitter> list.
    pub list: Vec<Rc<RefCell<StandardOverlayEmitter>>>,
    /// java: protected int nextIndex = 0.
    pub next_index: usize,
}

impl StandardOverlayEmitterPool {
    /// java: OverlayEmitterPool().
    pub fn new() -> StandardOverlayEmitterPool {
        StandardOverlayEmitterPool {
            list: Vec::new(),
            next_index: 0,
        }
    }

    /// java: OverlayEmitter get().
    pub fn get(&mut self) -> Rc<RefCell<StandardOverlayEmitter>> {
        if self.next_index >= self.list.len() {
            self.list.push(Rc::new(RefCell::new(StandardOverlayEmitter::new())));
        }
        let emitter = Rc::clone(&self.list[self.next_index]);
        self.next_index += 1;
        emitter
    }

    /// java: reset().
    pub fn reset(&mut self) {
        self.next_index = 0;
    }
}

/// java: StandardOverlayQuadProcessor.Factory extends
/// AbstractQuadProcessorFactory<StandardOverlayCtmProperties>.
pub struct StandardOverlayQuadProcessorFactory;

impl AbstractQuadProcessorFactory<StandardOverlayCtmProperties> for StandardOverlayQuadProcessorFactory {
    /// java: createProcessor(StandardOverlayCtmProperties properties, Sprite[] sprites).
    fn create_processor(
        &self,
        properties: &StandardOverlayCtmProperties,
        sprites: Vec<Option<Sprite>>,
    ) -> Box<dyn QuadProcessor> {
        // java: OverlayPropertiesSection overlaySection = properties.getOverlayPropertiesSection().
        let overlay_section = properties.get_overlay_properties_section();
        Box::new(StandardOverlayQuadProcessor::new(
            sprites,
            Rc::new(OverlayProcessingPredicate::from_properties(properties.base())),
            properties.base.base.match_tiles_set.clone(),
            properties.base.base.match_blocks_predicate.clone(),
            properties.connect_tiles_set.clone(),
            properties.connect_blocks_predicate.clone(),
            // java: the inherited connectionPredicate (ConnectionType implements
            // ConnectionPredicate); the port carries it as an Option, and valid
            // properties always have it set, so Block is the null-carry fallback.
            Rc::new(
                properties
                    .base
                    .connection_predicate
                    .clone()
                    .unwrap_or(ConnectionType::Block),
            ),
            overlay_section.get_tint_index(),
            overlay_section.get_tint_block().cloned(),
            overlay_section.get_layer(),
        ))
    }

    /// java: getTextureAmount(StandardOverlayCtmProperties properties).
    fn get_texture_amount(&self, _properties: &StandardOverlayCtmProperties) -> usize {
        17
    }

    /// java: supportsNullSprites(StandardOverlayCtmProperties properties).
    fn supports_null_sprites(&self, _properties: &StandardOverlayCtmProperties) -> bool {
        false
    }
}

#[cfg(test)]
mod overlay_processor_tests {
    use super::*;
    use std::any::Any;
    use std::collections::HashMap;
    use crate::parsing::Properties;
    use crate::processor_base::{
        ProcessingDataKey, QuadView, QuadVertexData,
    };

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
        fn add_emitter_consumer(&mut self, _consumer: Box<dyn FnOnce(&mut QuadEmitter)>) {}
        fn add_mesh(&mut self, _mesh: crate::processor_base::Mesh) {}
        fn get_extra_quad_emitter(&mut self) -> &mut QuadEmitter {
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

    /// java: a ConnectionPredicate stub that treats adjacent neighbours (the side walk)
    /// as connected and diagonal neighbours (the corner walk) as not connected.
    struct AdjacentConnect {
        base: BlockPos,
    }

    impl ConnectionPredicate for AdjacentConnect {
        fn should_connect_full(
            &self,
            _block_view: &BlockRenderView,
            _appearance_state: &BlockState,
            _state: &BlockState,
            _pos: &BlockPos,
            _other_appearance_state: &BlockState,
            _other_state: &BlockState,
            other_pos: &BlockPos,
            _face: Direction,
            _quad_sprite: &Sprite,
        ) -> bool {
            let dx = (other_pos.x - self.base.x).abs();
            let dy = (other_pos.y - self.base.y).abs();
            let dz = (other_pos.z - self.base.z).abs();
            dx + dy + dz == 1
        }
    }

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

    fn random_supplier() -> Random {
        Random { seed: 0 }
    }

    #[test]
    fn overlay_predicate_requires_unit_square() {
        // java: OverlayProcessingPredicate.fromProperties(properties) then
        // shouldProcessQuad -> QuadUtil.isQuadUnitSquare(quad).
        let properties = BaseCtmProperties::new(
            Properties::new(),
            Identifier::new("continuity", "props").unwrap(),
            "pack".to_string(),
            0,
            None,
            "standard_overlay".to_string(),
        );
        let predicate = OverlayProcessingPredicate::from_properties(&properties);
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::North, QuadVertexData::default());
        assert!(predicate.should_process_quad(
            &quad,
            &sprite("old"),
            &view,
            &state(),
            &state(),
            &BlockPos { x: 0, y: 0, z: 0 },
            &TestContext::new()
        ));

        // java: a quad that is not unit square (a vertex off the unit square) fails.
        let mut data = QuadVertexData::default();
        data.x[0] = 0.5;
        let quad = QuadView::with_data(Direction::North, data);
        assert!(!predicate.should_process_quad(
            &quad,
            &sprite("old"),
            &view,
            &state(),
            &state(),
            &BlockPos { x: 0, y: 0, z: 0 },
            &TestContext::new()
        ));
    }

    #[test]
    fn overlay_predicate_faces_gate() {
        // java: the BaseProcessingPredicate superclass gate runs before the
        // unit-square check.
        let mut faces = HashSet::new();
        faces.insert(Direction::North);
        let predicate = OverlayProcessingPredicate::new(Some(faces), None, None, None);
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::East, QuadVertexData::default());
        assert!(!predicate.should_process_quad(
            &quad,
            &sprite("old"),
            &view,
            &state(),
            &state(),
            &BlockPos { x: 0, y: 0, z: 0 },
            &TestContext::new()
        ));
    }

    #[test]
    fn simple_overlay_emits_into_pooled_emitter_and_next_processor() {
        let processor = SimpleOverlayQuadProcessor::new(
            Box::new(ConstSpriteProvider(Some(sprite("ov")))),
            Rc::new(FixedPredicate(true)),
            0,
            None,
            BlendMode::CutoutMipped,
        );
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::North, QuadVertexData::default());
        let mut context = TestContext::new();
        assert_eq!(
            processor.process_quad(
                &quad,
                &sprite("old"),
                &view,
                &state(),
                &state(),
                &BlockPos { x: 0, y: 0, z: 0 },
                &random_supplier,
                0,
                &mut context
            ),
            ProcessingResult::NextProcessor
        );

        let keys = processing_data_keys();
        {
            let pool = context.get_data(&keys.simple_overlay_emitter_pool);
            let pool = pool.borrow();
            let pool = pool.downcast_ref::<SimpleOverlayEmitterPool>().unwrap();
            assert_eq!(pool.next_index, 1);
            let emitter = &*pool.list[0].borrow();
            assert_eq!(emitter.face, Some(Direction::North));
            assert_eq!(emitter.sprite.as_ref().unwrap().id, sprite("ov").id);
            assert_eq!(emitter.color, -1);
            assert_eq!(emitter.material.as_ref().unwrap().blend_mode, BlendMode::CutoutMipped);
        }

        // java: the second processed quad takes the next pool slot.
        assert_eq!(
            processor.process_quad(
                &quad,
                &sprite("old"),
                &view,
                &state(),
                &state(),
                &BlockPos { x: 0, y: 0, z: 0 },
                &random_supplier,
                0,
                &mut context
            ),
            ProcessingResult::NextProcessor
        );
        {
            let pool = context.get_data(&keys.simple_overlay_emitter_pool);
            let pool = pool.borrow();
            assert_eq!(pool.downcast_ref::<SimpleOverlayEmitterPool>().unwrap().next_index, 2);
        }
    }

    #[test]
    fn simple_overlay_null_and_missing_sprites_emit_nothing() {
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::North, QuadVertexData::default());
        let mut context = TestContext::new();
        for new_sprite in [None, Some(missing_sprite())] {
            let processor = SimpleOverlayQuadProcessor::new(
                Box::new(ConstSpriteProvider(new_sprite)),
                Rc::new(FixedPredicate(true)),
                0,
                None,
                BlendMode::Cutout,
            );
            assert_eq!(
                processor.process_quad(
                    &quad,
                    &sprite("old"),
                    &view,
                    &state(),
                    &state(),
                    &BlockPos { x: 0, y: 0, z: 0 },
                    &random_supplier,
                    0,
                    &mut context
                ),
                ProcessingResult::NextProcessor
            );
        }
        let keys = processing_data_keys();
        let pool = context.get_data(&keys.simple_overlay_emitter_pool);
        let pool = pool.borrow();
        assert_eq!(pool.downcast_ref::<SimpleOverlayEmitterPool>().unwrap().next_index, 0);
    }

    #[test]
    fn simple_overlay_failed_predicate_emits_nothing() {
        let processor = SimpleOverlayQuadProcessor::new(
            Box::new(ConstSpriteProvider(Some(sprite("ov")))),
            Rc::new(FixedPredicate(false)),
            0,
            None,
            BlendMode::CutoutMipped,
        );
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::North, QuadVertexData::default());
        let mut context = TestContext::new();
        assert_eq!(
            processor.process_quad(
                &quad,
                &sprite("old"),
                &view,
                &state(),
                &state(),
                &BlockPos { x: 0, y: 0, z: 0 },
                &random_supplier,
                0,
                &mut context
            ),
            ProcessingResult::NextProcessor
        );
        let keys = processing_data_keys();
        let pool = context.get_data(&keys.simple_overlay_emitter_pool);
        let pool = pool.borrow();
        assert_eq!(pool.downcast_ref::<SimpleOverlayEmitterPool>().unwrap().next_index, 0);
    }

    #[test]
    fn emitter_pool_get_and_reset() {
        // java: get() hands out each emitter in order and wraps after reset().
        let mut pool = SimpleOverlayEmitterPool::new();
        let first = pool.get();
        let second = pool.get();
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(pool.next_index, 2);
        pool.reset();
        let again = pool.get();
        assert!(Rc::ptr_eq(&first, &again));
    }

    fn standard_processor(connection_predicate: Rc<dyn ConnectionPredicate>) -> StandardOverlayQuadProcessor {
        let sprites: Vec<Option<Sprite>> = (0..17).map(|i| Some(sprite(&format!("s{}", i)))).collect();
        StandardOverlayQuadProcessor::new(
            sprites,
            Rc::new(FixedPredicate(true)),
            None,
            None,
            None,
            None,
            connection_predicate,
            0,
            None,
            BlendMode::CutoutMipped,
        )
    }

    fn standard_process(
        processor: &StandardOverlayQuadProcessor,
    ) -> Rc<RefCell<Box<dyn Any>>> {
        let view = BlockRenderView;
        let quad = QuadView::with_data(Direction::North, QuadVertexData::default());
        let mut context = TestContext::new();
        assert_eq!(
            processor.process_quad(
                &quad,
                &sprite("old"),
                &view,
                &state(),
                &state(),
                &BlockPos { x: 0, y: 0, z: 0 },
                &random_supplier,
                0,
                &mut context
            ),
            ProcessingResult::NextProcessor
        );
        let keys = processing_data_keys();
        context.get_data(&keys.standard_overlay_emitter_pool)
    }

    #[test]
    fn standard_overlay_all_four_sides_use_sprite_8() {
        // java: 0b1111 -> prepareEmitter(getEmitter(dataProvider), lightFace, blockView,
        // pos, 8).
        let processor = standard_processor(Rc::new(FixedConnect(false)));
        let pool = standard_process(&processor);
        let pool = pool.borrow();
        let pool = pool.downcast_ref::<StandardOverlayEmitterPool>().unwrap();
        assert_eq!(pool.next_index, 1);
        let emitter = &*pool.list[0].borrow();
        assert_eq!(emitter.sprite_amount, 1);
        assert_eq!(emitter.sprites[0].as_ref().unwrap().id, sprite("s8").id);
        assert_eq!(emitter.face, Direction::North);
    }

    #[test]
    fn standard_overlay_no_sides_falls_back_to_corners() {
        // java: 0b0000 -> the corner walk applies c01/c12/c23/c30, sprites 2/0/14/16.
        let processor = standard_processor(Rc::new(AdjacentConnect { base: BlockPos { x: 0, y: 0, z: 0 } }));
        let pool = standard_process(&processor);
        let pool = pool.borrow();
        let pool = pool.downcast_ref::<StandardOverlayEmitterPool>().unwrap();
        assert_eq!(pool.next_index, 1);
        let emitter = &*pool.list[0].borrow();
        assert_eq!(emitter.sprite_amount, 4);
        assert_eq!(emitter.sprites[0].as_ref().unwrap().id, sprite("s2").id);
        assert_eq!(emitter.sprites[1].as_ref().unwrap().id, sprite("s0").id);
        assert_eq!(emitter.sprites[2].as_ref().unwrap().id, sprite("s14").id);
        assert_eq!(emitter.sprites[3].as_ref().unwrap().id, sprite("s16").id);
    }

    #[test]
    fn standard_overlay_no_apply_at_all_leaves_pool_empty() {
        // java: connections everywhere and no match predicates -> no side applies and
        // every corner corner-check fails on the connects, so no emitter is taken.
        let processor = standard_processor(Rc::new(FixedConnect(true)));
        let pool = standard_process(&processor);
        let pool = pool.borrow();
        assert_eq!(pool.downcast_ref::<StandardOverlayEmitterPool>().unwrap().next_index, 0);
    }

    #[test]
    fn standard_factory_texture_amount_and_no_null_sprites() {
        let factory = StandardOverlayQuadProcessorFactory;
        let properties = StandardOverlayCtmProperties::new(
            Properties::new(),
            Identifier::new("continuity", "props").unwrap(),
            "pack".to_string(),
            0,
            None,
            "standard_overlay".to_string(),
        );
        assert_eq!(
            AbstractQuadProcessorFactory::get_texture_amount(&factory, &properties),
            17
        );
        assert!(!AbstractQuadProcessorFactory::supports_null_sprites(&factory, &properties));
    }
}
