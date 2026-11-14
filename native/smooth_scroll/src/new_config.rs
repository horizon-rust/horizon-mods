//! Upstream cfg/NewConfig.java, 1:1. The subclass hook methods live on the
//! ConfigHooks trait; SmScCfg supplies them exactly like the Java overrides.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value as Json};

use crate::cfg_value::CfgValue;
use crate::seams;

pub struct NewConfigBase {
    pub file_name: String,
    pub root: Rc<RefCell<CfgValue>>,
    pub raw_root: RefCell<Rc<RefCell<CfgValue>>>,
}

pub trait ConfigHooks {
    fn problem_writing(&self) {}
    fn problem_reading(&self) {}
    fn file_not_found(&self) {}
    /// Data corrects config values (updates config values from previous
    /// format, etc.). Called before into_variables().
    fn data_correct_permanent(&self) -> Result<(), String> {
        Ok(())
    }
    /// Inserts config values into variables.
    fn into_variables(&self) -> Result<(), String> {
        Ok(())
    }
    /// Modifies the config temporarily (values changed don't get saved to the
    /// file). Called after into_variables().
    fn data_correct_temporary(&self) {}
}

impl NewConfigBase {
    pub fn new(file: &str, template: Rc<RefCell<CfgValue>>) -> Self {
        let raw_root = CfgValue::parse_json("root", None);
        let base = Self {
            file_name: file.to_string(),
            root: template,
            raw_root: RefCell::new(raw_root),
        };
        // (Java's NewConfig constructor dispatches loadAndSave virtually to the
        // subclass; here the subclass drives the single load itself.)
        base
    }

    pub fn load_and_save(&self, hooks: &dyn ConfigHooks) {
        let dont_save = self.from_file(hooks);
        let corrected = hooks.data_correct_permanent();
        match corrected {
            Ok(()) => {
                if !dont_save {
                    self.to_file(hooks);
                }
                if hooks.into_variables().is_err() {
                    hooks.problem_reading();
                }
                hooks.data_correct_temporary();
            }
            Err(_) => {
                hooks.problem_reading();
                seams::log_info("Exception while loading config");
            }
        }
    }

    pub fn save(&self, hooks: &dyn ConfigHooks) {
        self.to_file(hooks);
        let _ = hooks.into_variables();
        hooks.data_correct_temporary();
    }

    pub(crate) fn from_file(&self, hooks: &dyn ConfigHooks) -> bool {
        let cfgfile = seams::config_dir().join(&self.file_name);
        let mut dont_save = false;
        match std::fs::read_to_string(&cfgfile) {
            Ok(data) => {
                let parsed: Result<Json, _> = serde_json::from_str(&data);
                match parsed {
                    Ok(json_data) => {
                        let raw_root = CfgValue::parse_json("root", Some(json_data));
                        *self.raw_root.borrow_mut() = raw_root.clone();
                        CfgValue::match_values(&raw_root, &self.root);
                    }
                    Err(_) => {
                        hooks.problem_reading();
                        dont_save = true;
                    }
                }
            }
            Err(_) => {
                hooks.file_not_found();
            }
        }
        dont_save
    }

    pub(crate) fn to_file(&self, hooks: &dyn ConfigHooks) {
        let cfgfile = seams::config_dir().join(&self.file_name);
        let data = serde_json::to_string_pretty(&CfgValue::export_json(&self.root))
            .unwrap_or_else(|_| "{}".to_string());
        match std::fs::write(&cfgfile, data) {
            Ok(()) => {}
            Err(_) => {
                hooks.problem_writing();
            }
        }
    }

    pub fn get_config_for_modifying(&self) -> Rc<RefCell<CfgValue>> {
        self.root.clone()
    }
}

struct NopHooks;
impl ConfigHooks for NopHooks {}

/// Gson's pretty printing emits `null` for a null value; serde_json matches.
pub fn gson_null() -> Json {
    json!(null)
}
