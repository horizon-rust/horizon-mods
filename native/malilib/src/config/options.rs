use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Boolean(bool),
    Integer(i64),
    Double(f64),
    String(String),
    Color(u32),
    OptionList {
        values: Vec<String>,
        selected_index: usize,
    },
}

impl ConfigValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Double(d) => Some(*d),
            ConfigValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<u32> {
        match self {
            ConfigValue::Color(c) => Some(*c),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigOption {
    pub name: String,
    pub category: String,
    pub comment: String,
    pub default_value: ConfigValue,
    pub current_value: ConfigValue,
}

impl ConfigOption {
    pub fn new(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default: ConfigValue) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            comment: comment.into(),
            current_value: default.clone(),
            default_value: default,
        }
    }

    pub fn boolean(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default: bool) -> Self {
        Self::new(name, category, comment, ConfigValue::Boolean(default))
    }

    pub fn integer(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default: i64) -> Self {
        Self::new(name, category, comment, ConfigValue::Integer(default))
    }

    pub fn double(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default: f64) -> Self {
        Self::new(name, category, comment, ConfigValue::Double(default))
    }

    pub fn string(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default: impl Into<String>) -> Self {
        Self::new(name, category, comment, ConfigValue::String(default.into()))
    }

    pub fn color(name: impl Into<String>, category: impl Into<String>, comment: impl Into<String>, default_hex: u32) -> Self {
        Self::new(name, category, comment, ConfigValue::Color(default_hex))
    }

    pub fn reset_to_default(&mut self) {
        self.current_value = self.default_value.clone();
    }

    pub fn is_modified(&self) -> bool {
        self.current_value != self.default_value
    }
}
