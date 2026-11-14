
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
