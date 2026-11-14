//! Upstream versionless/config/{FirstPersonSettings, VanillaHands,
//! ConfigUpgrader}.java, 1:1.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// versionless/config/VanillaHands.java.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VanillaHands {
    Off,
    All,
    AllDouble,
    Items,
}

/// versionless/config/FirstPersonSettings.java.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FirstPersonSettings {
    pub config_version: i32,
    pub enabled_by_default: bool,
    pub x_offset: i32,
    pub sneak_x_offset: i32,
    pub sit_x_offset: i32,
    pub render_stuck_features: bool,
    pub vanilla_hands_mode: VanillaHands,
    pub dynamic_mode: bool,
    pub vanilla_hands_skip_swimming: bool,
    pub auto_vanilla_hands: HashSet<String>,
    pub auto_toggle_mod_items: HashSet<String>,
    pub hidden_layers: HashSet<String>,
}

impl Default for FirstPersonSettings {
    /// The field initializers upstream writes.
    fn default() -> Self {
        Self {
            config_version: 2,
            enabled_by_default: true,
            x_offset: 0,
            sneak_x_offset: 0,
            sit_x_offset: 0,
            render_stuck_features: true,
            vanilla_hands_mode: VanillaHands::Off,
            dynamic_mode: true,
            vanilla_hands_skip_swimming: true,
            auto_vanilla_hands: HashSet::from([
                "antiqueatlas:antique_atlas".to_string(),
                "twilightforest:filled_magic_map".to_string(),
                "twilightforest:filled_maze_map".to_string(),
                "twilightforest:filled_ore_map".to_string(),
                "create:potato_cannon".to_string(),
                "create:extendo_grip".to_string(),
                "create:handheld_worldshaper".to_string(),
                "map_atlases:atlas".to_string(),
            ]),
            auto_toggle_mod_items: HashSet::from(["exposure:camera".to_string()]),
            hidden_layers: HashSet::new(),
        }
    }
}

/// versionless/config/ConfigUpgrader.java.
pub struct ConfigUpgrader;

impl ConfigUpgrader {
    pub fn upgrade_config(conf: &mut FirstPersonSettings) {
        if conf.config_version < 1 {
            // re-init the auto vanilla hands set
            conf.auto_vanilla_hands.extend([
                "antiqueatlas:antique_atlas".to_string(),
                "twilightforest:filled_magic_map".to_string(),
                "twilightforest:filled_maze_map".to_string(),
                "twilightforest:filled_ore_map".to_string(),
                "create:potato_cannon".to_string(),
                "create:extendo_grip".to_string(),
                "create:handheld_worldshaper".to_string(),
                "map_atlases:atlas".to_string(),
            ]);
            conf.config_version = 1;
        }
        if conf.config_version < 2 {
            conf.auto_toggle_mod_items.insert("exposure:camera".to_string());
            conf.config_version = 2;
        }
    }
}
