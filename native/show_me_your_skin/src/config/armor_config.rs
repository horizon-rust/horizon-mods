//! nl.enjarai.showmeyourskin.config.ArmorConfig (1:1), with the nested
//! PieceConfig and ArmorPieceConfig. The serde field names are the upstream
//! ENDEC keys (StructEndecBuilder fieldOf strings); the Gson config-file form
//! (Java field names, camelCase) goes through to_gson/from_gson.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::seams::EquipmentSlot;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PieceConfig {
    #[serde(rename = "base")]
    pub base: bool,
    #[serde(rename = "glint")]
    pub glint: bool,
}

impl Default for PieceConfig {
    fn default() -> PieceConfig {
        PieceConfig {
            base: true,
            glint: true,
        }
    }
}

impl PieceConfig {
    pub fn new(base: bool, glint: bool) -> PieceConfig {
        PieceConfig { base, glint }
    }

    pub fn vanilla_values() -> PieceConfig {
        PieceConfig {
            base: true,
            glint: true,
        }
    }

    pub fn set_all(&mut self, value: bool) {
        self.base = value;
        self.glint = value;
    }

    pub fn trim(&self) -> bool {
        true
    }

    /// Java hashCode parity: 31-stacked Boolean.hashCode (1231 / 1237).
    pub fn hash_code(&self) -> i32 {
        let mut result = boolean_hash_code(self.base);
        result = 31 * result + boolean_hash_code(self.glint);
        result
    }

    pub fn to_gson(&self) -> Value {
        json!({ "base": self.base, "glint": self.glint })
    }

    pub fn from_gson(v: &Value) -> PieceConfig {
        let m = obj(v);
        PieceConfig {
            base: m.get("base").and_then(Value::as_bool).unwrap_or(true),
            glint: m.get("glint").and_then(Value::as_bool).unwrap_or(true),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmorPieceConfig {
    #[serde(rename = "base")]
    pub base: bool,
    #[serde(rename = "glint")]
    pub glint: bool,
    #[serde(rename = "trim")]
    pub trim: bool,
}

impl Default for ArmorPieceConfig {
    fn default() -> ArmorPieceConfig {
        ArmorPieceConfig {
            base: true,
            glint: true,
            trim: true,
        }
    }
}

impl ArmorPieceConfig {
    pub fn new(base: bool, glint: bool, trim: bool) -> ArmorPieceConfig {
        ArmorPieceConfig { base, glint, trim }
    }

    pub fn vanilla_values() -> ArmorPieceConfig {
        ArmorPieceConfig {
            base: true,
            glint: true,
            trim: true,
        }
    }

    pub fn set_all(&mut self, value: bool) {
        self.base = value;
        self.trim = value;
        self.glint = value;
    }

    /// Java override: returns the trim field (PieceConfig#trim returns true).
    pub fn trim(&self) -> bool {
        self.trim
    }

    pub fn hash_code(&self) -> i32 {
        let mut result = boolean_hash_code(self.base);
        result = 31 * result + boolean_hash_code(self.trim);
        result = 31 * result + boolean_hash_code(self.glint);
        result
    }

    pub fn to_gson(&self) -> Value {
        json!({ "base": self.base, "glint": self.glint, "trim": self.trim })
    }

    pub fn from_gson(v: &Value) -> ArmorPieceConfig {
        let m = obj(v);
        ArmorPieceConfig {
            base: m.get("base").and_then(Value::as_bool).unwrap_or(true),
            glint: m.get("glint").and_then(Value::as_bool).unwrap_or(true),
            trim: m.get("trim").and_then(Value::as_bool).unwrap_or(true),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmorConfig {
    #[serde(rename = "head")]
    pub head: ArmorPieceConfig,
    #[serde(rename = "chest")]
    pub chest: ArmorPieceConfig,
    #[serde(rename = "legs")]
    pub legs: ArmorPieceConfig,
    #[serde(rename = "feet")]
    pub feet: ArmorPieceConfig,
    #[serde(rename = "elytra")]
    pub elytra: PieceConfig,
    #[serde(rename = "shield")]
    pub shield: PieceConfig,
    #[serde(rename = "hat")]
    pub hat: PieceConfig,
    #[serde(rename = "show_in_combat")]
    pub show_in_combat: bool,
    #[serde(rename = "show_name_tag")]
    pub show_name_tag: bool,
    #[serde(rename = "force_elytra_when_flying")]
    pub force_elytra_when_flying: bool,
}

impl Default for ArmorConfig {
    fn default() -> ArmorConfig {
        ArmorConfig::vanilla_values()
    }
}

fn boolean_hash_code(b: bool) -> i32 {
    if b { 1231 } else { 1237 }
}

fn obj(v: &Value) -> &Map<String, Value> {
    v.as_object()
        .map(|m| m as &Map<String, Value>)
        .unwrap_or(&*EMPTY)
}

static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

impl ArmorConfig {
    /// Java: the default-constructed ArmorConfig (VANILLA_VALUES).
    pub fn vanilla_values() -> ArmorConfig {
        ArmorConfig {
            head: ArmorPieceConfig::vanilla_values(),
            chest: ArmorPieceConfig::vanilla_values(),
            legs: ArmorPieceConfig::vanilla_values(),
            feet: ArmorPieceConfig::vanilla_values(),
            elytra: PieceConfig::vanilla_values(),
            shield: PieceConfig::vanilla_values(),
            hat: PieceConfig::vanilla_values(),
            show_in_combat: true,
            show_name_tag: true,
            force_elytra_when_flying: true,
        }
    }

    /// Java: getEquipmentConfig(slot), IllegalArgumentException on the
    /// non-armor slots.
    pub fn get_equipment_config(&self, slot: EquipmentSlot) -> &ArmorPieceConfig {
        match slot {
            EquipmentSlot::Head => &self.head,
            EquipmentSlot::Chest => &self.chest,
            EquipmentSlot::Legs => &self.legs,
            EquipmentSlot::Feet => &self.feet,
            other => panic!("Unexpected value: {:?}", other),
        }
    }

    /// Java: ensureValid() is empty upstream.
    pub fn ensure_valid(&self) {}

    /// Java: copy(). Rust Clone is the same deep copy.
    pub fn copy(&self) -> ArmorConfig {
        self.clone()
    }

    pub fn clear(&mut self) {
        self.head = ArmorPieceConfig::vanilla_values();
        self.chest = ArmorPieceConfig::vanilla_values();
        self.legs = ArmorPieceConfig::vanilla_values();
        self.feet = ArmorPieceConfig::vanilla_values();
        self.elytra = PieceConfig::vanilla_values();
        self.shield = PieceConfig::vanilla_values();
        self.hat = PieceConfig::vanilla_values();
        self.show_in_combat = true;
        self.show_name_tag = true;
        self.force_elytra_when_flying = true;
    }

    /// Gson form (Java field names, what the config file holds).
    pub fn to_gson(&self) -> Value {
        json!({
            "head": self.head.to_gson(),
            "chest": self.chest.to_gson(),
            "legs": self.legs.to_gson(),
            "feet": self.feet.to_gson(),
            "elytra": self.elytra.to_gson(),
            "shield": self.shield.to_gson(),
            "hat": self.hat.to_gson(),
            "showInCombat": self.show_in_combat,
            "showNameTag": self.show_name_tag,
            "forceElytraWhenFlying": self.force_elytra_when_flying,
        })
    }

    pub fn from_gson(v: &Value) -> ArmorConfig {
        let m = obj(v);
        ArmorConfig {
            head: m
                .get("head")
                .map(ArmorPieceConfig::from_gson)
                .unwrap_or_else(ArmorPieceConfig::vanilla_values),
            chest: m
                .get("chest")
                .map(ArmorPieceConfig::from_gson)
                .unwrap_or_else(ArmorPieceConfig::vanilla_values),
            legs: m
                .get("legs")
                .map(ArmorPieceConfig::from_gson)
                .unwrap_or_else(ArmorPieceConfig::vanilla_values),
            feet: m
                .get("feet")
                .map(ArmorPieceConfig::from_gson)
                .unwrap_or_else(ArmorPieceConfig::vanilla_values),
            elytra: m
                .get("elytra")
                .map(PieceConfig::from_gson)
                .unwrap_or_else(PieceConfig::vanilla_values),
            shield: m
                .get("shield")
                .map(PieceConfig::from_gson)
                .unwrap_or_else(PieceConfig::vanilla_values),
            hat: m
                .get("hat")
                .map(PieceConfig::from_gson)
                .unwrap_or_else(PieceConfig::vanilla_values),
            show_in_combat: m
                .get("showInCombat")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            show_name_tag: m
                .get("showNameTag")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            force_elytra_when_flying: m
                .get("forceElytraWhenFlying")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_defaults() {
        let v = ArmorConfig::vanilla_values();
        assert!(v.head.base && v.head.glint && v.head.trim);
        assert!(v.elytra.base && v.elytra.glint);
        assert!(v.show_in_combat && v.show_name_tag && v.force_elytra_when_flying);
    }

    #[test]
    fn piece_trim_override() {
        let piece = PieceConfig::new(true, false);
        assert!(piece.trim());
        let armor = ArmorPieceConfig::new(true, false, false);
        assert!(!armor.trim());
        let mut armor2 = ArmorPieceConfig::default();
        armor2.set_all(false);
        assert_eq!(armor2, ArmorPieceConfig::new(false, false, false));
    }

    #[test]
    fn equipment_config_panics_on_non_armor() {
        let v = ArmorConfig::vanilla_values();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                v.get_equipment_config(EquipmentSlot::MainHand)
            }))
            .is_err()
        );
        assert_eq!(v.get_equipment_config(EquipmentSlot::Legs), &v.legs);
    }

    #[test]
    fn endec_and_gson_forms_round_trip() {
        let mut cfg = ArmorConfig::vanilla_values();
        cfg.head.set_all(false);
        cfg.show_in_combat = false;

        let enc = serde_json::to_string(&cfg).unwrap();
        assert!(enc.contains("\"show_in_combat\":false"));
        assert_eq!(serde_json::from_str::<ArmorConfig>(&enc).unwrap(), cfg);

        let gson = cfg.to_gson();
        assert_eq!(gson["showInCombat"], Value::Bool(false));
        assert!(gson.get("show_in_combat").is_none());
        assert_eq!(ArmorConfig::from_gson(&gson), cfg);
    }

    #[test]
    fn hash_code_parity() {
        let p = PieceConfig::new(true, true);
        assert_eq!(p.hash_code(), 31 * 1231 + 1231);
        let a = ArmorPieceConfig::new(false, false, true);
        assert_eq!(a.hash_code(), 31 * (31 * 1237 + 1231) + 1237);
    }

    #[test]
    fn clear_resets() {
        let mut cfg = ArmorConfig::vanilla_values();
        cfg.hat.set_all(false);
        cfg.show_name_tag = false;
        cfg.clear();
        assert_eq!(cfg, ArmorConfig::vanilla_values());
    }
}
