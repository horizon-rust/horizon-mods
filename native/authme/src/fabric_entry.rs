
//! Exact 1:1 of me.axieum.mcmod.authme.impl.fabric.AuthMeFabric: the
//! Fabric platform mod entry. The Fabric loader hooks are host seams; the
//! init sequence (config migration, then AuthMe#init) is carried verbatim.

use crate::auth_me::{init as auth_me_init, LOGGER};

/// Java: `onInitialize()` - migrates the old config file, then cascades
/// into AuthMe#init().
pub fn on_initialize(config_dir: &str) {
    // Migrate the old `config/authme.json5` configuration file
    migrate_config(config_dir);
    // Cascade mod initialisation
    auth_me_init();
}

/// Java: `migrateConfig()` - migrates the old `authme.json5` config to
/// `authme.jsonc` (rename on first sight; errors are logged, never raised).
pub fn migrate_config(config_dir: &str) -> bool {
    // Path configDir = FabricLoader.getInstance().getConfigDir()
    let old_config = format!("{}/authme.json5", config_dir);
    let new_config = format!("{}/authme.jsonc", config_dir);
    // if (Files.exists(oldConfig) && Files.notExists(newConfig))
    let old_exists = std::path::Path::new(&old_config).exists();
    let new_exists = std::path::Path::new(&new_config).exists();
    if old_exists && !new_exists {
        LOGGER.warn(&format!(
            "Found old config file '{}', renaming to '{}'",
            old_config, new_config
        ));
        return match std::fs::rename(&old_config, &new_config) {
            Ok(()) => true,
            Err(e) => {
                LOGGER.error(&format!("Could not migrate old config file to '{}' ({})", new_config, e));
                false
            }
        };
    }
    false
}
