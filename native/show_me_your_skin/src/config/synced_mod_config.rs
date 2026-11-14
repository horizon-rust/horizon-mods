//! nl.enjarai.showmeyourskin.config.SyncedModConfig (1:1). The serde field
//! names are the upstream ENDEC optional-field keys; the Gson form (Java
//! record component names, what ModConfigServer's file holds) goes through
//! to_gson/from_gson.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncedModConfig {
    #[serde(rename = "allow_not_show_in_combat", default = "yes")]
    pub allow_not_show_in_combat: bool,
    #[serde(rename = "allow_not_show_name_tag", default = "yes")]
    pub allow_not_show_name_tag: bool,
    #[serde(rename = "allow_not_force_elytra_when_flying", default = "yes")]
    pub allow_not_force_elytra_when_flying: bool,
}

fn yes() -> bool {
    true
}

fn obj(v: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
    v.as_object()
        .map(|m| m as &Map<String, Value>)
        .unwrap_or(&*EMPTY)
}

impl SyncedModConfig {
    /// Java record ctor: SyncedModConfig(allowNotShowInCombat,
    /// allowNotShowNameTag, allowNotForceElytraWhenFlying).
    pub fn new(
        allow_not_show_in_combat: bool,
        allow_not_show_name_tag: bool,
        allow_not_force_elytra_when_flying: bool,
    ) -> SyncedModConfig {
        SyncedModConfig {
            allow_not_show_in_combat,
            allow_not_show_name_tag,
            allow_not_force_elytra_when_flying,
        }
    }

    /// Gson form (record component names, ModConfigServer's file).
    pub fn to_gson(&self) -> Value {
        json!({
            "allowNotShowInCombat": self.allow_not_show_in_combat,
            "allowNotShowNameTag": self.allow_not_show_name_tag,
            "allowNotForceElytraWhenFlying": self.allow_not_force_elytra_when_flying,
        })
    }

    pub fn from_gson(v: &Value) -> SyncedModConfig {
        let m = obj(v);
        SyncedModConfig {
            allow_not_show_in_combat: m
                .get("allowNotShowInCombat")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            allow_not_show_name_tag: m
                .get("allowNotShowNameTag")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            allow_not_force_elytra_when_flying: m
                .get("allowNotForceElytraWhenFlying")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_fields() {
        let c = SyncedModConfig::new(false, false, false);
        assert!(!c.allow_not_show_in_combat);
        assert!(!c.allow_not_show_name_tag);
        assert!(!c.allow_not_force_elytra_when_flying);
    }

    #[test]
    fn endec_optional_defaults_true() {
        // Java: optionalFieldOf(..., true) - absent keys deserialize to true.
        let parsed: SyncedModConfig = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed, SyncedModConfig::new(true, true, true));
        let v = SyncedModConfig::new(false, true, false);
        let enc = serde_json::to_value(&v).unwrap();
        assert_eq!(enc["allow_not_show_in_combat"], Value::Bool(false));
        let round: SyncedModConfig = serde_json::from_value(enc).unwrap();
        assert_eq!(round, v);
    }

    #[test]
    fn gson_form_round_trip() {
        let c = SyncedModConfig::new(true, false, true);
        let g = c.to_gson();
        assert_eq!(g["allowNotShowInCombat"], Value::Bool(true));
        assert!(g.get("allow_not_show_in_combat").is_none());
        assert_eq!(SyncedModConfig::from_gson(&g), c);
    }
}
