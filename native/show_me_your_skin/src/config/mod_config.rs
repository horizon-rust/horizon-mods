//! nl.enjarai.showmeyourskin.config.ModConfig (1:1). The config file is the
//! Gson form (Java field names); the client-level Minecraft calls go through
//! seams::with_level_access exactly where the Java code calls them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::armor_config::ArmorConfig;
use crate::MODID;
use crate::seams;

fn obj(v: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
    v.as_object()
        .map(|m| m as &Map<String, Value>)
        .unwrap_or(&*EMPTY)
}

/// Java: public record LocalOverride(String title, ArmorConfig config).
/// Gson keeps the null entries that ensureValid() later removes, so both
/// fields stay Option and serialize as null.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct LocalOverride {
    #[serde(rename = "title")]
    pub title: Option<String>,
    #[serde(rename = "config")]
    pub config: Option<ArmorConfig>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModConfig {
    #[serde(rename = "globalEnabled")]
    pub global_enabled: bool,
    #[serde(rename = "overridesEnabledInServerMode")]
    pub overrides_enabled_in_server_mode: bool,
    #[serde(rename = "combatCooldown")]
    pub combat_cooldown: f32,
    #[serde(rename = "fadeOutTime")]
    pub fade_out_time: f32,
    #[serde(rename = "global")]
    pub global: ArmorConfig,
    #[serde(rename = "overrides")]
    pub overrides: HashMap<Uuid, LocalOverride>,
}

impl Default for ModConfig {
    fn default() -> ModConfig {
        ModConfig {
            global_enabled: true,
            overrides_enabled_in_server_mode: false,
            combat_cooldown: 16.0,
            fade_out_time: 2.0,
            global: ArmorConfig::vanilla_values(),
            overrides: HashMap::new(),
        }
    }
}

/// Java: public static final Path CONFIG_FILE = ...getConfigDir().resolve(MODID + ".json").
pub fn config_file() -> PathBuf {
    seams::config_dir().join(format!("{}.json", MODID))
}

thread_local! {
    /// Java: public static ModConfig INSTANCE = loadConfigFile(CONFIG_FILE, new ModConfig()).
    static INSTANCE: RefCell<Option<ModConfig>> = const { RefCell::new(None) };
}

/// Java: AbstractModConfig#loadConfigFile(file, default): parse the file when
/// present, fall back to the default, then save so new fields get written.
pub fn load_config_file(file: &PathBuf, default: ModConfig) -> ModConfig {
    let loaded = std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<ModConfig>(&text).ok())
        .unwrap_or(default);
    write_gson(file, &loaded);
    loaded
}

/// Java: cicada's Gson save path (pretty, null fields written).
fn write_gson(file: &PathBuf, cfg: &ModConfig) {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let gson = to_gson(cfg);
    let _ = std::fs::write(
        file,
        serde_json::to_string_pretty(&gson).unwrap_or_default(),
    );
}

pub fn to_gson(cfg: &ModConfig) -> Value {
    let overrides: Map<String, Value> = cfg
        .overrides
        .iter()
        .map(|(uuid, o)| {
            (
                uuid.to_string(),
                json!({ "title": o.title, "config": o.config.as_ref().map(|c| c.to_gson()) }),
            )
        })
        .collect();
    json!({
        "globalEnabled": cfg.global_enabled,
        "overridesEnabledInServerMode": cfg.overrides_enabled_in_server_mode,
        "combatCooldown": cfg.combat_cooldown,
        "fadeOutTime": cfg.fade_out_time,
        "global": cfg.global.to_gson(),
        "overrides": Value::Object(overrides),
    })
}

pub fn from_gson(v: &Value) -> ModConfig {
    let m = obj(v);
    let mut cfg = ModConfig::default();
    if let Some(ov) = m.get("overrides").and_then(Value::as_object) {
        cfg.overrides = ov
            .iter()
            .filter_map(|(k, o)| {
                let uuid = Uuid::parse_str(k).ok()?;
                Some((
                    uuid,
                    LocalOverride {
                        title: o.get("title").and_then(Value::as_str).map(str::to_string),
                        config: o.get("config").map(ArmorConfig::from_gson),
                    },
                ))
            })
            .collect();
    }
    if let Some(x) = m.get("globalEnabled").and_then(Value::as_bool) {
        cfg.global_enabled = x;
    }
    if let Some(x) = m
        .get("overridesEnabledInServerMode")
        .and_then(Value::as_bool)
    {
        cfg.overrides_enabled_in_server_mode = x;
    }
    if let Some(x) = m.get("combatCooldown").and_then(Value::as_f64) {
        cfg.combat_cooldown = x as f32;
    }
    if let Some(x) = m.get("fadeOutTime").and_then(Value::as_f64) {
        cfg.fade_out_time = x as f32;
    }
    if let Some(x) = m.get("global") {
        cfg.global = ArmorConfig::from_gson(x);
    }
    cfg
}

/// Java: ModConfig.INSTANCE reads. The shared instance initializes from the
/// config file on first access (Java static init).
pub fn with_instance<R>(f: impl FnOnce(&ModConfig) -> R) -> R {
    INSTANCE.with(|i| {
        let mut slot = i.borrow_mut();
        if slot.is_none() {
            *slot = Some(load_config_file(&config_file(), ModConfig::default()));
        }
        f(slot.as_ref().unwrap())
    })
}

/// Java: ModConfig.INSTANCE field writes/mutations.
pub fn with_instance_mut<R>(f: impl FnOnce(&mut ModConfig) -> R) -> R {
    INSTANCE.with(|i| {
        let mut slot = i.borrow_mut();
        if slot.is_none() {
            *slot = Some(load_config_file(&config_file(), ModConfig::default()));
        }
        f(slot.as_mut().unwrap())
    })
}

/// Java: set INSTANCE directly (config reload).
pub fn set_instance(cfg: ModConfig) {
    INSTANCE.with(|i| *i.borrow_mut() = Some(cfg));
}

impl ModConfig {
    /// Java: getOverrideOrGlobal(uuid).
    pub fn get_override_or_global(&self, uuid: Uuid) -> ArmorConfig {
        let server_available = seams::with_level_access(|l| l.server_available);
        let use_client_values = self.overrides_enabled_in_server_mode || !server_available;

        if use_client_values {
            if let Some(config) = self.overrides.get(&uuid).and_then(|o| o.config.clone()) {
                return config;
            }
        }

        if server_available {
            if let Some(config) = seams::with_level_access(|l| (l.player_configs.clone())(uuid)) {
                if config != ArmorConfig::vanilla_values() {
                    return config;
                }
            }
        }

        if use_client_values {
            return self.global.clone();
        }

        ArmorConfig::vanilla_values()
    }

    /// Java: getApplicable(uuid).
    pub fn get_applicable(&self, uuid: Uuid) -> ArmorConfig {
        if !self.global_enabled {
            return ArmorConfig::vanilla_values();
        }

        let mut applicable = self.get_override_or_global(uuid);
        if applicable.show_in_combat && seams::is_in_combat(uuid) {
            return ArmorConfig::vanilla_values();
        }

        applicable = applicable.copy();

        // Java: level != null gate then player.isFallFlying().
        if applicable.force_elytra_when_flying && seams::with_level_access(|l| l.level_available) {
            let flying = seams::with_level_access(|l| (l.player_fall_flying.clone())(uuid));
            if flying == Some(true) {
                applicable.elytra = ArmorConfig::vanilla_values().elytra;
            }
        }

        applicable
    }

    /// Java: getOverride(uuid) - overrides.get(uuid).config, which NPEs in
    /// Java when the entry is absent; None is that absent state.
    pub fn get_override(&self, uuid: Uuid) -> Option<ArmorConfig> {
        self.overrides.get(&uuid).and_then(|o| o.config.clone())
    }

    /// Java: deleteOverride(uuid).
    pub fn delete_override(&mut self, uuid: Uuid) {
        self.overrides.remove(&uuid);
    }

    /// Java: ensureValid().
    pub fn ensure_valid(&mut self) {
        if self.combat_cooldown < 0.0 {
            self.combat_cooldown = 0.0;
        }
        if self.fade_out_time < 0.0 {
            self.fade_out_time = 0.0;
        }
        if self.fade_out_time > self.combat_cooldown {
            self.fade_out_time = self.combat_cooldown;
        }

        self.global.ensure_valid();
        // Fix to remove old overrides entries
        self.overrides
            .retain(|_, o| o.config.is_some() && o.title.is_some());
        for o in self.overrides.values_mut() {
            if let Some(config) = o.config.as_mut() {
                config.ensure_valid();
            }
        }

        self.save();
    }

    /// Java: AbstractModConfig#save().
    pub fn save(&self) {
        write_gson(&config_file(), self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::armor_config::PieceConfig;
    use crate::seams::{LevelAccess, set_config_dir, set_level_access};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smsys-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn p1() -> Uuid {
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
    }
    fn p2() -> Uuid {
        Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
    }

    fn offline() {
        set_level_access(LevelAccess::noop());
        crate::seams::set_combat_check(std::rc::Rc::new(|_| false));
    }

    #[test]
    fn override_or_global_offline() {
        offline();
        let mut cfg = ModConfig::default();
        cfg.overrides.insert(
            p1(),
            LocalOverride {
                title: Some("p1".into()),
                config: Some(ArmorConfig {
                    show_name_tag: false,
                    ..ArmorConfig::vanilla_values()
                }),
            },
        );
        // override wins when present
        assert!(!cfg.get_override_or_global(p1()).show_name_tag);
        // absent override falls back to global
        assert!(cfg.get_override_or_global(p2()).show_name_tag);
        cfg.global_enabled = false;
        // getApplicable short-circuits to vanilla when the global switch is off
        assert_eq!(cfg.get_applicable(p1()), ArmorConfig::vanilla_values());
    }

    #[test]
    fn server_mode_uses_component_config() {
        let mut cfg = ModConfig::default();
        cfg.overrides_enabled_in_server_mode = false;
        let mut server_cfg = ArmorConfig::vanilla_values();
        server_cfg.shield.set_all(false);
        let level = LevelAccess {
            server_available: true,
            level_available: true,
            player_configs: std::rc::Rc::new(move |uuid| {
                if uuid == p2() {
                    Some(server_cfg.clone())
                } else {
                    Some(ArmorConfig::vanilla_values())
                }
            }),
            player_fall_flying: std::rc::Rc::new(|_| None),
        };
        set_level_access(level);
        // server component config (non-vanilla) wins over global
        assert!(!cfg.get_override_or_global(p2()).shield.base);
        // server component config equal to vanilla falls through to global
        assert!(cfg.get_override_or_global(p1()).shield.base);
        // with the server live and no override, use_client_values is false,
        // so global is never returned: vanilla values come back
        assert_eq!(
            cfg.get_override_or_global(p1()),
            ArmorConfig::vanilla_values()
        );
    }

    #[test]
    fn combat_and_flying_gates() {
        offline();
        crate::seams::set_combat_check(std::rc::Rc::new(|uuid| uuid == p1()));
        let mut cfg = ModConfig::default();
        cfg.global.show_in_combat = true;
        // in-combat player with showInCombat: vanilla
        assert_eq!(cfg.get_applicable(p1()), ArmorConfig::vanilla_values());
        // out of combat: global applies
        assert!(cfg.get_applicable(p2()).head.base);
        crate::seams::set_combat_check(std::rc::Rc::new(|_| false));

        // forceElytraWhenFlying + a flying player resets the elytra piece
        cfg.global.elytra.set_all(false);
        let level = LevelAccess {
            server_available: false,
            level_available: true,
            player_configs: std::rc::Rc::new(|_| None),
            player_fall_flying: std::rc::Rc::new(move |uuid| Some(uuid == p2())),
        };
        set_level_access(level);
        assert_eq!(
            cfg.get_applicable(p2()).elytra,
            PieceConfig::vanilla_values()
        );
        assert_eq!(cfg.get_applicable(p1()).elytra, cfg.global.elytra);
        offline();
    }

    #[test]
    fn ensure_valid_clamps_and_prunes() {
        offline();
        let dir = temp_dir("ensure-valid");
        set_config_dir(dir.clone());
        let mut cfg = ModConfig::default();
        cfg.combat_cooldown = -1.0;
        cfg.fade_out_time = 100.0;
        cfg.overrides.insert(
            p1(),
            LocalOverride {
                title: None,
                config: None,
            },
        );
        let mut keep = ArmorConfig::vanilla_values();
        keep.head.set_all(false);
        cfg.overrides.insert(
            p2(),
            LocalOverride {
                title: Some("p2".into()),
                config: Some(keep.clone()),
            },
        );

        cfg.ensure_valid();
        assert_eq!(cfg.combat_cooldown, 0.0);
        assert_eq!(cfg.fade_out_time, 0.0);
        assert!(cfg.overrides.contains_key(&p2()));
        assert!(!cfg.overrides.contains_key(&p1()));
        // the clamped config was saved to the Gson file
        let text = std::fs::read_to_string(dir.join("showmeyourskin.json")).unwrap();
        assert!(text.contains("\"combatCooldown\": 0.0"));
        let reread = from_gson(&serde_json::from_str::<Value>(&text).unwrap());
        assert_eq!(reread, cfg);
    }

    #[test]
    fn delete_and_get_override() {
        offline();
        let mut cfg = ModConfig::default();
        cfg.overrides.insert(
            p1(),
            LocalOverride {
                title: Some("p1".into()),
                config: Some(ArmorConfig::vanilla_values()),
            },
        );
        assert!(cfg.get_override(p1()).is_some());
        cfg.delete_override(p1());
        // Java getOverride NPEs on the absent entry; None is that state
        assert!(cfg.get_override(p1()).is_none());
    }
}
