//! 1:1 of the Continuity client.properties.overlay package, by PepperCode1
//! (LGPL-3.0-only upstream; the exact 1:1 port carries the upstream license and credit).

use std::collections::HashSet;
use std::rc::Rc;

use crate::connecting_properties::{
    BasicConnectingCtmProperties, OrientedConnectingCtmProperties, RandomCtmProperties,
    RepeatCtmProperties,
};
use crate::processor_enums::OrientationMode;
use crate::connecting_properties::CtmProperties;
use crate::properties::BaseCtmProperties;
use crate::parsing::{get, parse_block_states, parse_match_tiles, BlockStateMatcher, Properties};
use crate::processor_base::BlendMode;
use crate::seams::RedirectHandler;
use crate::seams::{
    identifier_from_single, registry_block_get_default_state, BlockState, Identifier,
};

/// java: client.properties.overlay.OverlayPropertiesSection.
pub struct OverlayPropertiesSection {
    properties: Properties,
    resource_id: Identifier,
    pack_id: String,

    /// java: protected int tintIndex = -1.
    tint_index: i32,
    /// java: @Nullable protected BlockState tintBlock.
    tint_block: Option<BlockState>,
    /// java: protected BlendMode layer = BlendMode.CUTOUT_MIPPED.
    layer: BlendMode,
}

impl OverlayPropertiesSection {
    /// java: OverlayPropertiesSection(Properties properties, Identifier resourceId, String packId).
    pub fn new(properties: Properties, resource_id: Identifier, pack_id: String) -> OverlayPropertiesSection {
        OverlayPropertiesSection {
            properties,
            resource_id,
            pack_id,
            tint_index: -1,
            tint_block: None,
            layer: BlendMode::CutoutMipped,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.parse_tint_index();
        self.parse_tint_block();
        self.parse_layer();
    }

    /// java: parseTintIndex().
    fn parse_tint_index(&mut self) {
        let Some(tint_index_str) = get(&self.properties, "tintIndex") else {
            return;
        };
        match tint_index_str.trim().parse::<i32>() {
            Ok(tint_index) if tint_index >= 0 => {
                self.tint_index = tint_index;
            }
            _ => {
                // java: NumberFormatException and a negative index both land in the warn.
                eprintln!(
                    "[continuity] Invalid 'tintIndex' value '{}' in file '{}' in pack '{}'",
                    tint_index_str, self.resource_id, self.pack_id
                );
            }
        }
    }

    /// java: parseTintBlock().
    fn parse_tint_block(&mut self) {
        let Some(tint_block_str) = get(&self.properties, "tintBlock") else {
            return;
        };

        let parts: Vec<&str> = tint_block_str.trim().splitn(3, ':').collect();
        // java: parts.length != 0 always holds for the split; an unparseable or empty
        // leading part throws InvalidIdentifierException into the same warn below.
        let block_id: Option<Identifier> = if parts.len() == 1 || parts[1].contains('=') {
            // java: new Identifier(parts[0]).
            identifier_from_single(parts[0])
        } else {
            Identifier::new(parts[0], parts[1]).ok()
        };

        match block_id {
            Some(block_id) => match registry_block_get_default_state(&block_id) {
                Some(block) => {
                    self.tint_block = Some(block);
                }
                None => {
                    eprintln!(
                        "[continuity] Unknown block '{}' in 'tintBlock' value '{}' in file '{}' in pack '{}'",
                        block_id, tint_block_str, self.resource_id, self.pack_id
                    );
                }
            },
            None => {
                eprintln!(
                    "[continuity] Invalid 'tintBlock' value '{}' in file '{}' in pack '{}'",
                    tint_block_str, self.resource_id, self.pack_id
                );
            }
        }
    }

    /// java: parseLayer().
    fn parse_layer(&mut self) {
        let Some(layer_str) = get(&self.properties, "layer") else {
            return;
        };
        let parsed_layer = match layer_str.trim().to_lowercase().as_str() {
            "cutout_mipped" => Some(BlendMode::CutoutMipped),
            "cutout" => Some(BlendMode::Cutout),
            "translucent" => Some(BlendMode::Translucent),
            _ => None,
        };
        match parsed_layer {
            Some(layer) => {
                self.layer = layer;
            }
            None => {
                // java: the missing closing quote after the value is upstream's typo,
                // carried verbatim.
                eprintln!(
                    "[continuity] Unknown 'layer' value '{} in file '{}' in pack '{}'",
                    layer_str, self.resource_id, self.pack_id
                );
            }
        }
    }

    /// java: getTintIndex().
    pub fn get_tint_index(&self) -> i32 {
        self.tint_index
    }

    /// java: @Nullable getTintBlock().
    pub fn get_tint_block(&self) -> Option<&BlockState> {
        self.tint_block.as_ref()
    }

    /// java: getLayer().
    pub fn get_layer(&self) -> BlendMode {
        self.layer
    }
}

/// java: client.properties.overlay.StandardOverlayCtmProperties extends
/// BasicConnectingCtmProperties implements OverlayPropertiesSection.Provider.
pub struct StandardOverlayCtmProperties {
    pub base: BasicConnectingCtmProperties,
    pub overlay_section: OverlayPropertiesSection,
    /// java: @Nullable protected Set<Identifier> connectTilesSet.
    pub connect_tiles_set: Option<HashSet<Identifier>>,
    /// java: @Nullable protected Predicate<BlockState> connectBlocksPredicate, carried
    /// as the compiled matcher.
    pub connect_blocks_predicate: Option<BlockStateMatcher>,
}

impl StandardOverlayCtmProperties {
    /// java: StandardOverlayCtmProperties(properties, resourceId, packId, packPriority,
    /// resourceManager, method).
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> StandardOverlayCtmProperties {
        // rust carrying: java shares one Properties object; the port clones it into
        // the overlay section (a HashMap value table).
        let overlay_section = OverlayPropertiesSection::new(properties.clone(), resource_id.clone(), pack_id.clone());
        StandardOverlayCtmProperties {
            base: BasicConnectingCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            overlay_section,
            connect_tiles_set: None,
            connect_blocks_predicate: None,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.overlay_section.init();
        self.parse_connect_tiles();
        self.parse_connect_blocks();
    }

    /// java: parseConnectTiles(resourceManager).
    fn parse_connect_tiles(&mut self) {
        // java: PropertiesParsingHelper.parseMatchTiles(properties, "connectTiles", resourceId,
        // packId, ResourceRedirectHandler.get(resourceManager)).
        self.connect_tiles_set = parse_match_tiles(
            &self.base.base.properties,
            "connectTiles",
            &self.base.base.resource_id,
            &self.base.base.pack_id,
            self.base.base.redirect_handler.as_deref(),
        );
    }

    /// java: parseConnectBlocks().
    fn parse_connect_blocks(&mut self) {
        self.connect_blocks_predicate = parse_block_states(
            &self.base.base.properties,
            "connectBlocks",
            &self.base.base.resource_id,
            &self.base.base.pack_id,
        );
    }

    /// java: @Nullable getConnectTilesSet().
    pub fn get_connect_tiles_set(&self) -> Option<&HashSet<Identifier>> {
        self.connect_tiles_set.as_ref()
    }

    /// java: @Nullable getConnectBlocksPredicate().
    pub fn get_connect_blocks_predicate(&self) -> Option<&BlockStateMatcher> {
        self.connect_blocks_predicate.as_ref()
    }
}

/// java: OverlayPropertiesSection.Provider.
pub trait OverlayPropertiesSectionProvider {
    /// java: OverlayPropertiesSection getOverlayPropertiesSection().
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection;
}

impl OverlayPropertiesSectionProvider for StandardOverlayCtmProperties {
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection {
        &self.overlay_section
    }
}

impl crate::processor_base::AbstractProcessorProps for StandardOverlayCtmProperties {
    fn base(&self) -> &crate::properties::BaseCtmProperties {
        &self.base.base
    }
}

/// java: client.properties.overlay.BaseOverlayCtmProperties extends BaseCtmProperties
/// implements OverlayPropertiesSection.Provider.
pub struct BaseOverlayCtmProperties {
    pub base: BaseCtmProperties,
    pub overlay_section: OverlayPropertiesSection,
}

impl BaseOverlayCtmProperties {
    /// java: BaseOverlayCtmProperties(properties, resourceId, pack, packPriority,
    /// resourceManager, method).
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> BaseOverlayCtmProperties {
        // rust carrying: java shares one Properties object; the port clones it into
        // the overlay section (a HashMap value table).
        let overlay_section = OverlayPropertiesSection::new(properties.clone(), resource_id.clone(), pack_id.clone());
        BaseOverlayCtmProperties {
            base: BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            overlay_section,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.overlay_section.init();
    }
}

impl CtmProperties for BaseOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

impl OverlayPropertiesSectionProvider for BaseOverlayCtmProperties {
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection {
        &self.overlay_section
    }
}

/// java: client.properties.overlay.OrientedConnectingOverlayCtmProperties extends
/// OrientedConnectingCtmProperties implements OverlayPropertiesSection.Provider.
pub struct OrientedConnectingOverlayCtmProperties {
    pub base: OrientedConnectingCtmProperties,
    pub overlay_section: OverlayPropertiesSection,
}

impl OrientedConnectingOverlayCtmProperties {
    /// java: OrientedConnectingOverlayCtmProperties(properties, resourceId, pack,
    /// packPriority, resourceManager, method, defaultOrientationMode).
    pub fn with_default_orientation_mode(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
        default_orientation_mode: OrientationMode,
    ) -> OrientedConnectingOverlayCtmProperties {
        let overlay_section = OverlayPropertiesSection::new(properties.clone(), resource_id.clone(), pack_id.clone());
        OrientedConnectingOverlayCtmProperties {
            base: OrientedConnectingCtmProperties::with_default_orientation_mode(
                properties,
                resource_id,
                pack_id,
                pack_priority,
                redirect_handler,
                method,
                default_orientation_mode,
            ),
            overlay_section,
        }
    }

    /// java: OrientedConnectingOverlayCtmProperties(properties, resourceId, pack,
    /// packPriority, resourceManager, method) delegating to OrientationMode.NONE.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> OrientedConnectingOverlayCtmProperties {
        Self::with_default_orientation_mode(
            properties,
            resource_id,
            pack_id,
            pack_priority,
            redirect_handler,
            method,
            OrientationMode::None,
        )
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.overlay_section.init();
    }
}

impl CtmProperties for OrientedConnectingOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base.base
    }
}

impl OverlayPropertiesSectionProvider for OrientedConnectingOverlayCtmProperties {
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection {
        &self.overlay_section
    }
}

/// java: client.properties.overlay.RandomOverlayCtmProperties extends
/// RandomCtmProperties implements OverlayPropertiesSection.Provider.
pub struct RandomOverlayCtmProperties {
    pub base: RandomCtmProperties,
    pub overlay_section: OverlayPropertiesSection,
}

impl RandomOverlayCtmProperties {
    /// java: RandomOverlayCtmProperties(properties, resourceId, pack, packPriority,
    /// resourceManager, method).
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> RandomOverlayCtmProperties {
        let overlay_section = OverlayPropertiesSection::new(properties.clone(), resource_id.clone(), pack_id.clone());
        RandomOverlayCtmProperties {
            base: RandomCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            overlay_section,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.overlay_section.init();
    }
}

impl CtmProperties for RandomOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

impl OverlayPropertiesSectionProvider for RandomOverlayCtmProperties {
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection {
        &self.overlay_section
    }
}

/// java: client.properties.overlay.RepeatOverlayCtmProperties extends
/// RepeatCtmProperties implements OverlayPropertiesSection.Provider.
pub struct RepeatOverlayCtmProperties {
    pub base: RepeatCtmProperties,
    pub overlay_section: OverlayPropertiesSection,
}

impl RepeatOverlayCtmProperties {
    /// java: RepeatOverlayCtmProperties(properties, resourceId, pack, packPriority,
    /// resourceManager, method).
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> RepeatOverlayCtmProperties {
        let overlay_section = OverlayPropertiesSection::new(properties.clone(), resource_id.clone(), pack_id.clone());
        RepeatOverlayCtmProperties {
            base: RepeatCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method),
            overlay_section,
        }
    }

    /// java: init().
    pub fn init(&mut self) {
        self.base.init();
        self.overlay_section.init();
    }
}

impl CtmProperties for RepeatOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

impl OverlayPropertiesSectionProvider for RepeatOverlayCtmProperties {
    fn get_overlay_properties_section(&self) -> &OverlayPropertiesSection {
        &self.overlay_section
    }
}

// rust carrying: StandardOverlayCtmProperties extends BasicConnectingCtmProperties, so it
// needs the connecting-trait impl the other subclasses carry.

impl CtmProperties for StandardOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

// rust carrying: the erased CtmLoaderObj factories downcast through these impls.

impl crate::properties_loader::CtmProperties for StandardOverlayCtmProperties {
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

impl crate::properties_loader::CtmProperties for BaseOverlayCtmProperties {
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

impl crate::properties_loader::CtmProperties for OrientedConnectingOverlayCtmProperties {
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

impl crate::properties_loader::CtmProperties for RandomOverlayCtmProperties {
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

impl crate::properties_loader::CtmProperties for RepeatOverlayCtmProperties {
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

// rust carrying: AbstractProcessorProps for the overlay types that do not have one yet.

impl crate::processor_base::AbstractProcessorProps for BaseOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base
    }
}

impl crate::processor_base::AbstractProcessorProps for OrientedConnectingOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base.base.base
    }
}

impl crate::processor_base::AbstractProcessorProps for RandomOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

impl crate::processor_base::AbstractProcessorProps for RepeatOverlayCtmProperties {
    fn base(&self) -> &BaseCtmProperties {
        &self.base.base
    }
}

// rust carrying: the shared repeat validator also covers the overlay variant, reached
// through the contained RepeatCtmProperties.

impl crate::connecting_properties::TileAmountValidator<RepeatOverlayCtmProperties>
    for crate::connecting_properties::RepeatValidator
{
    fn validate_tile_amount(&self, amount: usize, properties: &RepeatOverlayCtmProperties) -> bool {
        crate::connecting_properties::TileAmountValidator::<
            crate::connecting_properties::RepeatCtmProperties,
        >::validate_tile_amount(self, amount, &properties.base)
    }
}
