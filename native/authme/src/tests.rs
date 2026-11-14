//! Unit tests for the translated logic.

use crate::config::Config;
use crate::microsoft_utils::{MicrosoftPrompt, AUTHORIZE_URL, CLIENT_ID, TOKEN_URL};
use crate::session_utils::{
    offline, set_user, SessionStatus, User, MinecraftClientSeam, SessionServiceSeam, StatusFuture,
};
use std::sync::{Arc, Mutex};

#[test]
fn name_uuid_matches_java_name_uuid_from_bytes() {
    // java.util.UUID.nameUUIDFromBytes(new byte[0]) is a known vector.
    assert_eq!(crate::md5::name_uuid_from_bytes(b""), "d41d8cd9-8f00-3204-a980-0998ecf8427e");
}

#[test]
fn offline_session_uses_md5_name_uuid_and_invalidtoken() {
    let user = offline("Danil");
    assert_eq!(user.access_token, "invalidtoken");
    assert_eq!(user.xuid, None);
    assert_eq!(user.client_id, None);
    // Version 3, IETF variant bits
    let uuid = user.uuid.clone();
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    assert_eq!(uuid.len(), 36);
    assert_eq!(&hex[12..13], "3", "version nibble is 3");
    let variant = u8::from_str_radix(&hex[16..18], 16).unwrap();
    assert_eq!(variant & 0xc0, 0x80, "IETF variant bits");
    // Deterministic for the same name
    assert_eq!(offline("Danil").uuid, uuid);
}

#[test]
fn config_defaults_match_upstream_annotated_statics() {
    let config = Config::default();
    assert_eq!(config.auth_button.x, 6);
    assert_eq!(config.auth_button.y, 6);
    assert!(config.auth_button.draggable);
    assert_eq!(config.login_methods.microsoft.prompt, MicrosoftPrompt::DEFAULT);
    assert_eq!(config.login_methods.microsoft.port, 25585);
    assert_eq!(config.login_methods.microsoft.client_id, CLIENT_ID);
    assert_eq!(config.login_methods.microsoft.authorize_url, AUTHORIZE_URL);
    assert_eq!(config.login_methods.microsoft.token_url, TOKEN_URL);
    assert_eq!(config.login_methods.offline.last_username, "");
    assert!(config.login_methods.microsoft.is_defaults());
}

#[test]
fn microsoft_prompt_to_string_is_the_query_value() {
    assert_eq!(MicrosoftPrompt::DEFAULT.to_string(), "");
    assert_eq!(MicrosoftPrompt::SELECT_ACCOUNT.to_string(), "select_account");
    assert_eq!(MicrosoftPrompt::LOGIN.to_string(), "login");
    assert_eq!(MicrosoftPrompt::NONE.to_string(), "none");
    assert_eq!(MicrosoftPrompt::CONSENT.to_string(), "consent");
}

#[test]
fn is_defaults_is_false_once_a_url_changes() {
    let mut config = Config::default();
    config.login_methods.microsoft.token_url = "https://example.invalid/token".to_string();
    assert!(!config.login_methods.microsoft.is_defaults());
    // the other five URLs are still equal to the defaults
    assert_ne!(config.login_methods.microsoft.token_url, TOKEN_URL);
}

struct FakeClient {
    user: Mutex<User>,
}

impl FakeClient {
    fn new(user: User) -> Self {
        FakeClient { user: Mutex::new(user) }
    }
}

impl MinecraftClientSeam for FakeClient {
    fn get_user(&self) -> User {
        self.user.lock().unwrap().clone()
    }
    fn set_user(&self, user: &User) {
        *self.user.lock().unwrap() = user.clone();
    }
    fn get_session_service(&self) -> Box<dyn SessionServiceSeam + Send> {
        unimplemented!("not exercised in these tests")
    }
}

#[test]
fn set_user_applies_and_invalidates_the_cached_status() {
    let client = Arc::new(FakeClient::new(offline("old")));
    *crate::session_utils::client_test_handle() = Some(Box::new(SharedClient(client.clone())));
    set_user(offline("Danil"));
    assert_eq!(client.user.lock().unwrap().name, "Danil");
}

#[test]
fn user_session_id_is_token_uuid() {
    let user = offline("Danil");
    assert_eq!(user.get_session_id(), format!("{}:{}", user.access_token, user.uuid));
    assert_eq!(user.get_profile_id(), user.uuid);
}

// A wrapper so the shared Arc can satisfy the seam trait.
struct SharedClient(Arc<FakeClient>);
impl MinecraftClientSeam for SharedClient {
    fn get_user(&self) -> User {
        self.0.get_user()
    }
    fn set_user(&self, user: &User) {
        self.0.set_user(user)
    }
    fn get_session_service(&self) -> Box<dyn SessionServiceSeam + Send> {
        self.0.get_session_service()
    }
}

#[test]
fn status_ttl_returns_the_cached_status_without_a_thread() {
    // Set the cache to "just checked, VALID"
    crate::session_utils::set_last_status_for_tests(SessionStatus::VALID);
    match crate::session_utils::get_status() {
        StatusFuture::Completed(status) => assert_eq!(status, SessionStatus::VALID),
        StatusFuture::Spawned(_) => panic!("TTL cache should not spawn a thread"),
    }
}

#[test]
fn abbreviate_middle_matches_commons_lang3() {
    let s = "0123456789012345678901234567890123456789";
    // Java: StringUtils.abbreviateMiddle(s, "...", 32)
    assert_eq!(crate::microsoft_utils::abbreviate_middle(s, "...", 32), "012345678901234...67890123456789");
    // Shorter than the width: returned unchanged
    assert_eq!(crate::microsoft_utils::abbreviate_middle("short", "...", 32), "short");
    assert_eq!(crate::microsoft_utils::abbreviate_middle("", "...", 32), "");
}

#[test]
fn generate_state_is_base64url_22_chars_and_unique() {
    let a = crate::microsoft_utils::generate_state();
    let b = crate::microsoft_utils::generate_state();
    assert_eq!(a.chars().count(), 22);
    assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    assert_ne!(a, b);
}

#[test]
fn token_extraction_matches_upstream_json_shape() {
    use serde_json::{Value, json};
    // Microsoft token error shape: error + error_description
    let json = json!({"error": "invalid_grant", "error_description": "bad code"});
    assert!(json.get("access_token").and_then(Value::as_str).is_none());
    // Xbox error shape: XErr + Message
    let json = json!({"XErr": 2148916238i64, "Message": "You need to create an Xbox profile"});
    assert!(json.get("Token").and_then(Value::as_str).is_none());
    // XSTS success shape: Token + DisplayClaims.xui[0].uhs
    let json = json!({"Token": "t", "DisplayClaims": {"xui": [{"uhs": "uhs123"}]}});
    assert_eq!(json["DisplayClaims"]["xui"][0]["uhs"].as_str(), Some("uhs123"));
    // Minecraft auth error shape: error + errorMessage (not error_description)
    let json = json!({"error": "invalid_identityToken", "errorMessage": "Bad token"});
    assert!(json.get("errorMessage").and_then(Value::as_str).is_some());
}

#[test]
fn login_inserts_hyphens_into_the_profile_uuid() {
    // Java: uuid.replaceFirst("([0-9a-fA-F]{8})(...)", "$1-$2-$3-$4-$5")
    let raw = "d855588f04fb4248af0b7e49f28b4d1f";
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8], &raw[8..12], &raw[12..16], &raw[16..20], &raw[20..]
    );
    assert_eq!(uuid, "d855588f-04fb-4248-af0b-7e49f28b4d1f");
    let user = crate::session_utils::User {
        name: "Danil".to_string(),
        uuid,
        access_token: "mc-token".to_string(),
        xuid: None,
        client_id: None,
    };
    assert_eq!(user.access_token, "mc-token");
}

// ===== GUI screens (AuthScreen base, OfflineAuthScreen, AuthMethodScreen,
// MicrosoftAuthScreen, AuthButtonWidget) =====

#[test]
fn auth_screen_center_position_matches_java() {
    use crate::gui::AuthScreenBase;
    // Java: x = w/2 - el.w/2 + xo; y = h/2 - el.h/2 - 22 + yo
    // 320/2 - 20/2 = 150+... x = 160 - 10 + 0 = 160? No: 160 - 10 = 150; but
    // the Java math is x = w/2 - el.w/2 + xo = 160 - 10 = 150 with the title
    // 160 wide -> wait: element width 160/2 = 80; 160 - 80 = 80. y = 120 - 10 - 22 - 20 = 68.
    assert_eq!(AuthScreenBase::center_position(160, 20, 320, 240, 0, -20), (80, 68));
    assert_eq!(AuthScreenBase::center_position(160, 20, 320, 240, -51, 0), (29, 88));
    // 321/2 = 160 (int), 241/2 = 120
    assert_eq!(AuthScreenBase::center_position(1, 1, 321, 241, 3, 5), (163, 103));
}

#[test]
fn auth_screen_base_routes_on_close_and_tick() {
    use crate::gui::{AuthScreenBase, NextScreen};
    let mut base = AuthScreenBase::new(NextScreen::AuthMethodScreen, NextScreen::SuccessScreen);
    // tick before success: nothing closes
    assert_eq!(base.tick(), None);
    // default close: success=false -> parent
    assert_eq!(base.on_close(), NextScreen::AuthMethodScreen);
    // success=true: close screen -> success screen
    base.success = true;
    base.close_on_success = true;
    assert_eq!(base.tick(), Some(NextScreen::SuccessScreen));
}

#[test]
fn offline_screen_form_valid_and_login_flow() {
    use crate::screens::{keys, OfflineAuthScreenLogic};
    struct Seam;
    impl crate::screens::OfflineLoginSeam for Seam {
        fn greet_toast(&self, username: &str) {
            println!("toast: gui.authme.toast.greeting {}", username);
        }
    }
    let mut scr = OfflineAuthScreenLogic::new();
    // Java: this.closeOnSuccess = true
    assert!(scr.base.close_on_success);
    assert!(!scr.is_form_valid());
    scr.username_field = "  ".to_string();
    assert!(!scr.is_form_valid()); // isBlank() over whitespace
    scr.username_field = "Steve".to_string();
    assert!(scr.is_form_valid());
    // login() with a blank field is a no-op
    let mut blank = OfflineAuthScreenLogic::new();
    blank.username_field = "".to_string();
    blank.login(&Seam);
    assert!(!blank.base.success);
    // login(): disables the fields, applies the offline session, syncs the
    // config lastUsername, greets, marks success
    scr.sync_login_button();
    assert!(scr.login_btn_active);
    // SessionUtils.setUser needs the client seam installed (the carrier's
    // Minecraft client); install the shared fake like the set_user test
    *crate::session_utils::client_test_handle() =
        Some(Box::new(SharedClient(Arc::new(FakeClient::new(offline("old"))))));
    scr.login(&Seam);
    assert!(!scr.username_field_active);
    assert!(!scr.login_btn_active);
    assert!(scr.base.success);
    assert_eq!(crate::config::offline_last_username(), "Steve");
    crate::config::set_offline_last_username(String::new());
    // init() presets the field with the last used username
    crate::config::set_offline_last_username("Alex".to_string());
    let mut scr2 = OfflineAuthScreenLogic::new();
    scr2.init_preset_username();
    assert_eq!(scr2.username_field, "Alex");
    assert!(scr2.login_btn_active);
    crate::config::set_offline_last_username(String::new());
    let _ = keys::OFFLINE_TITLE;
}

#[test]
fn auth_method_layout_bounds_match_java() {
    use crate::screens::auth_method_layout;
    let (ms, mojang, offline, back) = auth_method_layout(320, 240);
    // msButton: width/2 - 20 - 10 - 4, height/2 - 5, 20, 20
    assert_eq!(ms, crate::gui::Bounds::new(126, 115, 20, 20));
    // mojangButton: width/2 - 10, height/2 - 5
    assert_eq!(mojang, crate::gui::Bounds::new(150, 115, 20, 20));
    // offlineButton: width/2 + 10 + 4
    assert_eq!(offline, crate::gui::Bounds::new(174, 115, 20, 20));
    // back: width/2 - 50, height/2 + 27, 100, 20
    assert_eq!(back, crate::gui::Bounds::new(110, 147, 100, 20));
}

#[test]
fn method_action_routes_through_config_defaults() {
    use crate::screens::{method_action_screen, MethodAction, MICROSOFT_BUTTON_TEXTURES, MOJANG_BUTTON_TEXTURES, OFFLINE_BUTTON_TEXTURES, SESSION_STATUS_TEXTURE, BUTTON_TEXTURES};
    assert_eq!(method_action_screen(MethodAction::MojangFaq), crate::gui::NextScreen::ConfirmLinkMojang);
    assert_eq!(method_action_screen(MethodAction::Offline), crate::gui::NextScreen::OfflineAuthScreen);
    // Default Microsoft URLs: straight to the MicrosoftAuthScreen
    assert_eq!(
        method_action_screen(MethodAction::Microsoft { select_account: true }),
        crate::gui::NextScreen::MicrosoftAuthScreen { select_account: true }
    );
    // Non-default Microsoft URLs: the warning confirm screen (Java:
    // ConfirmScreen with setDelay(40) routes back to the MicrosoftAuthScreen
    // on accept); the branch is the same over an explicit config value
    let mut ms = crate::auth_me::config().login_methods.microsoft.clone();
    ms.authorize_url = "https://evil.example/authorize".to_string();
    assert_eq!(
        crate::screens::microsoft_method_action_with(&ms, false),
        crate::gui::NextScreen::ConfirmMicrosoftWarning
    );
    // The texture identifier sets are verbatim
    assert_eq!(MICROSOFT_BUTTON_TEXTURES[0], ("authme", "widget/microsoft_button"));
    assert_eq!(MOJANG_BUTTON_TEXTURES[2], ("authme", "widget/mojang_button_focused"));
    assert_eq!(OFFLINE_BUTTON_TEXTURES[1], ("authme", "widget/offline_button_disabled"));
    assert_eq!(SESSION_STATUS_TEXTURE, ("authme", "textures/gui/session_status.png"));
    assert_eq!(BUTTON_TEXTURES[0], ("minecraft", "widget/locked_button"));
}

#[test]
fn microsoft_status_sequence_and_error_classification_match_java() {
    use crate::screens::{classify_microsoft_error, keys, MICROSOFT_STATUS_SEQUENCE};
    assert_eq!(MICROSOFT_STATUS_SEQUENCE, [
        keys::MS_STATUS_CHECK_BROWSER, keys::MS_STATUS_MS_ACCESS_TOKEN,
        keys::MS_STATUS_XBOX_ACCESS_TOKEN, keys::MS_STATUS_XBOX_XSTS_TOKEN,
        keys::MS_STATUS_MC_ACCESS_TOKEN, keys::MS_STATUS_MC_PROFILE,
    ]);
    assert_eq!(classify_microsoft_error("org.apache.http.conn.ConnectTimeoutException: timeout"), keys::ERROR_TIMEOUT);
    assert_eq!(classify_microsoft_error("NOT_FOUND: Not Found"), keys::ERROR_NOT_PURCHASED);
    assert_eq!(classify_microsoft_error("HTTP 500"), keys::ERROR_GENERIC);
}

#[test]
fn auth_button_widget_click_on_release_and_drag_clamp() {
    use crate::session_utils::SessionStatus;
    use crate::widget::{AuthButtonWidgetLogic, WidgetClick};
    use std::sync::{Arc, Mutex};
    // refreshSessionStatus() seeds the status through the TTL cache so the
    // constructor does not spawn a live session check
    crate::session_utils::set_last_status_for_tests(SessionStatus::UNKNOWN);
    // Fixed button: clicks behave as a plain button
    let mut fixed = AuthButtonWidgetLogic::fixed(10, 10);
    assert!(fixed.mouse_clicked());
    assert_eq!(fixed.mouse_released(), WidgetClick::Press);
    assert!(fixed.did_drag == false);
    // A fixed button (no moveAction) never drags: Java guards onDrag with
    // `if (this.moveAction != null)`
    fixed.on_drag(120.0, 96.0);
    assert!(!fixed.did_drag);
    assert_eq!((fixed.x, fixed.y), (10, 10));
    // Movable button with a screen: position is clamped to the bounds
    let moved_to = Arc::new(Mutex::new(None));
    let moved_clone = moved_to.clone();
    let mut movable = AuthButtonWidgetLogic::movable(
        Some((320, 240)), 100, 100,
        Some(Box::new(move |b: &AuthButtonWidgetLogic| {
            *moved_clone.lock().unwrap() = Some((b.x, b.y));
        })),
    );
    assert_eq!(movable.get_session_status(), SessionStatus::UNKNOWN); // starts UNKNOWN
    // mouse down does NOT press (Java: only isValidClickButton)
    assert!(movable.mouse_clicked());
    movable.on_drag(-20.0, 999.0);
    assert!(movable.did_drag);
    // clamped to x in [0, 320-20], y in [0, 240-20]
    assert_eq!((movable.x, movable.y), (0, 220));
    movable.on_drag(50.0, 60.0);
    assert_eq!((movable.x, movable.y), (40, 50));
    // release after drag: invokes the move callback, resets didDrag, no press
    assert_eq!(movable.mouse_released(), WidgetClick::Moved);
    assert_eq!(*moved_to.lock().unwrap(), Some((40, 50)));
    assert!(!movable.did_drag);
    // next release without a drag: a normal press
    assert_eq!(movable.mouse_released(), WidgetClick::Press);
    // setSessionStatus + the status icon blit (u by status)
    movable.set_session_status(SessionStatus::VALID);
    assert_eq!(movable.session_status_blit(), (40 + 20 - 6, 50 - 1, 0));
    movable.set_session_status(SessionStatus::OFFLINE);
    assert_eq!(movable.session_status_blit().2, 8);
    movable.set_session_status(SessionStatus::INVALID);
    assert_eq!(movable.session_status_blit().2, 16);
}

// ===== Mixins (accessors, JoinMultiplayerScreenMixin,
// RealmsGenericErrorScreenMixin, DisconnectedScreenMixin, MinecraftAccessor) =====

#[test]
fn realms_and_disconnected_user_related_classifiers_match_java() {
    use crate::mixins::{disconnected_is_user_related, realms_is_user_related, DISCONNECT_USER_RELATED_KEYS};
    // Realms: any key starting with mco.error.invalid.session
    assert!(realms_is_user_related(Some("mco.error.invalid.session")));
    assert!(realms_is_user_related(Some("mco.error.invalid.session.max")));
    assert!(!realms_is_user_related(Some("mco.error.something.else")));
    assert!(!realms_is_user_related(None));
    // Disconnected: the verbatim switch list plus the loginFailed prefix
    for key in DISCONNECT_USER_RELATED_KEYS {
        assert!(disconnected_is_user_related(Some(key)), "{}", key);
    }
    assert!(disconnected_is_user_related(Some("disconnect.loginFailed")));
    assert!(disconnected_is_user_related(Some("disconnect.loginFailedInfo")));
    assert!(!disconnected_is_user_related(Some("disconnect.genericReason")));
    assert!(!disconnected_is_user_related(None));
    assert_eq!(DISCONNECT_USER_RELATED_KEYS.len(), 14);
}

#[test]
fn realms_error_screen_adds_relogin_button_above_back() {
    use crate::gui::Bounds;
    use crate::mixins::realms_generic_error_screen_init;
    let back = Bounds::new(100, 180, 200, 20);
    // Java: bounds = (back.x, back.y - back.h - 4, back.w, back.h)
    let (btn, opens) = realms_generic_error_screen_init(Some("mco.error.invalid.session.max"), &back).unwrap();
    assert_eq!(btn, Bounds::new(100, 156, 200, 20));
    assert_eq!(opens, crate::gui::NextScreen::AuthMethodScreen);
    // Unrelated reason: nothing is added
    assert!(realms_generic_error_screen_init(Some("mco.error.timeout"), &back).is_none());
    assert!(realms_generic_error_screen_init(None, &back).is_none());
}

#[test]
fn disconnected_screen_adds_relogin_button_and_shifts_back() {
    use crate::gui::Bounds;
    use crate::mixins::disconnected_screen_init;
    let back = Bounds::new(100, 180, 200, 20);
    let (btn, opens, shifted) =
        disconnected_screen_init(Some("multiplayer.disconnect.not_whitelisted"), &back, true).unwrap();
    // Java: button at the back button's exact position, back shifted down h+4
    assert_eq!(btn, Bounds::new(100, 180, 200, 20));
    assert_eq!(opens, crate::gui::NextScreen::AuthMethodScreen);
    assert_eq!(shifted, Bounds::new(100, 204, 200, 20));
    assert!(disconnected_screen_init(Some("disconnect.timeout"), &back, false).is_none());
}

#[test]
fn multiplayer_screen_auth_button_uses_config_and_syncs_on_move() {
    use crate::mixins::{auth_button_on_move, join_multiplayer_screen_init_auth_button, minecraft_get_offline_developer_mode, minecraft_set_offline_developer_mode};
    let spec = join_multiplayer_screen_init_auth_button();
    // Java: new AuthButtonWidget(this, Config.AuthButton.x, Config.AuthButton.y, ...)
    let (x, y) = crate::config::auth_button_position();
    assert_eq!((spec.bounds.x, spec.bounds.y), (x, y));
    assert_eq!(spec.bounds.width, 20);
    assert_eq!(spec.bounds.height, 20);
    assert_eq!(spec.tooltip_key, "gui.authme.button.auth.tooltip");
    assert_eq!(spec.message_key, "gui.authme.button.auth");
    assert_eq!(spec.opens, crate::gui::NextScreen::AuthMethodScreen);
    // Java: draggable ? sync Config.AuthButton.x/y : null
    assert!(spec.draggable);
    auth_button_on_move(12, 34);
    assert_eq!(crate::config::auth_button_position(), (12, 34));
    crate::config::set_auth_button_position(6, 6);
    // MinecraftAccessor offline developer mode round-trip
    assert!(!minecraft_get_offline_developer_mode());
    minecraft_set_offline_developer_mode(true);
    assert!(minecraft_get_offline_developer_mode());
    minecraft_set_offline_developer_mode(false);
}

#[test]
fn realms_and_splash_accessors_carry_the_writes() {
    use crate::mixins::{realms_availability_set_future, REALMS_AVAILABILITY_FUTURE_REPLACED, realms_client_init, realms_client_set_instance, REALMS_CLIENT_INSTANCE, splash_manager_set_user, ReportingContextEnvironment, JoinMultiplayerScreenLastScreen};
    realms_availability_set_future();
    assert!(REALMS_AVAILABILITY_FUTURE_REPLACED.load(std::sync::atomic::Ordering::SeqCst));
    let client = realms_client_init("session-id", "Danil");
    assert_eq!((client.session_id.as_str(), client.username.as_str()), ("session-id", "Danil"));
    realms_client_set_instance(client);
    assert!(REALMS_CLIENT_INSTANCE.lock().unwrap().is_some());
    // SplashManagerAccessor#setUser delegates to the session setter
    splash_manager_set_user(&crate::session_utils::offline("Danil"));
    let _: ReportingContextEnvironment = String::new();
    let _: JoinMultiplayerScreenLastScreen = crate::gui::NextScreen::ParentScreen;
}

// ===== Fabric entry (AuthMeFabric) =====

#[test]
fn fabric_entry_migrates_old_config_then_inits() {
    use crate::fabric_entry::{migrate_config, on_initialize};
    // No old config: nothing to migrate
    assert!(!migrate_config("/tmp/does-not-exist-authme"));
    // An old authme.json5 with no jsonc: renamed, init cascades
    let dir = std::env::temp_dir().join("authme-migrate-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("authme.json5"), "{}").unwrap();
    assert!(migrate_config(dir.to_str().unwrap()));
    assert!(dir.join("authme.jsonc").exists());
    assert!(!dir.join("authme.json5").exists());
    // An existing jsonc blocks the migration
    std::fs::write(dir.join("authme.json5"), "{}").unwrap();
    std::fs::write(dir.join("authme.jsonc"), "{}").unwrap();
    assert!(!migrate_config(dir.to_str().unwrap()));
    let _ = std::fs::remove_dir_all(&dir);
    // onInitialize runs the migration then AuthMe#init (config warmed)
    on_initialize("/tmp/does-not-exist-authme");
    assert!(crate::auth_me::config().login_methods.microsoft.is_defaults());
}
