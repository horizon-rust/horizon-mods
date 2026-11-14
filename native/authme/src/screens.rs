
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

