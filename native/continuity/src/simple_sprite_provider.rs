//! 1:1 of me.pepperbell.continuity.client.processor.simple.{SpriteProvider,
//! CtmSpriteProvider}, by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity).
//! Upstream: src/main/java/me/pepperbell/continuity/client/processor/simple/.

use std::rc::Rc;

use crate::processor_base::{
    get_directions, BlockRenderView, BlockState, BlockPos, ProcessingDataProvider, QuadView, Random,
};
use crate::processor_enums::{Direction, OrientationMode, Symmetry};
use crate::connecting_properties::{
    ConnectionPredicate, OrientedConnectingCtmProperties, RandomCtmProperties, RepeatCtmProperties,
};
use crate::properties::BaseCtmProperties;
use crate::random_index_provider::{RandomIndexProvider, RandomIndexProviderFactory};
use crate::seams::{block_state_get_appearance, block_state_get_block_id, block_view_get_block_state, Sprite};

/// java: client.processor.simple.SpriteProvider.
pub trait SpriteProvider {
    /// java: @Nullable Sprite getSprite(quad, sprite, blockView, appearanceState,
    /// state, pos, randomSupplier, dataProvider).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite>;
}

/// java: SpriteProvider.Factory<T extends OrientedConnectingCtmProperties>.
pub trait SpriteProviderFactory<T> {
    /// java: SpriteProvider createSpriteProvider(Sprite[] sprites, T properties).
    fn create_sprite_provider(&self, sprites: Vec<Option<Sprite>>, properties: &T) -> Box<dyn SpriteProvider>;

    /// java: int getTextureAmount(T properties).
    fn get_texture_amount(&self, properties: &T) -> usize;
}

/// Indices for this array are formed from these bit values:
/// 128 64  32
/// 1   *   16
/// 2   4   8
pub const SPRITE_INDEX_MAP: [i32; 256] = [
    0, 3, 0, 3, 12, 5, 12, 15, 0, 3, 0, 3, 12, 5, 12, 15,
    1, 2, 1, 2, 4, 7, 4, 29, 1, 2, 1, 2, 13, 31, 13, 14,
    0, 3, 0, 3, 12, 5, 12, 15, 0, 3, 0, 3, 12, 5, 12, 15,
    1, 2, 1, 2, 4, 7, 4, 29, 1, 2, 1, 2, 13, 31, 13, 14,
    36, 17, 36, 17, 24, 19, 24, 43, 36, 17, 36, 17, 24, 19, 24, 43,
    16, 18, 16, 18, 6, 46, 6, 21, 16, 18, 16, 18, 28, 9, 28, 22,
    36, 17, 36, 17, 24, 19, 24, 43, 36, 17, 36, 17, 24, 19, 24, 43,
    37, 40, 37, 40, 30, 8, 30, 34, 37, 40, 37, 40, 25, 23, 25, 45,
    0, 3, 0, 3, 12, 5, 12, 15, 0, 3, 0, 3, 12, 5, 12, 15,
    1, 2, 1, 2, 4, 7, 4, 29, 1, 2, 1, 2, 13, 31, 13, 14,
    0, 3, 0, 3, 12, 5, 12, 15, 0, 3, 0, 3, 12, 5, 12, 15,
    1, 2, 1, 2, 4, 7, 4, 29, 1, 2, 1, 2, 13, 31, 13, 14,
    36, 39, 36, 39, 24, 41, 24, 27, 36, 39, 36, 39, 24, 41, 24, 27,
    16, 42, 16, 42, 6, 20, 6, 10, 16, 42, 16, 42, 28, 35, 28, 44,
    36, 39, 36, 39, 24, 41, 24, 27, 36, 39, 36, 39, 24, 41, 24, 27,
    37, 38, 37, 38, 30, 11, 30, 32, 37, 38, 37, 38, 25, 33, 25, 26,
];

/// java: CtmSpriteProvider implements SpriteProvider.
pub struct CtmSpriteProvider {
    sprites: Vec<Option<Sprite>>,
    connection_predicate: Rc<dyn ConnectionPredicate>,
    inner_seams: bool,
    orientation_mode: OrientationMode,
}

impl CtmSpriteProvider {
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
    ) -> CtmSpriteProvider {
        CtmSpriteProvider { sprites, connection_predicate, inner_seams, orientation_mode }
    }
}

impl SpriteProvider for CtmSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let directions = get_directions(self.orientation_mode, quad, appearance_state);
        let keys = crate::processor_base::processing_data_keys();
        let mutable_pos = data_provider.get_data(&keys.mutable_pos);
        let connections = Self::get_connections(
            &directions,
            self.connection_predicate.as_ref(),
            self.inner_seams,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );
        self.sprites[SPRITE_INDEX_MAP[connections as usize] as usize].clone()
    }
}

impl CtmSpriteProvider {
    /// java: static int getConnections(directions, connectionPredicate, innerSeams,
    /// mutablePos, blockView, appearanceState, state, pos, face, quadSprite).
    pub fn get_connections(
        directions: &[Direction; 4],
        connection_predicate: &dyn ConnectionPredicate,
        inner_seams: bool,
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> i32 {
        let mut connections = 0;
        for i in 0..4 {
            mutable_pos.set(pos, directions[i]);
            if connection_predicate.should_connect_inner_seams(
                block_view,
                appearance_state,
                state,
                pos,
                mutable_pos,
                face,
                quad_sprite,
                inner_seams,
            ) {
                connections |= 1 << (i * 2);
            }
        }
        for i in 0..4 {
            let index1 = i;
            let index2 = (i + 1) % 4;
            if ((connections >> (index1 * 2)) & 1) == 1 && ((connections >> (index2 * 2)) & 1) == 1 {
                mutable_pos.set(pos, directions[index1]);
                mutable_pos.move_direction(directions[index2]);
                if connection_predicate.should_connect_inner_seams(
                    block_view,
                    appearance_state,
                    state,
                    pos,
                    mutable_pos,
                    face,
                    quad_sprite,
                    inner_seams,
                ) {
                    connections |= 1 << (i * 2 + 1);
                }
            }
        }
        connections
    }
}

/// java: CtmSpriteProvider.Factory implements SpriteProvider.Factory
/// <OrientedConnectingCtmProperties>.
pub struct CtmSpriteProviderFactory;

impl SpriteProviderFactory<crate::connecting_properties::OrientedConnectingCtmProperties> for CtmSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::connecting_properties::OrientedConnectingCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(CtmSpriteProvider::new(
            sprites,
            Rc::new(properties.base.base.get_connection_predicate().expect("connection predicate missing")),
            properties.base.get_inner_seams(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &crate::connecting_properties::OrientedConnectingCtmProperties) -> usize {
        47
    }
}


/// java: FixedSpriteProvider implements SpriteProvider.
pub struct FixedSpriteProvider {
    sprite: Option<Sprite>,
}

impl FixedSpriteProvider {
    /// java: FixedSpriteProvider(Sprite sprite).
    pub fn new(sprite: Option<Sprite>) -> FixedSpriteProvider {
        FixedSpriteProvider { sprite }
    }
}

impl SpriteProvider for FixedSpriteProvider {
    /// java: getSprite(...) returns this.sprite.
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
        self.sprite.clone()
    }
}

/// java: FixedSpriteProvider.Factory implements SpriteProvider.Factory
/// <BaseCtmProperties>.
pub struct FixedSpriteProviderFactory;

impl SpriteProviderFactory<BaseCtmProperties> for FixedSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties) -> new FixedSpriteProvider(sprites[0]).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        _properties: &BaseCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(FixedSpriteProvider::new(sprites[0].clone()))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &BaseCtmProperties) -> usize {
        1
    }
}

/// Indices for this array are formed from these bit values:
/// 1   *   2
pub const HORIZONTAL_SPRITE_INDEX_MAP: [i32; 4] = [3, 2, 0, 1];

/// java: HorizontalSpriteProvider implements SpriteProvider.
pub struct HorizontalSpriteProvider {
    sprites: Vec<Option<Sprite>>,
    connection_predicate: Rc<dyn ConnectionPredicate>,
    inner_seams: bool,
    orientation_mode: OrientationMode,
}

impl HorizontalSpriteProvider {
    /// java: HorizontalSpriteProvider(sprites, connectionPredicate, innerSeams, orientationMode).
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
    ) -> HorizontalSpriteProvider {
        HorizontalSpriteProvider { sprites, connection_predicate, inner_seams, orientation_mode }
    }

    /// java: protected int getConnections(directions, mutablePos, blockView,
    /// appearanceState, state, pos, face, quadSprite).
    pub fn get_connections(
        &self,
        directions: &[Direction; 4],
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> i32 {
        let mut connections = 0;
        for i in 0..2 {
            mutable_pos.set(pos, directions[i * 2]);
            if self.connection_predicate.should_connect_inner_seams(
                block_view,
                appearance_state,
                state,
                pos,
                mutable_pos,
                face,
                quad_sprite,
                self.inner_seams,
            ) {
                connections |= 1 << i;
            }
        }
        connections
    }
}

impl SpriteProvider for HorizontalSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let directions = get_directions(self.orientation_mode, quad, appearance_state);
        let keys = crate::processor_base::processing_data_keys();
        let mutable_pos = data_provider.get_data(&keys.mutable_pos);
        let connections = self.get_connections(
            &directions,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );
        self.sprites[HORIZONTAL_SPRITE_INDEX_MAP[connections as usize] as usize].clone()
    }
}

/// java: HorizontalSpriteProvider.Factory implements SpriteProvider.Factory
/// <OrientedConnectingCtmProperties>.
pub struct HorizontalSpriteProviderFactory;

impl SpriteProviderFactory<OrientedConnectingCtmProperties> for HorizontalSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &OrientedConnectingCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(HorizontalSpriteProvider::new(
            sprites,
            Rc::new(properties.base.base.get_connection_predicate().expect("connection predicate missing")),
            properties.base.get_inner_seams(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &OrientedConnectingCtmProperties) -> usize {
        4
    }
}

/// Indices for this array are formed from these bit values:
/// 2
/// *
/// 1
pub const VERTICAL_SPRITE_INDEX_MAP: [i32; 4] = [3, 2, 0, 1];

/// java: VerticalSpriteProvider implements SpriteProvider.
pub struct VerticalSpriteProvider {
    sprites: Vec<Option<Sprite>>,
    connection_predicate: Rc<dyn ConnectionPredicate>,
    inner_seams: bool,
    orientation_mode: OrientationMode,
}

impl VerticalSpriteProvider {
    /// java: VerticalSpriteProvider(sprites, connectionPredicate, innerSeams, orientationMode).
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
    ) -> VerticalSpriteProvider {
        VerticalSpriteProvider { sprites, connection_predicate, inner_seams, orientation_mode }
    }

    /// java: protected int getConnections(directions, mutablePos, blockView,
    /// appearanceState, state, pos, face, quadSprite).
    pub fn get_connections(
        &self,
        directions: &[Direction; 4],
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> i32 {
        let mut connections = 0;
        for i in 0..2 {
            mutable_pos.set(pos, directions[i * 2 + 1]);
            if self.connection_predicate.should_connect_inner_seams(
                block_view,
                appearance_state,
                state,
                pos,
                mutable_pos,
                face,
                quad_sprite,
                self.inner_seams,
            ) {
                connections |= 1 << i;
            }
        }
        connections
    }
}

impl SpriteProvider for VerticalSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let directions = get_directions(self.orientation_mode, quad, appearance_state);
        let keys = crate::processor_base::processing_data_keys();
        let mutable_pos = data_provider.get_data(&keys.mutable_pos);
        let connections = self.get_connections(
            &directions,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );
        self.sprites[VERTICAL_SPRITE_INDEX_MAP[connections as usize] as usize].clone()
    }
}

/// java: VerticalSpriteProvider.Factory implements SpriteProvider.Factory
/// <OrientedConnectingCtmProperties>.
pub struct VerticalSpriteProviderFactory;

impl SpriteProviderFactory<OrientedConnectingCtmProperties> for VerticalSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &OrientedConnectingCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(VerticalSpriteProvider::new(
            sprites,
            Rc::new(properties.base.base.get_connection_predicate().expect("connection predicate missing")),
            properties.base.get_inner_seams(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &OrientedConnectingCtmProperties) -> usize {
        4
    }
}

/// Indices for this array are formed from these bit values:
/// 32  16  8
///     *
/// 1   2   4
pub const HORIZONTAL_VERTICAL_SECONDARY_SPRITE_INDEX_MAP: [i32; 64] = [
    3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3,
    4, 4, 5, 4, 4, 4, 4, 4, 3, 3, 6, 3, 3, 3, 3, 3,
    3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3,
    3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3,
];

/// java: HorizontalVerticalSpriteProvider extends HorizontalSpriteProvider.
pub struct HorizontalVerticalSpriteProvider {
    horizontal: HorizontalSpriteProvider,
}

impl HorizontalVerticalSpriteProvider {
    /// java: constructor delegates to super.
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
    ) -> HorizontalVerticalSpriteProvider {
        HorizontalVerticalSpriteProvider {
            horizontal: HorizontalSpriteProvider::new(
                sprites,
                connection_predicate,
                inner_seams,
                orientation_mode,
            ),
        }
    }

    /// java: protected int getSecondaryConnections(directions, mutablePos, blockView,
    /// appearanceState, state, pos, face, quadSprite).
    pub fn get_secondary_connections(
        &self,
        directions: &[Direction; 4],
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> i32 {
        let mut connections = 0;
        for i in 0..2 {
            let direction = directions[i * 2 + 1];
            mutable_pos.set(pos, direction);
            if self.horizontal.connection_predicate.should_connect_inner_seams(
                block_view,
                appearance_state,
                state,
                pos,
                mutable_pos,
                face,
                quad_sprite,
                self.horizontal.inner_seams,
            ) {
                connections |= 1 << (i * 3 + 1);
                for j in 0..2 {
                    mutable_pos.set(pos, direction);
                    mutable_pos.move_direction(directions[((i + j) % 2) * 2]);
                    if self.horizontal.connection_predicate.should_connect_inner_seams(
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        mutable_pos,
                        face,
                        quad_sprite,
                        self.horizontal.inner_seams,
                    ) {
                        connections |= 1 << (i * 3 + j * 2);
                    }
                }
            }
        }
        connections
    }
}

impl SpriteProvider for HorizontalVerticalSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let directions = get_directions(self.horizontal.orientation_mode, quad, appearance_state);
        let keys = crate::processor_base::processing_data_keys();
        let mutable_pos = data_provider.get_data(&keys.mutable_pos);
        let connections = self.horizontal.get_connections(
            &directions,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );
        if connections != 0 {
            self.horizontal.sprites[HORIZONTAL_SPRITE_INDEX_MAP[connections as usize] as usize].clone()
        } else {
            let secondary_connections = self.get_secondary_connections(
                &directions,
                &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
                block_view,
                appearance_state,
                state,
                pos,
                quad.light_face(),
                sprite,
            );
            self.horizontal.sprites
                [HORIZONTAL_VERTICAL_SECONDARY_SPRITE_INDEX_MAP[secondary_connections as usize] as usize]
                .clone()
        }
    }
}

/// java: HorizontalVerticalSpriteProvider.Factory implements SpriteProvider.Factory
/// <OrientedConnectingCtmProperties>.
pub struct HorizontalVerticalSpriteProviderFactory;

impl SpriteProviderFactory<OrientedConnectingCtmProperties> for HorizontalVerticalSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &OrientedConnectingCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(HorizontalVerticalSpriteProvider::new(
            sprites,
            Rc::new(properties.base.base.get_connection_predicate().expect("connection predicate missing")),
            properties.base.get_inner_seams(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &OrientedConnectingCtmProperties) -> usize {
        7
    }
}

/// Indices for this array are formed from these bit values:
/// 32     16
/// 1   *   8
/// 2       4
pub const VERTICAL_HORIZONTAL_SECONDARY_SPRITE_INDEX_MAP: [i32; 64] = [
    3, 6, 3, 3, 3, 6, 3, 3, 4, 5, 4, 4, 3, 6, 3, 3,
    3, 6, 3, 3, 3, 6, 3, 3, 3, 6, 3, 3, 3, 6, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
];

/// java: VerticalHorizontalSpriteProvider extends VerticalSpriteProvider.
pub struct VerticalHorizontalSpriteProvider {
    vertical: VerticalSpriteProvider,
}

impl VerticalHorizontalSpriteProvider {
    /// java: constructor delegates to super.
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        connection_predicate: Rc<dyn ConnectionPredicate>,
        inner_seams: bool,
        orientation_mode: OrientationMode,
    ) -> VerticalHorizontalSpriteProvider {
        VerticalHorizontalSpriteProvider {
            vertical: VerticalSpriteProvider::new(
                sprites,
                connection_predicate,
                inner_seams,
                orientation_mode,
            ),
        }
    }

    /// java: protected int getSecondaryConnections(directions, mutablePos, blockView,
    /// appearanceState, state, pos, face, quadSprite).
    pub fn get_secondary_connections(
        &self,
        directions: &[Direction; 4],
        mutable_pos: &mut BlockPos,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> i32 {
        let mut connections = 0;
        for i in 0..2 {
            let direction = directions[i * 2];
            mutable_pos.set(pos, direction);
            // java: note the upstream call swaps appearanceState/state here:
            // shouldConnect(blockView, state, appearanceState, pos, mutablePos, face, quadSprite, innerSeams).
            if self.vertical.connection_predicate.should_connect_inner_seams(
                block_view,
                state,
                appearance_state,
                pos,
                mutable_pos,
                face,
                quad_sprite,
                self.vertical.inner_seams,
            ) {
                connections |= 1 << (i * 3);
                for j in 0..2 {
                    mutable_pos.set(pos, direction);
                    mutable_pos.move_direction(directions[((i + j) % 2) * 2 + 1]);
                    if self.vertical.connection_predicate.should_connect_inner_seams(
                        block_view,
                        appearance_state,
                        state,
                        pos,
                        mutable_pos,
                        face,
                        quad_sprite,
                        self.vertical.inner_seams,
                    ) {
                        connections |= 1 << ((i * 3 + j * 2 + 5) % 6);
                    }
                }
            }
        }
        connections
    }
}

impl SpriteProvider for VerticalHorizontalSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let directions = get_directions(self.vertical.orientation_mode, quad, appearance_state);
        let keys = crate::processor_base::processing_data_keys();
        let mutable_pos = data_provider.get_data(&keys.mutable_pos);
        let connections = self.vertical.get_connections(
            &directions,
            &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
            block_view,
            appearance_state,
            state,
            pos,
            quad.light_face(),
            sprite,
        );
        if connections != 0 {
            self.vertical.sprites[VERTICAL_SPRITE_INDEX_MAP[connections as usize] as usize].clone()
        } else {
            let secondary_connections = self.get_secondary_connections(
                &directions,
                &mut mutable_pos.borrow_mut().downcast_mut::<BlockPos>().unwrap(),
                block_view,
                appearance_state,
                state,
                pos,
                quad.light_face(),
                sprite,
            );
            self.vertical.sprites
                [VERTICAL_HORIZONTAL_SECONDARY_SPRITE_INDEX_MAP[secondary_connections as usize] as usize]
                .clone()
        }
    }
}

/// java: VerticalHorizontalSpriteProvider.Factory implements SpriteProvider.Factory
/// <OrientedConnectingCtmProperties>.
pub struct VerticalHorizontalSpriteProviderFactory;

impl SpriteProviderFactory<OrientedConnectingCtmProperties> for VerticalHorizontalSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &OrientedConnectingCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(VerticalHorizontalSpriteProvider::new(
            sprites,
            Rc::new(properties.base.base.get_connection_predicate().expect("connection predicate missing")),
            properties.base.get_inner_seams(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties).
    fn get_texture_amount(&self, _properties: &OrientedConnectingCtmProperties) -> usize {
        7
    }
}

/// java: RandomSpriteProvider implements SpriteProvider.
pub struct RandomSpriteProvider {
    sprites: Vec<Option<Sprite>>,
    index_provider: Box<dyn RandomIndexProvider>,
    random_loops: i32,
    symmetry: Symmetry,
    linked: bool,
}

impl RandomSpriteProvider {
    /// java: RandomSpriteProvider(sprites, indexProvider, randomLoops, symmetry, linked).
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        index_provider: Box<dyn RandomIndexProvider>,
        random_loops: i32,
        symmetry: Symmetry,
        linked: bool,
    ) -> RandomSpriteProvider {
        RandomSpriteProvider { sprites, index_provider, random_loops, symmetry, linked }
    }
}

impl SpriteProvider for RandomSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        _sprite: &Sprite,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        let face = quad.light_face();

        let mut x = pos.x;
        let mut y = pos.y;
        let z = pos.z;

        if self.linked {
            // java: Block block = appearanceState.getBlock();
            let block = block_state_get_block_id(appearance_state);
            let keys = crate::processor_base::processing_data_keys();
            let mutable_pos = data_provider.get_data(&keys.mutable_pos);
            {
                let mut guard = mutable_pos.borrow_mut();
                let mutable_pos = guard.downcast_mut::<BlockPos>().unwrap();
                // java: BlockPos.Mutable mutablePos = dataProvider.getData(ProcessingDataKeys.MUTABLE_POS).set(pos);
                *mutable_pos = *pos;

                let mut i = 0;
                loop {
                    // java: mutablePos.setY(mutablePos.getY() - 1);
                    mutable_pos.y -= 1;
                    i += 1;
                    // java: while (i < 3 && block == blockView.getBlockState(mutablePos)
                    //         .getAppearance(blockView, mutablePos, face, state, pos).getBlock());
                    let neighbor = block_view_get_block_state(block_view, mutable_pos);
                    let neighbor_appearance =
                        block_state_get_appearance(&neighbor, block_view, mutable_pos, face, state, pos);
                    if !(i < 3 && block == block_state_get_block_id(&neighbor_appearance)) {
                        break;
                    }
                }
                // java: y = mutablePos.getY() + 1;
                y = mutable_pos.y + 1;
            }
        }

        // java: int seed = MathUtil.mix(x, y, z, symmetry.apply(face).ordinal(), randomLoops);
        let seed = crate::math_util::mix(x, y, z, self.symmetry.apply(face).ordinal() as i32, self.random_loops);
        // java: return sprites[indexProvider.getRandomIndex(seed)];
        self.sprites[self.index_provider.get_random_index(seed) as usize].clone()
    }
}

/// java: RandomSpriteProvider.Factory implements SpriteProvider.Factory
/// <RandomCtmProperties>.
pub struct RandomSpriteProviderFactory;

impl SpriteProviderFactory<RandomCtmProperties> for RandomSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &RandomCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        if sprites.len() == 1 {
            // java: return new FixedSpriteProvider(sprites[0]);
            return Box::new(FixedSpriteProvider::new(sprites[0].clone()));
        }
        // java: return new RandomSpriteProvider(sprites,
        //         properties.getIndexProviderFactory().createIndexProvider(sprites.length),
        //         properties.getRandomLoops(), properties.getSymmetry(), properties.getLinked());
        let index_provider =
            properties.get_index_provider_factory().create_index_provider(sprites.len() as i32);
        Box::new(RandomSpriteProvider::new(
            sprites,
            index_provider,
            properties.get_random_loops(),
            properties.get_symmetry(),
            properties.get_linked(),
        ))
    }

    /// java: getTextureAmount(properties) returns properties.getSpriteIds().size().
    fn get_texture_amount(&self, properties: &RandomCtmProperties) -> usize {
        properties.base.get_tile_amount()
    }
}

/// java: RepeatSpriteProvider implements SpriteProvider.
pub struct RepeatSpriteProvider {
    sprites: Vec<Option<Sprite>>,
    width: i32,
    height: i32,
    symmetry: Symmetry,
    orientation_mode: OrientationMode,
}

impl RepeatSpriteProvider {
    /// java: RepeatSpriteProvider(sprites, width, height, symmetry, orientationMode).
    pub fn new(
        sprites: Vec<Option<Sprite>>,
        width: i32,
        height: i32,
        symmetry: Symmetry,
        orientation_mode: OrientationMode,
    ) -> RepeatSpriteProvider {
        RepeatSpriteProvider { sprites, width, height, symmetry, orientation_mode }
    }
}

impl SpriteProvider for RepeatSpriteProvider {
    /// java: getSprite(...).
    fn get_sprite(
        &self,
        quad: &QuadView,
        _sprite: &Sprite,
        _block_view: &BlockRenderView,
        appearance_state: &BlockState,
        _state: &BlockState,
        pos: &BlockPos,
        _random_supplier: &dyn Fn() -> Random,
        _data_provider: &dyn ProcessingDataProvider,
    ) -> Option<Sprite> {
        // java: Direction face = symmetry.apply(quad.lightFace());
        let face = self.symmetry.apply(quad.light_face());

        let x = pos.x;
        let y = pos.y;
        let z = pos.z;

        let mut sprite_x: i32;
        let mut sprite_y: i32;
        match face {
            Direction::Down => {
                // MCPatcher uses a different formula for the down face.
                // It is not used here to maintain Optifine parity.
                // spriteX = -x;
                // spriteY = -z;
                sprite_x = x;
                sprite_y = -z - 1;
            }
            Direction::Up => {
                sprite_x = x;
                sprite_y = z;
            }
            Direction::North => {
                sprite_x = -x - 1;
                sprite_y = -y;
            }
            Direction::South => {
                sprite_x = x;
                sprite_y = -y;
            }
            Direction::West => {
                sprite_x = z;
                sprite_y = -y;
            }
            Direction::East => {
                sprite_x = -z - 1;
                sprite_y = -y;
            }
        }

        // java: switch (orientationMode.getOrientation(quad, appearanceState)).
        let orientation = self.orientation_mode
            .get_orientation(face, crate::processor_base::block_state_get_axis(appearance_state).map(|axis| axis.ordinal()));
        match orientation {
            1 => {
                let temp = sprite_x;
                sprite_x = -sprite_y - 1;
                sprite_y = temp;
            }
            2 => {
                sprite_x = -sprite_x - 1;
                sprite_y = -sprite_y - 1;
            }
            3 => {
                let temp = sprite_x;
                sprite_x = sprite_y;
                sprite_y = -temp - 1;
            }
            4 => {
                sprite_x = -sprite_x - 1;
            }
            5 => {
                let temp = sprite_x;
                sprite_x = sprite_y;
                sprite_y = temp;
            }
            6 => {
                sprite_y = -sprite_y - 1;
            }
            7 => {
                let temp = sprite_x;
                sprite_x = -sprite_y - 1;
                sprite_y = -temp - 1;
            }
            _ => {}
        }

        sprite_x %= self.width;
        if sprite_x < 0 {
            sprite_x += self.width;
        }
        sprite_y %= self.height;
        if sprite_y < 0 {
            sprite_y += self.height;
        }

        // java: return sprites[width * spriteY + spriteX];
        self.sprites[(self.width * sprite_y + sprite_x) as usize].clone()
    }
}

/// java: RepeatSpriteProvider.Factory implements SpriteProvider.Factory
/// <RepeatCtmProperties>.
pub struct RepeatSpriteProviderFactory;

impl SpriteProviderFactory<RepeatCtmProperties> for RepeatSpriteProviderFactory {
    /// java: createSpriteProvider(sprites, properties).
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &RepeatCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        Box::new(RepeatSpriteProvider::new(
            sprites,
            properties.get_width(),
            properties.get_height(),
            properties.get_symmetry(),
            properties.get_orientation_mode(),
        ))
    }

    /// java: getTextureAmount(properties) returns properties.getWidth() * properties.getHeight().
    fn get_texture_amount(&self, properties: &RepeatCtmProperties) -> usize {
        (properties.get_width() * properties.get_height()) as usize
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{any::Any, cell::RefCell, collections::HashMap, rc::Rc};
    use crate::processor_base::{processing_data_keys, ProcessingDataKey};
    use crate::seams::Identifier;
    use crate::random_index_provider::RandomIndexProvider as Rip;

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

    fn state() -> BlockState {
        BlockState { block_id: 0, state_id: 0, sprites: vec![] }
    }

    struct TestContext {
        map: RefCell<HashMap<usize, Rc<RefCell<Box<dyn Any>>>>>,
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

    /// java: a ConnectionPredicate stub answering every shouldConnect with a constant.
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

    struct ConstIndex(i32);

    impl Rip for ConstIndex {
        fn get_random_index(&self, _random: i32) -> i32 {
            self.0
        }
    }

    fn ids(sprites: &[Option<Sprite>]) -> Vec<Identifier> {
        sprites.iter().map(|s| s.as_ref().unwrap().id.clone()).collect()
    }

    fn four_sprites() -> Vec<Option<Sprite>> {
        vec![Some(sprite("a")), Some(sprite("b")), Some(sprite("c")), Some(sprite("d"))]
    }

    fn ctx() -> TestContext {
        let _ = processing_data_keys();
        TestContext { map: RefCell::new(HashMap::new()) }
    }

    #[test]
    fn fixed_returns_constant_sprite() {
        let s0 = sprite("a");
        let provider = FixedSpriteProvider::new(Some(s0.clone()));
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &s0, &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, s0.id);
    }

    #[test]
    fn horizontal_all_connect_selects_map_index_1() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[1].clone();
        let provider = HorizontalSpriteProvider::new(sprites, Rc::new(FixedConnect(true)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 3, y: 4, z: 5 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn horizontal_none_connect_selects_map_index_3() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[3].clone();
        let provider = HorizontalSpriteProvider::new(sprites, Rc::new(FixedConnect(false)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn vertical_none_connect_selects_map_index_3() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[3].clone();
        let provider = VerticalSpriteProvider::new(sprites, Rc::new(FixedConnect(false)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn hv_primary_path_selects_horizontal_map() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[1].clone();
        let provider = HorizontalVerticalSpriteProvider::new(sprites, Rc::new(FixedConnect(true)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn hv_secondary_path_selects_secondary_map_index_3() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[3].clone();
        let provider = HorizontalVerticalSpriteProvider::new(sprites, Rc::new(FixedConnect(false)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn vh_swapped_call_runs_and_selects_primary_map() {
        let sprites = four_sprites();
        let expected = ids(&sprites)[1].clone();
        let provider = VerticalHorizontalSpriteProvider::new(sprites, Rc::new(FixedConnect(true)), false, OrientationMode::None);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn random_uses_index_provider() {
        let sprites = vec![Some(sprite("a")), Some(sprite("b"))];
        let expected = ids(&sprites)[1].clone();
        let provider = RandomSpriteProvider::new(sprites, Box::new(ConstIndex(1)), 0, Symmetry::None, false);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 7, y: 2, z: -3 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn random_linked_walks_down_column_without_panic() {
        let sprites = vec![Some(sprite("a")), Some(sprite("b"))];
        let expected = ids(&sprites)[0].clone();
        let provider = RandomSpriteProvider::new(sprites, Box::new(ConstIndex(0)), 0, Symmetry::None, true);
        let view = BlockRenderView;
        let st = state();
        let pos = BlockPos { x: 1, y: 9, z: 2 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &view, &st, &st, &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn repeat_down_face_selects_index_width() {
        // java: face Down -> spriteX = x = 0, spriteY = -z - 1 = -1 -> +height -> 1;
        // orientation None -> 0; index = width * 1 + 0 = 2.
        let sprites = four_sprites();
        let expected = ids(&sprites)[2].clone();
        let provider = RepeatSpriteProvider::new(sprites, 2, 2, Symmetry::None, OrientationMode::None);
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::Down), &sprite("q"), &BlockRenderView, &state(), &state(), &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn repeat_north_face_wraps_negative_sprite_x() {
        // java: face North -> spriteX = -x - 1 = -1 -> +width -> 1; spriteY = -y = 0;
        // index = width * 0 + 1 = 1.
        let sprites = four_sprites();
        let expected = ids(&sprites)[1].clone();
        let provider = RepeatSpriteProvider::new(sprites, 2, 2, Symmetry::None, OrientationMode::None);
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let got = provider.get_sprite(&QuadView::bare(Direction::North), &sprite("q"), &BlockRenderView, &state(), &state(), &pos, &|| Random { seed: 0 }, &ctx());
        assert_eq!(got.unwrap().id, expected);
    }

    #[test]
    fn repeat_texture_amount_is_width_times_height() {
        assert_eq!(RepeatSpriteProviderFactory.get_texture_amount(&new_repeat_properties(3, 4)), 12);
    }

    fn new_repeat_properties(width: i32, height: i32) -> RepeatCtmProperties {
        let mut p = RepeatCtmProperties::new(
            crate::parsing::Properties::new(),
            Identifier::new("minecraft", "optifine/ctm/repeat.properties").unwrap(),
            "pack".into(),
            0,
            None,
            "repeat".into(),
        );
        p.width = width;
        p.height = height;
        p
    }
}

// rust carrying: the overlay properties types delegate their sprite-provider wiring to
// the same factories the standard types use, reached through the contained base, matching
// the upstream Factory<? super T> provider lookup through the inheritance chain.

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
> for CtmSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
> for HorizontalSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
> for VerticalSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
> for HorizontalVerticalSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
> for VerticalHorizontalSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::OrientedConnectingOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::OrientedConnectingCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::BaseOverlayCtmProperties,
> for FixedSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::BaseOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::properties::BaseCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::BaseOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::properties::BaseCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::RandomOverlayCtmProperties,
> for RandomSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::RandomOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::RandomCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::RandomOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::RandomCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}

impl crate::simple_sprite_provider::SpriteProviderFactory<
    crate::overlay_properties::RepeatOverlayCtmProperties,
> for RepeatSpriteProviderFactory
{
    fn create_sprite_provider(
        &self,
        sprites: Vec<Option<Sprite>>,
        properties: &crate::overlay_properties::RepeatOverlayCtmProperties,
    ) -> Box<dyn SpriteProvider> {
        SpriteProviderFactory::<crate::connecting_properties::RepeatCtmProperties>::create_sprite_provider(
            self, sprites, &properties.base,
        )
    }

    fn get_texture_amount(
        &self,
        properties: &crate::overlay_properties::RepeatOverlayCtmProperties,
    ) -> usize {
        SpriteProviderFactory::<crate::connecting_properties::RepeatCtmProperties>::get_texture_amount(
            self, &properties.base,
        )
    }
}
