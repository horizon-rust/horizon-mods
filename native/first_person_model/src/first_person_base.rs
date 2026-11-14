//! Upstream versionless/FirstPersonBase.java, 1:1.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use crate::config::{ConfigUpgrader, FirstPersonSettings};

/// versionless/FirstPersonBase.java. The static LOGGER stays in seams::log.
pub struct FirstPersonBase {
    /// Stays true till the first entity is rendered when the player is
    /// supposed to be rendered, or during extraction in 26.1+.
    pub is_rendering_player: Cell<bool>,
    /// Stays true for the entire render time of the entity, even after the
    /// entity itself.
    pub is_rendering_player_post: Cell<bool>,
    pub enabled: Cell<bool>,
    pub config: RefCell<Option<FirstPersonSettings>>,
    settings_file: PathBuf,
}

impl FirstPersonBase {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            is_rendering_player: Cell::new(false),
            is_rendering_player_post: Cell::new(false),
            enabled: Cell::new(true),
            config: RefCell::new(None),
            // new File("config", "firstperson.json")
            settings_file: PathBuf::from("config").join("firstperson.json"),
        })
    }

    pub fn load_config(&self) {
        if self.settings_file.exists() {
            match std::fs::read_to_string(&self.settings_file) {
                Ok(text) => match serde_json::from_str::<FirstPersonSettings>(&text) {
                    Ok(cfg) => *self.config.borrow_mut() = Some(cfg),
                    Err(err) => {
                        crate::seams::log::warn(
                            "Unable to load the config, creating a new one.",
                            Some(&err.to_string()),
                        );
                    }
                },
                Err(err) => {
                    crate::seams::log::warn(
                        "Unable to load the config, creating a new one.",
                        Some(&err.to_string()),
                    );
                }
            }
        }
        if self.config.borrow().is_none() {
            *self.config.borrow_mut() = Some(FirstPersonSettings::default());
        }
        ConfigUpgrader::upgrade_config(self.config.borrow_mut().as_mut().unwrap());
        self.write_settings();
        let enabled = self.config.borrow().as_ref().unwrap().enabled_by_default;
        self.set_enabled(enabled);
    }

    pub fn write_settings(&self) {
        let json = serde_json::to_string_pretty(self.config.borrow().as_ref().unwrap())
            .unwrap_or_else(|_| "{}".to_string());
        if let Some(parent) = self.settings_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = std::fs::write(&self.settings_file, json) {
            crate::seams::log::warn("Error while saving the config!", Some(&err.to_string()));
        }
    }

    pub fn reset_settings(&self) {
        *self.config.borrow_mut() = Some(FirstPersonSettings::default());
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    pub fn is_rendering_player(&self) -> bool {
        self.is_rendering_player.get()
    }

    pub fn set_rendering_player(&self, value: bool) {
        self.is_rendering_player.set(value);
    }

    /// Checks if a class exists or not. On this port the mod-support classes
    /// are compiled in, so a class "exists" when its feature was registered.
    pub fn is_valid_class(name: &str) -> bool {
        // Class.forName: the port checks the known support class names.
        matches!(name, "dev.kosmx.playerAnim.core.impl.AnimationProcessor")
    }
}
