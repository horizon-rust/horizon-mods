//! Upstream: common/config/ (1:1). Gson pretty-printing maps to serde_json {:#}; Java field names kept.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;

use crate::compat;
use crate::global;
use crate::network::UpdateChannelC2SPacket;
use crate::seams;

// ---------------------------------------------------------------------------

/// Upstream: config/IConfig.java
pub trait IConfig {
    fn validate(&mut self);
    fn on_update(&mut self);
    /// Lombok-generated @ToString snapshot, used in the "Saved <config>" / "Loaded <config>" log lines.
    fn to_string_snapshot(&self) -> String;
}

// ---------------------------------------------------------------------------
// Enums. Each keeps get(String) (case-insensitive, null -> None) and the
// lowercase toString override.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelMode {
    #[serde(rename = "AUTO")] Auto,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "GLOBAL")] Global,
    #[serde(rename = "TEAM_ONLY")] TeamOnly,
}

impl ChannelMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "AUTO" => Self::Auto,
            "DISABLED" => Self::Disabled,
            "GLOBAL" => Self::Global,
            "TEAM_ONLY" => Self::TeamOnly,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Disabled => "DISABLED",
            Self::Global => "GLOBAL",
            Self::TeamOnly => "TEAM_ONLY",
        }
    }
}

impl std::fmt::Display for ChannelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerInfoMode {
    #[serde(rename = "HOLD")] Hold,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "ALWAYS")] Always,
    #[serde(rename = "COMPACT")] Compact,
}

impl PlayerInfoMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "HOLD" => Self::Hold,
            "DISABLED" => Self::Disabled,
            "ALWAYS" => Self::Always,
            "COMPACT" => Self::Compact,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Hold => "HOLD",
            Self::Disabled => "DISABLED",
            Self::Always => "ALWAYS",
            Self::Compact => "COMPACT",
        }
    }
}

impl std::fmt::Display for PlayerInfoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamColorMode {
    #[serde(rename = "FULL")] Full,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "PING_ONLY")] PingOnly,
    #[serde(rename = "LABELS_ONLY")] LabelsOnly,
}

impl TeamColorMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "FULL" => Self::Full,
            "DISABLED" => Self::Disabled,
            "PING_ONLY" => Self::PingOnly,
            "LABELS_ONLY" => Self::LabelsOnly,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Disabled => "DISABLED",
            Self::PingOnly => "PING_ONLY",
            Self::LabelsOnly => "LABELS_ONLY",
        }
    }
}

impl std::fmt::Display for TeamColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

// ---------------------------------------------------------------------------
// ConfigHandler<T>. Upstream keeps a configHash from Lombok @EqualsAndHashCode and
// skips save when unchanged; the hash here runs over the serialized JSON (same effect).
// ---------------------------------------------------------------------------

pub struct ConfigHandler<T: IConfig + Serialize + for<'de> Deserialize<'de>> {
    config: T,
    config_hash: u64,
    config_path: String,
}

impl<T: IConfig + Serialize + for<'de> Deserialize<'de> + Default> ConfigHandler<T> {
    /// ConfigHandler.of(class, configExtension): runs the legacy config migration first,
    /// then resolves MOD_ID + extension through the platform context service.
    pub fn of(config_extension: &str) -> Self {
        compat::legacy_migrate_config(config_extension);
        let path = seams::context_service().resolve_config_dir(&format!("{}{}", global::MOD_ID, config_extension));
        Self::new(path)
    }

    pub fn new(config_path: String) -> Self {
        Self {
            config: T::default(),
            config_hash: 0,
            config_path,
        }
    }

    pub fn config(&self) -> &T {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut T {
        &mut self.config
    }

    fn hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        serde_json::to_string(&self.config).expect("serialize").hash(&mut h);
        h.finish()
    }

    pub fn save(&mut self) {
        let next = self.hash();
        if self.config_hash == next {
            return;
        }

        self.config.on_update();

        let path = Path::new(&self.config_path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    global::log_error(&format!("Creating Config failed: {}", e));
                    return;
                }
            }
            if let Err(e) = std::fs::File::create(path) {
                global::log_error(&format!("Creating Config failed: {}", e));
                return;
            }
        }

        // GsonBuilder().setPrettyPrinting(): pretty JSON.
        match serde_json::to_string_pretty(&self.config).map(|s| std::fs::write(path, s)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                global::log_error(&format!("Saving Config failed: {}", e));
                return;
            }
            Err(e) => {
                global::log_error(&format!("Saving Config failed: {}", e));
                return;
            }
        }

        self.config_hash = self.hash();
        global::log_info(&format!("Saved {}", self.config.to_string_snapshot()));
    }

    pub fn load(&mut self) {
        let path = Path::new(&self.config_path);
        if !path.exists() {
            self.save();
            return;
        }

        // gson.fromJson can hand back null upstream (broken file); parse failure here plays that part.
        let mut parsed_ok = false;
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(c) = serde_json::from_slice::<T>(&bytes) {
                self.config = c;
                parsed_ok = true;
            }
        }

        if !parsed_ok {
            self.config = T::default();
            global::log_error("Config is broken -> reset to defaults");
            self.save();
            return;
        }

        self.config.validate();
        self.config_hash = self.hash();
        global::log_info(&format!("Loaded {}", self.config.to_string_snapshot()));
    }
}

// ---------------------------------------------------------------------------
// ClientConfig. All field names, defaults, hidden-from-settings fields, and the
// per-server channel map stay exactly upstream.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    #[serde(rename = "pingVolume")]
    pub ping_volume: i32,
    #[serde(rename = "pingDuration")]
    pub ping_duration: i32,
    #[serde(rename = "pingDistance")]
    pub ping_distance: i32,
    #[serde(rename = "correctionPeriod")]
    pub correction_period: f32,
    #[serde(rename = "itemIconVisible")]
    pub item_icon_visible: bool,
    #[serde(rename = "directionIndicatorVisible")]
    pub direction_indicator_visible: bool,
    #[serde(rename = "playerInfoMode")]
    pub player_info_mode: PlayerInfoMode,
    #[serde(rename = "teamColorMode")]
    pub team_color_mode: TeamColorMode,
    #[serde(rename = "pingSize")]
    pub ping_size: i32,
    pub channel: String,
    /// Gson keeps Java field names; this one serializes as "serverChannels".
    #[serde(rename = "serverChannels")]
    pub server_channels: BTreeMap<String, String>,

    // hidden from the settings screen
    #[serde(rename = "removeRadius")]
    pub remove_radius: i32,
    #[serde(rename = "raycastDistance")]
    pub raycast_distance: i32,
    #[serde(rename = "safeZoneLeft")]
    pub safe_zone_left: i32,
    #[serde(rename = "safeZoneRight")]
    pub safe_zone_right: i32,
    #[serde(rename = "safeZoneTop")]
    pub safe_zone_top: i32,
    #[serde(rename = "safeZoneBottom")]
    pub safe_zone_bottom: i32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            ping_volume: 100,
            ping_duration: 7,
            ping_distance: 2048,
            correction_period: 1.0,
            item_icon_visible: true,
            direction_indicator_visible: true,
            player_info_mode: PlayerInfoMode::Hold,
            team_color_mode: TeamColorMode::Full,
            ping_size: 100,
            channel: String::new(),
            server_channels: BTreeMap::new(),
            remove_radius: 10,
            raycast_distance: 1024,
            safe_zone_left: 5,
            safe_zone_right: 5,
            safe_zone_top: 5,
            safe_zone_bottom: 60,
        }
    }
}

pub const TPS: i32 = 20;
// Enum value lists in declaration order (upstream: Class.getEnumConstants()).

impl PlayerInfoMode {
    pub const VALUES: [PlayerInfoMode; 4] = [
        PlayerInfoMode::Hold,
        PlayerInfoMode::Disabled,
        PlayerInfoMode::Always,
        PlayerInfoMode::Compact,
    ];
}

impl TeamColorMode {
    pub const VALUES: [TeamColorMode; 4] = [
        TeamColorMode::Full,
        TeamColorMode::Disabled,
        TeamColorMode::PingOnly,
        TeamColorMode::LabelsOnly,
    ];
}

impl ClientConfig {
    // The plain Lombok getters/setters the settings screen drives; get_channel /
    // set_channel keep their custom bodies above.

    pub fn get_ping_volume(&self) -> i32 {
        self.ping_volume
    }
    pub fn set_ping_volume(&mut self, ping_volume: i32) {
        self.ping_volume = ping_volume;
    }
    pub fn get_ping_duration(&self) -> i32 {
        self.ping_duration
    }
    pub fn set_ping_duration(&mut self, ping_duration: i32) {
        self.ping_duration = ping_duration;
    }
    pub fn get_ping_distance(&self) -> i32 {
        self.ping_distance
    }
    pub fn set_ping_distance(&mut self, ping_distance: i32) {
        self.ping_distance = ping_distance;
    }
    pub fn get_correction_period(&self) -> f32 {
        self.correction_period
    }
    pub fn set_correction_period(&mut self, correction_period: f32) {
        self.correction_period = correction_period;
    }
    pub fn is_item_icon_visible(&self) -> bool {
        self.item_icon_visible
    }
    pub fn set_item_icon_visible(&mut self, item_icon_visible: bool) {
        self.item_icon_visible = item_icon_visible;
    }
    pub fn is_direction_indicator_visible(&self) -> bool {
        self.direction_indicator_visible
    }
    pub fn set_direction_indicator_visible(&mut self, direction_indicator_visible: bool) {
        self.direction_indicator_visible = direction_indicator_visible;
    }
    pub fn get_player_info_mode(&self) -> PlayerInfoMode {
        self.player_info_mode
    }
    pub fn set_player_info_mode(&mut self, player_info_mode: PlayerInfoMode) {
        self.player_info_mode = player_info_mode;
    }
    pub fn get_team_color_mode(&self) -> TeamColorMode {
        self.team_color_mode
    }
    pub fn set_team_color_mode(&mut self, team_color_mode: TeamColorMode) {
        self.team_color_mode = team_color_mode;
    }
    pub fn get_ping_size(&self) -> i32 {
        self.ping_size
    }
    pub fn set_ping_size(&mut self, ping_size: i32) {
        self.ping_size = ping_size;
    }
}

pub const MAX_PING_DURATION: i32 = 60;
pub const MAX_PING_DISTANCE: i32 = 2048;
pub const MAX_CORRECTION_PERIOD: f32 = 5.0;
pub const MAX_CHANNEL_LENGTH: usize = 128;

impl ClientConfig {
    /// The per-IP override: serverChannels.getOrDefault(ip, "") when in a game, else the plain channel.
    pub fn get_channel(&self) -> String {
        if let Some(ip) = crate::core::GameContext::current_server_ip() {
            return self
                .server_channels
                .get(&ip)
                .cloned()
                .unwrap_or_default();
        }
        self.channel.clone()
    }

    pub fn set_channel(&mut self, new_channel: String) {
        if let Some(ip) = crate::core::GameContext::current_server_ip() {
            self.server_channels.insert(ip, new_channel);
        } else {
            self.channel = new_channel;
        }
    }
}

impl IConfig for ClientConfig {
    fn validate(&mut self) {
        if self.channel.len() > MAX_CHANNEL_LENGTH {
            self.channel = self.channel.chars().take(MAX_CHANNEL_LENGTH).collect();
        }

        for (_, channel) in self.server_channels.iter_mut() {
            if channel.len() > MAX_CHANNEL_LENGTH {
                *channel = channel.chars().take(MAX_CHANNEL_LENGTH).collect();
            }
        }
    }

    fn on_update(&mut self) {
        if seams::game().is_some() {
            seams::network_service().send_to_server(&UpdateChannelC2SPacket::new(Some(self.get_channel())));
        }
    }

    fn to_string_snapshot(&self) -> String {
        // Lombok @ToString shape: ClientConfig(pingVolume=.., ..)
        format!(
            "ClientConfig(pingVolume={}, pingDuration={}, pingDistance={}, correctionPeriod={}, itemIconVisible={}, directionIndicatorVisible={}, playerInfoMode={}, teamColorMode={}, pingSize={}, channel={}, serverChannels={}, removeRadius={}, raycastDistance={}, safeZoneLeft={}, safeZoneRight={}, safeZoneTop={}, safeZoneBottom={})",
            self.ping_volume, self.ping_duration, self.ping_distance, self.correction_period,
            self.item_icon_visible, self.direction_indicator_visible, self.player_info_mode,
            self.team_color_mode, self.ping_size, self.channel,
            format!("{:?}", self.server_channels).replace('{', "{").replace('}', "}"),
            self.remove_radius, self.raycast_distance, self.safe_zone_left,
            self.safe_zone_right, self.safe_zone_top, self.safe_zone_bottom
        )
    }
}

/// HANDLER = ConfigHandler.of(ClientConfig.class, ".json") — path resolved by the context service seam.
pub fn client_handler() -> &'static mut ConfigHandler<ClientConfig> {
    static HANDLER: OnceLock<usize> = OnceLock::new();
    let ptr = *HANDLER.get_or_init(|| Box::into_raw(Box::new(ConfigHandler::of(".json"))) as *mut ConfigHandler<ClientConfig> as usize);
    unsafe { &mut *(ptr as *mut ConfigHandler<ClientConfig>) }
}

// ---------------------------------------------------------------------------
// ServerConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Gson serializes the Java field name "defaultChannelMode".
    #[serde(rename = "defaultChannelMode")]
    pub default_channel_mode: ChannelMode,
    #[serde(rename = "playerTrackingEnabled")]
    pub player_tracking_enabled: bool,
    #[serde(rename = "msToRegenerate")]
    pub ms_to_regenerate: i64,
    #[serde(rename = "rateLimit")]
    pub rate_limit: i64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            default_channel_mode: ChannelMode::Auto,
            player_tracking_enabled: true,
            ms_to_regenerate: 1000,
            rate_limit: 5,
        }
    }
}

impl IConfig for ServerConfig {
    fn validate(&mut self) {
        if self.ms_to_regenerate < 0 {
            self.ms_to_regenerate = 1000;
        }

        if self.rate_limit < 0 {
            self.rate_limit = 0;
        }
    }

    fn on_update(&mut self) {
        crate::core::ServerCore::init();
    }

    fn to_string_snapshot(&self) -> String {
        format!(
            "ServerConfig(defaultChannelMode={}, playerTrackingEnabled={}, msToRegenerate={}, rateLimit={})",
            self.default_channel_mode, self.player_tracking_enabled, self.ms_to_regenerate, self.rate_limit
        )
    }
}

/// HANDLER = ConfigHandler.of(ServerConfig.class, ".server.json")
pub fn server_handler() -> &'static mut ConfigHandler<ServerConfig> {
    static HANDLER: OnceLock<usize> = OnceLock::new();
    let ptr = *HANDLER.get_or_init(|| Box::into_raw(Box::new(ConfigHandler::of(".json"))) as *mut ConfigHandler<ServerConfig> as usize);
    unsafe { &mut *(ptr as *mut ConfigHandler<ServerConfig>) }
}
