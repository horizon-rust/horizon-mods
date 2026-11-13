use std::collections::{HashMap, HashSet};
use super::keybind::{KeyAction, KeyModifier, KeybindMulti};

#[derive(Debug, Clone)]
pub struct RegisteredHotkey {
    pub name: String,
    pub keybind: KeybindMulti,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct HotkeyManager {
    hotkeys: HashMap<String, RegisteredHotkey>,
    active_keys: HashSet<String>,
    current_modifiers: KeyModifier,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, keybind: KeybindMulti, description: impl Into<String>) {
        let name_str = name.into();
        self.hotkeys.insert(
            name_str.clone(),
            RegisteredHotkey {
                name: name_str,
                keybind,
                description: description.into(),
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredHotkey> {
        self.hotkeys.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut RegisteredHotkey> {
        self.hotkeys.get_mut(name)
    }

    pub fn is_key_pressed(&self, key: &str) -> bool {
        self.active_keys.contains(&key.to_uppercase())
    }

    pub fn handle_key_event(&mut self, key: &str, pressed: bool, modifiers: KeyModifier) -> Vec<String> {
        self.current_modifiers = modifiers;
        let upper_key = key.to_uppercase();

        if pressed {
            self.active_keys.insert(upper_key.clone());
        } else {
            self.active_keys.remove(&upper_key);
        }

        let mut triggered = Vec::new();

        for hotkey in self.hotkeys.values() {
            let binds = &hotkey.keybind;
            if binds.keys.is_empty() {
                continue;
            }

            if binds.modifiers != KeyModifier::NONE && !binds.modifiers.matches(&self.current_modifiers) {
                continue;
            }

            match binds.action {
                KeyAction::Press => {
                    if pressed && binds.keys.iter().all(|k| self.active_keys.contains(k)) {
                        if binds.keys.contains(&upper_key) {
                            triggered.push(hotkey.name.clone());
                        }
                    }
                }
                KeyAction::Release => {
                    if !pressed && binds.keys.contains(&upper_key) {
                        triggered.push(hotkey.name.clone());
                    }
                }
                KeyAction::Hold | KeyAction::Toggle => {
                    if binds.keys.iter().all(|k| self.active_keys.contains(k)) {
                        triggered.push(hotkey.name.clone());
                    }
                }
            }
        }

        triggered
    }
}
