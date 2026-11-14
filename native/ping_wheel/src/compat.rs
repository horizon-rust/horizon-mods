//! Upstream: common/compat/ (1:1).
use crate::seams::{Matrix4f, Vec3};

/// Upstream compat/Vector4f.java: a mutable 4-vector that transforms through a Matrix4f.
#[derive(Clone, Copy, Debug)]
pub struct Vector4f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4f {
    pub fn new(vec3d: &Vec3, w: f32) -> Self {
        Self { x: vec3d.x as f32, y: vec3d.y as f32, z: vec3d.z as f32, w }
    }

    pub fn mul(mut self, mat: &Matrix4f) -> Self {
        // Upstream wraps com.mojang.math.Vector4f#transform(Matrix4f); the matrix multiply lives in the seam.
        let t = crate::seams::transform4(mat, self.x, self.y, self.z, self.w);
        self.x = t.0;
        self.y = t.1;
        self.z = t.2;
        self.w = t.3;
        self
    }

    pub fn div(mut self, scalar: f32) -> Self {
        let inv = 1f32 / scalar;
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
        self.w *= inv;
        self
    }
}

/// Upstream compat/Component.java: literal / empty / translatable factory.
/// The seam renders to a display string because the MC Component tree is a host type.
pub struct Component;

impl Component {
    pub fn literal(text: &str) -> String {
        text.to_string()
    }

    pub fn empty() -> String {
        String::new()
    }

    pub fn translatable(key: &str, args: &[&str]) -> String {
        crate::seams::translate_component(key, args)
    }
}

// ---------------------------------------------------------------------------
// LegacyMigrationHandler.migrateConfig (the part ConfigHandler.of calls).
// The key-mapping / resource-pack migration halves follow with the resource phase.
// ---------------------------------------------------------------------------

pub fn legacy_migrate_config(config_extension: &str) {
    let ctx = crate::seams::context_service();
    let legacy_config_path = ctx.resolve_config_dir(&format!("ping-wheel{}", config_extension));
    let new_config_path = ctx.resolve_config_dir(&format!("{}{}", crate::global::MOD_ID, config_extension));

    if !std::path::Path::new(&legacy_config_path).exists() {
        return;
    }

    match std::fs::rename(&legacy_config_path, &new_config_path) {
        Ok(()) => crate::global::log_info(&format!("Renamed legacy config: {} -> {}", legacy_config_path, new_config_path)),
        Err(e) => crate::global::log_error(&format!("Failed to rename legacy config: {}", e)),
    }
}


// ---------------------------------------------------------------------------
// compat/LegacyMigrationHandler.java (1:1). TODO upstream: remove a few
// versions/months down the line; introduced with 1.12.0 on 09.10.2025.
// ---------------------------------------------------------------------------

static GAME_OPTIONS_SAVE_NEEDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static NOTIFY_DEPRECATED_RESOURCE_PACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static MIGRATED_PING_KEY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
static MIGRATED_SETTINGS_KEY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// Migrated KEY_BINDING_PING key (tokens are constructed per call in this port;
/// the host applies the migrated key to the live binding).
pub fn migrated_ping_key() -> i32 {
    MIGRATED_PING_KEY.load(std::sync::atomic::Ordering::SeqCst)
}

/// Migrated KEY_BINDING_SETTINGS key.
pub fn migrated_settings_key() -> i32 {
    MIGRATED_SETTINGS_KEY.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn migrate_key_mappings() {
    use std::sync::atomic::Ordering;

    let ctx = crate::seams::context_service();
    let options_path = ctx.resolve_game_dir("options.txt");

    if !std::path::Path::new(&options_path).exists() {
        crate::global::log_warn("Unable to find game options for migration routine");
        return;
    }

    match std::fs::read_to_string(&options_path) {
        Err(e) => crate::global::log_error(&format!("Failed to read options.txt to transfer legacy keybinds: {}", e)),
        Ok(content) => {
            for line in content.lines() {
                if !line.contains("ping-wheel") {
                    continue;
                }

                // Upstream line.split(":")[1]; a line without a colon would throw there,
                // the port skips it.
                let Some(key_string) = line.split(':').nth(1) else {
                    continue;
                };
                let key_key = crate::seams::input_constants_get_key(key_string);

                if line.contains("ping-location") {
                    MIGRATED_PING_KEY.store(key_key, Ordering::SeqCst);
                    crate::global::log_info(&format!("Migrated KEY_BINDING_PING: {}", key_key));
                } else if line.contains("open-settings") {
                    MIGRATED_SETTINGS_KEY.store(key_key, Ordering::SeqCst);
                    crate::global::log_info(&format!("Migrated KEY_BINDING_SETTINGS: {}", key_key));
                }

                GAME_OPTIONS_SAVE_NEEDED.store(true, Ordering::SeqCst);
            }
        }
    }
}

pub fn on_tick() {
    use std::sync::atomic::Ordering;

    let Some(game) = crate::seams::game() else {
        return;
    };

    if GAME_OPTIONS_SAVE_NEEDED.swap(false, Ordering::SeqCst) {
        game.save_options();
    }

    if NOTIFY_DEPRECATED_RESOURCE_PACK.load(Ordering::SeqCst) && game.player().is_some() {
        // msg = literal("a resource pack is using the old mod-id, please update ")
        //     + literal("\"assets/ping-wheel\"").withStyle(ChatFormatting.GRAY)
        //     + " to "
        //     + literal("\"assets/pingwheel\"").withStyle(ChatFormatting.GRAY)
        let msg = "a resource pack is using the old mod-id, please update \
\u{a7}7\"assets/ping-wheel\"\u{a7}r to \u{a7}7\"assets/pingwheel\"";
        game.send_player_message(&crate::resource::with_mod_prefix(msg));
        NOTIFY_DEPRECATED_RESOURCE_PACK.store(false, Ordering::SeqCst);
    }
}

/// Host seam: ResourceManager.hasResource(location) for the legacy-texture probe.
pub fn check_resources(has_resource: &dyn Fn(&str, &str) -> bool) {
    use std::sync::atomic::Ordering;

    let legacy_textures = [
        ("ping-wheel", "ping"),
        ("ping-wheel", "textures/ping.png"),
        ("ping-wheel", "textures/arrow.png"),
    ];

    NOTIFY_DEPRECATED_RESOURCE_PACK.store(false, Ordering::SeqCst);

    for (namespace, path) in legacy_textures {
        if has_resource(namespace, path) {
            NOTIFY_DEPRECATED_RESOURCE_PACK.store(true, Ordering::SeqCst);
        }
    }
}

pub fn notify_deprecated_resource_pack() -> bool {
    NOTIFY_DEPRECATED_RESOURCE_PACK.load(std::sync::atomic::Ordering::SeqCst)
}
