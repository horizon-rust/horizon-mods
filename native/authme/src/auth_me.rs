
//! Exact 1:1 of me.axieum.mcmod.authme.api.AuthMe. The resourcefulconfig
//! Configurator and SLF4J logger are host seams with the same shapes.

fn log(level: &str, msg: &str) {
    #[cfg(test)]
    println!("[{}] [Auth Me] {}", level, msg);
    #[allow(unused_variables)]
    #[cfg(not(test))]
    let _ = (level, msg);
}

pub const MOD_ID: &str = "authme";
pub const MOD_NAME: &str = "Auth Me";
pub const MOJANG_ACCOUNT_MIGRATION_FAQ_URL: &str = "https://aka.ms/MinecraftPostMigrationFAQ";

/// The mod logger seam (SLF4J `LoggerFactory.getLogger(MOD_NAME)` on
/// upstream); a unit struct so call sites read the same.
pub struct LOGGER;

impl LOGGER {
    #[allow(non_snake_case)]
    pub fn info(&self, msg: &str) { log("info", msg) }
    #[allow(non_snake_case)]
    pub fn warn(&self, msg: &str) { log("warn", msg) }
    #[allow(non_snake_case)]
    pub fn error(&self, msg: &str) { log("error", msg) }
    #[allow(non_snake_case)]
    pub fn error_with(&self, msg: &str, err: &str) { log("error", &format!("{} ({})", msg, err)) }
    fn log(&self, level: &str, msg: &str) {
        #[cfg(test)]
        println!("[{}] [Auth Me] {}", level, msg);
        #[cfg(not(test))]
        let _ = (level, msg);
    }
}

use crate::config::Config;
use std::sync::OnceLock;

/// The mod configuration seam (resourcefulconfig Configurator on upstream):
/// the single live Config, initialised with the annotated defaults on first
/// use. The resourcefulconfig loader joins the config-screen batch later.
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// The live configuration (Java: the Configurator holds the annotated
/// statics; the carrier's config screen mutates through the same slot).
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

/// Initialises the multi-platform mod: registers the configuration.
pub fn init() {
    // CONFIG.register(Config.class) - the static Config is registered with
    // the resourcefulconfig loader in the carrier; the port warms the
    // defaults here to match the loader's eager parse.
    let _ = config();
}
