//! 1:1 of the Continuity client.resource.CustomBlockLayers, by PepperCode1
//! (LGPL-3.0).

use std::cell::RefCell;

use crate::parsing::{parse_block_states, BlockStateMatcher, Properties};
use crate::seams::{
    block_state_is_opaque_full_cube, get_resource, parse_properties_stream, BlockRenderView,
    BlockPos, BlockState, Identifier, ResourceManager,
};

fn warn(msg: &str) {
    // java: ContinuityClient.LOGGER.error(...).
    eprintln!("[continuity] {msg}");
}

/// java: CustomBlockLayers.LOCATION = new Identifier("optifine/block.properties").
pub fn location() -> Identifier {
    Identifier::new("optifine", "block.properties").unwrap()
}

/// java: private enum BlockLayer over the vanilla RenderLayers. The enum value is
/// the port's render-layer surface; the engine seam maps it onto its layers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockLayer {
    Solid,
    Cutout,
    CutoutMipped,
    Translucent,
}

impl BlockLayer {
    /// java: public static final BlockLayer[] VALUES = values().
    pub const VALUES: [BlockLayer; 4] = [
        BlockLayer::Solid,
        BlockLayer::Cutout,
        BlockLayer::CutoutMipped,
        BlockLayer::Translucent,
    ];

    /// java: key = name().toLowerCase(Locale.ROOT).
    pub fn key(&self) -> &'static str {
        match self {
            BlockLayer::Solid => "solid",
            BlockLayer::Cutout => "cutout",
            BlockLayer::CutoutMipped => "cutout_mipped",
            BlockLayer::Translucent => "translucent",
        }
    }

    /// java: getLayer().
    pub fn get_layer(&self) -> BlockLayer {
        *self
    }
}

struct CustomBlockLayersState {
    /// java: Predicate<BlockState>[] LAYER_PREDICATES.
    layer_predicates: [Option<BlockStateMatcher>; 4],
    empty: bool,
    disable_solid_check: bool,
}

impl Default for CustomBlockLayersState {
    fn default() -> Self {
        CustomBlockLayersState {
            layer_predicates: [None, None, None, None],
            empty: true,
            disable_solid_check: false,
        }
    }
}

thread_local! {
    static STATE: RefCell<CustomBlockLayersState> = RefCell::new(CustomBlockLayersState::default());
}

/// java: CustomBlockLayers.isEmpty().
pub fn is_empty() -> bool {
    STATE.with(|state| state.borrow().empty)
}

/// java: CustomBlockLayers.getLayer(state) - @Nullable RenderLayer.
pub fn get_layer(state: &BlockState) -> Option<BlockLayer> {
    STATE.with(|cell| {
        let state_cell = cell.borrow();
        // java: if (!disableSolidCheck) { if (state.isOpaqueFullCube(EmptyBlockView.INSTANCE,
        // BlockPos.ORIGIN)) return null; }
        if !state_cell.disable_solid_check
            && block_state_is_opaque_full_cube(state, &BlockRenderView, &BlockPos { x: 0, y: 0, z: 0 })
        {
            return None;
        }

        // java: for (int i = 0; i < BlockLayer.VALUES.length; i++) { if
        // (LAYER_PREDICATES[i] != null && LAYER_PREDICATES[i].test(state)) return
        // BlockLayer.VALUES[i].getLayer(); }
        for (i, layer) in BlockLayer::VALUES.iter().enumerate() {
            if let Some(predicate) = &state_cell.layer_predicates[i] {
                if predicate.test(state) {
                    return Some(layer.get_layer());
                }
            }
        }
        None
    })
}

/// java: private static void reload(ResourceManager manager).
pub fn reload(_resource_manager: &ResourceManager) {
    // java: empty = true; System.arraycopy(EMPTY_LAYER_PREDICATES, 0, LAYER_PREDICATES,
    // 0, EMPTY_LAYER_PREDICATES.length); disableSolidCheck = false.
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.empty = true;
        state.layer_predicates = [None, None, None, None];
        state.disable_solid_check = false;
    });

    // java: Optional<Resource> optionalResource = manager.getResource(LOCATION);
    // if (optionalResource.isPresent()) { Resource resource = optionalResource.get();
    // try (InputStream inputStream = resource.getInputStream()) { Properties properties
    // = new Properties(); properties.load(inputStream); reload(properties, LOCATION,
    // resource.getResourcePackName()); } catch (IOException e) { LOGGER.error(...); } }
    if let Some(resource) = get_resource(&location()) {
        match crate::seams::resource_get_input_stream(&resource) {
            Some(contents) => {
                let properties = parse_properties_stream(&contents);
                reload_properties(&properties, &location(), &resource.pack_name);
            }
            None => {
                warn(&format!(
                    "Failed to load custom block layers from file '{}' from pack '{}'",
                    location().path, resource.pack_name
                ));
            }
        }
    }
}

/// java: private static void reload(Properties properties, Identifier fileLocation,
/// String packId).
pub fn reload_properties(properties: &Properties, file_location: &Identifier, pack_id: &str) {
    // java: for (BlockLayer blockLayer : BlockLayer.VALUES) { String propertyKey =
    // "layer." + blockLayer.getKey(); Predicate<BlockState> predicate =
    // PropertiesParsingHelper.parseBlockStates(properties, propertyKey, fileLocation,
    // packId); if (predicate != null && predicate !=
    // PropertiesParsingHelper.EMPTY_BLOCK_STATE_PREDICATE) {
    // LAYER_PREDICATES[blockLayer.ordinal()] = predicate; empty = false; } }
    for (i, layer) in BlockLayer::VALUES.iter().enumerate() {
        let property_key = format!("layer.{}", layer.key());
        let predicate = parse_block_states(properties, &property_key, file_location, pack_id);
        if let Some(predicate) = predicate {
            // java: the EMPTY_BLOCK_STATE_PREDICATE identity check: the parser's
            // empty result matches nothing.
            let is_empty_predicate =
                predicate.blocks.is_empty() && predicate.property_maps.is_empty();
            if !is_empty_predicate {
                STATE.with(|cell| {
                    let mut state = cell.borrow_mut();
                    state.layer_predicates[i] = Some(predicate);
                    state.empty = false;
                });
            }
        }
    }

    // java: String disableSolidCheckStr = properties.getProperty("disableSolidCheck");
    // if (disableSolidCheckStr != null) disableSolidCheck =
    // Boolean.parseBoolean(disableSolidCheckStr.trim());
    if let Some(disable_solid_check_str) = properties.get("disableSolidCheck") {
        let disable_solid_check = disable_solid_check_str.trim().eq_ignore_ascii_case("true");
        STATE.with(|cell| {
            cell.borrow_mut().disable_solid_check = disable_solid_check;
        });
    }
}

/// java: CustomBlockLayers.ReloadListener (SimpleSynchronousResourceReloadListener).
pub struct ReloadListener;

impl ReloadListener {
    /// java: ReloadListener.ID = ContinuityClient.asId("custom_block_layers").
    pub fn id() -> Identifier {
        Identifier::new("continuity", "custom_block_layers").unwrap()
    }

    /// java: ReloadListener.init() - registers INSTANCE with the client resource
    /// reload listeners.
    pub fn init() {
        crate::seams::register_client_reload_listener(
            Self::id(),
            std::rc::Rc::new(|manager: &ResourceManager| {
                reload(manager);
            }),
        );
    }

    /// java: ReloadListener.reload(manager).
    pub fn reload(manager: &ResourceManager) {
        reload(manager);
    }

    /// java: ReloadListener.getFabricId().
    pub fn get_fabric_id() -> Identifier {
        Self::id()
    }
}

#[cfg(test)]
mod custom_block_layers_tests {
    use super::*;
    use crate::seams::{set_engine_block_id_hook, set_engine_registry_blocks, BlockEntry};
    use std::collections::HashMap;

    fn block_id(path: &str) -> Identifier {
        Identifier::new("minecraft", path).unwrap()
    }

    fn state(id: u64) -> BlockState {
        BlockState { block_id: id, state_id: id, sprites: Vec::new() }
    }

    fn reset() {
        // java: the manager-level reload is what resets the statics.
        reload(&ResourceManager::default());
        set_engine_registry_blocks(None);
        set_engine_block_id_hook(None);
    }

    #[test]
    fn empty_when_no_layer_properties() {
        reset();
        assert!(is_empty());
        assert!(get_layer(&state(1)).is_none());
    }

    #[test]
    fn layer_predicate_parsed_and_matched() {
        reset();
        let mut blocks = HashMap::new();
        blocks.insert(
            block_id("oak_planks"),
            BlockEntry { id: block_id("oak_planks"), properties: Vec::new(), property_values: HashMap::new() },
        );
        set_engine_registry_blocks(Some(blocks));

        let mut props = Properties::new();
        props.insert("layer.solid".to_string(), "minecraft:oak_planks".to_string());
        reload_properties(&props, &location(), "default");
        assert!(!is_empty());

        set_engine_block_id_hook(Some(block_id("oak_planks")));
        assert_eq!(get_layer(&state(1)), Some(BlockLayer::Solid));
        // java: a state on another block does not match the layer predicate.
        set_engine_block_id_hook(Some(block_id("stone")));
        assert!(get_layer(&state(2)).is_none());
        reset();
    }

    #[test]
    fn disable_solid_check_flag_round_trips() {
        reset();
        let mut props = Properties::new();
        props.insert("disableSolidCheck".to_string(), "true".to_string());
        reload_properties(&props, &location(), "default");
        STATE.with(|cell| assert!(cell.borrow().disable_solid_check));
        reset();
        STATE.with(|cell| assert!(!cell.borrow().disable_solid_check));
    }

    #[test]
    fn layer_keys_are_java_names_lowercased() {
        // java: key = name().toLowerCase(Locale.ROOT), cutout_mipped included.
        assert_eq!(BlockLayer::VALUES[0].key(), "solid");
        assert_eq!(BlockLayer::VALUES[1].key(), "cutout");
        assert_eq!(BlockLayer::VALUES[2].key(), "cutout_mipped");
        assert_eq!(BlockLayer::VALUES[3].key(), "translucent");
    }
}
