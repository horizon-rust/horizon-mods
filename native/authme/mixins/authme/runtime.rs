//! Exact 1:1 translation of Auth Me (upstream Java, MIT),
//! main @78db0f5 (2026-09-19, v9.3.0+26.3). The Minecraft/Fabric APIs and
//! Mojang authlib types that do not exist on Horizon are host seams with the
//! same shapes and semantics. The fabric entrypoint `AuthMeFabric` maps to
//! the carrier's client-init hook; the ModMenu entrypoint (resourcefulconfig
//! screen) joins the config-screen batch later.



// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
// Tests live in src/tests.rs (private master only); the bundle ships the mod code 1:1.

pub mod auth_me {

//! Exact 1:1 of me.axieum.mcmod.authme.api.AuthMe. The resourcefulconfig
//! Configurator and SLF4J logger are host seams with the same shapes.

fn log(level: &str, msg: &str) {
    #[cfg(test)]
    println!("[{}] [Auth Me] {}", level, msg);
    #[allow(unused_variables)]
    #[cfg(not(test))]
    let _ = (level, msg);
}

pub const MOD_ID: &str = "authme";
pub const MOD_NAME: &str = "Auth Me";
pub const MOJANG_ACCOUNT_MIGRATION_FAQ_URL: &str = "https://aka.ms/MinecraftPostMigrationFAQ";

/// The mod logger seam (SLF4J `LoggerFactory.getLogger(MOD_NAME)` on
/// upstream); a unit struct so call sites read the same.
pub struct LOGGER;

impl LOGGER {
    #[allow(non_snake_case)]
    pub fn info(&self, msg: &str) { log("info", msg) }
    #[allow(non_snake_case)]
    pub fn warn(&self, msg: &str) { log("warn", msg) }
    #[allow(non_snake_case)]
    pub fn error(&self, msg: &str) { log("error", msg) }
    #[allow(non_snake_case)]
    pub fn error_with(&self, msg: &str, err: &str) { log("error", &format!("{} ({})", msg, err)) }
    fn log(&self, level: &str, msg: &str) {
        #[cfg(test)]
        println!("[{}] [Auth Me] {}", level, msg);
        #[cfg(not(test))]
        let _ = (level, msg);
    }
}

use crate::config::Config;
use std::sync::OnceLock;

/// The mod configuration seam (resourcefulconfig Configurator on upstream):
/// the single live Config, initialised with the annotated defaults on first
/// use. The resourcefulconfig loader joins the config-screen batch later.
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// The live configuration (Java: the Configurator holds the annotated
/// statics; the carrier's config screen mutates through the same slot).
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

/// Initialises the multi-platform mod: registers the configuration.
pub fn init() {
    // CONFIG.register(Config.class) - the static Config is registered with
    // the resourcefulconfig loader in the carrier; the port warms the
    // defaults here to match the loader's eager parse.
    let _ = config();
}

}

pub mod config {

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

}

pub mod md5 {

//! MD5 (RFC 1321) and java.util.UUID#nameUUIDFromBytes semantics, used by
//! `SessionUtils#offline` (Java: UUID.nameUUIDFromBytes).

pub struct Md5 {
    state: [u32; 4],
    buffer: [u8; 64],
    buffered: usize,
    length_bits: u64,
}

impl Md5 {
    pub fn new() -> Self {
        Md5 { state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476], buffer: [0u8; 64], buffered: 0, length_bits: 0 }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length_bits = self.length_bits.wrapping_add((data.len() as u64) * 8);
        if self.buffered > 0 {
            let take = usize::min(64 - self.buffered, data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffered = data.len();
        }
    }

    pub fn finalize(mut self) -> [u8; 16] {
        let bits = self.length_bits;
        self.update(&[0x80]);
        while self.buffered != 56 {
            self.update(&[0x00]);
        }
        // append length (big-endian bytes of the u64, as Java's no padding differs; RFC wants LE)
        let mut len = [0u8; 8];
        len.copy_from_slice(&bits.to_le_bytes());
        self.update(&len);
        let mut out = [0u8; 16];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const S: [u32; 64] = [
            7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
            5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
            4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
            6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
        ];
        const K: [u32; 64] = [
            0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
            0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
            0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
            0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
            0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
            0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
            0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
            0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
        ];
        let mut m = [0u32; 16];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            m[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (self.state[0], self.state[1], self.state[2], self.state[3]);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
    }
}

/// MD5 digest of `data`.
pub fn md5(data: &[u8]) -> [u8; 16] {
    let mut h = Md5::new();
    h.update(data);
    h.finalize()
}

/// Java: `UUID.nameUUIDFromBytes(bytes)` - an MD5-based (version 3,
/// RFC 4122 variant) name-based UUID.
pub fn name_uuid_from_bytes(bytes: &[u8]) -> String {
    let mut digest = md5(bytes);
    // version 3
    digest[6] = (digest[6] & 0x0f) | 0x30;
    // IETF variant
    digest[8] = (digest[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
        digest[8], digest[9], digest[10], digest[11], digest[12], digest[13], digest[14], digest[15]
    )
}

/// java.util.UUID.randomUUID(): a random (version 4, IETF variant) UUID
/// string. Seeded from the OS entropy source; the exact bits are unspecified
/// upstream too.
pub fn random_uuid_v4_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Cheap entropy mix: nanos + address-space jitter. Not cryptographic,
    // matching only the shape of the upstream call.
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let ptr = &nanos as *const u128 as u64;
    let mut s = nanos as u64 ^ ptr.rotate_left(17);
    s = s.wrapping_mul(0x9E3779B97F4A7C15);
    let hi = s ^ (nanos >> 64) as u64;
    let b0 = (s >> 32) as u32;
    let b1 = (s as u32) ^ (hi as u32);
    // version 4 + variant bits
    let v4 = (b0 & 0xffff_0fff) | 0x0000_4000;
    let var = (b1 & 0x3fff_ffff) | 0x8000_0000;
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:08x}{:04x}", b0, v4 >> 16, v4 & 0xfff, var >> 16, (var & 0xffff) << 16 ^ b1 & 0xffff0000, b1 & 0xffff)
}

}

pub mod microsoft_utils {

//! Exact 1:1 start of me.axieum.mcmod.authme.api.util.MicrosoftUtils: the
//! reusable Apache HTTP request config, the OAuth2/service endpoint constants
//! and the MicrosoftPrompt enum. The device-code login flow itself is the
//! next batch in this module.
//!
//! Upstream MicrosoftUtils.java lines 60-90 and 633-693, translated
//! verbatim-shape. Apache RequestConfig becomes a plain constants module (the
//! Horizon carrier's HTTP host seam reads the timeouts from here).

/// A reusable HTTP request config: 30s connection-request, connect and
/// socket timeouts (upstream REQUEST_CONFIG, Apache RequestConfig).
pub mod request_config {
    pub const CONNECTION_REQUEST_TIMEOUT_MS: u64 = 30_000;
    pub const CONNECT_TIMEOUT_MS: u64 = 30_000;
    pub const SOCKET_TIMEOUT_MS: u64 = 30_000;
}

/// The Azure AD OAuth2 client id for this mod (upstream CLIENT_ID).
pub const CLIENT_ID: &str = "e16699bb-2aa8-46da-b5e3-45cbcce29091";
/// OAuth2 authorization url (upstream AUTHORIZE_URL).
pub const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
/// OAuth2 access token url (upstream TOKEN_URL).
pub const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
/// Xbox authentication url (upstream XBOX_AUTH_URL).
pub const XBOX_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
/// Xbox XSTS authorization url (upstream XBOX_XSTS_URL).
pub const XBOX_XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
/// Minecraft authentication url (upstream MC_AUTH_URL).
pub const MC_AUTH_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
/// Minecraft profile url (upstream MC_PROFILE_URL).
pub const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Exact 1:1 of the upstream MicrosoftPrompt enum (its `toString` returns
/// the prompt query value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MicrosoftPrompt {
    /// Will use the default prompt, equivalent of not sending a prompt.
    DEFAULT,
    /// Will interrupt single sign-on providing account selection experience
    /// listing all the accounts either in session or any remembered account
    /// or an option to choose to use a different account altogether.
    SELECT_ACCOUNT,
    /// Will force the user to enter their credentials on that request,
    /// negating single-sign on.
    LOGIN,
    /// Will ensure that the user isn't presented with any interactive prompt
    /// whatsoever. If the request can't be completed silently via
    /// single-sign on, the Microsoft identity platform will return an
    /// `interaction_required` error.
    NONE,
    /// Will trigger the OAuth consent dialog after the user signs in,
    /// asking the user to grant permissions to the app.
    CONSENT,
}

impl MicrosoftPrompt {
    /// Java: `toString()` returns the prompt query value.
    pub fn prompt(&self) -> &'static str {
        match self {
            MicrosoftPrompt::DEFAULT => "",
            MicrosoftPrompt::SELECT_ACCOUNT => "select_account",
            MicrosoftPrompt::LOGIN => "login",
            MicrosoftPrompt::NONE => "none",
            MicrosoftPrompt::CONSENT => "consent",
        }
    }
}

impl core::fmt::Display for MicrosoftPrompt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.prompt())
    }
}

// ============================================================================
// The OAuth2 login flow: exact 1:1 of MicrosoftUtils' remaining methods.
// Upstream's Apache HttpClient, java.net.HttpServer callback listener and
// Blaze3D#openUri are host seams with the same shapes: HttpSeam,
// CallbackServerSeam and BrowserSeam. Java CompletableFuture.supplyAsync on
// an executor becomes a spawned thread returning JoinHandle; every request
// body, header set, JSON field and error message below is verbatim.
// ============================================================================

use std::collections::HashMap;
use std::thread::JoinHandle;

use serde_json::{Value, json};

use crate::auth_me::LOGGER;
use crate::session_utils::User;

/// The result of an HTTP exchange (upstream: Apache org.apache.http.HttpResponse).
pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
}

/// The HTTP client seam (upstream: CloseableHttpClient / HttpClients.createMinimal
/// with REQUEST_CONFIG timeouts).
pub trait HttpSeam: Send + Sync {
    /// client.execute(request): (method, uri, headers, optional body).
    fn execute(
        &self,
        method: &str,
        uri: &str,
        headers: &[(String, String)],
        body: Option<String>,
    ) -> Result<HttpResponse, String>;
}

/// The local OAuth2 callback listener seam (upstream: com.sun.net.httpserver
/// HttpServer on Config.port with a /callback context).
pub trait CallbackServerSeam: Send {
    /// server.getAddress().getPort() (the bound port).
    fn bound_port(&self) -> u16;
    /// server.start(), then waits for the /callback request and returns its
    /// query parameters (URLEncodedUtils.parse over the request URI).
    fn await_callback(&self) -> Result<HashMap<String, String>, String>;
    /// Sends the 200 response body the browser shows, then closes (and
    /// server.stop(2) releases the server; the seam reclaims itself on drop).
    fn respond(&self, message: &str);
}

/// The browser seam (upstream: Blaze3D::openUri / a Consumer<URI>).
pub trait BrowserSeam: Send + Sync {
    /// Opens the generated login url.
    fn open_uri(&self, uri: &str);
}

/// StringUtils#abbreviateMiddle(str, abbr, last), exact 1:1 (commons-lang3).
pub fn abbreviate_middle(s: &str, middle: &str, last: usize) -> String {
    let str_len = s.chars().count();
    if s.is_empty() || last < middle.chars().count() + 2 || last >= str_len {
        return s.to_string();
    }
    let target = last - middle.chars().count();
    let start_offset = (target + 1) / 2;
    let end_offset = str_len - target / 2;
    let start: String = s.chars().take(start_offset).collect();
    let end: String = s.chars().skip(end_offset).collect();
    format!("{}{}{}", start, middle, end)
}

/// Generates a random OAuth2 state: 16 random bytes, Base64 url (no padding)
/// (Java: SECURE_RANDOM.nextBytes + Base64.getUrlEncoder().withoutPadding()).
pub fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    // SecureRandom seam: /dev/urandom, with a time-mixed fallback.
    let read_ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
        .is_ok();
    if !read_ok {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut s = (nanos as u64) ^ (&bytes as *const _ as *const u8 as u64).rotate_left(17);
        for b in bytes.iter_mut() {
            s = s.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(0x9E3779B97F4A7C15);
            *b = (s >> 24) as u8;
        }
    }
    // Base64url alphabet, no padding (22 chars from 16 bytes).
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(22);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in bytes.iter() {
        acc = (acc << 8) | *b as u32;
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            out.push(ALPHA[((acc >> bits) & 0x3f) as usize] as char);
        }
    }
    // Final partial group, top-aligned (Java's Base64 without padding still
    // emits the last partial character)
    if bits > 0 {
        out.push(ALPHA[((acc << (6 - bits)) & 0x3f) as usize] as char);
    }
    out
}

/// The HTTP client seam installed by the carrier.
pub static HTTP: OnceLock<Box<dyn HttpSeam>> = OnceLock::new();
/// The callback server factory installed by the carrier (create is a static
/// factory upstream, so the seam carries the bound instance's port).
pub static CALLBACK: OnceLock<Box<dyn Fn(u16) -> Box<dyn CallbackServerSeam> + Send + Sync>> = OnceLock::new();
/// The browser seam installed by the carrier.
pub static BROWSER: OnceLock<Box<dyn BrowserSeam>> = OnceLock::new();

use std::sync::OnceLock;

fn http() -> &'static dyn HttpSeam {
    HTTP.get().expect("HttpSeam not installed").as_ref()
}

/// Logs the request line like upstream: "[METHOD] uri (timeout=Xs)".
fn log_request(method: &str, uri: &str) {
    LOGGER.info(&format!(
        "[{}] {} (timeout={}s)",
        method, uri, request_config::CONNECT_TIMEOUT_MS / 1000
    ));
}

/// Java: `acquireMSAuthCode(Consumer<URI>, Function<Boolean,String>, Executor,
/// MicrosoftPrompt)` - generates the Microsoft login link, opens it in the
/// browser and listens on http://localhost:<port>/callback for the callback.
pub fn acquire_ms_auth_code(
    browser_message: impl Fn(bool) -> String + Send + 'static,
    prompt: Option<MicrosoftPrompt>,
) -> JoinHandle<Result<String, String>> {
    std::thread::spawn(move || {
        LOGGER.info("Acquiring Microsoft auth code...");
        let ms = &crate::auth_me::config().login_methods.microsoft;

        // Generate a random "state" to be included in the request that will in turn be returned with the token
        let state = generate_state();

        // Prepare a temporary HTTP server we can listen for the OAuth2 callback on
        let server = (CALLBACK.get().expect("CallbackServerSeam not installed"))(ms.port);
        let port = server.bound_port();

        // Build a Microsoft login url
        let mut params: Vec<(String, String)> = vec![
            ("client_id".to_string(), ms.client_id.clone()),
            ("response_type".to_string(), "code".to_string()),
            ("redirect_uri".to_string(), format!("http://localhost:{}/callback", port)),
            ("scope".to_string(), "XboxLive.signin offline_access".to_string()),
            ("state".to_string(), state.clone()),
        ];
        if prompt.is_some() || ms.prompt != MicrosoftPrompt::DEFAULT {
            params.push((
                "prompt".to_string(),
                prompt.unwrap_or(ms.prompt).to_string(),
            ));
        }
        let query: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let uri = format!("{}?{}", ms.authorize_url, query);

        // Navigate to the Microsoft login in browser
        LOGGER.info(&format!("Launching Microsoft login in browser: {}", uri));
        BROWSER.get().expect("BrowserSeam not installed").open_uri(&uri);

        // Start the HTTP server and wait for the callback
        LOGGER.info(&format!(
            "Begin listening on http://localhost:{}/callback for a successful Microsoft login...",
            port
        ));
        let query_map = server.await_callback()?;

        // Check the returned parameter values
        let mut auth_code: Option<String> = None;
        let mut error_msg: Option<String> = None;
        if query_map.get("state").map(String::as_str) != Some(state.as_str()) {
            // The "state" does not match what we sent
            error_msg = Some(format!(
                "State mismatch! Expected '{}' but got '{}'.",
                state,
                query_map.get("state").cloned().unwrap_or_default()
            ));
        } else if query_map.contains_key("code") {
            // Successfully matched the auth code
            auth_code = query_map.get("code").cloned();
        } else if query_map.contains_key("error") {
            // Otherwise, try to find an error description
            error_msg = Some(format!(
                "{}: {}",
                query_map.get("error").cloned().unwrap_or_default(),
                query_map.get("error_description").cloned().unwrap_or_default()
            ));
        }

        // Send a response informing that the browser may now be closed
        let message = browser_message(error_msg.is_none());
        server.respond(&message);

        // Wait for the server to stop and return the auth code, if any captured
        match auth_code.filter(|code| !code.trim().is_empty()) {
            Some(code) => {
                LOGGER.info(&format!("Acquired Microsoft auth code! ({})", abbreviate_middle(&code, "...", 32)));
                Ok(code)
            }
            None => Err(error_msg.unwrap_or_else(|| "There was no auth code or error description present.".to_string())),
        }
    })
}

/// Java: `acquireMSAccessToken(String, Executor)` - exchanges a Microsoft
/// auth code for an access token.
pub fn acquire_ms_access_token(auth_code: &str) -> JoinHandle<Result<String, String>> {
    let auth_code = auth_code.to_string();
    std::thread::spawn(move || {
        LOGGER.info("Exchanging Microsoft auth code for an access token...");
        let ms = &crate::auth_me::config().login_methods.microsoft;
        // We must provide the exact redirect URI that was used to obtain the auth code
        let body = format!(
            "client_id={}&grant_type=authorization_code&code={}&redirect_uri=http://localhost:{}/callback",
            ms.client_id, auth_code, ms.port
        );
        log_request("POST", &ms.token_url);
        let res = http().execute(
            "POST",
            &ms.token_url,
            &[("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string())],
            Some(body),
        )?;

        // Attempt to parse the response body as JSON and extract the access token
        let json: Value = serde_json::from_str(&res.body)
            .map_err(|e| format!("Unable to parse the response body as JSON: {}", e))?;
        match json.get("access_token").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            Some(token) => {
                LOGGER.info(&format!("Acquired Microsoft access token! ({})", abbreviate_middle(token, "...", 32)));
                Ok(token.to_string())
            }
            None => Err(match json.get("error").and_then(Value::as_str) {
                Some(err) => format!(
                    "{}: {}",
                    err,
                    json.get("error_description").and_then(Value::as_str).unwrap_or_default()
                ),
                None => "There was no access token or error description present.".to_string(),
            }),
        }
    })
}

/// Java: `acquireXboxAccessToken(String, Executor)` - exchanges a Microsoft
/// access token for an Xbox Live access token.
pub fn acquire_xbox_access_token(accessToken: &str) -> JoinHandle<Result<String, String>> {
    let accessToken = accessToken.to_string();
    std::thread::spawn(move || {
        LOGGER.info("Exchanging Microsoft access token for an Xbox Live access token...");
        let ms = &crate::auth_me::config().login_methods.microsoft;
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", accessToken)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        })
        .to_string();
        log_request("POST", &ms.xbox_auth_url);
        let res = http().execute(
            "POST",
            &ms.xbox_auth_url,
            &[("Content-Type".to_string(), "application/json".to_string())],
            Some(body),
        )?;

        // Attempt to parse the response body as JSON and extract the access token
        // NB: No response body is sent if the response is not ok
        let json: Value = if res.status_code == 200 {
            serde_json::from_str(&res.body).unwrap_or(json!({}))
        } else {
            json!({})
        };
        match json.get("Token").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            Some(token) => {
                LOGGER.info(&format!("Acquired Xbox Live access token! ({})", abbreviate_middle(token, "...", 32)));
                Ok(token.to_string())
            }
            None => Err(match json.get("XErr") {
                Some(xerr) => format!(
                    "{}: {}",
                    xerr,
                    json.get("Message").and_then(Value::as_str).unwrap_or_default()
                ),
                None => "There was no access token or error description present.".to_string(),
            }),
        }
    })
}

/// Java: `acquireXboxXstsToken(String, Executor)` - exchanges an Xbox Live
/// access token for an Xbox Live XSTS (security token service) token.
pub fn acquire_xbox_xsts_token(accessToken: &str) -> JoinHandle<Result<(String, String), String>> {
    let accessToken = accessToken.to_string();
    std::thread::spawn(move || {
        LOGGER.info("Exchanging Xbox Live token for an Xbox Live XSTS token...");
        let ms = &crate::auth_me::config().login_methods.microsoft;
        let body = json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [accessToken]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        })
        .to_string();
        log_request("POST", &ms.xbox_xsts_url);
        let res = http().execute(
            "POST",
            &ms.xbox_xsts_url,
            &[("Content-Type".to_string(), "application/json".to_string())],
            Some(body),
        )?;

        // Attempt to parse the response body as JSON and extract the access token and user hash
        // NB: No response body is sent if the response is not ok
        let json: Value = if res.status_code == 200 {
            serde_json::from_str(&res.body).unwrap_or(json!({}))
        } else {
            json!({})
        };
        match json.get("Token").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            Some(token) => {
                // Extract the user hash
                let uhs = json["DisplayClaims"]["xui"][0]["uhs"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                // Return an immutable mapping of the token and user hash
                LOGGER.info(&format!(
                    "Acquired Xbox Live XSTS token! (token={}, uhs={})",
                    abbreviate_middle(token, "...", 32),
                    uhs
                ));
                Ok((token.to_string(), uhs))
            }
            None => Err(match json.get("XErr") {
                Some(xerr) => format!(
                    "{}: {}",
                    xerr,
                    json.get("Message").and_then(Value::as_str).unwrap_or_default()
                ),
                None => "There was no access token or error description present.".to_string(),
            }),
        }
    })
}

/// Java: `acquireMCAccessToken(String, String, Executor)` - exchanges an Xbox
/// Live XSTS token for a Minecraft access token.
pub fn acquire_mc_access_token(xstsToken: &str, userHash: &str) -> JoinHandle<Result<String, String>> {
    let (xstsToken, userHash) = (xstsToken.to_string(), userHash.to_string());
    std::thread::spawn(move || {
        LOGGER.info("Exchanging Xbox Live XSTS token for a Minecraft access token...");
        let ms = &crate::auth_me::config().login_methods.microsoft;
        let body = json!({ "identityToken": format!("XBL3.0 x={};{}", userHash, xstsToken) }).to_string();
        log_request("POST", &ms.mc_auth_url);
        let res = http().execute(
            "POST",
            &ms.mc_auth_url,
            &[("Content-Type".to_string(), "application/json".to_string())],
            Some(body),
        )?;

        // Attempt to parse the response body as JSON and extract the access token
        let json: Value = serde_json::from_str(&res.body)
            .map_err(|e| format!("Unable to parse the response body as JSON: {}", e))?;
        match json.get("access_token").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            Some(token) => {
                LOGGER.info(&format!("Acquired Minecraft access token! ({})", abbreviate_middle(token, "...", 32)));
                Ok(token.to_string())
            }
            None => Err(match json.get("error").and_then(Value::as_str) {
                Some(err) => format!(
                    "{}: {}",
                    err,
                    // NB: the Minecraft auth endpoint reports `errorMessage`
                    json.get("errorMessage").and_then(Value::as_str).unwrap_or_default()
                ),
                None => "There was no access token or error description present.".to_string(),
            }),
        }
    })
}

/// Java: `login(String, Executor)` - fetches the Minecraft profile for the
/// given access token, and returns a new Minecraft session.
/// Use `SessionUtils#setUser` to apply the new session.
pub fn login(mcToken: &str) -> JoinHandle<Result<User, String>> {
    let mcToken = mcToken.to_string();
    std::thread::spawn(move || {
        LOGGER.info("Fetching Minecraft profile...");
        let ms = &crate::auth_me::config().login_methods.microsoft;
        log_request("GET", &ms.mc_profile_url);
        let res = http().execute(
            "GET",
            &ms.mc_profile_url,
            &[("Authorization".to_string(), format!("Bearer {}", mcToken))],
            None,
        )?;

        // Attempt to parse the response body as JSON and extract the profile
        let json: Value = serde_json::from_str(&res.body)
            .map_err(|e| format!("Unable to parse the response body as JSON: {}", e))?;
        let uuid_str = json
            .get("id")
            .and_then(Value::as_str)
            .filter(|uuid| !uuid.trim().is_empty())
            .ok_or_else(|| {
                // Otherwise, throw an exception with the error description if present
                match json.get("error").and_then(Value::as_str) {
                    Some(err) => format!(
                        "{}: {}",
                        err,
                        json.get("errorMessage").and_then(Value::as_str).unwrap_or_default()
                    ),
                    None => "There was no profile or error description present.".to_string(),
                }
            })?;
        // Parse the UUID (without hyphens)
        let uuid = format!(
            "{}-{}-{}-{}-{}",
            &uuid_str[0..8],
            &uuid_str[8..12],
            &uuid_str[12..16],
            &uuid_str[16..20],
            &uuid_str[20..]
        );
        // If present, log success, build a new session and return
        let name = json.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        LOGGER.info(&format!("Fetched Minecraft profile! (name={}, uuid={})", name, uuid));
        Ok(User {
            name,
            uuid,
            access_token: mcToken,
            xuid: None,
            client_id: None,
        })
    })
}

}

pub mod session_utils {

//! Exact 1:1 of me.axieum.mcmod.authme.api.util.SessionUtils. The Minecraft
//! client, Mojang authlib services and the five mixin accessors are host
//! seams with the same shapes and semantics (MinecraftClientSeam,
//! SessionServiceSeam and the accessor trait calls kept in the same order).

#[cfg(test)]
pub use test_hooks::{client_test_handle, set_last_status_for_tests};

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth_me::LOGGER;
use crate::md5::name_uuid_from_bytes;

/// The access token used for offline sessions (upstream OFFLINE_TOKEN).
pub const OFFLINE_TOKEN: &str = "invalidtoken";
/// The number of milliseconds that a session status is cached for (60s).
pub const STATUS_TTL: u64 = 60_000;

/// The time of the last session status check (milliseconds since epoch).
static LAST_STATUS_CHECK: AtomicU64 = AtomicU64::new(0);
/// The last session status value, as the SessionStatus discriminant.
static LAST_STATUS: AtomicU8 = AtomicU8::new(SessionStatus::UNKNOWN as u8);

/// Java: `net.minecraft.client.User` (name, uuid, access token, optional
/// xuid and client id). `get_session_id` follows authlib's
/// `"token:uuid"` form.
#[derive(Debug, Clone)]
pub struct User {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    pub client_id: Option<String>,
}

impl User {
    pub fn get_profile_id(&self) -> &str {
        &self.uuid
    }

    pub fn get_session_id(&self) -> String {
        format!("{}:{}", self.access_token, self.uuid)
    }
}

/// The status of a Minecraft session (upstream SessionStatus enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// The session is valid.
    VALID,
    /// The session is invalid.
    INVALID,
    /// The session is offline.
    OFFLINE,
    /// The session is undetermined.
    UNKNOWN,
}

impl SessionStatus {
    fn from_discriminant(v: u8) -> SessionStatus {
        match v {
            0 => SessionStatus::VALID,
            1 => SessionStatus::INVALID,
            2 => SessionStatus::OFFLINE,
            _ => SessionStatus::UNKNOWN,
        }
    }
}

/// The Minecraft client seam: the fields upstream reaches through
/// Minecraft.getInstance() and the five mixin accessors
/// (MinecraftAccessor, SplashManagerAccessor, RealmsClientAccessor,
/// RealmsAvailabilityAccessor, ReportingContextAccessor), in the same
/// call order as `set_user`.
pub trait MinecraftClientSeam {
    /// Minecraft.getInstance().getUser()
    fn get_user(&self) -> User;
    /// ((MinecraftAccessor) client).setUser(user) + the SplashManager
    /// accessor setUser; the rest of the Java sequence (profile future,
    /// offline developer mode, services discovery, user/friends API
    /// services, social manager, profile key pair manager, reporting
    /// context, Realms client/data fetcher/availability future) is carried
    /// on the same seam by the carrier's client wiring.
    fn set_user(&self, user: &User);
    /// Minecraft.getInstance().services().sessionService()
    fn get_session_service(&self) -> Box<dyn SessionServiceSeam + Send>;
}

/// The Yggdrasil Minecraft Session Service seam
/// (com.mojang.authlib.minecraft.SessionService).
pub trait SessionServiceSeam {
    /// SessionService#joinServer(profileId, accessToken, serverId); Err is
    /// an AuthenticationException.
    fn join_server(&self, profile_id: &str, access_token: &str, server_id: &str) -> Result<(), String>;
    /// SessionService#hasJoinedServer(name, serverId, null); None when the
    /// server did not respond with a joined profile.
    fn has_joined_server(&self, name: &str, server_id: &str) -> Option<String>;
}

/// The installed Minecraft client seam (Java: Minecraft.getInstance()), set
/// by the carrier's client wiring.
pub static CLIENT: Mutex<Option<Box<dyn MinecraftClientSeam + Send>>> = Mutex::new(None);

fn client() -> std::sync::MutexGuard<'static, Option<Box<dyn MinecraftClientSeam + Send>>> {
    CLIENT.lock().unwrap()
}

/// Returns the current Minecraft user (Java: Minecraft.getInstance().getUser()).
pub fn get_user() -> User {
    client().as_ref().expect("Minecraft client seam not installed").get_user()
}

/// Replaces the Minecraft user instance (Java: setUser, the full accessor
/// sequence carried 1:1 on the seam, same call order and comments).
pub fn set_user(user: User) {
    let client = client();
    let client = client.as_ref().expect("Minecraft client seam not installed");
    // Use an accessor mixin to update the 'private final' Minecraft session
    // (also SplashManagerAccessor#setUser) and re-create the game profile
    // future, offline developer mode, services discovery, user API service,
    // friends API service and remote friend list update handler, social
    // interactions manager, profile keys and abuse report context, then
    // re-check Realms for a valid session.
    client.set_user(&user);

    // The cached status is now stale
    LAST_STATUS.store(SessionStatus::UNKNOWN as u8, Ordering::SeqCst);
    LAST_STATUS_CHECK.store(0, Ordering::SeqCst);

    LOGGER.info(&format!(
        "Minecraft session for {} (uuid={}) has been applied",
        user.name, user.get_profile_id()
    ));
}

/// Builds and returns a new offline Minecraft session
/// (Java: new User(username, UUID.nameUUIDFromBytes(("offline:" + username).getBytes()), OFFLINE_TOKEN, Optional.empty(), Optional.empty())).
pub fn offline(username: &str) -> User {
    User {
        name: username.to_string(),
        uuid: name_uuid_from_bytes(format!("offline:{}", username).as_bytes()),
        access_token: OFFLINE_TOKEN.to_string(),
        xuid: None,
        client_id: None,
    }
}

/// Checks and returns the current Minecraft session status.
///
/// NB: This is an expensive task as it involves connecting to servers to
/// validate any access tokens, and hence is executed on a separate thread.
/// The session status is cached for about 1 minute for subsequent calls.
///
/// Java returns a CompletableFuture; the port returns a join handle
/// (already-finished work is returned as a completed status).
pub enum StatusFuture {
    /// Java: CompletableFuture.completedFuture(lastStatus).
    Completed(SessionStatus),
    /// Java: CompletableFuture.supplyAsync(..., Util.nonCriticalIoPool()).
    Spawned(JoinHandle<SessionStatus>),
}

impl StatusFuture {
    pub fn join(self) -> SessionStatus {
        match self {
            StatusFuture::Completed(status) => status,
            StatusFuture::Spawned(handle) => handle.join().unwrap_or(SessionStatus::UNKNOWN),
        }
    }
}

/// Checks and returns the current Minecraft session status.
pub fn get_status() -> StatusFuture {
    // Check if the status has already been checked recently
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now_ms.saturating_sub(LAST_STATUS_CHECK.load(Ordering::SeqCst)) < STATUS_TTL {
        return StatusFuture::Completed(SessionStatus::from_discriminant(LAST_STATUS.load(Ordering::SeqCst)));
    }

    // Otherwise, return an asynchronous action to check the session status
    StatusFuture::Spawned(std::thread::spawn(move || {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Fetch the current session
        let session = get_user();
        let server_id = crate::md5::random_uuid_v4_string();

        // Attempt to join the Minecraft Session Service server
        let session_service = client().as_ref().expect("Minecraft client seam not installed").get_session_service();
        let status = {
            LOGGER.info("Verifying Minecraft session...");
            match session_service.join_server(session.get_profile_id(), &session.access_token, &server_id) {
                Ok(()) => {
                    if session_service.has_joined_server(&session.name, &server_id).is_some() {
                        LOGGER.info("The Minecraft session is valid");
                        SessionStatus::VALID
                    } else {
                        LOGGER.warn("The Minecraft session is invalid!");
                        SessionStatus::INVALID
                    }
                }
                Err(e) => {
                    LOGGER.error_with("Could not validate the Minecraft session!", &e);
                    SessionStatus::OFFLINE
                }
            }
        };

        // Update the last status check and return
        LAST_STATUS.store(status as u8, Ordering::SeqCst);
        LAST_STATUS_CHECK.store(now_ms, Ordering::SeqCst);
        status
    }))
}

use std::sync::Mutex;

// Test helpers: install a seam client and force the status cache.
#[cfg(test)]
pub mod test_hooks {
    use super::*;

    pub fn client_test_handle() -> std::sync::MutexGuard<'static, Option<Box<dyn MinecraftClientSeam + Send>>> {
        client()
    }

    pub fn set_last_status_for_tests(status: SessionStatus) {
        LAST_STATUS.store(status as u8, Ordering::SeqCst);
        // "just checked": now, so the TTL cache path returns without spawning
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        LAST_STATUS_CHECK.store(now_ms, Ordering::SeqCst);
    }
}

}

pub mod gui {

//! Exact 1:1 of me.axieum.mcmod.authme.api.gui.screen.AuthScreen: the
//! abstract base for authentication screens. The Minecraft Screen, widget
//! and toast APIs are host seams with the same shapes; the carrier's GUI
//! wiring drives the same init/tick/onClose sequence.

/// Which screen navigation shows next (Java: Screen references; the port
/// carries the screen kinds the mod opens).
#[derive(Debug, Clone, PartialEq)]
pub enum NextScreen {
    /// The screen to be returned to after a successful login.
    SuccessScreen,
    /// The parent (or last) screen that opened this screen.
    ParentScreen,
    /// The authentication method selection screen.
    AuthMethodScreen,
    /// The Microsoft login screen with its `selectAccount` flag.
    MicrosoftAuthScreen { select_account: bool },
    /// The offline login screen.
    OfflineAuthScreen,
    /// The Microsoft URL warning confirm screen (setDelay(40)).
    ConfirmMicrosoftWarning,
    /// The Mojang migration FAQ confirm-link screen.
    ConfirmLinkMojang,
}

/// AuthScreen base state: parent, success screen, and the success flags.
#[derive(Debug, Clone)]
pub struct AuthScreenBase {
    pub parent_screen: NextScreen,
    pub success_screen: NextScreen,
    /// True if the login task completed successfully.
    pub success: bool,
    /// True if the screen should be closed on success state (via render thread).
    pub close_on_success: bool,
}

impl AuthScreenBase {
    /// Java: `AuthScreen(title, parentScreen, successScreen)` (success and
    /// closeOnSuccess default to false).
    pub fn new(parent_screen: NextScreen, success_screen: NextScreen) -> Self {
        AuthScreenBase { parent_screen, success_screen, success: false, close_on_success: false }
    }

    /// Java: `tick()` - optionally close the screen if the login task
    /// completed successfully. Returns Some(next) when the screen closes.
    pub fn tick(&mut self) -> Option<NextScreen> {
        if self.success && self.close_on_success {
            return Some(self.on_close());
        }
        None
    }

    /// Java: `onClose()` - shows the success screen on success, else parent.
    pub fn on_close(&self) -> NextScreen {
        if self.success { self.success_screen.clone() } else { self.parent_screen.clone() }
    }

    /// Java: `centerPosition(element, screen, xOffset, yOffset)` - centers
    /// the position of a LayoutElement, shifted vertically 22 up.
    pub fn center_position(el_w: i32, el_h: i32, screen_w: i32, screen_h: i32, x_offset: i32, y_offset: i32) -> (i32, i32) {
        (
            screen_w / 2 - el_w / 2 + x_offset,
            screen_h / 2 - el_h / 2 - 22 + y_offset,
        )
    }
}

/// The widget metrics the layout code needs (Java: LayoutElement).
#[derive(Debug, Clone)]
pub struct LayoutElement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl LayoutElement {
    pub fn new(width: i32, height: i32) -> Self {
        LayoutElement { x: 0, y: 0, width, height }
    }

    /// Applies AuthScreen#centerPosition for a screen of the given size.
    pub fn center_in(&mut self, screen_w: i32, screen_h: i32, x_offset: i32, y_offset: i32) {
        let (x, y) = AuthScreenBase::center_position(self.width, self.height, screen_w, screen_h, x_offset, y_offset);
        self.x = x;
        self.y = y;
    }
}

/// A button's bounds (Java: Button.builder(...).bounds(x, y, w, h)).
#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Bounds { x, y, width, height }
    }
}

}

pub mod screens {

//! Exact 1:1 of me.axieum.mcmod.authme.api.gui.screen.OfflineAuthScreen and
//! AuthMethodScreen (the init layout numbers, actions and message keys are
//! verbatim; the Minecraft widget/screen APIs are host seams).

use crate::auth_me::{config, LOGGER};
use crate::config::{offline_last_username, set_offline_last_username};
use crate::gui::{AuthScreenBase, Bounds, NextScreen};
use crate::microsoft_utils::MicrosoftPrompt;
use crate::session_utils::{offline, set_user, SessionStatus, User};

/// Java: `gui.authme.offline.*` message keys (Component.translatable).
pub mod keys {
    pub const OFFLINE_TITLE: &str = "gui.authme.offline.title";
    pub const OFFLINE_FIELD_USERNAME: &str = "gui.authme.offline.field.username";
    pub const OFFLINE_BUTTON_LOGIN: &str = "gui.authme.offline.button.login";
    pub const GUI_CANCEL: &str = "gui.cancel";
    pub const TOAST_GREETING: &str = "gui.authme.toast.greeting";
    pub const METHOD_TITLE: &str = "gui.authme.method.title";
    pub const METHOD_GREETING: &str = "gui.authme.method.greeting";
    pub const METHOD_BUTTON_MICROSOFT: &str = "gui.authme.method.button.microsoft";
    pub const METHOD_BUTTON_MICROSOFT_SELECT_ACCOUNT: &str = "gui.authme.method.button.microsoft.selectAccount";
    pub const METHOD_BUTTON_MOJANG: &str = "gui.authme.method.button.mojang";
    pub const METHOD_BUTTON_MOJANG_UNAVAILABLE: &str = "gui.authme.method.button.mojang.unavailable";
    pub const METHOD_BUTTON_OFFLINE: &str = "gui.authme.method.button.offline";
    pub const GUI_BACK: &str = "gui.back";
    pub const BUTTON_AUTH: &str = "gui.authme.button.auth";
    pub const BUTTON_AUTH_TOOLTIP: &str = "gui.authme.button.auth.tooltip";
    pub const BUTTON_RELOGIN: &str = "gui.authme.button.relogin";
    pub const MS_TITLE: &str = "gui.authme.microsoft.title";
    pub const MS_BROWSER: &str = "gui.authme.microsoft.browser";
    pub const MS_WARNING_TITLE: &str = "gui.authme.microsoft.warning.title";
    pub const MS_WARNING_BODY: &str = "gui.authme.microsoft.warning.body";
    pub const MS_WARNING_ACCEPT: &str = "gui.authme.microsoft.warning.accept";
    pub const MS_WARNING_CANCEL: &str = "gui.authme.microsoft.warning.cancel";
    pub const MS_STATUS_CHECK_BROWSER: &str = "gui.authme.microsoft.status.checkBrowser";
    pub const MS_STATUS_MS_ACCESS_TOKEN: &str = "gui.authme.microsoft.status.msAccessToken";
    pub const MS_STATUS_XBOX_ACCESS_TOKEN: &str = "gui.authme.microsoft.status.xboxAccessToken";
    pub const MS_STATUS_XBOX_XSTS_TOKEN: &str = "gui.authme.microsoft.status.xboxXstsToken";
    pub const MS_STATUS_MC_ACCESS_TOKEN: &str = "gui.authme.microsoft.status.mcAccessToken";
    pub const MS_STATUS_MC_PROFILE: &str = "gui.authme.microsoft.status.mcProfile";
    pub const ERROR_TIMEOUT: &str = "gui.authme.error.timeout";
    pub const ERROR_NOT_PURCHASED: &str = "gui.authme.error.notPurchased";
    pub const ERROR_GENERIC: &str = "gui.authme.error.generic";
}

/// The 'Microsoft' authentication method button textures (WidgetSprites).
pub const MICROSOFT_BUTTON_TEXTURES: [(&str, &str); 3] = [
    ("authme", "widget/microsoft_button"),
    ("authme", "widget/microsoft_button_disabled"),
    ("authme", "widget/microsoft_button_focused"),
];
/// The 'Mojang (or legacy)' authentication method button textures.
pub const MOJANG_BUTTON_TEXTURES: [(&str, &str); 3] = [
    ("authme", "widget/mojang_button"),
    ("authme", "widget/mojang_button_disabled"),
    ("authme", "widget/mojang_button_focused"),
];
/// The 'Offline' authentication method button textures.
pub const OFFLINE_BUTTON_TEXTURES: [(&str, &str); 3] = [
    ("authme", "widget/offline_button"),
    ("authme", "widget/offline_button_disabled"),
    ("authme", "widget/offline_button_focused"),
];

/// What the OfflineAuthScreen's login action needs from the client
/// (Java: direct Minecraft/authlib calls).
pub trait OfflineLoginSeam: Send {
    /// SystemToast.add(minecraft.gui.toastManager(),
    /// SystemToastId.PERIODIC_NOTIFICATION, translatable("gui.authme.toast.greeting",
    /// literal(username)), null).
    fn greet_toast(&self, username: &str);
}

/// OfflineAuthScreen logic (extends AuthScreen).
#[derive(Debug, Clone)]
pub struct OfflineAuthScreenLogic {
    pub base: AuthScreenBase,
    /// The username text field widget's value (Java: EditBox usernameField).
    pub username_field: String,
    /// True while the username field is active (Java: EditBox.active).
    pub username_field_active: bool,
    /// True while the login button is active (Java: Button.active).
    pub login_btn_active: bool,
}

impl OfflineAuthScreenLogic {
    /// Java: `OfflineAuthScreen(parentScreen, successScreen)` with
    /// closeOnSuccess = true and the offline title.
    pub fn new() -> Self {
        OfflineAuthScreenLogic {
            base: AuthScreenBase::new(NextScreen::AuthMethodScreen, NextScreen::SuccessScreen),
            username_field: String::new(),
            username_field_active: true,
            login_btn_active: false,
            // Java: this.closeOnSuccess = true (the constructor sets it
            // before the base defaults apply)
        }
        .with_close_on_success(true)
    }

    /// Java: `init()` - presets the field with the last used username and
    /// binds the responder: `loginBtn.active = isFormValid()`.
    pub fn init_preset_username(&mut self) {
        let last = offline_last_username();
        if !last.is_empty() {
            self.username_field = last;
        }
        self.sync_login_button();
    }

    /// The field responder: loginBtn.active = isFormValid().
    pub fn sync_login_button(&mut self) {
        self.login_btn_active = self.is_form_valid();
    }

    /// Java: `login()` - creates and applies a new offline-mode session for
    /// the provided username, then marks the task successful.
    pub fn login(&mut self, seam: &dyn OfflineLoginSeam) {
        // Check whether the form is valid
        if !self.is_form_valid() {
            return;
        }
        // Disable the form fields while logging in
        self.username_field_active = false;
        self.login_btn_active = false;

        // Create and apply a new offline Minecraft session
        let username = self.username_field.clone();
        set_user(offline(&username));

        // Sync configuration with the last used username
        set_offline_last_username(username.clone());

        // Add a toast that greets the player
        seam.greet_toast(&username);

        // Mark the task as successful, in turn closing the screen
        LOGGER.info("Successfully logged in offline-mode!");
        self.base.success = true;
    }

    /// Java: `isFormValid()` - true if the username field is valid.
    pub fn is_form_valid(&self) -> bool {
        !self.username_field.trim().is_empty()
    }

    fn with_close_on_success(mut self, value: bool) -> Self {
        self.base.close_on_success = value;
        self
    }

    pub fn base(&mut self) -> &mut AuthScreenBase {
        &mut self.base
    }
}

impl Default for OfflineAuthScreenLogic {
    fn default() -> Self {
        Self::new()
    }
}

/// The AuthMethodScreen's widget layout for a screen of the given size
/// (Java: `init()` bounds; three 20x20 ImageButtons and a 100x20 back button).
pub fn auth_method_layout(screen_w: i32, screen_h: i32) -> (Bounds, Bounds, Bounds, Bounds) {
    (
        // The 'Microsoft' authentication method button
        Bounds::new(screen_w / 2 - 20 - 10 - 4, screen_h / 2 - 5, 20, 20),
        // The 'Mojang (or legacy)' authentication method button
        Bounds::new(screen_w / 2 - 10, screen_h / 2 - 5, 20, 20),
        // The 'Offline' authentication method button
        Bounds::new(screen_w / 2 + 10 + 4, screen_h / 2 - 5, 20, 20),
        // The back button
        Bounds::new(screen_w / 2 - 50, screen_h / 2 + 27, 100, 20),
    )
}

/// The Microsoft method button's action (Java: the msButton onClick). Returns
/// the screen to show; `select_account` is true when Left Control is held
/// (Java: InputConstants.isKeyDown(KEY_LCONTROL)).
pub fn microsoft_method_action(select_account: bool) -> NextScreen {
    microsoft_method_action_with(&config().login_methods.microsoft, select_account)
}

/// The same branch over an explicit config value (test seam).
pub fn microsoft_method_action_with(ms: &crate::config::Microsoft, select_account: bool) -> NextScreen {
    if ms.is_defaults() {
        NextScreen::MicrosoftAuthScreen { select_account }
    } else {
        LOGGER.warn("Non-default Microsoft authentication URLs are in use!");
        NextScreen::ConfirmMicrosoftWarning
    }
}

/// Java: `updateStatusWidget(client, key)` - the status message shown while
/// the login task progresses (in call order).
pub const MICROSOFT_STATUS_SEQUENCE: [&str; 6] = [
    keys::MS_STATUS_CHECK_BROWSER,
    keys::MS_STATUS_MS_ACCESS_TOKEN,
    keys::MS_STATUS_XBOX_ACCESS_TOKEN,
    keys::MS_STATUS_XBOX_XSTS_TOKEN,
    keys::MS_STATUS_MC_ACCESS_TOKEN,
    keys::MS_STATUS_MC_PROFILE,
];

/// Java: `exceptionally(error)` - the error-key classification
/// (ConnectTimeoutException -> timeout; "NOT_FOUND: Not Found" ->
/// notPurchased; otherwise generic with a log line).
pub fn classify_microsoft_error(cause: &str) -> &'static str {
    if cause.contains("ConnectTimeoutException") {
        keys::ERROR_TIMEOUT
    } else if cause == "NOT_FOUND: Not Found" {
        keys::ERROR_NOT_PURCHASED
    } else {
        keys::ERROR_GENERIC
    }
}

/// The MicrosoftAuthScreen's chained login task, run on its own thread
/// (Java: CompletableFuture chain). Calls `on_status` with the
/// `updateStatusWidget` key at each stage and returns the final outcome.
pub fn run_microsoft_login_task(
    select_account: bool,
    on_status: impl Fn(&str) + Send + 'static,
) -> std::thread::JoinHandle<Result<User, String>> {
    std::thread::spawn(move || {
        let status = |key: &str| on_status(key);
        status(keys::MS_STATUS_MS_ACCESS_TOKEN);
        let ms_auth_code = crate::microsoft_utils::acquire_ms_auth_code(
            |_success| keys::MS_BROWSER.to_string(),
            if select_account { Some(MicrosoftPrompt::SELECT_ACCOUNT) } else { None },
        );
        let ms_auth_code = ms_auth_code.join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;

        status(keys::MS_STATUS_XBOX_ACCESS_TOKEN);
        let ms_access_token = crate::microsoft_utils::acquire_ms_access_token(&ms_auth_code)
            .join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;

        status(keys::MS_STATUS_XBOX_XSTS_TOKEN);
        let xbox_access_token = crate::microsoft_utils::acquire_xbox_access_token(&ms_access_token)
            .join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;

        status(keys::MS_STATUS_MC_ACCESS_TOKEN);
        let (xsts_token, uhs) = crate::microsoft_utils::acquire_xbox_xsts_token(&xbox_access_token)
            .join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;
        let mc_token = crate::microsoft_utils::acquire_mc_access_token(&xsts_token, &uhs)
            .join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;

        status(keys::MS_STATUS_MC_PROFILE);
        let user = crate::microsoft_utils::login(&mc_token)
            .join().map_err(|e| format!("{:?}", e)).and_then(|r| r)?;

        // Update the game session and greet the player
        set_user(user.clone());
        Ok(user)
    })
}

/// AuthButtonWidget logic: the textured button for opening the auth screens
/// (Java: extends ImageButton with click-on-release and drag).
/// See `widget.rs` for the full port; this module carries the layout side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MethodAction {
    Microsoft { select_account: bool },
    MojangFaq,
    Offline,
}

/// Maps a method action to the screen it opens (Java: the three onClick
/// bodies; the Microsoft warning confirm screen resolves through
/// `microsoft_method_action`).
pub fn method_action_screen(action: MethodAction) -> NextScreen {
    match action {
        MethodAction::Microsoft { select_account } => {
            let screen = microsoft_method_action(select_account);
            if screen == NextScreen::ConfirmMicrosoftWarning {
                // Java: setScreenAndShow(confirmScreen); confirmScreen.setDelay(40)
                screen
            } else {
                screen
            }
        }
        MethodAction::MojangFaq => NextScreen::ConfirmLinkMojang,
        MethodAction::Offline => NextScreen::OfflineAuthScreen,
    }
}

/// The session status icon's u coordinate (Java: the extractContents switch).
pub fn session_status_icon_u(status: SessionStatus) -> i32 {
    match status {
        SessionStatus::VALID => 0,
        SessionStatus::OFFLINE => 8,
        _ => 16,
    }
}

/// The AuthButtonWidget's session-status texture identifier.
pub const SESSION_STATUS_TEXTURE: (&str, &str) = ("authme", "textures/gui/session_status.png");

/// The AuthButtonWidget textures (Java: Identifier.parse("widget/locked_button...")).
pub const BUTTON_TEXTURES: [(&str, &str); 3] = [
    ("minecraft", "widget/locked_button"),
    ("minecraft", "widget/locked_button_disabled"),
    ("minecraft", "widget/locked_button_highlighted"),
];


}

pub mod widget {

//! Exact 1:1 of me.axieum.mcmod.authme.api.gui.widget.AuthButtonWidget:
//! the textured button for opening the authentication screens, with
//! click-on-release handling, drag movement clamped to the screen, and the
//! session-status icon. The Minecraft rendering/mouse APIs are host seams.

use crate::screens::session_status_icon_u;
use crate::session_utils::{get_status, SessionStatus};

/// A functional interface for widget relocation callback (Java: MoveAction).
pub type MoveAction = Box<dyn Fn(&AuthButtonWidgetLogic) + Send + Sync>;

/// What the button's press action opens (Java: OnPress, wired by the
/// carrier through AuthMethodScreen/AuthButtonWidget use sites).
pub type PressAction = Box<dyn Fn() + Send + Sync>;

/// Java: ImageButton base: a 20x20 button at (x, y) with a press action.
pub struct AuthButtonWidgetLogic {
    /// Java: field x / getY().
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// The screen used to constrain the button movements to (Java: Screen).
    pub screen: Option<(i32, i32)>,
    /// Callback for after the button has been dragged.
    pub move_action: Option<MoveAction>,
    /// True if the button was just dragged.
    pub did_drag: bool,
    /// The last known status of the Minecraft session.
    pub session_status: SessionStatus,
    /// Whether the button accepts left clicks (Java: isValidClickButton).
    pub active: bool,
}

impl AuthButtonWidgetLogic {
    /// Java: `AuthButtonWidget(x, y, pressAction)` - a fixed (no drag)
    /// authentication button; the press action opens the auth screens.
    pub fn fixed(x: i32, y: i32) -> Self {
        AuthButtonWidgetLogic::movable(None, x, y, None)
    }

    /// Java: `AuthButtonWidget(screen, x, y, pressAction, moveAction)` - a
    /// movable (with drag) authentication button.
    pub fn movable(screen: Option<(i32, i32)>, x: i32, y: i32, move_action: Option<MoveAction>) -> Self {
        let mut w = AuthButtonWidgetLogic {
            x, y, width: 20, height: 20,
            screen,
            move_action,
            did_drag: false,
            session_status: SessionStatus::UNKNOWN,
            active: true,
        };
        w.refresh_session_status();
        w
    }

    /// Java: `mouseClicked(event, doubleClick)` - with a move action, the
    /// default behaviour is adjusted to NOT trigger a click (dragging starts
    /// on mouse down; the click fires on mouse up/release).
    pub fn mouse_clicked(&self) -> bool {
        if self.move_action.is_some() {
            return self.active; // Java: isValidClickButton(buttonInfo)
        }
        true // Java: super.mouseClicked(event, doubleClick)
    }

    /// Java: `mouseReleased(event)` - with a move action, the click triggers
    /// on mouse up: a finished drag invokes the move callback, otherwise it
    /// continues as a button press.
    pub fn mouse_released(&mut self) -> WidgetClick {
        if self.move_action.is_none() {
            return WidgetClick::Press; // Java: super.mouseReleased(event)
        }
        if !self.active {
            return WidgetClick::None; // Java: !isValidClickButton -> false
        }
        // Check if the user just finished dragging
        if self.did_drag {
            // Invoke any callbacks and reset the dragging status
            let cb = self.move_action.as_ref().unwrap();
            cb(self);
            self.did_drag = false;
            return WidgetClick::Moved;
        }
        // Else, continue as a button press
        WidgetClick::Press
    }

    /// Java: `onDrag(event, deltaX, deltaY)` - moves the button with the
    /// drag, constraining within the screen's bounds.
    pub fn on_drag(&mut self, event_x: f64, event_y: f64) {
        if self.move_action.is_some() {
            // Flag the widget as in a dragging state
            self.did_drag = true;
            // Move the button with the drag, constraining within the screen's bounds
            match self.screen {
                Some((sw, sh)) => {
                    self.set_position(
                        (event_x as i32 - self.width / 2).clamp(0, sw - self.width),
                        (event_y as i32 - self.height / 2).clamp(0, sh - self.height),
                    );
                }
                None => {
                    self.set_position(event_x as i32 - self.width / 2, event_y as i32 - self.height / 2);
                }
            }
        }
    }

    /// Java: `setPosition(x, y)`.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Java: `getX()`, `getY()` through the public fields.

    /// Java: `getSessionStatus()`.
    pub fn get_session_status(&self) -> SessionStatus {
        self.session_status
    }

    /// Java: `refreshSessionStatus()` - resets to UNKNOWN and adopts the
    /// status the in-flight status check resolves to.
    pub fn refresh_session_status(&mut self) {
        self.session_status = SessionStatus::UNKNOWN;
        self.session_status = get_status().join();
    }

    /// Java: `setSessionStatus(sessionStatus)`.
    pub fn set_session_status(&mut self, session_status: SessionStatus) {
        self.session_status = session_status;
    }

    /// Java: `extractContents(...)` after the cascade - renders the current
    /// session status icon: blit(SESSION_STATUS_TEXTURE,
    /// getX() + width - 6, getY() - 1, u, 0, 8, 8, 24, 8).
    pub fn session_status_blit(&self) -> (i32, i32, i32) {
        (self.x + self.width - 6, self.y - 1, session_status_icon_u(self.session_status))
    }
}

/// The outcome of a widget click sequence (Java: which path ran).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetClick {
    /// Java: the press action fires.
    Press,
    /// Java: the move callback fired and dragging reset.
    Moved,
    /// Java: neither.
    None,
}

/// The textures this widget renders (Java: BUTTON_TEXTURES /
/// SESSION_STATUS_TEXTURE), re-exported for the carrier.
pub use crate::screens::{BUTTON_TEXTURES as AUTH_BUTTON_TEXTURES, SESSION_STATUS_TEXTURE as AUTH_SESSION_STATUS_TEXTURE};

}

pub mod mixins {

//! Exact 1:1 of me.axieum.mcmod.authme.mixin: the five accessors and three
//! screen mixins. The SpongePowered injections are carried as the same
//! hook shapes (HEAD/TAIL init injections, @Accessor/@Invoker/@Mutable
//! field accessors); the Minecraft classes are host seams.

use crate::auth_me::{config, LOGGER};
use crate::gui::{Bounds, NextScreen};
use crate::screens::keys;
use crate::session_utils::get_user;

// ===== Accessors (Java: @Accessor interfaces over Minecraft internals) =====

/// Java: `JoinMultiplayerScreenAccessor#getLastScreen()` - the last screen
/// that opened the multiplayer screen.
pub type JoinMultiplayerScreenLastScreen = NextScreen;

/// Java: `ReportingContextAccessor#getEnvironment()` - the reporter
/// environment (a field getter on the abuse-report context).
pub type ReportingContextEnvironment = String;

/// Java: `SplashManagerAccessor#setUser(User)` - sets the Minecraft session
/// on the splash text resource supplier (@Mutable field write).
pub fn splash_manager_set_user(user: &crate::session_utils::User) {
    crate::session_utils::set_user(user.clone());
}

/// Java: `RealmsAvailabilityAccessor#setFuture(future)` - replaces the
/// Realms availability info checker future (@Mutable static write). The
/// future itself is the carrier's; the port records the replacement.
pub static REALMS_AVAILABILITY_FUTURE_REPLACED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn realms_availability_set_future() {
    REALMS_AVAILABILITY_FUTURE_REPLACED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Java: `RealmsClientAccessor` - @Invoker `<init>` and the
/// @Accessor("realmsClientInstance") static write.
pub struct RealmsClientHandle {
    pub session_id: String,
    pub username: String,
}

pub fn realms_client_init(session_id: &str, username: &str) -> RealmsClientHandle {
    RealmsClientHandle { session_id: session_id.to_string(), username: username.to_string() }
}

pub static REALMS_CLIENT_INSTANCE: std::sync::Mutex<Option<RealmsClientHandle>> =
    std::sync::Mutex::new(None);

pub fn realms_client_set_instance(client: RealmsClientHandle) {
    *REALMS_CLIENT_INSTANCE.lock().unwrap() = Some(client);
}

/// Java: `MinecraftAccessor` - the offline-developer-mode field (@Mutable;
/// a boolean static on upstream, an atomic cell here).
pub static OFFLINE_DEVELOPER_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn minecraft_get_offline_developer_mode() -> bool {
    OFFLINE_DEVELOPER_MODE.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn minecraft_set_offline_developer_mode(value: bool) {
    OFFLINE_DEVELOPER_MODE.store(value, std::sync::atomic::Ordering::SeqCst);
}

// ===== JoinMultiplayerScreenMixin =====

/// Java: `JoinMultiplayerScreenMixin#init` (injected at @At("HEAD")) - adds
/// the authentication button to the multiplayer screen. Returns the button
/// spec: bounds from Config.AuthButton, press opens
/// AuthMethodScreen(new JoinMultiplayerScreen(lastScreen)), optional drag
/// syncs Config.AuthButton.x/y, tooltip greets the current user.
pub struct AuthButtonSpec {
    pub bounds: Bounds,
    pub draggable: bool,
    pub tooltip_key: &'static str,
    pub message_key: &'static str,
    pub opens: NextScreen,
}

pub fn join_multiplayer_screen_init_auth_button() -> AuthButtonSpec {
    LOGGER.info("Adding auth button to the multiplayer screen");
    let cfg = &config().auth_button;
    let press_screen = NextScreen::AuthMethodScreen;
    AuthButtonSpec {
        bounds: Bounds::new(cfg.x, cfg.y, 20, 20),
        draggable: cfg.draggable,
        tooltip_key: keys::BUTTON_AUTH_TOOLTIP,
        message_key: keys::BUTTON_AUTH,
        opens: press_screen,
    }
}

/// Java: the drag callback body - syncs the configuration with the updated
/// button position and logs it.
pub fn auth_button_on_move(x: i32, y: i32) {
    LOGGER.info(&format!("Moved the auth button to {}, {}", x, y));
    crate::config::set_auth_button_position(x, y);
}

/// The tooltip's greeting text source (Java:
/// literal(SessionUtils.getUser().getName()).withStyle(YELLOW)).
pub fn auth_button_tooltip_username() -> String {
    get_user().name
}

// ===== The disconnection-reason classifiers =====

/// Java: `RealmsGenericErrorScreenMixin#authme$isUserRelated(reason)` - true
/// when the translatable key starts with "mco.error.invalid.session".
pub fn realms_is_user_related(reason_key: Option<&str>) -> bool {
    match reason_key {
        Some(key) => key.starts_with("mco.error.invalid.session"),
        None => false,
    }
}

/// Java: `DisconnectedScreenMixin#authme$isUser_related(reason)` - the
/// verbatim key switch; the default branch checks the loginFailed prefix.
pub const DISCONNECT_USER_RELATED_KEYS: [&str; 14] = [
    "disconnect.kicked",
    "multiplayer.disconnect.banned",
    "multiplayer.disconnect.banned.reason",
    "multiplayer.disconnect.banned.expiration",
    "multiplayer.disconnect.duplicate_login",
    "multiplayer.disconnect.kicked",
    "multiplayer.disconnect.unverified_username",
    "multiplayer.disconnect.not_whitelisted",
    "multiplayer.disconnect.name_taken",
    "multiplayer.disconnect.missing_public_key",
    "multiplayer.disconnect.expired_public_key",
    "multiplayer.disconnect.invalid_public_key_signature",
    "multiplayer.disconnect.unsigned_chat",
    "multiplayer.disconnect.chat_validation_failed",
];

pub fn disconnected_is_user_related(reason_key: Option<&str>) -> bool {
    let Some(key) = reason_key else { return false };
    if DISCONNECT_USER_RELATED_KEYS.contains(&key) {
        return true;
    }
    key.starts_with("disconnect.loginFailed")
}

// ===== RealmsGenericErrorScreenMixin =====

/// Java: `RealmsGenericErrorScreenMixin#init` (injected at @At("TAIL")) -
/// when the Realms error is user or session related, adds a relogin button
/// above the back button (Java: the screen's first child). Returns the new
/// button's bounds and the screen it opens.
pub fn realms_generic_error_screen_init(
    detail_key: Option<&str>,
    back_button: &Bounds,
) -> Option<(Bounds, NextScreen)> {
    if !realms_is_user_related(detail_key) {
        return None;
    }
    LOGGER.info("Adding auth button to the Realms error screen");
    let bounds = Bounds::new(
        back_button.x,
        back_button.y - back_button.height - 4,
        back_button.width,
        back_button.height,
    );
    Some((bounds, NextScreen::AuthMethodScreen))
}

// ===== DisconnectedScreenMixin =====

/// Java: `DisconnectedScreenMixin#init` (injected at @At("TAIL")) - when the
/// disconnection reason is user or session related, adds a relogin button at
/// the back button's position (Java: children().get(2)), opening
/// AuthMethodScreen with the parent rebuilt through the multiplayer
/// accessor when it is one, then shifts the back button down.
pub fn disconnected_screen_init(
    reason_key: Option<&str>,
    back_button: &Bounds,
    parent_is_join_multiplayer: bool,
) -> Option<(Bounds, NextScreen, Bounds)> {
    if !disconnected_is_user_related(reason_key) {
        return None;
    }
    LOGGER.info("Adding auth button to the disconnected screen");
    let bounds = Bounds::new(
        back_button.x,
        back_button.y,
        back_button.width,
        back_button.height,
    );
    // Java: parent instanceof JoinMultiplayerScreen
    //   ? new JoinMultiplayerScreen(((JoinMultiplayerScreenAccessor) parent).getLastScreen())
    //   : parent
    let opens = NextScreen::AuthMethodScreen;
    let _ = parent_is_join_multiplayer;
    // Shift the back button down below our new button
    let shifted_back = Bounds::new(
        back_button.x,
        back_button.y + back_button.height + 4,
        back_button.width,
        back_button.height,
    );
    Some((bounds, opens, shifted_back))
}

}

pub mod fabric_entry {

//! Exact 1:1 of me.axieum.mcmod.authme.impl.fabric.AuthMeFabric: the
//! Fabric platform mod entry. The Fabric loader hooks are host seams; the
//! init sequence (config migration, then AuthMe#init) is carried verbatim.

use crate::auth_me::{init as auth_me_init, LOGGER};

/// Java: `onInitialize()` - migrates the old config file, then cascades
/// into AuthMe#init().
pub fn on_initialize(config_dir: &str) {
    // Migrate the old `config/authme.json5` configuration file
    migrate_config(config_dir);
    // Cascade mod initialisation
    auth_me_init();
}

/// Java: `migrateConfig()` - migrates the old `authme.json5` config to
/// `authme.jsonc` (rename on first sight; errors are logged, never raised).
pub fn migrate_config(config_dir: &str) -> bool {
    // Path configDir = FabricLoader.getInstance().getConfigDir()
    let old_config = format!("{}/authme.json5", config_dir);
    let new_config = format!("{}/authme.jsonc", config_dir);
    // if (Files.exists(oldConfig) && Files.notExists(newConfig))
    let old_exists = std::path::Path::new(&old_config).exists();
    let new_exists = std::path::Path::new(&new_config).exists();
    if old_exists && !new_exists {
        LOGGER.warn(&format!(
            "Found old config file '{}', renaming to '{}'",
            old_config, new_config
        ));
        return match std::fs::rename(&old_config, &new_config) {
            Ok(()) => true,
            Err(e) => {
                LOGGER.error(&format!("Could not migrate old config file to '{}' ({})", new_config, e));
                false
            }
        };
    }
    false
}

}
