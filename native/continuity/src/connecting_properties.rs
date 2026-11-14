//! 1:1 of the client.properties connecting/random/repeat classes, plus the
//! client.processor.ConnectionPredicate interface staged here until the
//! processor module lands (its ConnectionType enum lives inside
//! BasicConnectingCtmProperties upstream, same as here).

use std::collections::HashMap;
use std::rc::Rc;

use crate::parsing::{parse_orientation_mode, parse_symmetry, Properties};
use crate::processor_enums::{Direction, OrientationMode, Symmetry};
use crate::properties::BaseCtmProperties;
use crate::random_index_provider::{
    RandomIndexProviderFactory, UnweightedFactory, WeightedFactory,
};
use crate::seams::{
    block_state_get_appearance, block_view_get_block_state, sprite_calculator_get_sprites,
    BlockPos, BlockRenderView, BlockState, Identifier, RedirectHandler, Sprite, SpriteId,
};

/// java: Boolean.parseBoolean(str).
fn parse_boolean(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/// java: weightsStr.trim().split("[ ,]") - single space or comma separators,
/// trailing empty strings dropped (interior empties kept, the warn index follows
/// the Java split array).
fn java_split_char_class(s: &str) -> Vec<String> {
    let mut parts: Vec<String> = s
        .split(|c: char| c == ' ' || c == ',')
        .map(|p| p.to_string())
        .collect();
    while parts.last().map_or(false, |p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// java: me.pepperbell.continuity.api.client.CtmProperties interface, carried as a
/// trait over the concrete properties structs (composition stands in for Java
/// inheritance, base() names the upstream superclass chain).
pub trait CtmProperties {
    /// The upstream BaseCtmProperties instance (up through the inheritance chain).
    fn base(&self) -> &BaseCtmProperties;

    /// java: getMethod().
    fn get_method(&self) -> &str {
        &self.base().method
    }

    /// java: getResourceId().
    fn get_resource_id(&self) -> &Identifier {
        &self.base().resource_id
    }

    /// java: getPackId().
    fn get_pack_id(&self) -> &str {
        &self.base().pack_id
    }

    /// java: getTileAmount().
    fn get_tile_amount(&self) -> usize {
        self.base().get_tile_amount()
    }

    /// java: isValid().
    fn is_valid(&self) -> bool {
        self.base().is_valid()
    }
}

impl CtmProperties for BaseCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        self
    }
}

/// java: CtmProperties.Factory<T>.
pub type CtmPropertiesFactory<T> = Rc<
    dyn Fn(
        Properties,
        Identifier,
        String,
        i32,
        Option<Rc<dyn RedirectHandler>>,
        String,
    ) -> Option<Box<T>>,
>;

/// java: client.properties.TileAmountValidator interface.
pub trait TileAmountValidator<T: CtmProperties + ?Sized> {
    /// java: validateTileAmount(int amount, T properties).
    fn validate_tile_amount(&self, amount: usize, properties: &T) -> bool;
}

/// java: TileAmountValidator.wrapFactory - validates the created properties' tile amount.
pub fn wrap_factory<T: CtmProperties + 'static, V: TileAmountValidator<T> + 'static + ?Sized>(
    factory: CtmPropertiesFactory<T>,
    validator: Rc<V>,
) -> CtmPropertiesFactory<T> {
    Rc::new(
        move |properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            let ctm_properties =
                factory(properties, resource_id, pack_id, pack_priority, redirect_handler, method)?;
            if validator.validate_tile_amount(ctm_properties.get_tile_amount(), ctm_properties.as_ref()) {
                return Some(ctm_properties);
            }
            None
        },
    )
}

/// java: TileAmountValidator.Exactly.
pub struct Exactly {
    pub target_amount: usize,
}

impl Exactly {
    /// java: Exactly(int targetAmount).
    pub fn new(target_amount: usize) -> Exactly {
        Exactly { target_amount }
    }
}

impl<T: CtmProperties + ?Sized> TileAmountValidator<T> for Exactly {
    fn validate_tile_amount(&self, amount: usize, properties: &T) -> bool {
        if amount == self.target_amount {
            return true;
        }
        eprintln!(
            "[continuity] Method '{}' requires exactly {} tiles but {} were provided in file '{}' in pack '{}'",
            properties.get_method(),
            self.target_amount,
            amount,
            properties.get_resource_id(),
            properties.get_pack_id()
        );
        false
    }
}

/// java: TileAmountValidator.AtLeast.
pub struct AtLeast {
    pub target_amount: usize,
}

impl AtLeast {
    /// java: AtLeast(int targetAmount).
    pub fn new(target_amount: usize) -> AtLeast {
        AtLeast { target_amount }
    }
}

impl<T: CtmProperties + ?Sized> TileAmountValidator<T> for AtLeast {
    fn validate_tile_amount(&self, amount: usize, properties: &T) -> bool {
        if amount >= self.target_amount {
            return true;
        }
        eprintln!(
            "[continuity] Method '{}' requires at least {} tiles but only {} were provided in file '{}' in pack '{}'",
            properties.get_method(),
            self.target_amount,
            amount,
            properties.get_resource_id(),
            properties.get_pack_id()
        );
        false
    }
}

/// java: client.processor.ConnectionPredicate (staged here until the processor module lands).
pub trait ConnectionPredicate {
    /// java: shouldConnect(blockView, appearanceState, state, pos, otherAppearanceState,
    /// otherState, otherPos, face, quadSprite).
    fn should_connect_full(
        &self,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        other_appearance_state: &BlockState,
        other_state: &BlockState,
        other_pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> bool;

    /// java: shouldConnect(blockView, appearanceState, state, pos, otherPos, face, quadSprite)
    /// default method.
    fn should_connect(
        &self,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        other_pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> bool {
        let other_state = block_view_get_block_state(block_view, other_pos);
        let other_appearance_state =
            block_state_get_appearance(&other_state, block_view, other_pos, face, state, pos);
        self.should_connect_full(
            block_view,
            appearance_state,
            state,
            pos,
            &other_appearance_state,
            &other_state,
            other_pos,
            face,
            quad_sprite,
        )
    }

    /// java: shouldConnect(blockView, appearanceState, state, pos, otherPos /*Mutable*/,
    /// face, quadSprite, innerSeams) default method.
    fn should_connect_inner_seams(
        &self,
        block_view: &BlockRenderView,
        appearance_state: &BlockState,
        state: &BlockState,
        pos: &BlockPos,
        other_pos: &mut BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
        inner_seams: bool,
    ) -> bool {
        if self.should_connect(block_view, appearance_state, state, pos, other_pos, face, quad_sprite) {
            if inner_seams {
                other_pos.move_direction(face);
                return !self.should_connect(
                    block_view,
                    appearance_state,
                    state,
                    pos,
                    other_pos,
                    face,
                    quad_sprite,
                );
            } else {
                return true;
            }
        }
        false
    }
}

/// java: BasicConnectingCtmProperties.ConnectionType enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionType {
    Block,
    Tile,
    State,
}

impl ConnectionType {
    /// java: ConnectionType.valueOf(str.trim().toUpperCase(Locale.ROOT)).
    pub fn value_of(name: &str) -> Option<ConnectionType> {
        match name.trim().to_ascii_uppercase().as_str() {
            "BLOCK" => Some(ConnectionType::Block),
            "TILE" => Some(ConnectionType::Tile),
            "STATE" => Some(ConnectionType::State),
            _ => None,
        }
    }
}

impl ConnectionPredicate for ConnectionType {
    fn should_connect_full(
        &self,
        _block_view: &BlockRenderView,
        appearance_state: &BlockState,
        _state: &BlockState,
        _pos: &BlockPos,
        other_appearance_state: &BlockState,
        _other_state: &BlockState,
        _other_pos: &BlockPos,
        face: Direction,
        quad_sprite: &Sprite,
    ) -> bool {
        match self {
            ConnectionType::Block => {
                // java: appearanceState.getBlock() == otherAppearanceState.getBlock().
                appearance_state.same_block(other_appearance_state)
            }
            ConnectionType::Tile => {
                if appearance_state.is_same_state(other_appearance_state) {
                    return true;
                }
                sprite_calculator_get_sprites(other_appearance_state, face)
                    .contains(&quad_sprite.id)
            }
            ConnectionType::State => appearance_state.is_same_state(other_appearance_state),
        }
    }
}

/// java: public class BasicConnectingCtmProperties extends BaseCtmProperties.
pub struct BasicConnectingCtmProperties {
    pub base: BaseCtmProperties,
    /// java: protected ConnectionPredicate connectionPredicate.
    pub connection_predicate: Option<ConnectionType>,
}

impl BasicConnectingCtmProperties {
    /// java: constructor stores the parse inputs.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> BasicConnectingCtmProperties {
        BasicConnectingCtmProperties {
            base: BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            connection_predicate: None,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_connect();
        self.detect_connect();
        self.validate_connect();
    }

    /// java: parseConnect().
    fn parse_connect(&mut self) {
        let connect_str = match self.base.properties.get("connect") {
            Some(s) => s.clone(),
            None => return,
        };

        match ConnectionType::value_of(&connect_str) {
            Some(connection_type) => self.connection_predicate = Some(connection_type),
            None => eprintln!(
                "[continuity] Unknown 'connect' value '{}' in file '{}' in pack '{}'",
                connect_str, self.base.resource_id, self.base.pack_id
            ),
        }
    }

    /// java: detectConnect().
    fn detect_connect(&mut self) {
        if self.connection_predicate.is_none() {
            if self.base.match_blocks_predicate.is_some() {
                self.connection_predicate = Some(ConnectionType::Block);
            } else if self.base.match_tiles_set.is_some() {
                self.connection_predicate = Some(ConnectionType::Tile);
            }
        }
    }

    /// java: validateConnect().
    fn validate_connect(&mut self) {
        if self.connection_predicate.is_none() {
            eprintln!(
                "[continuity] No valid connection type provided in file '{}' in pack '{}'",
                self.base.resource_id, self.base.pack_id
            );
            self.base.valid = false;
        }
    }

    /// java: getConnectionPredicate().
    pub fn get_connection_predicate(&self) -> Option<ConnectionType> {
        self.connection_predicate
    }
}

impl CtmProperties for BasicConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

/// java: public class ConnectingCtmProperties extends BasicConnectingCtmProperties.
pub struct ConnectingCtmProperties {
    pub base: BasicConnectingCtmProperties,
    /// java: protected boolean innerSeams = false.
    pub inner_seams: bool,
}

impl ConnectingCtmProperties {
    /// java: constructor.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> ConnectingCtmProperties {
        ConnectingCtmProperties {
            base: BasicConnectingCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            inner_seams: false,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_inner_seams();
    }

    /// java: parseInnerSeams().
    fn parse_inner_seams(&mut self) {
        let inner_seams_str = match self.base.base.properties.get("innerSeams") {
            Some(s) => s.clone(),
            None => return,
        };

        // java: Boolean.parseBoolean(innerSeamsStr.trim()).
        self.inner_seams = parse_boolean(inner_seams_str.trim());
    }

    /// java: getInnerSeams().
    pub fn get_inner_seams(&self) -> bool {
        self.inner_seams
    }
}

impl CtmProperties for ConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

/// java: public class OrientedConnectingCtmProperties extends ConnectingCtmProperties.
pub struct OrientedConnectingCtmProperties {
    pub base: ConnectingCtmProperties,
    /// java: protected OrientationMode orientationMode.
    pub orientation_mode: OrientationMode,
}

impl OrientedConnectingCtmProperties {
    /// java: constructor with defaultOrientationMode.
    pub fn with_default_orientation_mode(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
        default_orientation_mode: OrientationMode,
    ) -> OrientedConnectingCtmProperties {
        OrientedConnectingCtmProperties {
            base: ConnectingCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            orientation_mode: default_orientation_mode,
        }
    }

    /// java: constructor defaulting to OrientationMode.TEXTURE.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> OrientedConnectingCtmProperties {
        Self::with_default_orientation_mode(
            properties,
            resource_id,
            pack_id,
            pack_priority,
            redirect_handler,
            method,
            OrientationMode::Texture,
        )
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_orient();
    }

    /// java: parseOrient().
    fn parse_orient(&mut self) {
        // java: PropertiesParsingHelper.parseOrientationMode(properties, "orient", resourceId, packId).
        let parsed = parse_orientation_mode(
            &self.base.base.base.properties,
            "orient",
            &self.base.base.base.resource_id,
            &self.base.base.base.pack_id,
        );
        if let Some(orientation_mode) = parsed {
            self.orientation_mode = orientation_mode;
        }
    }

    /// java: getOrientationMode().
    pub fn get_orientation_mode(&self) -> OrientationMode {
        self.orientation_mode
    }
}

impl CtmProperties for OrientedConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base
    }
}

/// java: public class CompactConnectingCtmProperties extends OrientedConnectingCtmProperties.
pub struct CompactConnectingCtmProperties {
    pub base: OrientedConnectingCtmProperties,
    /// java: @Nullable protected Int2IntMap tileReplacementMap.
    pub tile_replacement_map: Option<HashMap<i32, i32>>,
}

impl CompactConnectingCtmProperties {
    /// java: constructor with defaultOrientationMode.
    pub fn with_default_orientation_mode(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
        default_orientation_mode: OrientationMode,
    ) -> CompactConnectingCtmProperties {
        CompactConnectingCtmProperties {
            base: OrientedConnectingCtmProperties::with_default_orientation_mode(
                properties,
                resource_id,
                pack_id,
                pack_priority,
                redirect_handler,
                method,
                default_orientation_mode,
            ),
            tile_replacement_map: None,
        }
    }

    /// java: constructor defaulting to OrientationMode.TEXTURE.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> CompactConnectingCtmProperties {
        Self::with_default_orientation_mode(
            properties,
            resource_id,
            pack_id,
            pack_priority,
            redirect_handler,
            method,
            OrientationMode::Texture,
        )
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_tile_replacements();
    }

    /// java: parseTileReplacements() - over properties.stringPropertyNames().
    fn parse_tile_replacements(&mut self) {
        let keys: Vec<String> = self.base.base.base.base.properties.keys().cloned().collect();
        for key in keys {
            if let Some(index_str) = key.strip_prefix("ctm.") {
                // java: Integer.parseInt, NumberFormatException -> continue.
                let index: i32 = match index_str.parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if index < 0 {
                    continue;
                }

                let value_str = self
                    .base
                    .base
                    .base
                    .base
                    .properties
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();
                // java: Integer.parseInt(valueStr.trim()).
                let value: i32 = match value_str.trim().parse() {
                    Ok(v) => v,
                    Err(_) => {
                        eprintln!(
                            "[continuity] Invalid '{}' value '{}' in file '{}' in pack '{}'",
                            key,
                            value_str,
                            self.base.base.base.base.resource_id,
                            self.base.base.base.base.pack_id
                        );
                        continue;
                    }
                };
                // TODO: deduplicate code (upstream comment).
                if value < 0 {
                    eprintln!(
                        "[continuity] Invalid '{}' value '{}' in file '{}' in pack '{}'",
                        key,
                        value_str,
                        self.base.base.base.base.resource_id,
                        self.base.base.base.base.pack_id
                    );
                    continue;
                }

                self.tile_replacement_map
                    .get_or_insert_with(HashMap::new)
                    .insert(index, value);
            }
        }
    }

    /// java: @Nullable getTileReplacementMap().
    pub fn get_tile_replacement_map(&self) -> Option<&HashMap<i32, i32>> {
        self.tile_replacement_map.as_ref()
    }

    /// java: getOrientationMode() inherited from OrientedConnectingCtmProperties.
    pub fn get_orientation_mode(&self) -> OrientationMode {
        self.base.get_orientation_mode()
    }
}

impl CtmProperties for CompactConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base.base
    }
}

/// java: public class RandomCtmProperties extends BaseCtmProperties.
pub struct RandomCtmProperties {
    pub base: BaseCtmProperties,
    /// java: protected RandomIndexProvider.Factory indexProviderFactory = UnweightedFactory.INSTANCE.
    pub index_provider_factory: Rc<dyn RandomIndexProviderFactory>,
    /// java: protected int randomLoops = 0.
    pub random_loops: i32,
    /// java: protected Symmetry symmetry = Symmetry.NONE.
    pub symmetry: Symmetry,
    /// java: protected boolean linked = false.
    pub linked: bool,
}

impl RandomCtmProperties {
    /// java: constructor.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> RandomCtmProperties {
        RandomCtmProperties {
            base: BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            index_provider_factory: Rc::new(UnweightedFactory::INSTANCE),
            random_loops: 0,
            symmetry: Symmetry::None,
            linked: false,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_weights();
        self.parse_random_loops();
        self.parse_symmetry();
        self.parse_linked();
    }

    /// java: parseWeights().
    fn parse_weights(&mut self) {
        let weights_str = match self.base.properties.get("weights") {
            Some(s) => s.clone(),
            None => return,
        };

        let weight_strs = java_split_char_class(weights_str.trim());
        if !weight_strs.is_empty() {
            let mut weights: Vec<i32> = Vec::new();

            for (i, weight_str) in weight_strs.iter().enumerate() {
                if weight_str.is_empty() {
                    continue;
                }

                // java: weightStr.split("-", 2) (limit 2 keeps trailing empties).
                let parts: Vec<&str> = weight_str.splitn(2, '-').collect();
                let mut consumed = false;
                if parts.len() == 2 {
                    if let (Ok(min), Ok(max)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                        if min > 0 && max > 0 && max >= min {
                            for weight in min..=max {
                                weights.push(weight);
                            }
                            consumed = true;
                        }
                    }
                } else if parts.len() == 1 {
                    if let Ok(weight) = parts[0].parse::<i32>() {
                        if weight > 0 {
                            weights.push(weight);
                            consumed = true;
                        }
                    }
                }
                // java: NumberFormatException falls through to the warn.
                if !consumed {
                    eprintln!(
                        "[continuity] Invalid 'weights' element '{}' at index '{}' in file '{}' in pack '{}'",
                        weight_str, i, self.base.resource_id, self.base.pack_id
                    );
                }
            }

            if !weights.is_empty() {
                self.index_provider_factory = Rc::new(WeightedFactory::new(weights));
            }
        }
    }

    /// java: parseRandomLoops().
    fn parse_random_loops(&mut self) {
        let random_loops_str = match self.base.properties.get("randomLoops") {
            Some(s) => s.clone(),
            None => return,
        };

        if let Ok(random_loops) = random_loops_str.trim().parse::<i32>() {
            if (0..=9).contains(&random_loops) {
                self.random_loops = random_loops;
                return;
            }
        }
        eprintln!(
            "[continuity] Invalid 'randomLoops' value '{}' in file '{}' in pack '{}'",
            random_loops_str, self.base.resource_id, self.base.pack_id
        );
    }

    /// java: parseSymmetry().
    fn parse_symmetry(&mut self) {
        // java: PropertiesParsingHelper.parseSymmetry(properties, "symmetry", resourceId, packId).
        let parsed = parse_symmetry(
            &self.base.properties,
            "symmetry",
            &self.base.resource_id,
            &self.base.pack_id,
        );
        if let Some(symmetry) = parsed {
            self.symmetry = symmetry;
        }
    }

    /// java: parseLinked().
    fn parse_linked(&mut self) {
        let linked_str = match self.base.properties.get("linked") {
            Some(s) => s.clone(),
            None => return,
        };

        // java: Boolean.parseBoolean(linkedStr.trim()).
        self.linked = parse_boolean(linked_str.trim());
    }

    /// java: getIndexProviderFactory().
    pub fn get_index_provider_factory(&self) -> Rc<dyn RandomIndexProviderFactory> {
        self.index_provider_factory.clone()
    }

    /// java: getRandomLoops().
    pub fn get_random_loops(&self) -> i32 {
        self.random_loops
    }

    /// java: getSymmetry().
    pub fn get_symmetry(&self) -> Symmetry {
        self.symmetry
    }

    /// java: getLinked().
    pub fn get_linked(&self) -> bool {
        self.linked
    }
}

impl CtmProperties for RandomCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

/// java: public class RepeatCtmProperties extends BaseCtmProperties.
pub struct RepeatCtmProperties {
    pub base: BaseCtmProperties,
    /// java: protected int width.
    pub width: i32,
    /// java: protected int height.
    pub height: i32,
    /// java: protected Symmetry symmetry = Symmetry.NONE.
    pub symmetry: Symmetry,
    /// java: protected OrientationMode orientationMode = OrientationMode.NONE.
    pub orientation_mode: OrientationMode,
}

impl RepeatCtmProperties {
    /// java: constructor.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> RepeatCtmProperties {
        RepeatCtmProperties {
            base: BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            width: 0,
            height: 0,
            symmetry: Symmetry::None,
            orientation_mode: OrientationMode::None,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.parse_width();
        self.parse_height();
        self.parse_symmetry();
        self.parse_orient();
    }

    /// java: parseWidth().
    fn parse_width(&mut self) {
        let width_str = match self.base.properties.get("width") {
            Some(s) => s.clone(),
            None => {
                eprintln!(
                    "[continuity] No 'width' value provided in file '{}' in pack '{}'",
                    self.base.resource_id, self.base.pack_id
                );
                self.base.valid = false;
                return;
            }
        };

        if let Ok(width) = width_str.trim().parse::<i32>() {
            if width > 0 {
                self.width = width;
                return;
            }
        }
        eprintln!(
            "[continuity] Invalid 'width' value '{}' in file '{}' in pack '{}'",
            width_str, self.base.resource_id, self.base.pack_id
        );
        self.base.valid = false;
    }

    /// java: parseHeight().
    fn parse_height(&mut self) {
        let height_str = match self.base.properties.get("height") {
            Some(s) => s.clone(),
            None => {
                eprintln!(
                    "[continuity] No 'height' value provided in file '{}' in pack '{}'",
                    self.base.resource_id, self.base.pack_id
                );
                self.base.valid = false;
                return;
            }
        };

        if let Ok(height) = height_str.trim().parse::<i32>() {
            if height > 0 {
                self.height = height;
                return;
            }
        }
        eprintln!(
            "[continuity] Invalid 'height' value '{}' in file '{}' in pack '{}'",
            height_str, self.base.resource_id, self.base.pack_id
        );
        self.base.valid = false;
    }

    /// java: parseSymmetry().
    fn parse_symmetry(&mut self) {
        // java: PropertiesParsingHelper.parseSymmetry(properties, "symmetry", resourceId, packId).
        let parsed = parse_symmetry(
            &self.base.properties,
            "symmetry",
            &self.base.resource_id,
            &self.base.pack_id,
        );
        if let Some(symmetry) = parsed {
            self.symmetry = symmetry;
        }
    }

    /// java: parseOrient().
    fn parse_orient(&mut self) {
        // java: PropertiesParsingHelper.parseOrientationMode(properties, "orient", resourceId, packId).
        let parsed = parse_orientation_mode(
            &self.base.properties,
            "orient",
            &self.base.resource_id,
            &self.base.pack_id,
        );
        if let Some(orientation_mode) = parsed {
            self.orientation_mode = orientation_mode;
        }
    }

    /// java: getWidth().
    pub fn get_width(&self) -> i32 {
        self.width
    }

    /// java: getHeight().
    pub fn get_height(&self) -> i32 {
        self.height
    }

    /// java: getSymmetry().
    pub fn get_symmetry(&self) -> Symmetry {
        self.symmetry
    }

    /// java: getOrientationMode().
    pub fn get_orientation_mode(&self) -> OrientationMode {
        self.orientation_mode
    }
}

impl CtmProperties for RepeatCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

/// java: public static class Validator<T extends RepeatCtmProperties> implements
/// TileAmountValidator<T>; carried concretely over RepeatCtmProperties.
pub struct RepeatValidator;

impl TileAmountValidator<RepeatCtmProperties> for RepeatValidator {
    fn validate_tile_amount(&self, amount: usize, properties: &RepeatCtmProperties) -> bool {
        // java: int targetAmount = properties.getWidth() * properties.getHeight().
        let target_amount = properties.get_width().wrapping_mul(properties.get_height());
        if target_amount >= 0 && amount == target_amount as usize {
            return true;
        }
        eprintln!(
            "[continuity] Method '{}' requires exactly {} tiles but {} were provided in file '{}' in pack '{}'",
            properties.get_method(),
            target_amount,
            amount,
            properties.get_resource_id(),
            properties.get_pack_id()
        );
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::BaseCtmProperties as _;

    fn props(entries: &[(&str, &str)]) -> Properties {
        let mut p = Properties::new();
        for (k, v) in entries {
            p.insert(k.to_string(), v.to_string());
        }
        p
    }

    fn rid() -> Identifier {
        Identifier::new("minecraft", "test.properties").unwrap()
    }

    fn state(block_id: u64, state_id: u64, sprites: Vec<SpriteId>) -> BlockState {
        BlockState {
            block_id,
            state_id,
            sprites,
        }
    }

    fn sprite(n: u64) -> Sprite {
        // SpriteId is carried as an Identifier; simple distinct ids per test.
        Sprite {
            id: Identifier::new("minecraft", &format!("sprite{n}")).unwrap(),
            min_u: 0.0,
            min_v: 0.0,
            max_u: 1.0,
            max_v: 1.0,
            animation_frame_delta: 0.0,
            emissive_sprite: None,
        }
    }

    #[test]
    fn connect_parse_unknown_falls_back_and_invalidates() {
        let mut p = BasicConnectingCtmProperties::new(
            props(&[("connect", "MAYBE")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert!(p.get_connection_predicate().is_none());
        assert!(!p.is_valid());
    }

    #[test]
    fn connect_detect_block_and_tile() {
        // matchBlocks wins over matchTiles -> BLOCK.
        let mut p = BasicConnectingCtmProperties::new(
            props(&[("matchTiles", "a b"), ("matchBlocks", "minecraft:stone")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_connection_predicate(), Some(ConnectionType::Block));

        // matchTiles only -> TILE.
        let mut p = BasicConnectingCtmProperties::new(
            props(&[("matchTiles", "a b")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_connection_predicate(), Some(ConnectionType::Tile));
    }

    #[test]
    fn connect_value_of_case_insensitive() {
        assert_eq!(ConnectionType::value_of(" tile "), Some(ConnectionType::Tile));
        assert_eq!(ConnectionType::value_of("state"), Some(ConnectionType::State));
    }

    #[test]
    fn connection_type_block_state_tile() {
        let view = BlockRenderView;
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let s1 = state(1, 10, vec![]);
        let s1b = state(1, 10, vec![]);
        let s2 = state(1, 11, vec![sprite(7).id]);

        // BLOCK: same block, any state.
        assert!(ConnectionType::Block.should_connect_full(&view, &s1, &s1, &pos, &s2, &s2, &pos, Direction::North, &sprite(0)));

        // STATE: exact state only.
        assert!(ConnectionType::State.should_connect_full(&view, &s1, &s1, &pos, &s1b, &s1b, &pos, Direction::North, &sprite(0)));
        assert!(!ConnectionType::State.should_connect_full(&view, &s1, &s1, &pos, &s2, &s2, &pos, Direction::North, &sprite(0)));

        // TILE: same state short-circuits, else the quad sprite must be in the other's sprites.
        assert!(!ConnectionType::Tile.should_connect_full(&view, &s1, &s1, &pos, &s2, &s2, &pos, Direction::North, &sprite(0)));
        assert!(ConnectionType::Tile.should_connect_full(&view, &s1, &s1, &pos, &s2, &s2, &pos, Direction::North, &sprite(7)));
        assert!(!ConnectionType::Tile.should_connect_full(&view, &s1, &s1, &pos, &s2, &s2, &pos, Direction::North, &sprite(8)));
    }

    #[test]
    fn inner_seams_flag() {
        let mut p = ConnectingCtmProperties::new(
            props(&[("connect", "block"), ("innerSeams", " true ")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert!(p.get_inner_seams());

        let mut p = ConnectingCtmProperties::new(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        p.init();
        assert!(!p.get_inner_seams());
    }

    #[test]
    fn orient_defaults_to_texture_and_parses() {
        let mut p = OrientedConnectingCtmProperties::new(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        p.init();
        assert_eq!(p.get_orientation_mode(), OrientationMode::Texture);

        let mut p = OrientedConnectingCtmProperties::new(
            props(&[("orient", "state_axis")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_orientation_mode(), OrientationMode::StateAxis);
    }

    #[test]
    fn compact_tile_replacements() {
        let mut p = CompactConnectingCtmProperties::new(
            props(&[("ctm.0", "5"), ("ctm.2", " 7 "), ("ctm.x", "1"), ("ctm.-1", "3"), ("ctm.3", "-1"), ("ctm.-2", "-2")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        let map = p.get_tile_replacement_map().unwrap();
        assert_eq!(map.get(&0), Some(&5));
        assert_eq!(map.get(&2), Some(&7));
        assert!(map.get(&3).is_none());
        assert!(map.get(&-1).is_none());
        assert!(map.get(&-2).is_none());
    }

    #[test]
    fn compact_defaults_to_texture() {
        let p = CompactConnectingCtmProperties::new(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        assert_eq!(p.get_orientation_mode(), OrientationMode::Texture);
        assert!(p.get_tile_replacement_map().is_none());
    }

    #[test]
    fn random_weights_ranges_and_singles() {
        let mut p = RandomCtmProperties::new(
            props(&[("weights", "1, 2-3 ,x 4")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        // 1 + range 2-3 + invalid 'x' + 4 -> weights [1, 2, 3, 4].
        let provider = p.get_index_provider_factory().create_index_provider(5);
        assert_eq!(provider.get_random_index(0), 0);
        // sum 10 + the average fill (10/4 = 2) -> 12; 7 % 12 walks to index 3.
        assert_eq!(provider.get_random_index(7), 3);
        assert!(p.get_index_provider_factory().create_index_provider(3).get_random_index(11) <= 2);

        // Unweighted stays the default when weights is empty after filtering.
        let mut p = RandomCtmProperties::new(props(&[("weights", "  , ")]), rid(), "pack".into(), 0, None, "ctm".into());
        p.init();
        assert_eq!(p.get_index_provider_factory().create_index_provider(4).get_random_index(10), 2);
    }

    #[test]
    fn random_loops_symmetry_linked() {
        let mut p = RandomCtmProperties::new(
            props(&[("randomLoops", "5"), ("symmetry", "opposite"), ("linked", "TRUE")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_random_loops(), 5);
        assert_eq!(p.get_symmetry(), Symmetry::Opposite);
        assert!(p.get_linked());

        let mut p = RandomCtmProperties::new(
            props(&[("randomLoops", "10"), ("symmetry", "nope"), ("linked", "false")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_random_loops(), 0);
        assert_eq!(p.get_symmetry(), Symmetry::None);
        assert!(!p.get_linked());
    }

    #[test]
    fn repeat_width_height_and_validator() {
        let mut p = RepeatCtmProperties::new(
            props(&[("width", "2"), ("height", "3"), ("symmetry", "all"), ("orient", "none")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        assert_eq!(p.get_width(), 2);
        assert_eq!(p.get_height(), 3);
        assert_eq!(p.get_symmetry(), Symmetry::All);
        assert_eq!(p.get_orientation_mode(), OrientationMode::None);

        let validator = RepeatValidator;
        assert!(validator.validate_tile_amount(6, &p));
        assert!(!validator.validate_tile_amount(5, &p));
        assert!(!validator.validate_tile_amount(7, &p));
    }

    #[test]
    fn repeat_missing_dimensions_invalidate() {
        let mut p = RepeatCtmProperties::new(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        p.init();
        assert!(!p.is_valid());

        let mut p = RepeatCtmProperties::new(props(&[("width", "0"), ("height", "-2")]), rid(), "pack".into(), 0, None, "ctm".into());
        p.init();
        assert!(!p.is_valid());
    }

    #[test]
    fn exactly_and_at_least_validators() {
        let mut p = RepeatCtmProperties::new(
            props(&[("width", "1"), ("height", "1")]),
            rid(),
            "pack".into(),
            0,
            None,
            "ctm".into(),
        );
        p.init();
        // Tiles are empty until tiles/tile parsing; seed the count through the base fields.
        p.base.tiles = vec![
            Identifier::new("minecraft", "a").unwrap(),
            Identifier::new("minecraft", "b").unwrap(),
        ];

        assert!(Exactly::new(2).validate_tile_amount(p.get_tile_amount(), &p));
        assert!(!Exactly::new(3).validate_tile_amount(p.get_tile_amount(), &p));
        assert!(AtLeast::new(2).validate_tile_amount(p.get_tile_amount(), &p));
        assert!(!AtLeast::new(3).validate_tile_amount(p.get_tile_amount(), &p));
    }

    #[test]
    fn wrap_factory_filters_on_tile_amount() {
        fn repeat_factory(
            properties: Properties,
            resource_id: Identifier,
            pack_id: String,
            pack_priority: i32,
            redirect_handler: Option<Rc<dyn RedirectHandler>>,
            method: String,
        ) -> Option<Box<RepeatCtmProperties>> {
            let mut p = RepeatCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method);
            p.width = 2;
            p.height = 2;
            p.base.tiles = vec![
                Identifier::new("minecraft", "a").unwrap(),
                Identifier::new("minecraft", "b").unwrap(),
                Identifier::new("minecraft", "c").unwrap(),
                Identifier::new("minecraft", "d").unwrap(),
            ];
            Some(Box::new(p))
        }

        let wrapped = wrap_factory(
            Rc::new(repeat_factory),
            Rc::new(Exactly::new(4)),
        );
        let built = wrapped(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        assert!(built.is_some());

        let wrapped = wrap_factory(
            Rc::new(repeat_factory),
            Rc::new(Exactly::new(5)),
        );
        let built = wrapped(props(&[]), rid(), "pack".into(), 0, None, "ctm".into());
        assert!(built.is_none());
    }

    #[test]
    fn connection_predicate_default_methods() {
        // java: shouldConnect(view, ..., otherPos, face, quadSprite) resolves the
        // other state through the view and appearance through the state.
        let view = BlockRenderView;
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let other_pos = BlockPos { x: 1, y: 0, z: 0 };
        let s = state(1, 1, vec![]);
        // The stub view returns block 0 / state 0, so a same-state check fails.
        assert!(!ConnectionType::State.should_connect(&view, &s, &s, &pos, &other_pos, Direction::North, &sprite(0)));

        // innerSeams variant: no connect -> false; connect with innerSeams moves the
        // mutable pos one step along the face and negates the second check.
        let mut mpos = BlockPos { x: 1, y: 0, z: 0 };
        let a = state(1, 1, vec![]);
        // The stub view's other state (block 0) differs from block 1 -> no connect.
        assert!(!ConnectionType::Block.should_connect_inner_seams(&view, &a, &a, &pos, &mut mpos, Direction::North, &sprite(0), true));

        // Drive the inner-seams path with a state the stub view agrees with: the
        // first check connects, innerSeams moves the mutable pos one step north and
        // negates the second (still-connecting) check -> false.
        let zero = state(0, 0, vec![]);
        assert!(!ConnectionType::Block.should_connect_inner_seams(&view, &zero, &zero, &pos, &mut mpos, Direction::North, &sprite(0), true));
        // After the call the mutable pos moved one step north.
        assert_eq!(mpos.z, -1);
        // Without innerSeams the first connect passes straight through.
        let mut mpos2 = BlockPos { x: 1, y: 0, z: 0 };
        assert!(ConnectionType::Block.should_connect_inner_seams(&view, &zero, &zero, &pos, &mut mpos2, Direction::North, &sprite(0), false));
    }

    #[test]
    fn math_util_and_random_providers() {
        // java: MathUtil.removeSignBit (the mask clears only the sign bit).
        assert_eq!(crate::math_util::remove_sign_bit(-1), 2147483647);
        assert_eq!(crate::math_util::remove_sign_bit(-2147483648), 0);
        // java: Unweighted % size.
        assert_eq!(UnweightedFactory::INSTANCE.create_index_provider(3).get_random_index(-4), 1);
        // java: Weighted walks weights with the maxIndex short-circuit.
        let weighted = WeightedFactory::new(vec![2, 1]).create_index_provider(2);
        assert_eq!(weighted.get_random_index(0), 0);
        assert_eq!(weighted.get_random_index(2), 1);
        assert_eq!(weighted.get_random_index(5), 1);
    }
}

// rust carrying: the erased CtmLoaderObj factories downcast the shared properties object
// back to the loader's type through these impls.

impl crate::properties_loader::CtmProperties for BasicConnectingCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl crate::properties_loader::CtmProperties for ConnectingCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl crate::properties_loader::CtmProperties for OrientedConnectingCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.base.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.base.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base.base.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl crate::properties_loader::CtmProperties for CompactConnectingCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.base.base.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.base.base.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base.base.base.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl crate::properties_loader::CtmProperties for RandomCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl crate::properties_loader::CtmProperties for RepeatCtmProperties {
    fn get_texture_dependencies(
        &mut self,
    ) -> std::collections::HashSet<crate::seams::SpriteId> {
        self.base.get_texture_dependencies()
    }

    fn compare_to(&self, other: &dyn crate::properties_loader::CtmProperties) -> std::cmp::Ordering {
        self.base.compare_to(other)
    }

    fn as_base(&self) -> Option<&BaseCtmProperties> {
        Some(&self.base)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// rust carrying: AbstractProcessorProps for the properties types the loader registrations
// wrap that do not have one yet.

impl crate::processor_base::AbstractProcessorProps for CompactConnectingCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base.base
    }
}

impl crate::processor_base::AbstractProcessorProps for RandomCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

impl crate::processor_base::AbstractProcessorProps for RepeatCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}
