//! Upstream `util/Utils.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::handler::GameProfile;

/// `fun GameProfile.isValidProfile()`: decode the textures property, read
/// `profileName`, and accept a version-4 UUID or a version-2 UUID whose name
/// matches. Upstream `runCatching { ... }.getOrNull()`; any decode failure
/// returns false.
pub fn is_valid_profile(profile: &GameProfile) -> bool {
    let textures = profile.properties.iter().find(|p| p.name == "textures");
    let profile_name = textures.and_then(|p| {
        base64::Engine::STANDARD
            .decode(&p.value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|json| {
                let key = "\"profileName\"";
                let start = json.find(key)? + key.len();
                let rest = json[start..].trim_start();
                let rest = rest.strip_prefix(':')?.trim_start();
                let end = rest.find('"')?;
                let after = &rest[end + 1..];
                let close = after.find('"')?;
                Some(after[..close].to_string())
            })
    });
    match profile_name {
        None => false,
        Some(profile_name) => {
            profile.id.1 == 4 || (profile.id.1 == 2 && profile.name == profile_name)
        }
    }
}

/// Minimal base64 (MIT-licensed crate was not in the tree, so a local decoder
/// keeps the port dependency-free).
mod base64 {
    pub struct DecodeError;

    pub struct Engine;

    impl Engine {
        pub const STANDARD: Engine = Engine;

        pub fn decode(&self, input: &str) -> Result<Vec<u8>, DecodeError> {
            let mut out = Vec::new();
            let mut buffer = 0u32;
            let mut bits = 0;
            for ch in input.chars() {
                let value = match ch {
                    'A'..='Z' => ch as u32 - 'A' as u32,
                    'a'..='z' => ch as u32 - 'a' as u32 + 26,
                    '0'..='9' => ch as u32 - '0' as u32 + 52,
                    '+' | '-' => 62,
                    '/' | '_' => 63,
                    '=' | '\n' | '\r' | ' ' | '\t' => continue,
                    _ => return Err(DecodeError),
                };
                buffer = (buffer << 6) | value;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((buffer >> bits) as u8);
                    buffer &= (1 << bits) - 1;
                }
            }
            Ok(out)
        }
    }
}
