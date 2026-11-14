
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
