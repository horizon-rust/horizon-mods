//! Upstream: common/resource/ (1:1).
use crate::compat::Component;
use crate::global::{MOD_ID, MOD_PREFIX};

// ---------------------------------------------------------------------------
// resource/LanguageUtils.java (1:1). MutableComponent trees are host-side; the
// compat::Component seam renders each factory to its display string.
// ---------------------------------------------------------------------------

pub struct LanguageUtils {
    pub key: String,
}

impl LanguageUtils {
    pub fn of(category: &str, path: &[&str]) -> Self {
        Self { key: format!("{}.{}.{}", category, MOD_ID, path.join(".")) }
    }

    pub fn settings(key: &str) -> Self {
        Self::of("settings", &[key])
    }

    pub fn command(key: &str) -> Self {
        Self::of("command", &[key])
    }

    pub fn key_of(category: &str, path: &[&str]) -> String {
        Self::of(category, path).key.clone()
    }

    pub fn get(&self, args: &[&str]) -> String {
        Component::translatable(&self.key, args)
    }

    pub fn path(&self, path: &[&str]) -> Self {
        Self { key: format!("{}.{}", self.key, path.join(".")) }
    }

    pub fn wrapped(&self) -> String {
        format!("({})", self.get(&[]))
    }
}

/// VALUE_INFINITE / VALUE_HIDDEN / UNIT_* statics, resolved once like upstream class-load.
pub fn value_infinite() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("value", &["infinite"]).get(&[])).clone()
}

pub fn value_hidden() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("value", &["hidden"]).get(&[])).clone()
}

pub const NEWLINE: &str = "\n";

pub fn unit_seconds() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["seconds"]).get(&[])).clone()
}

pub fn unit_meters() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["meters"]).get(&[])).clone()
}

pub fn unit_percent() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["percent"]).get(&[])).clone()
}

/// Upstream LanguageUtils.UNIT_PERCENT static: the percent unit formatted with
/// the value as its argument.
pub fn unit_percent_get(value: i32) -> String {
    LanguageUtils::of("unit", &["percent"]).get(&[&value.to_string()])
}

/// Upstream LanguageUtils.UNIT_SECONDS static; the correction-period option
/// formats its value first ("%.1f").
pub fn unit_seconds_get(value: &str) -> String {
    LanguageUtils::of("unit", &["seconds"]).get(&[value])
}

pub fn join(components: &[String]) -> String {
    let mut out = String::new();
    for (i, c) in components.iter().enumerate() {
        if i > 0 {
            out.push_str(NEWLINE);
        }
        out.push_str(c);
    }
    out
}

pub fn with_mod_prefix(component: &str) -> String {
    format!("{}{}", MOD_PREFIX, component)
}

pub fn from(any: &str) -> String {
    Component::literal(any)
}

// ---------------------------------------------------------------------------
// resource/ResourceConstants.java (1:1)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceLocationToken {
    pub namespace: &'static str,
    pub path: &'static str,
}

pub const PING_SOUND_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "ping" };
pub const PING_TEXTURE_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "textures/ping.png" };
pub const ARROW_TEXTURE_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "textures/arrow.png" };

const MOD_ID_IDENTITY: &str = MOD_ID;

/// SoundEvent(PING_SOUND_ID) as a token.
pub const PING_SOUND_EVENT: ResourceLocationToken = PING_SOUND_ID;


/// LanguageUtils.UNIT_METERS = LanguageUtils.of("unit", "meters")
pub fn unit_meters_lang() -> LanguageUtils {
    LanguageUtils::of("unit", &["meters"])
}

// ---------------------------------------------------------------------------
// resource/ResourceReloadListener.java (1:1)
// ---------------------------------------------------------------------------

pub(crate) static NUM_CUSTOM_TEXTURES: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Host seam for the net.minecraft.server.packs.resources.ResourceManager the
/// reload listener probes (hasResource for the legacy-id check, getResources for the count).
pub trait ResourceManagerSeam {
    fn has_resource(&self, namespace: &str, path: &str) -> bool;
    fn resources(&self, res: &crate::resource::ResourceLocationToken) -> Result<usize, String>;
}

pub struct ResourceReloadListener;

impl ResourceReloadListener {
    pub fn has_custom_texture() -> bool {
        NUM_CUSTOM_TEXTURES.load(std::sync::atomic::Ordering::SeqCst) > 1
    }

    /// Upstream reloadTextures: supplyAsync(checkResources + resource count, loadExecutor)
    /// -> thenCompose(helper::wait) -> thenAcceptAsync(no-op, applyExecutor). The port runs
    /// the preparation inline; the preparation barrier and apply executor are host seams.
    pub fn reload_textures(resource_manager: &dyn ResourceManagerSeam) {
        crate::compat::check_resources(&|ns, path| resource_manager.has_resource(ns, path));

        match resource_manager.resources(&PING_TEXTURE_ID) {
            Ok(count) => NUM_CUSTOM_TEXTURES.store(count as i32, std::sync::atomic::Ordering::SeqCst),
            Err(e) => crate::global::log_error(&format!("failed to gather resources: {}", e)),
        }
    }
}
