use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use super::options::{ConfigOption, ConfigValue};

#[derive(Debug, Default, Clone)]
pub struct ConfigManager {
    mod_id: String,
    options: HashMap<String, ConfigOption>,
    categories: Vec<String>,
}

impl ConfigManager {
    pub fn new(mod_id: impl Into<String>) -> Self {
        Self {
            mod_id: mod_id.into(),
            options: HashMap::new(),
            categories: Vec::new(),
        }
    }

    pub fn mod_id(&self) -> &str {
        &self.mod_id
    }

    pub fn register(&mut self, option: ConfigOption) {
        if !self.categories.contains(&option.category) {
            self.categories.push(option.category.clone());
        }
        let key = format!("{}.{}", option.category, option.name);
        self.options.insert(key, option);
    }

    pub fn get(&self, category: &str, name: &str) -> Option<&ConfigOption> {
        let key = format!("{}.{}", category, name);
        self.options.get(&key)
    }

    pub fn get_mut(&mut self, category: &str, name: &str) -> Option<&mut ConfigOption> {
        let key = format!("{}.{}", category, name);
        self.options.get_mut(&key)
    }

    pub fn get_bool(&self, category: &str, name: &str) -> bool {
        self.get(category, name)
            .and_then(|o| o.current_value.as_bool())
            .unwrap_or(false)
    }

    pub fn get_i64(&self, category: &str, name: &str) -> i64 {
        self.get(category, name)
            .and_then(|o| o.current_value.as_i64())
            .unwrap_or(0)
    }

    pub fn get_f64(&self, category: &str, name: &str) -> f64 {
        self.get(category, name)
            .and_then(|o| o.current_value.as_f64())
            .unwrap_or(0.0)
    }

    pub fn get_str(&self, category: &str, name: &str) -> Option<&str> {
        self.get(category, name)
            .and_then(|o| o.current_value.as_str())
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn options_in_category(&self, category: &str) -> Vec<&ConfigOption> {
        let mut opts = Vec::new();
        for (_key, opt) in &self.options {
            if opt.category == category {
                opts.push(opt);
            }
        }
        opts.sort_by(|a, b| a.name.cmp(&b.name));
        opts
    }

    pub fn load_from_json(&mut self, json_str: &str) -> Result<(), serde_json::Error> {
        let parsed: HashMap<String, serde_json::Value> = serde_json::from_str(json_str)?;

        for (category_name, cat_val) in parsed {
            if let Some(obj) = cat_val.as_object() {
                for (opt_name, val) in obj {
                    if let Some(opt) = self.get_mut(&category_name, &opt_name) {
                        match &opt.default_value {
                            ConfigValue::Boolean(_) => {
                                if let Some(b) = val.as_bool() {
                                    opt.current_value = ConfigValue::Boolean(b);
                                }
                            }
                            ConfigValue::Integer(_) => {
                                if let Some(i) = val.as_i64() {
                                    opt.current_value = ConfigValue::Integer(i);
                                }
                            }
                            ConfigValue::Double(_) => {
                                if let Some(f) = val.as_f64() {
                                    opt.current_value = ConfigValue::Double(f);
                                }
                            }
                            ConfigValue::String(_) => {
                                if let Some(s) = val.as_str() {
                                    opt.current_value = ConfigValue::String(s.to_string());
                                }
                            }
                            ConfigValue::Color(_) => {
                                if let Some(s) = val.as_str() {
                                    let hex = s.trim_start_matches("#").trim_start_matches("0x");
                                    if let Ok(c) = u32::from_str_radix(hex, 16) {
                                        opt.current_value = ConfigValue::Color(c);
                                    }
                                } else if let Some(i) = val.as_u64() {
                                    opt.current_value = ConfigValue::Color(i as u32);
                                }
                            }
                            ConfigValue::OptionList { values, .. } => {
                                if let Some(s) = val.as_str() {
                                    if let Some(pos) = values.iter().position(|v| v == s) {
                                        opt.current_value = ConfigValue::OptionList {
                                            values: values.clone(),
                                            selected_index: pos,
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        let mut root: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

        for opt in self.options.values() {
            let cat_map = root.entry(opt.category.clone()).or_default();
            let json_val = match &opt.current_value {
                ConfigValue::Boolean(b) => serde_json::Value::Bool(*b),
                ConfigValue::Integer(i) => serde_json::Value::Number((*i).into()),
                ConfigValue::Double(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                ConfigValue::String(s) => serde_json::Value::String(s.clone()),
                ConfigValue::Color(c) => serde_json::Value::String(format!("#{a:02X}{r:02X}{g:02X}{b:02X}",
                    a = (c >> 24) & 0xFF,
                    r = (c >> 16) & 0xFF,
                    g = (c >> 8) & 0xFF,
                    b = c & 0xFF,
                )),
                ConfigValue::OptionList { values, selected_index } => {
                    let s = values.get(*selected_index).cloned().unwrap_or_default();
                    serde_json::Value::String(s)
                }
            };
            cat_map.insert(opt.name.clone(), json_val);
        }

        serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let content = fs::read_to_string(path)?;
        self.load_from_json(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_json())
    }
}
