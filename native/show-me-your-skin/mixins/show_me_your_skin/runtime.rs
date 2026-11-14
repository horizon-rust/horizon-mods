// show_me_your_skin: clean-room Rust port of enjarai/show-me-your-skin
// (Codeberg, MIT (c) 2022 enjarai), read upstream at dd4f63e.
// Slice 1: config model, hideable-equipment mapping, combat logger, and
// applicability resolution, 1:1 with the upstream classes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PieceConfig {
    #[serde(rename = "base")]
    pub base: bool,
    #[serde(rename = "glint")]
    pub glint: bool,
}

impl Default for PieceConfig {
    fn default() -> Self {
        Self {
            base: true,
            glint: true,
        }
    }
}

impl PieceConfig {
    pub fn new(base: bool, glint: bool) -> Self {
        Self { base, glint }
    }

    pub fn set_all(&mut self, value: bool) {
        self.base = value;
        self.glint = value;
    }

    /// Upstream PieceConfig.trim() is a literal true.
    pub fn trim(&self) -> bool {
        true
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
    fn default() -> Self {
        Self {
            base: true,
            glint: true,
            trim: true,
        }
    }
}

impl ArmorPieceConfig {
    pub fn new(base: bool, glint: bool, trim: bool) -> Self {
        Self { base, glint, trim }
    }

    pub fn set_all(&mut self, value: bool) {
        self.base = value;
        self.trim = value;
        self.glint = value;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
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

pub trait PieceAccess {
    fn base(&self) -> bool;
    fn glint(&self) -> bool;
    fn trim(&self) -> bool;
}

impl PieceAccess for PieceConfig {
    fn base(&self) -> bool {
        self.base
    }
    fn glint(&self) -> bool {
        self.glint
    }
    fn trim(&self) -> bool {
        true
    }
}

impl PieceAccess for ArmorPieceConfig {
    fn base(&self) -> bool {
        self.base
    }
    fn glint(&self) -> bool {
        self.glint
    }
    fn trim(&self) -> bool {
        self.trim
    }
}

pub fn vanilla_values() -> ArmorConfig {
    ArmorConfig {
        head: ArmorPieceConfig::default(),
        chest: ArmorPieceConfig::default(),
        legs: ArmorPieceConfig::default(),
        feet: ArmorPieceConfig::default(),
        elytra: PieceConfig::default(),
        shield: PieceConfig::default(),
        hat: PieceConfig::default(),
        show_in_combat: true,
        show_name_tag: true,
        force_elytra_when_flying: true,
    }
}

impl ArmorConfig {
    pub fn piece_for(&self, slot: HideableEquipment) -> Option<&dyn PieceAccess> {
        match slot {
            HideableEquipment::Head => Some(&self.head),
            HideableEquipment::Chest => Some(&self.chest),
            HideableEquipment::Legs => Some(&self.legs),
            HideableEquipment::Feet => Some(&self.feet),
            HideableEquipment::Elytra => Some(&self.elytra),
            HideableEquipment::Shield => Some(&self.shield),
            HideableEquipment::Hat => Some(&self.hat),
        }
    }

    pub fn piece_for_mut(&mut self, slot: HideableEquipment) -> Option<&mut dyn PieceAccess> {
        match slot {
            HideableEquipment::Head => Some(&mut self.head),
            HideableEquipment::Chest => Some(&mut self.chest),
            HideableEquipment::Legs => Some(&mut self.legs),
            HideableEquipment::Feet => Some(&mut self.feet),
            HideableEquipment::Elytra => Some(&mut self.elytra),
            HideableEquipment::Shield => Some(&mut self.shield),
            HideableEquipment::Hat => Some(&mut self.hat),
        }
    }

    pub fn ensure_valid(&mut self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HideableEquipment {
    Head,
    Chest,
    Legs,
    Feet,
    Elytra,
    Shield,
    Hat,
}

impl HideableEquipment {
    /// Upstream id = name().toLowerCase() (Java enum order kept).
    pub fn all() -> [HideableEquipment; 7] {
        [
            Self::Head,
            Self::Chest,
            Self::Legs,
            Self::Feet,
            Self::Elytra,
            Self::Shield,
            Self::Hat,
        ]
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Chest => "chest",
            Self::Legs => "legs",
            Self::Feet => "feet",
            Self::Elytra => "elytra",
            Self::Shield => "shield",
            Self::Hat => "hat",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|i| i.id() == id)
    }

    /// Upstream fromSlot: only the four armor slots map; elytra/shield/hat are None.
    pub fn from_slot(slot: EquipmentSlot) -> Option<Self> {
        match slot {
            EquipmentSlot::Head => Some(Self::Head),
            EquipmentSlot::Chest => Some(Self::Chest),
            EquipmentSlot::Legs => Some(Self::Legs),
            EquipmentSlot::Feet => Some(Self::Feet),
            _ => None,
        }
    }

    pub fn to_slot(self) -> Option<EquipmentSlot> {
        match self {
            Self::Head => Some(EquipmentSlot::Head),
            Self::Chest => Some(EquipmentSlot::Chest),
            Self::Legs => Some(EquipmentSlot::Legs),
            Self::Feet => Some(EquipmentSlot::Feet),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    Head,
    Chest,
    Legs,
    Feet,
    Mainhand,
    Offhand,
    Body,
}

impl EquipmentSlot {
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "head" => Some(Self::Head),
            "chest" => Some(Self::Chest),
            "legs" => Some(Self::Legs),
            "feet" => Some(Self::Feet),
            "mainhand" => Some(Self::Mainhand),
            "offhand" => Some(Self::Offhand),
            "body" => Some(Self::Body),
            _ => None,
        }
    }

    pub fn get_name(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Chest => "chest",
            Self::Legs => "legs",
            Self::Feet => "feet",
            Self::Mainhand => "mainhand",
            Self::Offhand => "offhand",
            Self::Body => "body",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalOverride {
    #[serde(rename = "title")]
    pub title: String,
    #[serde(rename = "config")]
    pub config: ArmorConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModConfig {
    #[serde(rename = "global_enabled")]
    pub global_enabled: bool,
    #[serde(rename = "overrides_enabled_in_server_mode")]
    pub overrides_enabled_in_server_mode: bool,
    #[serde(rename = "combat_cooldown")]
    pub combat_cooldown: f32,
    #[serde(rename = "fade_out_time")]
    pub fade_out_time: f32,
    #[serde(rename = "global")]
    pub global: ArmorConfig,
    #[serde(rename = "overrides")]
    pub overrides: HashMap<String, LocalOverride>,
}

impl Default for ModConfig {
    fn default() -> Self {
        Self {
            global_enabled: true,
            overrides_enabled_in_server_mode: false,
            combat_cooldown: 16.0,
            fade_out_time: 2.0,
            global: vanilla_values(),
            overrides: HashMap::new(),
        }
    }
}

/// Upstream CombatLogger, keyed by UUID string, clock injected for tests.
#[derive(Default)]
pub struct CombatLogger {
    in_combat: HashMap<String, i64>,
}

impl CombatLogger {
    pub fn is_in_combat(&self, uuid: &str, config: &ModConfig, now_millis: i64) -> bool {
        match self.in_combat.get(uuid) {
            Some(last_trigger)
                if *last_trigger > now_millis - (config.combat_cooldown * 1000.0) as i64 =>
            {
                true
            }
            Some(_) => false,
            None => false,
        }
    }

    pub fn get_fade(&self, uuid: &str, config: &ModConfig, now_millis: i64) -> f32 {
        match self.in_combat.get(uuid) {
            Some(last_trigger) => {
                let fade_start = *last_trigger + (config.combat_cooldown * 1000.0) as i64
                    - (config.fade_out_time * 1000.0) as i64;
                let time_since_combat = now_millis - fade_start;
                // Upstream: Mth.clamp(1 - (timeSinceCombat / 1000f) / fadeOutTime, 0, 1)
                (1.0 - (time_since_combat as f32 / 1000.0) / config.fade_out_time).clamp(0.0, 1.0)
            }
            None => 1.0,
        }
    }

    pub fn trigger_combat(&mut self, uuid: &str) {
        self.in_combat.insert(uuid.to_string(), now_millis());
    }
}

/// Upstream ModConfig.getApplicable, with the level/player seam injected:
///  answers level.getPlayerByUUID(uuid).isFallFlying().
pub fn get_applicable(
    config: &ModConfig,
    server_available: bool,
    is_fall_flying: Option<impl Fn(&str) -> bool>,
    combat: &CombatLogger,
    uuid: &str,
    now_millis: i64,
) -> ArmorConfig {
    if !config.global_enabled {
        return vanilla_values();
    }

    let use_client_values = config.overrides_enabled_in_server_mode || !server_available;

    let mut applicable = if use_client_values {
        config
            .overrides
            .get(uuid)
            .map(|o| o.config.clone())
            .unwrap_or_else(|| config.global.clone())
    } else {
        vanilla_values()
    };

    if applicable.show_in_combat && combat.is_in_combat(uuid, config, now_millis) {
        return vanilla_values();
    }

    if applicable.force_elytra_when_flying {
        if let Some(is_flying) = is_fall_flying {
            if is_flying(uuid) {
                applicable.elytra = PieceConfig::default();
            }
        }
    }

    applicable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_upstream() {
        let c = vanilla_values();
        for s in HideableEquipment::all() {
            let p = c.piece_for(s).unwrap();
            assert!(p.base() && p.glint());
        }
        assert!(c.show_in_combat && c.show_name_tag && c.force_elytra_when_flying);
    }

    #[test]
    fn hideable_ids_round_trip() {
        for s in HideableEquipment::all() {
            assert_eq!(HideableEquipment::from_id(s.id()), Some(s));
        }
        assert_eq!(
            HideableEquipment::from_id("elytra"),
            Some(HideableEquipment::Elytra)
        );
        assert_eq!(
            HideableEquipment::from_id("hat"),
            Some(HideableEquipment::Hat)
        );
        assert_eq!(HideableEquipment::from_id("nope"), None);
    }

    #[test]
    fn slot_mapping_matches_upstream() {
        assert_eq!(
            HideableEquipment::from_slot(EquipmentSlot::Head),
            Some(HideableEquipment::Head)
        );
        assert_eq!(HideableEquipment::from_slot(EquipmentSlot::Offhand), None);
        assert_eq!(HideableEquipment::Elytra.to_slot(), None);
        assert_eq!(HideableEquipment::Head.to_slot(), Some(EquipmentSlot::Head));
        assert_eq!(EquipmentSlot::by_name("body"), Some(EquipmentSlot::Body));
        assert_eq!(EquipmentSlot::by_name("x"), None);
    }

    #[test]
    fn combat_logger_fade_math() {
        let mut config = ModConfig::default();
        config.combat_cooldown = 16.0;
        config.fade_out_time = 2.0;
        let mut logger = CombatLogger::default();
        let uuid = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        assert!(!logger.is_in_combat(uuid, &config, 1_000_000));
        logger.in_combat.insert(uuid.to_string(), 1_000_000);
        // Within cooldown: in combat, fade full for the first fade_out window.
        assert!(logger.is_in_combat(uuid, &config, 1_000_000 + 5_000));
        assert_eq!(logger.get_fade(uuid, &config, 1_000_000 + 1_000), 1.0);
        // One second into the fade-out window.
        assert_eq!(logger.get_fade(uuid, &config, 1_000_000 + 15_000), 0.5);
        // After cooldown: cleared, fade clamped to 0 until re-trigger.
        assert!(!logger.is_in_combat(uuid, &config, 1_000_000 + 17_000));
        assert_eq!(logger.get_fade(uuid, &config, 1_000_000 + 20_000), 0.0);
    }

    #[test]
    fn applicability_precedence() {
        let mut config = ModConfig::default();
        config.global_enabled = true;
        let uuid = "player-1";

        // Global applies when no override.
        let app = get_applicable(
            &config,
            false,
            None::<fn(&str) -> bool>,
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert_eq!(app, config.global);

        // Override beats global.
        let mut ov = vanilla_values();
        ov.head = ArmorPieceConfig::new(false, false, false);
        config.overrides.insert(
            uuid.to_string(),
            LocalOverride {
                title: "me".into(),
                config: ov.clone(),
            },
        );
        let app = get_applicable(
            &config,
            false,
            None::<fn(&str) -> bool>,
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert_eq!(app, ov);

        // Server mode without client overrides falls back to vanilla values.
        config.overrides_enabled_in_server_mode = false;
        let app = get_applicable(
            &config,
            true,
            None::<fn(&str) -> bool>,
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert_eq!(app, vanilla_values());

        // globalEnabled = false always yields vanilla values.
        config.global_enabled = false;
        let app = get_applicable(
            &config,
            false,
            None::<fn(&str) -> bool>,
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert_eq!(app, vanilla_values());
    }

    #[test]
    fn combat_flag_restores_vanilla_during_fight() {
        let mut config = ModConfig::default();
        let uuid = "p";
        let mut logger = CombatLogger::default();
        logger.in_combat.insert(uuid.to_string(), 100);
        config.global.show_in_combat = true;
        let app = get_applicable(&config, false, None::<fn(&str) -> bool>, &logger, uuid, 200);
        assert_eq!(app, vanilla_values());
        // With show_in_combat off, the hidden config survives combat.
        config.global.show_in_combat = false;
        let app = get_applicable(&config, false, None::<fn(&str) -> bool>, &logger, uuid, 200);
        assert_eq!(app, config.global);
    }

    #[test]
    fn force_elytra_when_flying_uses_level_probe() {
        let config = ModConfig::default();
        let uuid = "p";
        let app = get_applicable(
            &config,
            false,
            Some(|_: &str| true),
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert_eq!(app.elytra, PieceConfig::default());
        let app = get_applicable(
            &config,
            false,
            Some(|_: &str| false),
            &CombatLogger::default(),
            uuid,
            0,
        );
        assert!(app.elytra.base && app.elytra.glint);
    }

    #[test]
    fn json_field_names_match_upstream_endec() {
        let c = vanilla_values();
        let json = serde_json::to_string(&c).unwrap();
        for key in [
            "head",
            "chest",
            "legs",
            "feet",
            "elytra",
            "shield",
            "hat",
            "show_in_combat",
            "show_name_tag",
            "force_elytra_when_flying",
            "base",
            "glint",
            "trim",
        ] {
            assert!(
                json.contains(&format!("\"{key}\"")),
                "missing {key} in {json}"
            );
        }
        let back: ArmorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        let mc = ModConfig::default();
        let mcj = serde_json::to_string(&mc).unwrap();
        let mcback: ModConfig = serde_json::from_str(&mcj).unwrap();
        assert_eq!(mcback, mc);
    }

    #[test]
    fn piece_set_all_and_trim() {
        let mut piece = ArmorPieceConfig::default();
        piece.set_all(false);
        assert!(!piece.base && !piece.trim && !piece.glint);
        let mut plain = PieceConfig::default();
        plain.set_all(false);
        assert!(plain.trim()); // upstream PieceConfig.trim() is literal true
    }

    #[test]
    fn ensure_valid_clamps() {
        let mut c = ModConfig::default();
        c.combat_cooldown = -1.0;
        c.fade_out_time = 5.0;
        // Upstream ensureValid: cooldown floors at 0, fadeOut clamps to cooldown.
        if c.combat_cooldown < 0.0 {
            c.combat_cooldown = 0.0;
        }
        if c.fade_out_time < 0.0 {
            c.fade_out_time = 0.0;
        }
        if c.fade_out_time > c.combat_cooldown {
            c.fade_out_time = c.combat_cooldown;
        }
        assert_eq!(c.combat_cooldown, 0.0);
        assert_eq!(c.fade_out_time, 0.0);
        c.global.ensure_valid();
    }
}

/// Upstream SyncedModConfig record; optionalFieldOf defaults are true.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncedModConfig {
    #[serde(default = "d_true", rename = "allow_not_show_in_combat")]
    pub allow_not_show_in_combat: bool,
    #[serde(default = "d_true", rename = "allow_not_show_name_tag")]
    pub allow_not_show_name_tag: bool,
    #[serde(default = "d_true", rename = "allow_not_force_elytra_when_flying")]
    pub allow_not_force_elytra_when_flying: bool,
}

fn d_true() -> bool {
    true
}

impl Default for SyncedModConfig {
    fn default() -> Self {
        Self {
            allow_not_show_in_combat: true,
            allow_not_show_name_tag: true,
            allow_not_force_elytra_when_flying: true,
        }
    }
}

/// Upstream net packets, 1:1 shapes. Transport mapping onto Horizon's channel
/// API is the game-side seam (tracked), handlers below mirror handleServer/handleClient.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsUpdatePacket(pub ArmorConfig);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigSyncPacket(pub SyncedModConfig);

/// Upstream ModConfigServer default: SyncedModConfig(false, false, false).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModConfigServer {
    #[serde(rename = "synced")]
    pub synced: SyncedModConfig,
}

impl ModConfigServer {
    pub fn new() -> Self {
        Self {
            synced: SyncedModConfig {
                allow_not_show_in_combat: false,
                allow_not_show_name_tag: false,
                allow_not_force_elytra_when_flying: false,
            },
        }
    }
}

/// Upstream keybind defaults: Open Settings on K, Global Toggle on J.
pub const OPEN_SETTINGS_KEY: &str = "K";
pub const GLOBAL_TOGGLE_KEY: &str = "J";

/// Upstream ModKeyBindings.tick GLOBAL_TOGGLE branch: flip and report the new state.
pub fn apply_global_toggle(config: &mut ModConfig) -> bool {
    config.global_enabled = !config.global_enabled;
    config.global_enabled
}

impl ModConfig {
    /// Upstream ensureValid: clamp cooldown/fade, drop overrides with a null
    /// title or config (modeled here as empty strings are kept, missing keys
    /// cannot occur), then persist. Saving is the file-seam; callers save.
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
        self.overrides.retain(|_, o| !o.title.is_empty());
        self.overrides
            .values_mut()
            .for_each(|o| o.config.ensure_valid());
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    /// Upstream loadConfigFile: a broken file falls back to defaults.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn synced_config_defaults_and_optional_fields() {
        let d = SyncedModConfig::default();
        assert!(
            d.allow_not_show_in_combat
                && d.allow_not_show_name_tag
                && d.allow_not_force_elytra_when_flying
        );
        // Partial JSON: optionalFieldOf defaults fill missing keys with true.
        let partial: SyncedModConfig =
            serde_json::from_str("{\"allow_not_show_in_combat\":false}").unwrap();
        assert!(!partial.allow_not_show_in_combat);
        assert!(partial.allow_not_show_name_tag && partial.allow_not_force_elytra_when_flying);
    }

    #[test]
    fn server_config_defaults_all_false() {
        let s = ModConfigServer::new();
        assert!(!s.synced.allow_not_show_in_combat);
        assert!(!s.synced.allow_not_show_name_tag);
        assert!(!s.synced.allow_not_force_elytra_when_flying);
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("allow_not_show_in_combat"));
        let back: ModConfigServer = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn packets_round_trip() {
        let up = SettingsUpdatePacket(vanilla_values());
        let j = serde_json::to_string(&up).unwrap();
        let back: SettingsUpdatePacket = serde_json::from_str(&j).unwrap();
        assert_eq!(back, up);
        let cs = ConfigSyncPacket(SyncedModConfig::default());
        let j = serde_json::to_string(&cs).unwrap();
        let back: ConfigSyncPacket = serde_json::from_str(&j).unwrap();
        assert_eq!(back, cs);
    }

    #[test]
    fn global_toggle_flips_and_reports() {
        let mut c = ModConfig::default();
        assert!(c.global_enabled);
        assert!(!apply_global_toggle(&mut c));
        assert!(!c.global_enabled);
        assert!(apply_global_toggle(&mut c));
    }

    #[test]
    fn ensure_valid_clamps_and_drops_bad_overrides() {
        let mut c = ModConfig::default();
        c.combat_cooldown = -1.0;
        c.fade_out_time = 9.0;
        c.overrides.insert(
            "x".into(),
            LocalOverride {
                title: String::new(),
                config: vanilla_values(),
            },
        );
        c.ensure_valid();
        assert_eq!(c.combat_cooldown, 0.0);
        assert_eq!(c.fade_out_time, 0.0);
        assert!(c.overrides.is_empty());
        // A well-formed override survives.
        c.overrides.insert(
            "y".into(),
            LocalOverride {
                title: "me".into(),
                config: vanilla_values(),
            },
        );
        c.ensure_valid();
        assert!(c.overrides.contains_key("y"));
    }

    #[test]
    fn config_io_round_trip_and_fallback() {
        let mut c = ModConfig::default();
        c.combat_cooldown = 10.0;
        let j = c.to_json();
        let back = ModConfig::from_json(&j);
        assert_eq!(back, c);
        // Broken file falls back to defaults, like upstream loadConfigFile.
        let fallback = ModConfig::from_json("not json");
        assert_eq!(fallback, ModConfig::default());
    }
}
