
//! Exact 1:1 of me.axieum.mcmod.authme.api.Config (resourcefulconfig
//! annotations are host seams; the loader joins the config-screen batch).
//! Upstream keeps static fields mutated by the config screen; the port keeps
//! the same fields in one static Config (see auth_me::CONFIG).

use crate::microsoft_utils::{MicrosoftPrompt, AUTHORIZE_URL, CLIENT_ID, MC_AUTH_URL, MC_PROFILE_URL, TOKEN_URL, XBOX_AUTH_URL, XBOX_XSTS_URL};
use serde::{Deserialize, Serialize};

/// Authentication button configuration schema (Config.AuthButton).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthButton {
    /// X coordinate of the button on the multiplayer screen.
    pub x: i32,
    /// Y coordinate of the button on the multiplayer screen.
    pub y: i32,
    /// True if the button can be dragged to a new position.
    pub draggable: bool,
}

impl Default for AuthButton {
    fn default() -> Self {
        AuthButton { x: 6, y: 6, draggable: true }
    }
}

/// Login via Microsoft configuration schema (Config.LoginMethods.Microsoft).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Microsoft {
    /// Indicates the type of user interaction that is required.
    pub prompt: MicrosoftPrompt,
    /// The port from which to listen for OAuth2 callbacks.
    pub port: u16,
    /// OAuth2 client id.
    pub client_id: String,
    /// OAuth2 authorization url.
    pub authorize_url: String,
    /// OAuth2 access token url.
    pub token_url: String,
    /// Xbox authentication url.
    pub xbox_auth_url: String,
    /// Xbox XSTS authorization url.
    pub xbox_xsts_url: String,
    /// Minecraft authentication url.
    pub mc_auth_url: String,
    /// Minecraft profile url.
    pub mc_profile_url: String,
}

impl Default for Microsoft {
    fn default() -> Self {
        Microsoft {
            prompt: MicrosoftPrompt::DEFAULT,
            port: 25585,
            client_id: CLIENT_ID.to_string(),
            authorize_url: AUTHORIZE_URL.to_string(),
            token_url: TOKEN_URL.to_string(),
            xbox_auth_url: XBOX_AUTH_URL.to_string(),
            xbox_xsts_url: XBOX_XSTS_URL.to_string(),
            mc_auth_url: MC_AUTH_URL.to_string(),
            mc_profile_url: MC_PROFILE_URL.to_string(),
        }
    }
}

impl Microsoft {
    /// Determines whether the configured URLs differ from the defaults
    /// (Java: Objects.equals chain over all six URLs).
    pub fn is_defaults(&self) -> bool {
        self.authorize_url == AUTHORIZE_URL
            && self.token_url == TOKEN_URL
            && self.xbox_auth_url == XBOX_AUTH_URL
            && self.xbox_xsts_url == XBOX_XSTS_URL
            && self.mc_auth_url == MC_AUTH_URL
            && self.mc_profile_url == MC_PROFILE_URL
    }
}

/// Login offline configuration schema (Config.LoginMethods.Offline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offline {
    /// Last used username.
    pub last_username: String,
}

impl Default for Offline {
    fn default() -> Self {
        Offline { last_username: String::new() }
    }
}

/// Authentication methods configuration schema (Config.LoginMethods).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginMethods {
    pub microsoft: Microsoft,
    pub offline: Offline,
}

/// The mod configuration (Config; serialized as config/authme.json by the
/// carrier's configurator, matching upstream's resourcefulconfig file).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub auth_button: AuthButton,
    pub login_methods: LoginMethods,
}

impl Config {
    /// Java: `public Config() {}` with the annotated static-field defaults.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Java: `Config.LoginMethods.Offline.lastUsername` is a mutable static
/// field on upstream; the Rust translation keeps it in a lock cell so the
/// config value reads the same through `config().login_methods.offline`.
pub static OFFLINE_LAST_USERNAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Reads the last used offline username.
pub fn offline_last_username() -> String {
    OFFLINE_LAST_USERNAME.lock().unwrap().clone()
}

/// Java: `Config.LoginMethods.Offline.lastUsername = value`.
pub fn set_offline_last_username(value: String) {
    *OFFLINE_LAST_USERNAME.lock().unwrap() = value;
}

/// Java: `Config.AuthButton.x/y` are mutable static fields on upstream; the
/// Rust translation keeps the live position in a lock cell next to the
/// defaults in `config().auth_button`.
pub static AUTH_BUTTON_POSITION: std::sync::Mutex<(i32, i32)> = std::sync::Mutex::new((6, 6));

/// Java: reads `Config.AuthButton.x/y`.
pub fn auth_button_position() -> (i32, i32) {
    *AUTH_BUTTON_POSITION.lock().unwrap()
}

/// Java: `Config.AuthButton.x = value; Config.AuthButton.y = value`.
pub fn set_auth_button_position(x: i32, y: i32) {
    *AUTH_BUTTON_POSITION.lock().unwrap() = (x, y);
}
