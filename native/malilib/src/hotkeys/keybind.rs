use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct KeyModifier {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyModifier {
    pub const NONE: KeyModifier = KeyModifier {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    pub fn matches(&self, other: &KeyModifier) -> bool {
        self.shift == other.shift && self.ctrl == other.ctrl && self.alt == other.alt && self.meta == other.meta
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum KeyAction {
    #[default]
    Press,
    Release,
    Hold,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeybindMulti {
    pub keys: Vec<String>,
    pub modifiers: KeyModifier,
    pub action: KeyAction,
}

impl Default for KeybindMulti {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            modifiers: KeyModifier::NONE,
            action: KeyAction::Press,
        }
    }
}

impl KeybindMulti {
    pub fn new(keys: &[&str]) -> Self {
        Self {
            keys: keys.iter().map(|s| s.to_uppercase()).collect(),
            modifiers: KeyModifier::NONE,
            action: KeyAction::Press,
        }
    }

    pub fn with_modifiers(keys: &[&str], modifiers: KeyModifier) -> Self {
        Self {
            keys: keys.iter().map(|s| s.to_uppercase()).collect(),
            modifiers,
            action: KeyAction::Press,
        }
    }

    pub fn parse(s: &str) -> Self {
        let mut keys = Vec::new();
        let mut modifiers = KeyModifier::NONE;

        for part in s.split(&[',', '+'][..]) {
            let trimmed = part.trim().to_uppercase();
            match trimmed.as_str() {
                "SHIFT" | "LEFT_SHIFT" | "RIGHT_SHIFT" => modifiers.shift = true,
                "CTRL" | "CONTROL" | "LEFT_CONTROL" | "RIGHT_CONTROL" => modifiers.ctrl = true,
                "ALT" | "LEFT_ALT" | "RIGHT_ALT" => modifiers.alt = true,
                "META" | "SUPER" | "WIN" => modifiers.meta = true,
                other if !other.is_empty() => keys.push(other.to_string()),
                _ => {}
            }
        }

        Self {
            keys,
            modifiers,
            action: KeyAction::Press,
        }
    }

    pub fn to_display_string(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        for key in &self.keys {
            parts.push(key.as_str());
        }
        parts.join(" + ")
    }
}
