//! 1:1 of me.pepperbell.continuity.client.resource.ResourceRedirectHandler, plus the
//! get() accessor surface it reads (ReloadableResourceManagerImplAccessor /
//! LifecycledResourceManagerImplExtension). Upstream by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity).

use crate::model::BooleanState;
use crate::properties_loader::{invalid_identifier_state_disable, invalid_identifier_state_enable};
use crate::seams::{filename_utils_path, Identifier, ResourceManager};

/// java: public static final String SPRITE_PATH_START = "continuity_reserved/".
pub const SPRITE_PATH_START: &str = "continuity_reserved/";
/// java: public static final String PATH_START = "textures/" + SPRITE_PATH_START.
pub const PATH_START: &str = "textures/continuity_reserved/";
/// java: public static final String PATH_END = ".png".
pub const PATH_END: &str = ".png";
/// java: PATH_START.length().
pub const PATH_START_LENGTH: usize = PATH_START.len();
/// java: PATH_END.length().
pub const PATH_END_LENGTH: usize = PATH_END.len();
/// java: HEX_LENGTH = 8.
pub const HEX_LENGTH: usize = 8;
/// java: HEX_END = PATH_START_LENGTH + HEX_LENGTH.
pub const HEX_END: usize = PATH_START_LENGTH + HEX_LENGTH;
/// java: MIN_LENGTH = PATH_START_LENGTH + HEX_LENGTH + PATH_END_LENGTH.
pub const MIN_LENGTH: usize = PATH_START_LENGTH + HEX_LENGTH + PATH_END_LENGTH;

/// java: HEX_DIGITS.
const HEX_DIGITS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// java: org.apache.commons.io.FilenameUtils.indexOfExtension - the last '.' before any
/// last '/' separator, or -1.
pub fn filename_utils_index_of_extension(path: &str) -> i64 {
    let last_separator = path.rfind('/').map(|i| i as i64).unwrap_or(-1);
    let extension = path.rfind('.').map(|i| i as i64).unwrap_or(-1);
    if extension > last_separator {
        extension
    } else {
        -1
    }
}

/// java: private static abstract class RedirectInfo with its two anonymous subclasses
/// (with-extension and no-extension createPath).
#[derive(Clone, Debug)]
pub struct RedirectInfo {
    /// java: public final String defaultPath.
    pub default_path: String,
    /// java: the captured pathStart/pathEnd or whole-path from RedirectInfo.of.
    path_start: String,
    path_end: String,
}

impl RedirectInfo {
    /// java: RedirectInfo.of(path).
    pub fn of(path: &str) -> RedirectInfo {
        let extension_index = filename_utils_index_of_extension(path);
        if extension_index != -1 {
            let i = extension_index as usize;
            RedirectInfo {
                default_path: path.to_string(),
                path_start: path[..i].to_string(),
                path_end: path[i..].to_string(),
            }
        } else {
            RedirectInfo {
                default_path: path.to_string(),
                path_start: path.to_string(),
                path_end: String::new(),
            }
        }
    }

    /// java: createPath(suffix) - pathStart + suffix + pathEnd, or path + suffix.
    pub fn create_path(&self, suffix: &str) -> String {
        format!("{}{}{}", self.path_start, suffix, self.path_end)
    }
}

/// java: public class ResourceRedirectHandler.
#[derive(Clone, Debug)]
pub struct ResourceRedirectHandler {
    /// java: private final ObjectList<RedirectInfo> redirects.
    redirects: Vec<RedirectInfo>,
    /// java: private final Object2IntMap<String> indexMap with defaultReturnValue(-1).
    index_map: std::collections::HashMap<String, i64>,
    /// java: private int nextIndex = 0.
    next_index: i64,
}

impl Default for ResourceRedirectHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRedirectHandler {
    pub fn new() -> ResourceRedirectHandler {
        ResourceRedirectHandler {
            redirects: Vec::new(),
            index_map: std::collections::HashMap::new(),
            next_index: 0,
        }
    }

    /// java: public static ResourceRedirectHandler get(ResourceManager resourceManager) - the
    /// mixin accessors unwrap the active manager and hand back the extension's handler; the
    /// seam carries the handler on the manager itself and returns none when absent.
    pub fn get(resource_manager: &ResourceManager) -> Option<&ResourceRedirectHandler> {
        resource_manager.redirect_handler.as_ref()
    }

    /// java: public String getSourceSpritePath(String absolutePath).
    pub fn get_source_sprite_path(&mut self, absolute_path: &str) -> String {
        // java: indexMap.getInt(absolutePath) with default -1.
        let index = match self.index_map.get(absolute_path) {
            Some(index) => *index,
            None => {
                // java: RedirectInfo info = RedirectInfo.of(absolutePath);
                // index = nextIndex++; redirects.add(info); indexMap.put(absolutePath, index);
                let info = RedirectInfo::of(absolute_path);
                let index = self.next_index;
                self.next_index += 1;
                self.redirects.push(info);
                self.index_map.insert(absolute_path.to_string(), index);
                index
            }
        };
        format!("{}{}", SPRITE_PATH_START, to_hex(index as i32))
    }

    /// java: public Identifier redirect(Identifier id).
    pub fn redirect(&self, id: &Identifier) -> Identifier {
        let path = &id.path;
        if !path.starts_with(PATH_START) || !path.ends_with(PATH_END) {
            return id.clone();
        }

        let length = path.len();
        if length < MIN_LENGTH {
            return id.clone();
        }

        let index = parse_hex(path, PATH_START_LENGTH);
        if index < 0 || index as usize >= self.redirects.len() {
            return id.clone();
        }

        let info = &self.redirects[index as usize];
        let new_path = if length == MIN_LENGTH {
            // java: newPath = info.defaultPath;
            info.default_path.clone()
        } else {
            // java: String suffix = path.substring(HEX_END, length - PATH_END_LENGTH);
            // newPath = info.createPath(suffix);
            let suffix = &path[HEX_END..length - PATH_END_LENGTH];
            info.create_path(suffix)
        };

        // java: BooleanState invalidIdentifierState = InvalidIdentifierStateHolder.get();
        // invalidIdentifierState.enable(); Identifier newId = id.withPath(newPath);
        // invalidIdentifierState.disable();
        invalid_identifier_state_enable();
        let new_id = with_path(id, &new_path);
        invalid_identifier_state_disable();

        new_id
    }
}

/// java: id.withPath(newPath) - same namespace, new path.
fn with_path(id: &Identifier, new_path: &str) -> Identifier {
    Identifier::new(&id.namespace, new_path).unwrap()
}

/// java: public static int parseHex(String string, int startIndex) - lower-case hex only,
/// -1 on any other character.
pub fn parse_hex(string: &str, start_index: usize) -> i64 {
    let mut i: i64 = 0;
    let bytes = string.as_bytes();
    let mut char_pos = start_index;
    let end_index = start_index + HEX_LENGTH;
    while char_pos < end_index {
        i <<= 4;
        let c = bytes[char_pos];
        char_pos += 1;
        if (b'0'..=b'9').contains(&c) {
            i |= (c - b'0') as i64;
        } else if (b'a'..=b'f').contains(&c) {
            i |= (c - b'a' + 10) as i64;
        } else {
            return -1;
        }
    }
    i
}

/// java: public static String toHex(int i) - the shared HEX_BUFFER filled from the right.
pub fn to_hex(i: i32) -> String {
    let mut chars = [0u8; HEX_LENGTH];
    let mut char_pos = HEX_LENGTH;
    let mut value = i;
    loop {
        char_pos -= 1;
        chars[char_pos] = HEX_DIGITS[(value & 15) as usize] as u8;
        value = ((value as u32) >> 4) as i32; // java: i >>>= 4 (unsigned shift)
        if char_pos == 0 {
            break;
        }
    }
    String::from_utf8(chars.to_vec()).unwrap()
}

/// java: get() falls through to null when the manager is not the engine's lifecycle type.
pub fn handler_missing(_resource_manager: &ResourceManager) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_sprite_path_assigns_stable_indices() {
        // java: first call indexes and allocates, later calls reuse the same index.
        let mut handler = ResourceRedirectHandler::new();
        let a = handler.get_source_sprite_path("minecraft:block/stone");
        let a2 = handler.get_source_sprite_path("minecraft:block/stone");
        let b = handler.get_source_sprite_path("minecraft:block/dirt");
        assert_eq!(a, a2);
        assert_ne!(a, b);
        // java: SPRITE_PATH_START + toHex(index) - 8 lowercase hex digits.
        assert!(a.starts_with(SPRITE_PATH_START));
        assert_eq!(a.len(), SPRITE_PATH_START.len() + HEX_LENGTH);
    }

    #[test]
    fn redirect_resolves_default_and_suffixed_paths() {
        // java: redirect() maps textures/continuity_reserved/<hex>.png back to the
        // recorded absolute path, and appends suffixes before the extension.
        let mut handler = ResourceRedirectHandler::new();
        let source = "minecraft:textures/block/stone.png";
        let reserved = handler.get_source_sprite_path(source);
        let hex_part = reserved[SPRITE_PATH_START.len()..].to_string();
        let id = Identifier::new(
            "minecraft",
            &format!("{}{}.png", PATH_START, hex_part),
        )
        .unwrap();
        let redirected = handler.redirect(&id);
        // java: newPath = info.defaultPath - the full absolute path as recorded.
        assert_eq!(redirected.path, "minecraft:textures/block/stone.png");

        // java: suffix variant: path.substring(HEX_END, length - PATH_END_LENGTH)
        // inserted between pathStart and pathEnd.
        let suffixed = Identifier::new(
            "minecraft",
            &format!("{}{}_emissive.png", PATH_START, hex_part),
        )
        .unwrap();
        let redirected_suffix = handler.redirect(&suffixed);
        assert_eq!(redirected_suffix.path, "minecraft:textures/block/stone_emissive.png");
    }

    #[test]
    fn redirect_leaves_other_paths_alone() {
        // java: paths outside the reserved space return unchanged.
        let handler = ResourceRedirectHandler::new();
        let plain = Identifier::new("minecraft", "textures/block/stone.png").unwrap();
        let same = handler.redirect(&plain);
        assert_eq!(same.path, "textures/block/stone.png");
        // java: too-short reserved paths return unchanged.
        let short = Identifier::new("minecraft", "textures/continuity_reserved/.png").unwrap();
        let same_short = handler.redirect(&short);
        assert_eq!(same_short.path, "textures/continuity_reserved/.png");
    }

    #[test]
    fn parse_hex_rejects_non_hex() {
        // java: parseHex reads 8 lower-case hex digits from startIndex, -1 otherwise.
        assert_eq!(parse_hex("0000000f", 0), 15);
        assert_eq!(parse_hex("deadbeef", 0), 0xdeadbeef as i64);
        assert_eq!(parse_hex("0000000g", 0), -1);
        assert_eq!(parse_hex("0000000F", 0), -1);
    }

    #[test]
    fn to_hex_pads_to_eight_digits() {
        // java: toHex always emits 8 lower-case hex digits.
        assert_eq!(to_hex(0), "00000000");
        assert_eq!(to_hex(255), "000000ff");
        assert_eq!(to_hex(-1), "ffffffff");
        assert_eq!(to_hex(0xdeadbeefu32 as i32), "deadbeef");
    }

    #[test]
    fn index_of_extension_follows_commons_io() {
        // java: FilenameUtils.indexOfExtension - last '.' after the last '/', else -1.
        assert_eq!(filename_utils_index_of_extension("a/b.c"), 3);
        assert_eq!(filename_utils_index_of_extension("a.b/c"), -1);
        assert_eq!(filename_utils_index_of_extension("noundots"), -1);
        assert_eq!(filename_utils_index_of_extension("a/b.c.d"), 5);
    }

    #[test]
    fn filename_utils_path_unchanged() {
        // java: FilenameUtils.getPath stays as the earlier seam carries it.
        assert_eq!(filename_utils_path("optifine/ctm/x.properties"), "optifine/ctm/");
    }
}

// rust carrying: the handler carries per-load state (nextIndex) and is shared through the
// properties seam, so the RedirectHandler impl wraps it behind a RefCell, matching the
// mutable upstream handler the mixin hands to the resource manager.
pub struct SharedRedirectHandler(
    pub std::rc::Rc<std::cell::RefCell<ResourceRedirectHandler>>,
);

impl crate::seams::RedirectHandler for SharedRedirectHandler {
    fn get_source_sprite_path(&self, path: &str) -> Option<String> {
        Some(
            self.0
                .borrow_mut()
                .get_source_sprite_path(path),
        )
    }
}
