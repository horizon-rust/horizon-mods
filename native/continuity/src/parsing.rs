//! 1:1 of me.pepperbell.continuity.client.properties.PropertiesParsingHelper.

use std::collections::{HashMap, HashSet};

use crate::processor_enums::{OrientationMode, Symmetry};
use crate::seams::{filename_utils_path, registry_blocks, BlockEntry, Identifier, RedirectHandler};

/// java: EMPTY_BLOCK_STATE_PREDICATE = state -> false.
pub fn empty_block_state_predicate() -> bool {
    false
}

/// java: java.util.Properties (key/value string table).
pub type Properties = HashMap<String, String>;

pub(crate) fn get<'a>(properties: &'a Properties, key: &str) -> Option<&'a String> {
    properties.get(key)
}

fn warn(msg: &str) {
    // java: ContinuityClient.LOGGER.warn(...)
    eprintln!("[continuity] {msg}");
}

/// java: parseMatchTiles - @Nullable Set<Identifier>.
pub fn parse_match_tiles(
    properties: &Properties,
    property_key: &str,
    file_location: &Identifier,
    pack_id: &str,
    redirect_handler: Option<&dyn RedirectHandler>,
) -> Option<HashSet<Identifier>> {
    let match_tiles_str = get(properties, property_key)?;

    let match_tile_strs: Vec<&str> = match_tiles_str.trim().split(' ').collect();
    if match_tile_strs.is_empty() {
        return Some(HashSet::new());
    }

    let base_path = filename_utils_path(&file_location.path);
    let mut set: HashSet<Identifier> = HashSet::new();

    for (i, match_tile_str) in match_tile_strs.iter().enumerate() {
        if match_tile_str.is_empty() {
            continue;
        }

        // java: matchTileStr.split(":", 2) - at most two parts.
        let (mut namespace, mut path) = match match_tile_str.split_once(':') {
            Some((ns, p)) => (Some(ns.to_string()), p.to_string()),
            None => (None, match_tile_str.to_string()),
        };

        if path.ends_with(".png") {
            path.truncate(path.len() - 4);
        }

        if namespace.is_none() {
            if let Some(stripped) = path.strip_prefix("assets/minecraft/") {
                path = stripped.to_string();
            } else if let Some(stripped) = path.strip_prefix("./") {
                path = format!("{base_path}{stripped}");
            } else if let Some(stripped) = path.strip_prefix("~/") {
                path = format!("optifine/{stripped}");
            } else if let Some(stripped) = path.strip_prefix('/') {
                path = format!("optifine/{stripped}");
            }
        }

        if let Some(stripped) = path.strip_prefix("textures/") {
            path = stripped.to_string();
        } else if let Some(optifine_path) = path.strip_prefix("optifine/") {
            let handler = match redirect_handler {
                Some(handler) => handler,
                // java: if (redirectHandler == null) continue;
                None => continue,
            };
            path = handler
                .get_source_sprite_path(&format!("{optifine_path}.png"))
                .unwrap_or_default();
            if namespace.is_none() {
                namespace = Some(file_location.namespace.clone());
            }
        } else if !path.contains('/') {
            path = format!("block/{path}");
        }

        let namespace = namespace.unwrap_or_else(|| Identifier::DEFAULT_NAMESPACE.to_string());

        match Identifier::new(&namespace, &path) {
            Ok(id) => {
                set.insert(id);
            }
            Err(_) => warn(&format!(
                "Invalid '{property_key}' element '{match_tile_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
            )),
        }
    }

    Some(set)
}

/// java: parseBlockStates - the compiled predicate over the state surface.
#[derive(Clone, Debug, Default)]
pub struct BlockStateMatcher {
    /// java: blockSet - blocks matching outright.
    pub blocks: HashSet<Identifier>,
    /// java: propertyMaps - block id -> property name -> allowed values.
    pub property_maps: HashMap<Identifier, HashMap<String, Vec<String>>>,
}

impl BlockStateMatcher {
    /// java: the compiled Predicate<BlockState>.test(state): an entry matches when the
    /// state's block id is in the block set, or when the id carries a property map (the
    /// seam layer's state surface exposes no live properties yet, so a listed id with
    /// property constraints matches on the id alone until that seam lands).
    pub fn test(&self, state: &crate::seams::BlockState) -> bool {
        let block_id = crate::seams::block_state_get_block_id(state);
        if self.blocks.contains(&block_id) {
            return true;
        }
        self.property_maps.contains_key(&block_id)
    }
}

/// java: parseBlockStates - @Nullable Predicate<BlockState>, carried as the
/// compiled matcher; the seam layer tests a BlockState surface against it.
pub fn parse_block_states(
    properties: &Properties,
    property_key: &str,
    file_location: &Identifier,
    pack_id: &str,
) -> Option<BlockStateMatcher> {
    let block_states_str = get(properties, property_key)?;

    let block_state_strs: Vec<&str> = block_states_str.trim().split(' ').collect();
    if block_state_strs.is_empty() {
        return Some(BlockStateMatcher::default());
    }

    let mut block_set: HashSet<Identifier> = HashSet::new();
    let mut property_maps: HashMap<Identifier, HashMap<String, Vec<String>>> = HashMap::new();

    'block: for (i, part) in block_state_strs.iter().enumerate() {
        let block_state_str = part.trim();
        if block_state_str.is_empty() {
            continue;
        }

        let parts: Vec<&str> = block_state_str.split(':').collect();
        if parts.is_empty() {
            warn(&format!(
                "Invalid '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
            ));
            continue;
        }

        let (block_id, start_index) = if parts.len() == 1 || parts[1].contains('=') {
            match Identifier::parse(parts[0]) {
                Ok(id) => (id, 1),
                Err(_) => {
                    warn(&format!(
                        "Invalid '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                    ));
                    continue;
                }
            }
        } else {
            match Identifier::new(parts[0], parts[1]) {
                Ok(id) => (id, 2),
                Err(_) => {
                    warn(&format!(
                        "Invalid '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                    ));
                    continue;
                }
            }
        };

        // java: Registries.BLOCK.containsId / get.
        let blocks = registry_blocks();
        let entry: BlockEntry = match blocks.get(&block_id) {
            Some(entry) => entry.clone(),
            None => {
                warn(&format!(
                    "Unknown block '{block_id}' in '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                ));
                continue;
            }
        };

        if block_set.contains(&block_id) {
            continue;
        }

        if parts.len() > start_index {
            let mut property_map: HashMap<String, Vec<String>> = HashMap::new();

            for part in &parts[start_index..] {
                if part.is_empty() {
                    continue;
                }
                // java: part.split("=", 2)
                let (property_name, property_values_str) = match part.split_once('=') {
                    Some((n, v)) => (n.to_string(), v.to_string()),
                    None => {
                        warn(&format!(
                            "Invalid block property definition for block '{block_id}' in '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                        ));
                        continue 'block;
                    }
                };

                // java: block.getStateManager().getProperty(propertyName) == null.
                if !entry.properties.contains(&property_name) {
                    warn(&format!(
                        "Unknown block property '{property_name}' for block '{block_id}' in '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                    ));
                    continue 'block;
                }

                let property_value_strs: Vec<&str> = property_values_str.split(',').collect();
                if property_value_strs.is_empty() {
                    warn(&format!(
                        "Invalid block property definition for block '{block_id}' in '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                    ));
                    continue 'block;
                }

                let value_set = property_map.entry(property_name.clone()).or_default();
                for property_value_str in property_value_strs {
                    // java: property.parse(valueStr) -> Optional.
                    let parsed = entry.property_values.get(&property_name).map(|vs| vs.iter().any(|v| v == property_value_str));
                    match parsed {
                        Some(true) => value_set.push(property_value_str.to_string()),
                        _ => {
                            warn(&format!(
                                "Invalid block property value '{property_value_str}' for property '{property_name}' for block '{block_id}' in '{property_key}' element '{block_state_str}' at index {i} in file '{file_location}' in pack '{pack_id}'"
                            ));
                            continue 'block;
                        }
                    }
                }
            }

            if !property_map.is_empty() {
                match property_maps.remove(&block_id) {
                    None => {
                        property_maps.insert(block_id.clone(), property_map);
                    }
                    Some(existing) => {
                        let mut merged = existing;
                        for (property, value_set) in property_map {
                            merged.entry(property).or_default().extend(value_set);
                        }
                        property_maps.insert(block_id.clone(), merged);
                    }
                }
            }
        } else {
            block_set.insert(block_id.clone());
            property_maps.remove(&block_id);
        }
    }

    if block_set.is_empty() && property_maps.is_empty() {
        return Some(BlockStateMatcher::default());
    }

    Some(BlockStateMatcher { blocks: block_set, property_maps })
}

/// java: parseSymmetry - @Nullable Symmetry.
pub fn parse_symmetry(properties: &Properties, property_key: &str, file_location: &Identifier, pack_id: &str) -> Option<Symmetry> {
    let symmetry_str = get(properties, property_key)?;

    match Symmetry::parse(symmetry_str) {
        Some(symmetry) => Some(symmetry),
        None => {
            warn(&format!(
                "Unknown '{property_key}' value '{symmetry_str}' in file '{file_location}' in pack '{pack_id}'"
            ));
            None
        }
    }
}

/// java: parseOrientationMode - @Nullable OrientationMode.
pub fn parse_orientation_mode(properties: &Properties, property_key: &str, file_location: &Identifier, pack_id: &str) -> Option<OrientationMode> {
    let orientation_mode_str = get(properties, property_key)?;

    let parsed = match orientation_mode_str.trim().to_ascii_uppercase().as_str() {
        "NONE" => Some(OrientationMode::None),
        "STATE_AXIS" => Some(OrientationMode::StateAxis),
        "TEXTURE" => Some(OrientationMode::Texture),
        _ => None,
    };

    match parsed {
        Some(mode) => Some(mode),
        None => {
            warn(&format!(
                "Unknown '{property_key}' value '{orientation_mode_str}' in file '{file_location}' in pack '{pack_id}'"
            ));
            None
        }
    }
}

/// java: parseOptifineOnly.
pub fn parse_optifine_only(properties: &Properties, file_location: &Identifier) -> bool {
    if file_location.namespace != Identifier::DEFAULT_NAMESPACE {
        return false;
    }

    match get(properties, "optifineOnly") {
        None => false,
        Some(optifine_only_str) => optifine_only_str.trim() == "true",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> Identifier {
        Identifier::new("minecraft", "optifine/ctm/test.properties").unwrap()
    }

    #[test]
    fn match_tiles_split_and_default_namespace() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), " stone . ".into());
        let out = parse_match_tiles(&p, "matchTiles", &loc(), "pack", None).unwrap();
        assert!(out.contains(&Identifier::new("minecraft", "block/stone").unwrap()));
        assert!(out.contains(&Identifier::new("minecraft", "block/.").unwrap()));
    }

    #[test]
    fn match_tiles_strips_textures_prefix_and_png() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "textures/block/grass.png".into());
        let out = parse_match_tiles(&p, "matchTiles", &loc(), "pack", None).unwrap();
        assert!(out.contains(&Identifier::new("minecraft", "block/grass").unwrap()));
    }

    #[test]
    fn match_tiles_optifine_without_redirect_is_skipped() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "~/glass".into());
        let out = parse_match_tiles(&p, "matchTiles", &loc(), "pack", None).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn optifine_only_gated_on_default_namespace() {
        let mut p = Properties::new();
        p.insert("optifineOnly".into(), "true".into());
        assert!(parse_optifine_only(&p, &loc()));
        let other = Identifier::new("other", "x.properties").unwrap();
        assert!(!parse_optifine_only(&p, &other));
    }

    #[test]
    fn symmetry_and_orientation_parse() {
        let mut p = Properties::new();
        p.insert("symmetry".into(), " all ".into());
        assert_eq!(parse_symmetry(&p, "symmetry", &loc(), "pack"), Some(Symmetry::All));
        p.insert("orientationMode".into(), "STATE_AXIS".into());
        assert_eq!(parse_orientation_mode(&p, "orientationMode", &loc(), "pack"), Some(OrientationMode::StateAxis));
        p.insert("orientationMode".into(), "bogus".into());
        assert_eq!(parse_orientation_mode(&p, "orientationMode", &loc(), "pack"), None);
    }
}
