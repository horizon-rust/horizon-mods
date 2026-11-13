pub mod options;
pub mod manager;

pub use options::{ConfigOption, ConfigValue};
pub use manager::ConfigManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_registry_and_json() {
        let mut mgr = ConfigManager::new("malilib");
        mgr.register(ConfigOption::boolean("renderOverlay", "Generic", "Render overlay", true));
        mgr.register(ConfigOption::integer("renderDistance", "Generic", "Render distance", 64));
        mgr.register(ConfigOption::color("highlightColor", "Colors", "Highlight color", 0xFF00FF00));

        assert_eq!(mgr.get_bool("Generic", "renderOverlay"), true);
        assert_eq!(mgr.get_i64("Generic", "renderDistance"), 64);

        let json = mgr.to_json();
        assert!(json.contains("renderOverlay"));

        let mut mgr2 = ConfigManager::new("malilib");
        mgr2.register(ConfigOption::boolean("renderOverlay", "Generic", "Render overlay", false));
        mgr2.register(ConfigOption::integer("renderDistance", "Generic", "Render distance", 16));
        mgr2.load_from_json(&json).unwrap();

        assert_eq!(mgr2.get_bool("Generic", "renderOverlay"), true);
        assert_eq!(mgr2.get_i64("Generic", "renderDistance"), 64);
    }
}
