//! 1:1 of me.pepperbell.continuity.client.properties.BaseCtmProperties.

use std::collections::HashSet;
use std::rc::Rc;

use crate::parsing::{parse_block_states, parse_match_tiles, BlockStateMatcher, Properties};
use crate::processor_enums::Direction;
use crate::seams::{
    default_resource_pack_is, filename_utils_base_name, filename_utils_path, get_or_create_biome_holder,
    get_resource, missing_sprite_id, pattern_quote, regex_matches, to_sprite_id, Identifier, RedirectHandler,
    SpriteId,
};

pub const DIRECTION_AMOUNT: usize = 6;

/// java: IntPredicate lambdas over y, carried as the parsed shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightPredicate {
    Exact(i32),
    Range(i32, i32),
    AtLeast(i32),
    AtMost(i32),
}

impl HeightPredicate {
    pub fn test(self, y: i32) -> bool {
        match self {
            HeightPredicate::Exact(v) => y == v,
            HeightPredicate::Range(min, max) => y >= min && y <= max,
            HeightPredicate::AtLeast(min) => y >= min,
            HeightPredicate::AtMost(max) => y <= max,
        }
    }
}

/// java: BiomeSetPredicate over BiomeHolders, with negate applied.
#[derive(Clone, Debug, Default)]
pub struct BiomePredicate {
    pub holders: HashSet<Identifier>,
    pub negate: bool,
}

/// java: the blockEntityName predicate, equality or regex forms.
#[derive(Clone, Debug)]
pub enum NamePredicate {
    Equals(String),
    Pattern { pattern: String, case_insensitive: bool },
}

impl NamePredicate {
    pub fn test(&self, block_entity_name: &str) -> bool {
        match self {
            NamePredicate::Equals(s) => s == block_entity_name,
            NamePredicate::Pattern { pattern, case_insensitive } => {
                regex_matches(pattern, *case_insensitive, block_entity_name)
            }
        }
    }
}

/// java: split(str) with java.util.String.split trailing-empty removal.
fn split_trailing_empty<'a>(s: &'a str, sep: char) -> Vec<&'a str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while let Some(last) = parts.last() {
        if last.is_empty() {
            parts.pop();
        } else {
            break;
        }
    }
    parts
}

fn split_on<'a>(s: &'a str, seps: &[char]) -> Vec<&'a str> {
    let mut parts: Vec<&str> = s.split(|c| seps.contains(&c)).collect();
    while let Some(last) = parts.last() {
        if last.is_empty() {
            parts.pop();
        } else {
            break;
        }
    }
    parts
}

/// java: public class BaseCtmProperties implements CtmProperties.
pub struct BaseCtmProperties {
    pub properties: Properties,
    pub resource_id: Identifier,
    pub pack_id: String,
    pub pack_priority: i32,
    pub method: String,

    pub match_tiles_set: Option<HashSet<Identifier>>,
    pub match_blocks_predicate: Option<BlockStateMatcher>,
    pub tiles: Vec<Identifier>,
    /// java: null means all faces.
    pub faces: Option<HashSet<Direction>>,
    pub biome_predicate: Option<BiomePredicate>,
    pub height_predicate: Option<Vec<HeightPredicate>>,
    pub block_entity_name_predicate: Option<NamePredicate>,

    pub prioritized: bool,
    pub valid: bool,
    pub texture_dependencies: Option<HashSet<SpriteId>>,
    pub sprite_ids: Option<Vec<SpriteId>>,

    pub(crate) redirect_handler: Option<Rc<dyn RedirectHandler>>,
}

impl BaseCtmProperties {
    /// java: constructor stores the parse inputs.
    pub fn new(
        properties: Properties,
        resource_id: Identifier,
        pack_id: String,
        pack_priority: i32,
        redirect_handler: Option<Rc<dyn RedirectHandler>>,
        method: String,
    ) -> BaseCtmProperties {
        BaseCtmProperties {
            properties,
            resource_id,
            pack_id,
            pack_priority,
            method,
            match_tiles_set: None,
            match_blocks_predicate: None,
            tiles: Vec::new(),
            faces: None,
            biome_predicate: None,
            height_predicate: None,
            block_entity_name_predicate: None,
            prioritized: false,
            valid: true,
            texture_dependencies: None,
            sprite_ids: None,
            redirect_handler,
        }
    }

    /// java: special/skip and special/default, via ContinuityClient.asId.
    pub fn special_skip_id() -> Identifier {
        Identifier::new("minecraft", "special/skip").unwrap()
    }

    pub fn special_default_id() -> Identifier {
        Identifier::new("minecraft", "special/default").unwrap()
    }

    pub fn special_skip_sprite_id() -> SpriteId {
        to_sprite_id(Self::special_skip_id())
    }

    pub fn special_default_sprite_id() -> SpriteId {
        to_sprite_id(Self::special_default_id())
    }

    /// java: init() - the fixed parse order.
    pub fn init(&mut self) {
        self.parse_match_tiles();
        self.parse_match_blocks();
        self.detect_matches();
        self.validate_matches();
        self.parse_tiles();
        self.parse_faces();
        self.parse_biomes();
        self.parse_heights();
        self.parse_legacy_heights();
        self.parse_name();
        self.parse_prioritize();
        self.parse_resource_condition();
    }

    fn parse_match_tiles(&mut self) {
        self.match_tiles_set = parse_match_tiles(
            &self.properties,
            "matchTiles",
            &self.resource_id,
            &self.pack_id,
            self.redirect_handler.as_deref(),
        );
        if self.match_tiles_set.as_ref().is_some_and(|s| s.is_empty()) {
            self.valid = false;
        }
    }

    fn parse_match_blocks(&mut self) {
        self.match_blocks_predicate = parse_block_states(&self.properties, "matchBlocks", &self.resource_id, &self.pack_id);
        if self
            .match_blocks_predicate
            .as_ref()
            .is_some_and(|m| m.blocks.is_empty() && m.property_maps.is_empty())
        {
            // java: matchBlocksPredicate == EMPTY_BLOCK_STATE_PREDICATE.
            self.valid = false;
        }
    }

    fn detect_matches(&mut self) {
        if self.match_blocks_predicate.is_none() {
            let base_name = filename_utils_base_name(&self.resource_id.path);
            if let Some(rest) = base_name.strip_prefix("block_") {
                if let Ok(id) = Identifier::parse(rest) {
                    let blocks = crate::seams::registry_blocks();
                    if blocks.contains_key(&id) {
                        let mut m = BlockStateMatcher::default();
                        m.blocks.insert(id);
                        self.match_blocks_predicate = Some(m);
                    }
                }
            }
        }
    }

    fn validate_matches(&mut self) {
        if self.match_tiles_set.is_none() && self.match_blocks_predicate.is_none() {
            eprintln!(
                "[continuity] No tile or block matches provided in file '{}' in pack '{}'",
                self.resource_id, self.pack_id
            );
            self.valid = false;
        }
    }

    fn parse_tiles(&mut self) {
        let tiles_str = match self.properties.get("tiles") {
            None => {
                eprintln!(
                    "[continuity] No 'tiles' value provided in file '{}' in pack '{}'",
                    self.resource_id, self.pack_id
                );
                self.valid = false;
                return;
            }
            Some(s) => s.clone(),
        };

        let tile_strs = split_on(&tiles_str, &[' ', ',']);
        if tile_strs.is_empty() {
            return;
        }

        let base_path = filename_utils_path(&self.resource_id.path);
        let mut tiles: Vec<Identifier> = Vec::new();

        for (i, tile_str) in tile_strs.iter().enumerate() {
            if tile_str.is_empty() {
                continue;
            }

            if tile_str.ends_with("<skip>") || tile_str.ends_with("<skip>.png") {
                tiles.push(Self::special_skip_id());
                continue;
            } else if tile_str.ends_with("<default>") || tile_str.ends_with("<default>.png") {
                tiles.push(Self::special_default_id());
                continue;
            }

            // java: tileStr.split("-", 2); a non-numeric half falls through.
            let mut range_done = false;
            if let Some((min_str, max_str)) = tile_str.split_once('-') {
                if let (Ok(min), Ok(max)) = (min_str.parse::<i32>(), max_str.parse::<i32>()) {
                    if min <= max {
                        for tile in min..=max {
                            match Identifier::parse(&format!("{}{}.png", base_path, tile)) {
                                Ok(id) => tiles.push(id),
                                Err(_) => eprintln!(
                                    "[continuity] Invalid 'tiles' element '{}' at index {} in file '{}' in pack '{}'",
                                    tile_str, i, self.resource_id, self.pack_id
                                ),
                            }
                        }
                    } else {
                        eprintln!(
                            "[continuity] Invalid 'tiles' element '{}' at index {} in file '{}' in pack '{}'",
                            tile_str, i, self.resource_id, self.pack_id
                        );
                    }
                    range_done = true;
                }
            }
            if range_done {
                continue;
            }

            let (mut namespace, mut path) = match tile_str.split_once(':') {
                Some((ns, p)) => (Some(ns.to_string()), p.to_string()),
                None => (None, tile_str.to_string()),
            };

            if !path.ends_with(".png") {
                path.push_str(".png");
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

                if !path.starts_with("textures/") && !path.starts_with("optifine/") {
                    path = format!("{base_path}{path}");
                }

                if path.starts_with("optifine/") {
                    namespace = Some(self.resource_id.namespace.clone());
                }
            } else {
                if !path.contains('/') {
                    path = format!("textures/block/{path}");
                } else if !path.starts_with("textures/") && !path.starts_with("optifine/") {
                    path = format!("textures/{path}");
                }
            }

            let namespace = namespace.unwrap_or_else(|| Identifier::DEFAULT_NAMESPACE.to_string());

            match Identifier::new(&namespace, &path) {
                Ok(id) => tiles.push(id),
                Err(_) => eprintln!(
                    "[continuity] Invalid 'tiles' element '{}' at index {} in file '{}' in pack '{}'",
                    tile_str, i, self.resource_id, self.pack_id
                ),
            }
        }

        self.tiles = tiles;
    }

    fn parse_faces(&mut self) {
        let faces_str = match self.properties.get("faces") {
            None => return,
            Some(s) => s.clone(),
        };

        let face_strs = split_on(&faces_str, &[' ', ',']);
        if face_strs.is_empty() {
            self.valid = false;
            return;
        }

        let mut faces: HashSet<Direction> = HashSet::new();

        for (i, face_str) in face_strs.iter().enumerate() {
            if face_str.is_empty() {
                continue;
            }

            let face_str1 = face_str.to_ascii_uppercase();
            match face_str1.as_str() {
                "BOTTOM" => {
                    faces.insert(Direction::Down);
                }
                "TOP" => {
                    faces.insert(Direction::Up);
                }
                "SIDES" => {
                    // java: Iterators.addAll(faces, Direction.Type.HORIZONTAL.iterator()).
                    faces.insert(Direction::North);
                    faces.insert(Direction::South);
                    faces.insert(Direction::West);
                    faces.insert(Direction::East);
                }
                "ALL" => {
                    self.faces = None;
                    return;
                }
                other => match parse_direction(other) {
                    Some(d) => {
                        faces.insert(d);
                    }
                    None => eprintln!(
                        "[continuity] Unknown 'faces' element '{}' at index {} in file '{}' in pack '{}'",
                        face_str, i, self.resource_id, self.pack_id
                    ),
                },
            }
        }

        if faces.is_empty() {
            self.valid = false;
        } else if faces.len() == DIRECTION_AMOUNT {
            self.faces = None;
        } else {
            self.faces = Some(faces);
        }
    }

    fn parse_biomes(&mut self) {
        let biomes_str = match self.properties.get("biomes") {
            None => return,
            Some(s) => s.trim().to_string(),
        };

        if biomes_str.is_empty() {
            self.valid = false;
            return;
        }

        let mut negate = false;
        let mut biomes_str = biomes_str;
        if biomes_str.starts_with('!') {
            negate = true;
            biomes_str = biomes_str[1..].to_string();
        }

        let biome_strs: Vec<&str> = biomes_str.split(' ').collect();
        if biome_strs.is_empty() {
            if !negate {
                self.valid = false;
            }
            return;
        }

        let mut biome_holder_set: HashSet<Identifier> = HashSet::new();
        for (i, biome_str) in biome_strs.iter().enumerate() {
            if biome_str.is_empty() {
                continue;
            }
            match Identifier::parse(&biome_str.to_lowercase()) {
                Ok(biome_id) => {
                    biome_holder_set.insert(get_or_create_biome_holder(&biome_id));
                }
                Err(_) => eprintln!(
                    "[continuity] Invalid 'biomes' element '{}' at index {} in file '{}' in pack '{}'",
                    biome_str, i, self.resource_id, self.pack_id
                ),
            }
        }

        if !biome_holder_set.is_empty() {
            self.biome_predicate = Some(BiomePredicate { holders: biome_holder_set, negate });
        } else if !negate {
            self.valid = false;
        }
    }

    fn parse_heights(&mut self) {
        let heights_str = match self.properties.get("heights") {
            None => return,
            Some(s) => s.clone(),
        };

        let height_strs = split_on(&heights_str, &[' ', ',']);
        if height_strs.is_empty() {
            self.valid = false;
            return;
        }

        let mut predicate_list: Vec<HeightPredicate> = Vec::new();

        for (i, height_str) in height_strs.iter().enumerate() {
            if height_str.is_empty() {
                continue;
            }

            // java: heightStr.split("\\.\\.", 2)
            let mut handled = false;
            if let Some(idx) = height_str.find("..") {
                let (min_str, rest) = (&height_str[..idx], &height_str[idx + 2..]);
                // java: split(.., 2) drops a second separator's tail into the max part.
                handled = true;
                if rest.is_empty() {
                    if let Ok(min) = min_str.parse::<i32>() {
                        predicate_list.push(HeightPredicate::AtLeast(min));
                        continue;
                    }
                } else if min_str.is_empty() {
                    if let Ok(max) = rest.parse::<i32>() {
                        predicate_list.push(HeightPredicate::AtMost(max));
                        continue;
                    }
                } else if let (Ok(min), Ok(max)) = (min_str.parse::<i32>(), rest.parse::<i32>()) {
                    if min < max {
                        predicate_list.push(HeightPredicate::Range(min, max));
                    } else if min > max {
                        predicate_list.push(HeightPredicate::Range(max, min));
                    } else {
                        predicate_list.push(HeightPredicate::Exact(min));
                    }
                    continue;
                }
            }
            if handled {
                eprintln!(
                    "[continuity] Invalid 'heights' element '{}' at index {} in file '{}' in pack '{}'",
                    height_str, i, self.resource_id, self.pack_id
                );
                continue;
            }

            // java: parts.length == 1 branch - single element with optional parens.
            let height_str1: String = height_str.chars().filter(|c| *c != '(' && *c != ')').collect();
            if !height_str1.is_empty() {
                let start = if height_str1.starts_with('-') { 1 } else { 0 };
                let separator_index = height_str1[start..].find('-').map(|i| i + start);
                match separator_index {
                    None => {
                        if let Ok(height) = height_str1.parse::<i32>() {
                            predicate_list.push(HeightPredicate::Exact(height));
                            continue;
                        }
                    }
                    Some(sep) => {
                        if let (Ok(min), Ok(max)) =
                            (height_str1[..sep].parse::<i32>(), height_str1[sep + 1..].parse::<i32>())
                        {
                            if min < max {
                                predicate_list.push(HeightPredicate::Range(min, max));
                            } else if min > max {
                                predicate_list.push(HeightPredicate::Range(max, min));
                            } else {
                                predicate_list.push(HeightPredicate::Exact(min));
                            }
                            continue;
                        }
                    }
                }
            }

            eprintln!(
                "[continuity] Invalid 'heights' element '{}' at index {} in file '{}' in pack '{}'",
                height_str, i, self.resource_id, self.pack_id
            );
        }

        if !predicate_list.is_empty() {
            self.height_predicate = Some(predicate_list);
        } else {
            self.valid = false;
        }
    }

    fn parse_legacy_heights(&mut self) {
        if self.height_predicate.is_some() {
            return;
        }

        let min_height_str = self.properties.get("minHeight").cloned();
        let max_height_str = self.properties.get("maxHeight").cloned();
        let mut has_min_height = min_height_str.is_some();
        let mut has_max_height = max_height_str.is_some();
        if !has_min_height && !has_max_height {
            return;
        }

        let mut min = 0;
        let mut max = 0;
        if let Some(s) = &min_height_str {
            match s.trim().parse::<i32>() {
                Ok(v) => min = v,
                Err(_) => {
                    eprintln!(
                        "[continuity] Invalid 'minHeight' value '{}' in file '{}' in pack '{}'",
                        s, self.resource_id, self.pack_id
                    );
                    has_min_height = false;
                }
            }
        }
        if let Some(s) = &max_height_str {
            match s.trim().parse::<i32>() {
                Ok(v) => max = v,
                Err(_) => {
                    // java: upstream logs minHeightStr in this branch; kept 1:1.
                    eprintln!(
                        "[continuity] Invalid 'maxHeight' value '{}' in file '{}' in pack '{}'",
                        min_height_str.clone().unwrap_or_default(),
                        self.resource_id,
                        self.pack_id
                    );
                    has_max_height = false;
                }
            }
        }

        if has_min_height && has_max_height {
            self.height_predicate = Some(vec![if min < max {
                HeightPredicate::Range(min, max)
            } else if min > max {
                HeightPredicate::Range(max, min)
            } else {
                HeightPredicate::Exact(min)
            }]);
        } else if has_min_height {
            self.height_predicate = Some(vec![HeightPredicate::AtLeast(min)]);
        } else if has_max_height {
            self.height_predicate = Some(vec![HeightPredicate::AtMost(max)]);
        }
    }

    fn parse_name(&mut self) {
        let name_str = match self.properties.get("name") {
            None => return,
            Some(s) => s.trim().to_string(),
        };

        let mut name_str = name_str;
        let mut is_pattern;
        let mut case_insensitive;

        if let Some(rest) = name_str.strip_prefix("regex:") {
            name_str = rest.to_string();
            is_pattern = false;
            case_insensitive = false;
        } else if let Some(rest) = name_str.strip_prefix("iregex:") {
            name_str = rest.to_string();
            is_pattern = false;
            case_insensitive = true;
        } else if let Some(rest) = name_str.strip_prefix("pattern:") {
            name_str = rest.to_string();
            is_pattern = true;
            case_insensitive = false;
        } else if let Some(rest) = name_str.strip_prefix("ipattern:") {
            name_str = rest.to_string();
            is_pattern = true;
            case_insensitive = true;
        } else {
            self.block_entity_name_predicate = Some(NamePredicate::Equals(name_str));
            return;
        }

        let mut pattern_str = name_str;
        if is_pattern {
            pattern_str = pattern_quote(&pattern_str);
            pattern_str = pattern_str.replace('?', "\\E.\\Q");
            pattern_str = pattern_str.replace('*', "\\E.*\\Q");
        }

        self.block_entity_name_predicate = Some(NamePredicate::Pattern { pattern: pattern_str, case_insensitive });
    }

    fn parse_prioritize(&mut self) {
        match self.properties.get("prioritize") {
            None => {
                self.prioritized = self.match_tiles_set.is_some();
            }
            Some(prioritize_str) => {
                self.prioritized = prioritize_str.trim() == "true";
            }
        }
    }

    fn parse_resource_condition(&mut self) {
        let conditions_str = match self.properties.get("resourceCondition") {
            None => return,
            Some(s) => s.clone(),
        };

        let condition_strs: Vec<&str> = conditions_str.trim().split('|').collect();
        if condition_strs.is_empty() {
            return;
        }

        for (i, condition_str) in condition_strs.iter().enumerate() {
            if condition_str.is_empty() {
                continue;
            }

            let parts: Vec<&str> = condition_str.splitn(2, '@').collect();
            if parts.is_empty() {
                eprintln!(
                    "[continuity] Invalid 'resourceCondition' element '{}' at index {} in file '{}' in pack '{}'",
                    condition_str, i, self.resource_id, self.pack_id
                );
                continue;
            }

            let resource_str = parts[0];
            let resource_id = match Identifier::parse(resource_str) {
                Ok(id) => id,
                Err(_) => {
                    eprintln!(
                        "[continuity] Invalid resource '{}' in 'resourceCondition' element '{}' at index {} in file '{}' in pack '{}'",
                        resource_str, condition_str, i, self.resource_id, self.pack_id
                    );
                    continue;
                }
            };

            let pack_str = parts.get(1).copied();

            match pack_str {
                None | Some("default") => {
                    if let Some(resource) = get_resource(&resource_id) {
                        // java: resource.getPack() != defaultPack.
                        if !resource.is_default_pack {
                            self.valid = false;
                            break;
                        }
                    }
                }
                Some("programmer_art") => {
                    if let Some(resource) = get_resource(&resource_id) {
                        if !default_resource_pack_is(&resource.pack_name) || resource.pack_name != "programmer_art" {
                            self.valid = false;
                            break;
                        }
                    }
                }
                Some(other) => {
                    eprintln!(
                        "[continuity] Unknown pack '{}' in 'resourceCondition' element '{}' at index {} in file '{}' in pack '{}'",
                        other, condition_str, i, self.resource_id, self.pack_id
                    );
                }
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// java: resolveTiles - lazily resolved sprite ids and dependencies.
    pub fn resolve_tiles(&mut self) {
        self.texture_dependencies = Some(HashSet::new());
        self.sprite_ids = Some(Vec::new());

        for tile in self.tiles.clone() {
            let sprite_id: SpriteId;
            if tile == Self::special_skip_id() {
                sprite_id = Self::special_skip_sprite_id();
            } else if tile == Self::special_default_id() {
                sprite_id = Self::special_default_sprite_id();
            } else {
                let mut path = tile.path.clone();
                if let Some(stripped) = path.strip_prefix("textures/") {
                    path = stripped.to_string();
                    if path.ends_with(".png") {
                        path.truncate(path.len() - 4);
                    }
                    sprite_id = to_sprite_id(Identifier::new(&tile.namespace, &path).unwrap());
                    self.texture_dependencies.as_mut().unwrap().insert(sprite_id.clone());
                } else if let Some(handler) = &self.redirect_handler {
                    path = handler.get_source_sprite_path(&path).unwrap_or_default();
                    sprite_id = to_sprite_id(Identifier::new(&tile.namespace, &path).unwrap());
                    self.texture_dependencies.as_mut().unwrap().insert(sprite_id.clone());
                } else {
                    sprite_id = missing_sprite_id();
                }
            }
            self.sprite_ids.as_mut().unwrap().push(sprite_id);
        }
    }

    pub fn get_tile_amount(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_prioritized(&self) -> bool {
        self.prioritized
    }

    pub fn get_sprite_ids(&mut self) -> &[SpriteId] {
        if self.sprite_ids.is_none() {
            self.resolve_tiles();
        }
        self.sprite_ids.as_deref().unwrap()
    }
}

/// java: Direction.valueOf(str) - enum-name lookup.
pub fn parse_direction(name: &str) -> Option<Direction> {
    match name {
        "DOWN" => Some(Direction::Down),
        "UP" => Some(Direction::Up),
        "NORTH" => Some(Direction::North),
        "SOUTH" => Some(Direction::South),
        "WEST" => Some(Direction::West),
        "EAST" => Some(Direction::East),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parse_match_tiles;
    use crate::seams::Identifier;

    fn loc(path: &str) -> Identifier {
        Identifier::new("minecraft", path).unwrap()
    }

    #[test]
    fn tiles_parse_specials_and_ranges() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "<skip> <default> 1-3 ./glass.png".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/test.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(props.is_valid());
        assert_eq!(props.get_tile_amount(), 6);
        assert!(props.tiles.contains(&BaseCtmProperties::special_skip_id()));
        assert!(props.tiles.contains(&BaseCtmProperties::special_default_id()));
        assert!(props.tiles.iter().any(|t| t == &Identifier::new("minecraft", "optifine/ctm/1.png").unwrap()));
        assert!(props.tiles.iter().any(|t| t == &Identifier::new("minecraft", "optifine/ctm/glass.png").unwrap()));
    }

    #[test]
    fn faces_parse_bottom_top_sides_and_all() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("faces".into(), "bottom top".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/f.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        let faces = props.faces.clone().unwrap();
        assert!(faces.contains(&Direction::Down) && faces.contains(&Direction::Up));
        assert_eq!(faces.len(), 2);

        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("faces".into(), "all".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/f.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(props.faces.is_none());
    }

    #[test]
    fn heights_parse_ranges_and_legacy() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("heights".into(), "0..20 30 40..".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/h.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        let preds = props.height_predicate.clone().unwrap();
        assert!(preds.iter().any(|p| *p == HeightPredicate::Range(0, 20)));
        assert!(preds.iter().any(|p| *p == HeightPredicate::Exact(30)));
        assert!(preds.iter().any(|p| *p == HeightPredicate::AtLeast(40)));

        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("minHeight".into(), "5".into());
        p.insert("maxHeight".into(), "9".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/h.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert_eq!(props.height_predicate, Some(vec![HeightPredicate::Range(5, 9)]));
    }

    #[test]
    fn name_predicate_forms() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("name".into(), "chest".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/n.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(props.block_entity_name_predicate.as_ref().unwrap().test("chest"));
        assert!(!props.block_entity_name_predicate.as_ref().unwrap().test("barrel"));

        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        p.insert("name".into(), "ipattern:chest*".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/n.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(matches!(props.block_entity_name_predicate, Some(NamePredicate::Pattern { .. })));
    }

    #[test]
    fn prioritize_defaults_to_match_tiles_presence() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/p.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(props.is_prioritized());

        let mut p = Properties::new();
        p.insert("matchBlocks".into(), "stone".into());
        p.insert("tiles".into(), "a".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/p.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(!props.is_prioritized());
    }

    #[test]
    fn invalid_without_matches() {
        let mut p = Properties::new();
        p.insert("tiles".into(), "a".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/x.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        assert!(!props.is_valid());
    }

    #[test]
    fn resolve_tiles_marks_dependencies() {
        let mut p = Properties::new();
        p.insert("matchTiles".into(), "stone".into());
        p.insert("tiles".into(), "textures/block/dirt.png".into());
        let mut props = BaseCtmProperties::new(p, loc("optifine/ctm/r.properties"), "pack".into(), 0, None, "ctm".into());
        props.init();
        let sprites = props.get_sprite_ids().to_vec();
        assert_eq!(sprites.len(), 1);
        assert_eq!(sprites[0], Identifier::new("minecraft", "block/dirt").unwrap());
        assert!(!props.texture_dependencies.unwrap().is_empty());
    }
}

impl BiomePredicate {
    /// java: BiomeSetPredicate.test(biome) - the biome's registry key maps to its
    /// holder id, membership is then checked with negate applied.
    pub fn test(&self, biome: &crate::seams::Biome) -> bool {
        self.holders.contains(&biome.id) != self.negate
    }
}
