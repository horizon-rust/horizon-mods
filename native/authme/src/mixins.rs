
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
