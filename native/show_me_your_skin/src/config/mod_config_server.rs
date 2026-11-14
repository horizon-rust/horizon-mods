//! nl.enjarai.showmeyourskin.config.ModConfigServer (1:1). The file is Gson
//! pretty-printed with nulls (upstream GSON builder settings).

use std::cell::RefCell;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::synced_mod_config::SyncedModConfig;
use crate::MODID;

fn obj(v: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
    v.as_object()
        .map(|m| m as &Map<String, Value>)
        .unwrap_or(&*EMPTY)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModConfigServer {
    #[serde(rename = "synced")]
    pub synced: SyncedModConfig,
}

impl Default for ModConfigServer {
    fn default() -> ModConfigServer {
        // Java field: public SyncedModConfig synced = new SyncedModConfig(false, false, false);
        ModConfigServer {
            synced: SyncedModConfig::new(false, false, false),
        }
    }
}

/// Java: public static final Path CONFIG_FILE = ...resolve(MODID + "-server.json").
pub fn config_file() -> PathBuf {
    crate::seams::config_dir().join(format!("{}-server.json", MODID))
}

thread_local! {
    /// Java: public static ModConfigServer INSTANCE;
    static INSTANCE: RefCell<Option<ModConfigServer>> = const { RefCell::new(None) };
}

/// Java: public static void load() { INSTANCE = loadConfigFile(CONFIG_FILE); }
pub fn load() {
    let cfg = load_config_file(&config_file());
    INSTANCE.with(|i| *i.borrow_mut() = Some(cfg));
}

/// Java: public void save().
pub fn save() {
    INSTANCE.with(|i| {
        if let Some(cfg) = i.borrow().as_ref() {
            save_config_file(&config_file(), cfg);
        }
    });
}

/// Java: loadConfigFile(file) - parse when present, default when the file is
/// absent or empty, always save back.
fn load_config_file(file: &PathBuf) -> ModConfigServer {
    let cfg = std::fs::read_to_string(file)
        .ok()
        .map(|text| from_gson(&serde_json::from_str::<Value>(&text).unwrap_or(Value::Null)))
        .unwrap_or_else(ModConfigServer::default);
    save_config_file(file, &cfg);
    cfg
}

fn save_config_file(file: &PathBuf, cfg: &ModConfigServer) {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let gson = json!({ "synced": cfg.synced.to_gson() });
    let _ = std::fs::write(
        file,
        serde_json::to_string_pretty(&gson).unwrap_or_default(),
    );
}

pub fn from_gson(v: &Value) -> ModConfigServer {
    let m = obj(v);
    ModConfigServer {
        synced: m
            .get("synced")
            .map(SyncedModConfig::from_gson)
            .unwrap_or_else(|| SyncedModConfig::new(false, false, false)),
    }
}

/// Java: INSTANCE assignment (tests and future reload paths).
pub fn set_instance(cfg: ModConfigServer) {
    INSTANCE.with(|i| *i.borrow_mut() = Some(cfg));
}

/// Java: ModConfigServer.INSTANCE reads.
pub fn with_instance<R>(f: impl FnOnce(Option<&ModConfigServer>) -> R) -> R {
    INSTANCE.with(|i| f(i.borrow().as_ref()))
}

/// Java: ModConfigServer.INSTANCE.synced() accessor (None falls back to the
/// all-false default, as an unset server config gates nothing).
pub fn synced() -> SyncedModConfig {
    with_instance(|i| i.map(|c| c.synced.clone()))
        .unwrap_or_else(|| SyncedModConfig::new(false, false, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_writes_defaults_then_reloads() {
        let dir = std::env::temp_dir().join(format!("smsys-server-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::seams::set_config_dir(dir.clone());

        load();
        let text = std::fs::read_to_string(dir.join("showmeyourskin-server.json")).unwrap();
        assert!(text.contains("\"synced\""));
        assert!(text.contains("\"allowNotShowInCombat\": false"));

        // a custom file reloads through the same path
        std::fs::write(
            dir.join("showmeyourskin-server.json"),
            "{\"synced\": {\"allowNotShowInCombat\": true}}",
        )
        .unwrap();
        load();
        with_instance(|i| assert_eq!(i.unwrap().synced.allow_not_show_in_combat, true));
    }
}
