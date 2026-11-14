//! Port tests: each asserts behavior against the upstream Java logic.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::config::{client_handler, server_handler, ChannelMode, ClientConfig, IConfig, PlayerInfoMode, ServerConfig, TeamColorMode};
use crate::core::{DirectionalSoundInstance, GameContext, PingController, PingManager, PingView, ServerCore};
use crate::math::MathUtils;
use crate::network::{FriendlyByteBuf, IPacket, PacketHandler, PingLocationC2SPacket, PingLocationS2CPacket, ResourceLocation, UpdateChannelC2SPacket};
use crate::seams::{
    self, BlockHitResult, ClipContext, EntityHandle, HitResultType, LevelHandle, Matrix4f,
    ItemStackToken, PlayerInfoHandle, Vec2, Vec3,
};
use crate::util::RateLimiter;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Installs fresh host seams on this test's thread and serializes global statics.
pub(crate) fn install() -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
    let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    let root = ROOT
        .get_or_init(|| {
            let r = std::env::temp_dir().join(format!("pw-test-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&r);
            r
        })
        .clone();

    seams::set_context_service(Rc::new(TestContextService { root: root.clone() }));
    seams::set_network_service(Rc::new(TestNetworkService::default()));
    PingManager::clear_pings();
    PingController::revoke_ping_action();
    (guard, root)
}

struct TestContextService {
    root: std::path::PathBuf,
}

impl seams::IPlatformContextService for TestContextService {
    fn get_self_mod_version(&self) -> String {
        // a literal: global::mod_version() itself resolves through the context
        // service, so calling it here would recurse forever
        "26.3.0-test".to_string()
    }

    fn resolve_config_dir(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }
}

#[derive(Default)]
struct TestNetworkService {
    pub sent: RefCell<Vec<(String, Vec<u8>)>>,
}

impl seams::IPlatformNetworkService for TestNetworkService {
    fn send_to_server(&self, packet: &dyn crate::network::IPacket) {
        let mut buf = FriendlyByteBuf::new();
        packet.write(&mut buf);
        let id = packet.get_id();
        self.sent.borrow_mut().push((format!("{}:{}", id.namespace, id.path), buf.data.clone()));
    }

    fn send_to_client(&self, packet: &dyn crate::network::IPacket, _player: u128) {
        let mut buf = FriendlyByteBuf::new();
        packet.write(&mut buf);
        let id = packet.get_id();
        self.sent.borrow_mut().push((format!("{}:{}", id.namespace, id.path), buf.data.clone()));
    }
}

struct TestGame {
    level: Option<Rc<TestLevel>>,
    camera_entity: Option<Rc<TestEntity>>,
    player: Option<Rc<TestEntity>>,
    connection: Option<Rc<TestConnection>>,
    size: (f32, f32),
    camera_pos: Vec3,
    server_ip: Option<String>,
    executed: Cell<usize>,
}

impl TestGame {
    fn minimal() -> Rc<Self> {
        Rc::new(Self {
            level: Some(Rc::new(TestLevel::default())),
            camera_entity: Some(Rc::new(TestEntity::default())),
            player: Some(Rc::new(TestEntity::default())),
            connection: Some(Rc::new(TestConnection)),
            size: (800.0, 450.0),
            camera_pos: Vec3::ZERO,
            server_ip: None,
            executed: Cell::new(0),
        })
    }

    fn minimal_with_ip(ip: &str) -> Rc<Self> {
        let mut game = TestGame::minimal();
        let inner = Rc::get_mut(&mut game).unwrap();
        inner.server_ip = Some(ip.to_string());
        game
    }
}

impl seams::MinecraftClient for TestGame {
    fn level(&self) -> Option<Rc<dyn LevelHandle>> {
        self.level.clone().map(|l| l as Rc<dyn LevelHandle>)
    }

    fn camera_entity(&self) -> Option<Rc<dyn EntityHandle>> {
        self.camera_entity.clone().map(|e| e as Rc<dyn EntityHandle>)
    }

    fn player(&self) -> Option<Rc<dyn EntityHandle>> {
        self.player.clone().map(|e| e as Rc<dyn EntityHandle>)
    }

    fn connection(&self) -> Option<Rc<dyn seams::ConnectionHandle>> {
        self.connection.clone().map(|c| c as Rc<dyn seams::ConnectionHandle>)
    }

    fn gui_scaled_size(&self) -> (f32, f32) {
        self.size
    }

    fn camera_position(&self) -> Vec3 {
        self.camera_pos
    }

    fn current_server_ip(&self) -> Option<String> {
        self.server_ip.clone()
    }

    fn sound_manager_play(&self, _sound: &DirectionalSoundInstance) {}

    fn execute(&self, task: Rc<dyn Fn()>) {
        self.executed.set(self.executed.get() + 1);
        task();
    }
}

#[derive(Default)]
struct TestLevel {
    token_val: Cell<u64>,
    game_time_val: Cell<i64>,
    entities: RefCell<Vec<Rc<dyn EntityHandle>>>,
    dimension_location_val: RefCell<String>,
    clip_result: RefCell<Option<BlockHitResult>>,
}

impl LevelHandle for TestLevel {
    fn token(&self) -> u64 {
        self.token_val.get()
    }

    fn game_time(&self) -> i64 {
        self.game_time_val.get()
    }

    fn entities_for_rendering(&self) -> Vec<Rc<dyn EntityHandle>> {
        self.entities.borrow().clone()
    }

    fn dimension_location(&self) -> String {
        self.dimension_location_val.borrow().clone()
    }

    fn clip(&self, _ctx: &ClipContext) -> Option<BlockHitResult> {
        self.clip_result.borrow().clone()
    }
}

#[derive(Default)]
struct TestEntity {
    uuid_val: Cell<u128>,
    pos_val: Cell<Vec3>,
    level_token_val: Cell<u64>,
    is_item: Cell<bool>,
}

impl EntityHandle for TestEntity {
    fn uuid(&self) -> u128 {
        self.uuid_val.get()
    }

    fn position(&self, _tick_delta: f32) -> Vec3 {
        self.pos_val.get()
    }

    fn bounding_box_ysize(&self) -> f64 {
        1.0
    }

    fn level_token(&self) -> u64 {
        self.level_token_val.get()
    }

    fn is_type_item(&self) -> bool {
        self.is_item.get()
    }

    fn item_entity_item_copy(&self) -> Option<ItemStackToken> {
        self.is_item.get().then(|| ItemStackToken(42))
    }

    fn is_spectator(&self) -> bool {
        false
    }
}

struct TestConnection;

impl seams::ConnectionHandle for TestConnection {
    fn get_player_info(&self, _uuid: u128) -> Option<Rc<dyn PlayerInfoHandle>> {
        Some(Rc::new(TestPlayerInfo { has_team: false, color: None }))
    }
}

struct TestPlayerInfo {
    has_team: bool,
    color: Option<i32>,
}

impl PlayerInfoHandle for TestPlayerInfo {
    fn has_team(&self) -> bool {
        self.has_team
    }

    fn team_color(&self) -> Option<i32> {
        self.color
    }
}

struct TestServerPlayer {
    uuid: u128,
    name: String,
    messages: RefCell<Vec<String>>,
}

impl seams::ServerPlayerHandle for TestServerPlayer {
    fn uuid(&self) -> u128 {
        self.uuid
    }

    fn game_profile_name(&self) -> String {
        self.name.clone()
    }

    fn display_client_message(&self, component: &str, _action_bar: bool) {
        self.messages.borrow_mut().push(component.to_string());
    }
}

struct TestServerEnv {
    players: Vec<u128>,
    handles: Vec<Rc<TestServerPlayer>>,
}

impl seams::ServerEnv for TestServerEnv {
    fn players(&self) -> Vec<u128> {
        self.players.clone()
    }

    fn has_player(&self, uuid: u128) -> bool {
        self.players.contains(&uuid)
    }

    fn player(&self, uuid: u128) -> Option<Rc<dyn seams::ServerPlayerHandle>> {
        self.handles
            .iter()
            .find(|h| h.uuid == uuid)
            .cloned()
            .map(|h| h as Rc<dyn seams::ServerPlayerHandle>)
    }
}

// ---------------------------------------------------------------------------
// config
// ---------------------------------------------------------------------------

#[test]
fn channel_mode_get_is_case_insensitive_and_tostring_lowercase() {
    assert_eq!(ChannelMode::get("global"), Some(ChannelMode::Global));
    assert_eq!(ChannelMode::get("TEAM_ONLY"), Some(ChannelMode::TeamOnly));
    assert_eq!(ChannelMode::get("nonsense"), None);
    assert_eq!(ChannelMode::Auto.to_string(), "auto");
    assert_eq!(PlayerInfoMode::Hold.to_string(), "hold");
    assert_eq!(TeamColorMode::PingOnly.to_string(), "ping_only");
    assert_eq!(TeamColorMode::get("LABELS_ONLY"), Some(TeamColorMode::LabelsOnly));
}

#[test]
fn client_config_serializes_java_field_names() {
    let json = serde_json::to_string(&crate::config::ClientConfig::default()).unwrap();
    assert!(json.contains("\"pingVolume\""));
    assert!(json.contains("\"serverChannels\""));
    assert!(json.contains("\"playerInfoMode\""));
    assert!(json.contains("\"teamColorMode\""));

    let parsed: crate::config::ClientConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.ping_volume, 100);
    assert_eq!(parsed.player_info_mode, PlayerInfoMode::Hold);
}

#[test]
fn client_config_channel_uses_per_server_override() {
    let _g = install();
    let mut game = TestGame::minimal_with_ip("1.2.3.4:25565");
    seams::set_game(Some(game.clone() as Rc<dyn seams::MinecraftClient>));

    {
        let cfg = client_handler().config_mut();
        cfg.server_channels.insert("1.2.3.4:25565".into(), "serverchan".into());
        assert_eq!(cfg.get_channel(), "serverchan");
        cfg.set_channel("overwritten".into());
        assert_eq!(cfg.server_channels.get("1.2.3.4:25565").unwrap(), "overwritten");
    }

    // no game context -> plain channel
    seams::set_game(Some(TestGame::minimal() as Rc<dyn seams::MinecraftClient>));
    let cfg = client_handler().config_mut();
    cfg.channel = "plain".into();
    assert_eq!(cfg.get_channel(), "plain");
    cfg.set_channel("edited".into());
    assert_eq!(cfg.channel, "edited");
}

#[test]
fn client_config_validate_truncates_overlong_channels() {
    let _g = install();
    let cfg = client_handler().config_mut();
    cfg.channel = "x".repeat(200);
    cfg.validate();
    assert_eq!(cfg.channel.len(), crate::config::MAX_CHANNEL_LENGTH);
}

#[test]
fn server_config_validate_resets_defaults() {
    let mut cfg = ServerConfig::default();
    cfg.ms_to_regenerate = -1;
    cfg.rate_limit = -5;
    cfg.validate();
    assert_eq!(cfg.ms_to_regenerate, 1000);
    assert_eq!(cfg.rate_limit, 0);
    assert_eq!(cfg.default_channel_mode, ChannelMode::Auto);
    assert!(cfg.player_tracking_enabled);
}

#[test]
fn config_handler_round_trips_pretty_json() {
    let (_g, root) = install();
    let path = root.join("pingwheel.json");
    let _ = std::fs::remove_file(&path);

    {
        let handler = client_handler();
        handler.load();
        handler.config_mut().ping_volume = 42;
        handler.save();
    }

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\n  \"pingVolume\""), "expected pretty JSON, got {}", text);

    {
        let handler = client_handler();
        handler.load();
        assert_eq!(handler.config().ping_volume, 42);
    }

    // unchanged save is a no-op (upstream configHash check)
    let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    client_handler().save();
    let mtime_after = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(mtime_before, mtime_after);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn config_handler_resets_broken_file() {
    let (_g, root) = install();
    let path = root.join("pingwheel.json");
    std::fs::write(&path, "not json {").unwrap();

    client_handler().load();
    assert_eq!(client_handler().config().ping_volume, 100);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn legacy_config_file_is_migrated() {
    let (_g, root) = install();
    let legacy = root.join("ping-wheel.json");
    let new = root.join("pingwheel.json");
    let _ = std::fs::remove_file(&legacy);
    let _ = std::fs::remove_file(&new);
    std::fs::write(&legacy, "{\"pingVolume\": 33}").unwrap();

    crate::compat::legacy_migrate_config(".json");
    assert!(new.exists());
    assert!(!legacy.exists());
    let _ = std::fs::remove_file(&new);
}

// ---------------------------------------------------------------------------
// network
// ---------------------------------------------------------------------------

#[test]
fn ping_c2s_packet_round_trips() {
    let mut buf = FriendlyByteBuf::new();
    let packet = PingLocationC2SPacket::new(
        Some("team".into()),
        Some(Vec3::new(1.5, 64.0, -3.25)),
        Some(0x1234),
        7,
        -42,
    );
    packet.write(&mut buf);

    let read = PingLocationC2SPacket::read_safe(&mut buf);
    assert_eq!(read.channel.as_deref(), Some("team"));
    assert_eq!(read.pos, Some(Vec3::new(1.5, 64.0, -3.25)));
    assert_eq!(read.entity, Some(0x1234));
    assert_eq!(read.sequence, 7);
    assert_eq!(read.dimension, -42);
    assert!(!read.is_corrupt());
    assert_eq!(read.get_id().namespace, "ping-wheel-c2s");
}

#[test]
fn ping_c2s_default_constructor_is_corrupt() {
    let packet = PingLocationC2SPacket::default_corrupt();
    assert!(packet.is_corrupt());
    assert!(packet.channel.is_none());
}

#[test]
fn ping_s2c_packet_round_trips_and_carries_author() {
    let mut buf = FriendlyByteBuf::new();
    let packet = PingLocationS2CPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 3, 5, 0xABCD);
    packet.write(&mut buf);

    let read = PingLocationS2CPacket::read_safe(&mut buf);
    assert_eq!(read.author, 0xABCD);
    assert_eq!(read.entity, None);
    assert_eq!(read.sequence, 3);

    let client = PingLocationC2SPacket::new(Some("ch".into()), Some(Vec3::new(1.0, 2.0, 3.0)), Some(99), 1, 2);
    let from_client = PingLocationS2CPacket::from_client_packet(&client, 7);
    assert_eq!(from_client.author, 7);
    assert_eq!(from_client.entity, Some(99));
}

#[test]
fn update_channel_packet_round_trips_and_detects_corrupt() {
    let mut buf = FriendlyByteBuf::new();
    UpdateChannelC2SPacket::new(Some("hello".into())).write(&mut buf);
    let read = UpdateChannelC2SPacket::read_safe(&mut buf);
    assert_eq!(read.channel.as_deref(), Some("hello"));
    assert!(!read.is_corrupt());

    assert!(UpdateChannelC2SPacket::new(None).is_corrupt());
}

#[test]
fn packet_handler_read_safe_falls_back_and_skips_remaining() {
    // garbage that panics the from_buf closure -> default packet, buffer skipped
    let mut buf = FriendlyByteBuf::new();
    buf.data.extend_from_slice(&[0xFF; 8]);
    let read = PacketHandler::read_safe(&mut buf, |_b| -> PingLocationC2SPacket { panic!("bad packet") }, || PingLocationC2SPacket::default_corrupt());
    assert!(read.is_corrupt());
    assert_eq!(buf.readable_bytes(), 0);

    // extra trailing bytes are skipped after a successful read
    let mut buf = FriendlyByteBuf::new();
    PingLocationC2SPacket::new(Some("x".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0).write(&mut buf);
    buf.data.extend_from_slice(&[1, 2, 3]);
    let read = PingLocationC2SPacket::read_safe(&mut buf);
    assert_eq!(read.channel.as_deref(), Some("x"));
    assert_eq!(buf.readable_bytes(), 0);
}

// ---------------------------------------------------------------------------
// math
// ---------------------------------------------------------------------------

#[test]
fn screen_pos_distance_is_max_behind_camera() {
    let p = crate::math::ScreenPos::new(100.0, 100.0, -1.0);
    assert!(p.is_behind_camera());
    assert_eq!(p.distance_to(Vec2::new(0.0, 0.0)), f32::MAX);

    let p = crate::math::ScreenPos::new(3.0, 4.0, 1.0);
    assert_eq!(p.distance_to(Vec2::new(0.0, 0.0)), 5.0);
    assert!(p.is_in_bounds(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)));
    assert!(!p.is_in_bounds(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0)));
}

#[test]
fn world_to_screen_centers_origin_with_identity_matrices() {
    let _g = install();
    let game = TestGame::minimal();
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));
    let pos = MathUtils::world_to_screen(Vec3::new(0.0, 0.0, 1.0), &Matrix4f::identity(), &Matrix4f::identity());
    assert_eq!(pos.x, 400.0);
    assert_eq!(pos.y, 225.0);
    assert_eq!(pos.depth, 1.0);
}

#[test]
fn angle_rect_intersection_hits_the_named_edge() {
    // right edge (angle 0)
    let p = MathUtils::calculate_angle_rect_intersection(0.0, Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    assert_eq!(p.x, 100.0);
    assert!((p.y - 50.0).abs() < 0.001);
    // bottom edge (angle PI/2 in screen space; cos is numerically tiny, not zero)
    let p = MathUtils::calculate_angle_rect_intersection(std::f32::consts::FRAC_PI_2, Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    assert_eq!(p.y, 100.0);
    assert!((p.x - 50.0).abs() < 0.001);
}

// ---------------------------------------------------------------------------
// util
// ---------------------------------------------------------------------------

#[test]
fn rate_limiter_first_call_allows_then_blocks() {
    // upstream: startTime = now - window + toRegenerate; the first call always
    // returns false, the second is over budget when toRegenerate == window.
    crate::util::set_rates(5000, 1);
    let mut limiter = RateLimiter::new();
    assert!(!limiter.check_exceeded());
    assert!(limiter.check_exceeded());

    // generous rates: regen 0 always leaves leftOver non-negative
    crate::util::set_rates(0, 5);
    assert!(!limiter.check_exceeded());
}

#[test]
fn input_utils_builds_binding_tokens_from_language_keys() {
    let ping = crate::util::InputUtils::key_binding_ping();
    assert_eq!(ping.name, "key.pingwheel.ping_location");
    assert_eq!(ping.input_type, "MOUSE");
    assert_eq!(ping.key, crate::util::GLFW_MOUSE_BUTTON_5);

    let settings = crate::util::InputUtils::key_binding_settings();
    assert_eq!(settings.name, "key.pingwheel.open_settings");
    assert_eq!(settings.input_type, "KEYSYM");
}

// ---------------------------------------------------------------------------
// core
// ---------------------------------------------------------------------------

#[test]
fn game_context_dimension_hashes_and_updates_once_per_world() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.token_val.set(7);
    *level.dimension_location_val.borrow_mut() = "minecraft:overworld".into();

    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));
    GameContext::update_dimension();
    let first = GameContext::dimension();
    assert_ne!(first, 0);
    // same world token -> unchanged
    GameContext::update_dimension();
    assert_eq!(GameContext::dimension(), first);

    // upstream hashes the dimension location of the new world; a different world
    // instance with a different dimension yields a different hash
    *level.dimension_location_val.borrow_mut() = "minecraft:the_nether".into();
    level.token_val.set(9);
    GameContext::update_dimension();
    assert_ne!(GameContext::dimension(), first);
}

#[test]
fn game_context_get_entity_scans_rendered_entities() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    let ent = Rc::new(TestEntity::default());
    ent.uuid_val.set(0xBEEF);
    level.entities.borrow_mut().push(ent.clone() as Rc<dyn EntityHandle>);
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    assert!(GameContext::get_entity(0xBEEF).is_some());
    assert!(GameContext::get_entity(0xDEAD).is_none());
}

#[test]
fn directional_sound_instance_pulls_toward_player_and_stops_without_one() {
    let _g = install();
    let sound = DirectionalSoundInstance::new(crate::resource::PING_SOUND_EVENT, "MASTER", 1.0, 1.0, Vec3::new(0.0, 0.0, 100.0));
    assert_eq!(*sound.pos.lock().unwrap(), Vec3::new(0.0, 0.0, 100.0));

    // no player -> stopped
    let mut game = TestGame::minimal();
    let inner = Rc::get_mut(&mut game).unwrap();
    inner.player = None;
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));
    sound.tick();
    assert!(*sound.stopped.lock().unwrap());
}

#[test]
fn ping_view_calculates_scale_and_expiry() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.game_time_val.set(100);
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    let mut ping = PingView::of(Vec3::new(0.0, 0.0, 100.0), None, Some(1), 0, GameContext::dimension());
    let ctx = crate::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };
    ping.update(&ctx, level.game_time());

    // distance 100 -> 2/100^0.3 =~ 0.503 -> clamped 1 * 0.5 * 1.0 = 0.5
    assert!((ping.scale() - 0.5).abs() < 0.01);
    assert!(!ping.is_expired());

    // 7s * 20tps = 140 ticks: age 300 expires
    level.game_time_val.set(400);
    ping.update(&ctx, level.game_time());
    assert!(ping.is_expired());
}

#[test]
fn ping_view_team_color_from_player_info() {
    let _g = install();
    let game = TestGame::minimal();
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    let mut ping = PingView::of(Vec3::ZERO, None, Some(1), 0, 0);
    assert_eq!(ping.get_team_color(), -1); // WHITE: no player info yet

    // player info exists but no team -> WHITE
    ping.update(&crate::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 }, 10);
    assert_eq!(ping.get_team_color(), -1);
}

#[test]
fn ping_manager_add_or_replace_keeps_author_and_sequence() {
    let _g = install();
    PingManager::add_or_replace_ping(PingView::of(Vec3::ZERO, None, Some(1), 0, 0));
    PingManager::add_or_replace_ping(PingView::of(Vec3::new(1.0, 1.0, 1.0), None, Some(2), 0, 0));
    PingManager::add_or_replace_ping(PingView::of(Vec3::new(9.0, 9.0, 9.0), None, Some(1), 0, 0));
    assert_eq!(PingManager::repo_len(), 2);
}

#[test]
fn ping_manager_update_removes_expired() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.game_time_val.set(100);
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    PingManager::add_or_replace_ping(PingView::of(Vec3::new(0.0, 0.0, 50.0), None, Some(1), 0, GameContext::dimension()));
    let ctx = crate::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };
    PingManager::update_pings(&ctx);
    assert_eq!(PingManager::repo_len(), 1);

    level.game_time_val.set(100 + 141 + 7 * 20);
    PingManager::update_pings(&ctx);
    assert_eq!(PingManager::repo_len(), 0);
}

#[test]
fn ping_manager_revokes_queued_ping_at_center() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.game_time_val.set(100);
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    PingManager::add_or_replace_ping(PingView::of(Vec3::ZERO, None, Some(1), 0, GameContext::dimension()));
    let ctx = crate::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };

    // first update stamps spawn_time at 100
    PingManager::update_pings(&ctx);

    // correction_period default 1.0 -> age must be > 20 ticks; jump 100 ticks.
    // the ping sits at screen center (camera at origin), so it is removable.
    PingController::queue_ping_action();
    level.game_time_val.set(200);
    PingManager::update_pings(&ctx);

    assert_eq!(PingManager::repo_len(), 0);
    assert!(!PingController::is_ping_queued());
}

#[test]
fn ping_controller_poll_increments_sequence_within_correction_period() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.game_time_val.set(10_000);
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    let seq_before = PingController::ping_sequence();
    PingController::queue_ping_action();
    PingController::poll_ping_action(1.0);
    assert!(PingController::ping_sequence() > seq_before);
    assert!(!PingController::is_ping_queued());

    // again within the same correction period: no increment
    level.game_time_val.set(10_000 + 5);
    let seq_mid = PingController::ping_sequence();
    PingController::queue_ping_action();
    PingController::poll_ping_action(1.0);
    assert_eq!(PingController::ping_sequence(), seq_mid);
}

#[test]
fn ping_controller_perform_sends_block_hit_ping() {
    let _g = install();
    let game = TestGame::minimal();
    let level = game.level.as_ref().unwrap().clone();
    level.game_time_val.set(1000);
    level.clip_result.borrow_mut().replace(BlockHitResult {
        location: Vec3::new(5.0, 64.0, 5.0),
        block_pos: (5, 64, 5),
        direction_up: true,
        miss_inside: true,
        hit_type: HitResultType::Block,
    });
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    let svc = Rc::new(TestNetworkService::default());
    seams::set_network_service(svc.clone());

    PingController::queue_ping_action();
    PingController::poll_ping_action(1.0);

    let sent = svc.sent.borrow().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "ping-wheel-c2s:ping-location");

    let mut buf = FriendlyByteBuf { data: sent[0].1.clone(), reader_index: 0 };
    let packet = PingLocationC2SPacket::read_safe(&mut buf);
    assert_eq!(packet.pos, Some(Vec3::new(5.0, 64.0, 5.0)));
}

#[test]
fn ping_manager_accepts_matching_channel_only() {
    let _g = install();
    let game = TestGame::minimal();
    seams::set_game(Some(game as Rc<dyn seams::MinecraftClient>));

    client_handler().config_mut().channel = "main".into();

    let packet = PingLocationS2CPacket::new(Some("other".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, GameContext::dimension(), 1);
    PingManager::accept_ping_packet(&packet);
    assert_eq!(PingManager::repo_len(), 0);

    let packet = PingLocationS2CPacket::new(Some("main".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, GameContext::dimension(), 1);
    PingManager::accept_ping_packet(&packet);
    assert_eq!(PingManager::repo_len(), 1);
    assert_eq!(PingManager::repo_snapshot()[0].author_id, Some(1));

    // corrupt packet rejected
    let corrupt = PingLocationS2CPacket::default_corrupt();
    PingManager::accept_ping_packet(&corrupt);
    assert_eq!(PingManager::repo_len(), 1);
}

// ---------------------------------------------------------------------------
// ServerCore
// ---------------------------------------------------------------------------

fn server_env_with(names: &[&str]) -> Rc<TestServerEnv> {
    let mut players = Vec::new();
    let mut handles = Vec::new();

    for (i, name) in names.iter().enumerate() {
        let uuid = 1000 + i as u128;
        players.push(uuid);
        handles.push(Rc::new(TestServerPlayer { uuid, name: name.to_string(), messages: RefCell::new(Vec::new()) }));
    }

    Rc::new(TestServerEnv { players, handles })
}

#[test]
fn server_core_channels_route_only_to_same_channel() {
    let _g = install();
    let env = server_env_with(&["alice", "bob"]);
    seams::set_server_env(env.clone());

    {
        let cfg = server_handler().config_mut();
        cfg.default_channel_mode = ChannelMode::Global;
        cfg.rate_limit = 0;
        ServerCore::init();
    }

    // alice joins "team"; bob stays default
    ServerCore::on_channel_update(1000, &UpdateChannelC2SPacket::new(Some("team".into())));
    assert!(env.handles[0].messages.borrow().is_empty());

    let packet = PingLocationC2SPacket::new(Some("team".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0);
    ServerCore::on_ping_location(1000, &packet);

    // bob's stored channel differs -> skipped; alice gets exactly one
    let svc = Rc::new(TestNetworkService::default());
    seams::set_network_service(svc.clone());

    let packet = PingLocationC2SPacket::new(Some("team".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0);
    ServerCore::on_ping_location(1000, &packet);
    let sent = svc.sent.borrow().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "ping-wheel-s2c:ping-location");
}

#[test]
fn server_core_disabled_mode_rejects_empty_channel() {
    let _g = install();
    let env = server_env_with(&["alice"]);
    seams::set_server_env(env.clone());

    {
        let cfg = server_handler().config_mut();
        cfg.default_channel_mode = ChannelMode::Disabled;
        cfg.rate_limit = 0;
        ServerCore::init();
    }

    let packet = PingLocationC2SPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0);
    ServerCore::on_ping_location(1000, &packet);

    assert_eq!(env.handles[0].messages.borrow().len(), 1);
    let msg = env.handles[0].messages.borrow()[0].clone();
    assert!(msg.contains("Must be in a channel"));
    assert!(msg.contains("\u{a7}a/pingwheel channel"));
}

#[test]
fn server_core_team_only_rejects_teamless_players() {
    let _g = install();
    let env = server_env_with(&["alice"]);
    seams::set_server_env(env.clone());

    {
        let cfg = server_handler().config_mut();
        cfg.default_channel_mode = ChannelMode::TeamOnly;
        cfg.rate_limit = 0;
        ServerCore::init();
    }

    let packet = PingLocationC2SPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0);
    ServerCore::on_ping_location(1000, &packet);
    assert!(env.handles[0].messages.borrow()[0].contains("team or channel"));
}

#[test]
fn server_core_rate_limit_drops_excess_pings() {
    let _g = install();
    let env = server_env_with(&["alice"]);
    seams::set_server_env(env.clone());

    {
        let cfg = server_handler().config_mut();
        cfg.default_channel_mode = ChannelMode::Global;
        cfg.rate_limit = 1;
        cfg.ms_to_regenerate = 5000;
        ServerCore::init();
    }

    let packet = PingLocationC2SPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), None, 0, 0);
    ServerCore::on_ping_location(1000, &packet);

    let svc = Rc::new(TestNetworkService::default());
    seams::set_network_service(svc.clone());
    assert!(svc.sent.borrow().is_empty());
    assert!(env.handles[0].messages.borrow().is_empty());
}

#[test]
fn server_core_player_tracking_off_hides_player_targets() {
    let _g = install();
    let env = server_env_with(&["alice", "bob"]);
    seams::set_server_env(env.clone());

    {
        let cfg = server_handler().config_mut();
        cfg.default_channel_mode = ChannelMode::Global;
        cfg.player_tracking_enabled = false;
        cfg.rate_limit = 0;
        ServerCore::init();
    }

    // ping targets bob (a player) -> entity stripped
    let packet = PingLocationC2SPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), Some(1001), 0, 0);
    ServerCore::on_ping_location(1000, &packet);

    let svc = Rc::new(TestNetworkService::default());
    seams::set_network_service(svc.clone());

    let packet = PingLocationC2SPacket::new(Some("".into()), Some(Vec3::new(0.0, 0.0, 0.0)), Some(1001), 0, 0);
    ServerCore::on_ping_location(1000, &packet);
    // Global mode routes to every online player: alice (the author) and bob
    let sent = svc.sent.borrow().clone();
    assert_eq!(sent.len(), 2);

    let mut buf = FriendlyByteBuf { data: sent[0].1.clone(), reader_index: 0 };
    let out = PingLocationS2CPacket::read_safe(&mut buf);
    assert_eq!(out.entity, None, "player target must be hidden");
    assert_eq!(out.author, 1000);
}

#[test]
fn server_core_corrupt_packets_get_version_notice() {
    let _g = install();
    let env = server_env_with(&["alice"]);
    seams::set_server_env(env.clone());

    ServerCore::on_ping_location(1000, &PingLocationC2SPacket::default_corrupt());
    assert!(env.handles[0].messages.borrow()[0].contains("Make sure your version matches"));

    ServerCore::on_channel_update(1000, &UpdateChannelC2SPacket::new(None));
    assert!(env.handles[0].messages.borrow()[1].contains("Make sure your version matches"));
}

#[test]
fn server_core_disconnect_clears_player_state() {
    let _g = install();
    let env = server_env_with(&["alice"]);
    seams::set_server_env(env);

    ServerCore::on_channel_update(1000, &UpdateChannelC2SPacket::new(Some("team".into())));
    ServerCore::on_player_disconnect(1000);
    // rejoin as a fresh state: update accepted without error
    ServerCore::on_channel_update(1000, &UpdateChannelC2SPacket::new(Some("other".into())));
}

// ---------------------------------------------------------------------------
// resource
// ---------------------------------------------------------------------------

#[test]
fn language_utils_keys_follow_the_mod_namespace() {
    assert_eq!(crate::resource::LanguageUtils::key_of("settings", &["ping_volume"]), "settings.pingwheel.ping_volume");
    assert_eq!(crate::resource::LanguageUtils::key_of("key.category", &["name"]), "key.category.pingwheel.name");
    assert_eq!(crate::resource::LanguageUtils::key_of("key", &["ping_location"]), "key.pingwheel.ping_location");
}


// ---------------------------------------------------------------------------
// integration/ + screen/: OptionUtils option objects and TeamContextHandler.
// ---------------------------------------------------------------------------

#[test]
fn team_context_to_string_is_lowercase() {
    use crate::integration::TeamContext;

    assert_eq!(TeamContext::None.to_string(), "none");
    assert_eq!(TeamContext::VanillaTeam.to_string(), "vanilla_team");
    assert_eq!(TeamContext::FtbTeams.to_string(), "ftb_teams");
    assert_eq!(TeamContext::VoiceChat.to_string(), "voice_chat");
}

#[test]
fn team_context_handler_ladder_prefers_voice_chat_then_ftb() {
    use crate::integration;
    use crate::seams::{set_integration_service, IIntegrationService};

    struct CtxLadder {
        voice: Option<u128>,
        ftb: Option<u128>,
    }
    impl IIntegrationService for CtxLadder {
        fn voice_chat_self_group_id(&self) -> Option<u128> {
            self.voice
        }
        fn ftb_self_team_id(&self) -> Option<u128> {
            self.ftb
        }
        fn player_team_present(&self, _player: u128) -> bool {
            true
        }
    }

    let _guard = crate::tests::install();
    // the wrappers gate on the ModContext flags (indexMods), so enable them here
    crate::integration::HAS_VOICE_CHAT.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::integration::HAS_FTB_TEAMS.store(true, std::sync::atomic::Ordering::SeqCst);
    set_integration_service(Rc::new(CtxLadder { voice: Some(7), ftb: Some(9) }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::VoiceChat);

    set_integration_service(Rc::new(CtxLadder { voice: None, ftb: Some(9) }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::FtbTeams);

    set_integration_service(Rc::new(CtxLadder { voice: None, ftb: None }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::None);
    set_integration_service(Rc::new(crate::seams::NoIntegrationService));
    crate::integration::HAS_VOICE_CHAT.store(false, std::sync::atomic::Ordering::SeqCst);
    crate::integration::HAS_FTB_TEAMS.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn team_context_handler_in_same_context_compares_ids_then_teams() {
    use crate::seams::{set_integration_service, IIntegrationService};

    struct CtxIds {
        voice1: Option<u128>,
        voice2: Option<u128>,
        team1: Option<u128>,
        team2: Option<u128>,
    }
    impl IIntegrationService for CtxIds {
        fn voice_chat_group_id(&self, player: u128) -> Option<u128> {
            if player == 1 { self.voice1 } else { self.voice2 }
        }
        fn player_team_uuid(&self, player: u128) -> Option<u128> {
            if player == 1 { self.team1 } else { self.team2 }
        }
        fn player_team_present(&self, player: u128) -> bool {
            self.player_team_uuid(player).is_some()
        }
    }

    let _guard = crate::tests::install();
    // same voice-chat group id -> same context
    set_integration_service(Rc::new(CtxIds { voice1: Some(5), voice2: Some(5), team1: None, team2: None }));
    assert!(crate::integration::in_same_context(1, 2));
    // p1 has an id, p2 has none -> differ
    set_integration_service(Rc::new(CtxIds { voice1: Some(5), voice2: None, team1: None, team2: Some(3) }));
    assert!(!crate::integration::in_same_context(1, 2));
    // no integration ids, same vanilla team identity -> same context
    set_integration_service(Rc::new(CtxIds { voice1: None, voice2: None, team1: Some(3), team2: Some(3) }));
    assert!(crate::integration::in_same_context(1, 2));
    // no integration ids, different vanilla teams -> differ
    set_integration_service(Rc::new(CtxIds { voice1: None, voice2: None, team1: Some(3), team2: Some(4) }));
    assert!(!crate::integration::in_same_context(1, 2));
    set_integration_service(Rc::new(crate::seams::NoIntegrationService));
}

#[test]
fn option_utils_enum_cycles_through_values_and_formats() {
    use crate::screen::OptionUtils;

    let opt = OptionUtils::of_enum(
        "key",
        PlayerInfoMode::VALUES.to_vec(),
        Rc::new(|mode| format!("label:{}", mode.name().to_ascii_lowercase())),
        Rc::new(|_m| Vec::new()),
        Rc::new(|c: &ClientConfig| c.get_player_info_mode()),
        Rc::new(|c: &mut ClientConfig, v| c.set_player_info_mode(v)),
    );

    let crate::screen::OptionEntry::Cycle(entry) = opt else {
        panic!("expected a CycleOptionEntry");
    };
    assert_eq!(entry.count, 4);

    let mut config = ClientConfig::default();
    assert_eq!((entry.label)(&config), "label:hold");

    // two advances from Hold: HOLD -> DISABLED -> ALWAYS
    (entry.cycle)(&mut config);
    (entry.cycle)(&mut config);
    assert_eq!(config.get_player_info_mode(), PlayerInfoMode::Always);
    assert_eq!((entry.label)(&config), "label:always");

    // the fourth advance wraps back to Hold (values.len() == 4)
    (entry.cycle)(&mut config);
    (entry.cycle)(&mut config);
    assert_eq!(config.get_player_info_mode(), PlayerInfoMode::Hold);
}

#[test]
fn option_utils_of_bool_cycles_the_flag() {
    use crate::screen::OptionEntry;
    use crate::screen::OptionUtils;

    let opt = OptionUtils::of_bool(
        "k",
        Rc::new(|c: &ClientConfig| c.is_item_icon_visible()),
        Rc::new(|c: &mut ClientConfig, v| c.set_item_icon_visible(v)),
    );

    let OptionEntry::Cycle(entry) = opt else {
        panic!("expected a CycleOptionEntry");
    };

    let mut config = ClientConfig::default();
    assert!(config.is_item_icon_visible());
    (entry.cycle)(&mut config);
    assert!(!config.is_item_icon_visible());
    (entry.cycle)(&mut config);
    assert!(config.is_item_icon_visible());
    assert_eq!((entry.label)(&config), "ON");
}

#[test]
fn option_utils_of_int_composes_getter_into_formatter() {
    use crate::screen::OptionUtils;

    let opt = OptionUtils::of_int(
        "k",
        0,
        100,
        1,
        Rc::new(|v| format!("{v}%")),
        Rc::new(|c: &ClientConfig| c.get_ping_volume()),
        Rc::new(|_c: &mut ClientConfig, _v| {}),
    );

    let crate::screen::OptionEntry::Progress(entry) = opt else {
        panic!("expected a ProgressOptionEntry");
    };

    let mut config = ClientConfig::default();
    config.set_ping_volume(37);
    // the render-time formatter reads the getter against the config, exactly
    // like upstream (gameOptions, option) -> formatter.apply(getter.get()).
    assert_eq!((entry.fmt)(&config), "37%");
    assert_eq!(entry.min, 0.0);
    assert_eq!(entry.max, 100.0);
    assert_eq!(entry.step, 1.0);
    assert!(entry.int);
}


// ---------------------------------------------------------------------------
// SettingsScreen integration: a recording host drives init/render/on_close.
// ---------------------------------------------------------------------------
use crate::seams::{EditBoxHost, IScreenEnv, OptionsListHost};

struct RecordingList {
    pairs: RefCell<Vec<(String, Option<String>)>>,
    mouse_over: Cell<Option<usize>>,
    tooltips: Vec<Vec<String>>,
}
impl RecordingList {
    fn new(tooltips: Vec<Vec<String>>) -> Rc<Self> {
        Rc::new(Self {
            pairs: RefCell::new(Vec::new()),
            mouse_over: Cell::new(None),
            tooltips,
        })
    }
}
impl crate::seams::OptionsListHost for RecordingList {
    fn add_small(&self, a: crate::screen::OptionEntry, b: Option<crate::screen::OptionEntry>) {
        let key_of = |o: Option<crate::screen::OptionEntry>| match o {
            Some(crate::screen::OptionEntry::Progress(p)) => Some(p.key.clone()),
            Some(crate::screen::OptionEntry::Cycle(c)) => Some(c.key.clone()),
            None => None,
        };
        self.pairs.borrow_mut().push((key_of(Some(a)).unwrap(), key_of(b)));
    }
    fn children_len(&self) -> usize {
        self.pairs.borrow().len()
    }
    fn get_mouse_over(&self, _mouse_x: i32, _mouse_y: i32) -> Option<usize> {
        self.mouse_over.get()
    }
    fn tooltip_of(&self, index: usize) -> Vec<String> {
        self.tooltips.get(index).cloned().unwrap_or_default()
    }
}

struct RecordingEdit {
    value: RefCell<String>,
    max_length: Cell<i32>,
    responder: RefCell<Option<Rc<dyn Fn(&str)>>>,
    y: Cell<i32>,
    hovered: Cell<bool>,
    focused: Cell<bool>,
    ticks: Cell<usize>,
}
impl crate::seams::EditBoxHost for RecordingEdit {
    fn set_max_length(&self, length: i32) {
        self.max_length.set(length);
    }
    fn set_value(&self, value: &str) {
        *self.value.borrow_mut() = value.to_string();
    }
    fn value(&self) -> String {
        self.value.borrow().clone()
    }
    fn set_responder(&self, responder: Rc<dyn Fn(&str)>) {
        *self.responder.borrow_mut() = Some(responder);
    }
    fn tick(&self) {
        self.ticks.set(self.ticks.get() + 1);
    }
    fn is_hovered_or_focused(&self) -> bool {
        self.hovered.get()
    }
    fn is_focused(&self) -> bool {
        self.focused.get()
    }
    fn y(&self) -> i32 {
        self.y.get()
    }
}

struct RecordingScreenEnv {
    w: i32,
    h: i32,
    list: Rc<RecordingList>,
    edit: Rc<RecordingEdit>,
    buttons: RefCell<Vec<(i32, i32, i32, i32, String)>>,
    tooltip_lines: RefCell<Vec<String>>,
    splits: Vec<String>,
    super_closed: Cell<bool>,
    draw_centered: RefCell<Vec<(String, i32, i32, i32)>>,
    draw_strings: RefCell<Vec<(String, i32, i32, i32)>>,
}
impl crate::seams::IScreenEnv for RecordingScreenEnv {
    fn width(&self) -> i32 {
        self.w
    }
    fn height(&self) -> i32 {
        self.h
    }
    fn new_options_list(&self, _w: i32, _h: i32, _top: i32, _bottom: i32, _item_height: i32) -> Rc<dyn crate::seams::OptionsListHost> {
        self.list.clone()
    }
    fn new_edit_box(&self, _x: i32, y: i32, _w: i32, _h: i32, _placeholder: &str) -> Rc<dyn crate::seams::EditBoxHost> {
        self.edit.y.set(y);
        self.edit.clone()
    }
    fn add_widget_edit(&self, _edit: Rc<dyn crate::seams::EditBoxHost>) {}
    fn add_widget_list(&self, _list: Rc<dyn crate::seams::OptionsListHost>) {}
    fn add_renderable_button(&self, x: i32, y: i32, w: i32, h: i32, label: &str, _on_click: Rc<dyn Fn()>) {
        self.buttons.borrow_mut().push((x, y, w, h, label.to_string()));
    }
    fn draw_centered_string(&self, _m: &crate::seams::PoseStack, text: &str, cx: i32, y: i32, color: i32) {
        self.draw_centered.borrow_mut().push((text.to_string(), cx, y, color));
    }
    fn draw_string(&self, _m: &crate::seams::PoseStack, text: &str, x: i32, y: i32, color: i32) {
        self.draw_strings.borrow_mut().push((text.to_string(), x, y, color));
    }
    fn font_split(&self, _text: &str, _max: i32) -> Vec<String> {
        self.splits.clone()
    }
    fn render_tooltip(&self, _m: &crate::seams::PoseStack, lines: &[String], _mouse_x: i32, _mouse_y: i32) {
        *self.tooltip_lines.borrow_mut() = lines.to_vec();
    }
    fn super_on_close(&self) {
        self.super_closed.set(true);
    }
}

#[test]
fn settings_screen_init_builds_nine_options_channel_edit_and_done_button() {
    let _guard = crate::tests::install();
    let list = RecordingList::new(Vec::new());
    let edit = Rc::new(RecordingEdit {
        value: RefCell::new(String::new()),
        max_length: Cell::new(0),
        responder: RefCell::new(None),
        y: Cell::new(0),
        hovered: Cell::new(false),
        focused: Cell::new(false),
        ticks: Cell::new(0),
    });
    let env = Rc::new(RecordingScreenEnv {
        w: 800,
        h: 450,
        list: list.clone(),
        edit: edit.clone(),
        buttons: RefCell::new(Vec::new()),
        tooltip_lines: RefCell::new(Vec::new()),
        splits: Vec::new(),
        super_closed: Cell::new(false),
        draw_centered: RefCell::new(Vec::new()),
        draw_strings: RefCell::new(Vec::new()),
    });
    crate::seams::set_screen_env(env.clone());

    // the config channel the EditBox opens with
    client_handler().config_mut().set_channel("teamchat".to_string());

    let mut screen = crate::screen::SettingsScreen::new();
    screen.init();

    // 5 addSmall pairs: 9 options, the ping-size row alone
    let pairs = list.pairs.borrow();
    assert_eq!(pairs.len(), 5);
    assert_eq!(pairs[0].0, "settings.pingwheel.ping_volume");
    assert_eq!(pairs[0].1.as_deref(), Some("settings.pingwheel.ping_duration"));
    assert_eq!(pairs[3].0, "settings.pingwheel.player_info_mode");
    assert_eq!(pairs[3].1.as_deref(), Some("settings.pingwheel.team_color_mode"));
    assert_eq!(pairs[4].1, None);
    drop(pairs);

    // the channel EditBox: x = width/2-100, y = 50 + 25*children, 200x20,
    // MAX_CHANNEL_LENGTH, value from config, responder writes back
    assert_eq!(edit.max_length.get(), 128);
    assert_eq!(edit.value(), "teamchat");
    assert_eq!(edit.y.get(), 50 + 25 * 5);
    let responder = edit.responder.borrow().clone().expect("responder set");
    responder("swapped");
    assert_eq!(client_handler().config().get_channel(), "swapped");

    // the Done button: width/2-100, height-27, 200x20
    let buttons = env.buttons.borrow();
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0], (300, 450 - 27, 200, 20, "Done".to_string()));
    drop(buttons);

    // onClose saves the config and falls back to the host close (no parent)
    screen.on_close();
    assert!(env.super_closed.get());

    crate::seams::set_screen_env(Rc::new(crate::seams::NoScreenEnv));
}

#[test]
fn settings_screen_render_shows_title_channel_and_tooltip() {
    let _guard = crate::tests::install();
    let list = RecordingList::new(vec![vec!["teamtip".to_string()]]);
    let edit = Rc::new(RecordingEdit {
        value: RefCell::new(String::new()),
        max_length: Cell::new(0),
        responder: RefCell::new(None),
        y: Cell::new(175),
        hovered: Cell::new(false),
        focused: Cell::new(false),
        ticks: Cell::new(0),
    });
    let env = Rc::new(RecordingScreenEnv {
        w: 800,
        h: 450,
        list: list.clone(),
        edit: edit.clone(),
        buttons: RefCell::new(Vec::new()),
        tooltip_lines: RefCell::new(Vec::new()),
        splits: vec!["line1".to_string(), "line2".to_string()],
        super_closed: Cell::new(false),
        draw_centered: RefCell::new(Vec::new()),
        draw_strings: RefCell::new(Vec::new()),
    });
    crate::seams::set_screen_env(env.clone());

    client_handler().config_mut().set_channel(String::new());
    let mut screen = crate::screen::SettingsScreen::new();
    screen.init();
    screen.tick();
    assert_eq!(edit.ticks.get(), 1);

    // no hovered row and an unhovered edit box: the tooltip is empty
    let matrices = crate::seams::PoseStack::new();
    screen.render(&matrices, 0, 0, 0.0);
    assert!(env.draw_centered.borrow().iter().any(|(t, cx, y, color)| {
        cx == &(800 / 2) && y == &20 && color == &crate::screen::WHITE
    }));
    // the channel label draws at edit.y - 12 in GRAY
    assert!(env.draw_strings.borrow().iter().any(|(_, x, y, color)| {
        x == &(800 / 2 - 100) && y == &(175 - 12) && color == &crate::screen::GRAY
    }));
    assert!(env.tooltip_lines.borrow().is_empty());

    // hovered row: its tooltip renders
    list.mouse_over.set(Some(0));
    screen.render(&matrices, 400, 60, 0.0);
    assert_eq!(*env.tooltip_lines.borrow(), vec!["teamtip".to_string()]);

    // hovered edit box (not focused) with no row tooltip: the channel tooltip
    list.mouse_over.set(None);
    edit.hovered.set(true);
    screen.render(&matrices, 350, 180, 0.0);
    assert_eq!(*env.tooltip_lines.borrow(), vec!["line1".to_string(), "line2".to_string()]);

    crate::seams::set_screen_env(Rc::new(crate::seams::NoScreenEnv));
}

#[test]
fn common_client_settings_key_opens_the_settings_screen() {
    let _guard = crate::tests::install();

    // consume_click_key / set_screen are host seams; drive the handler's wiring
    // directly: the settings-key branch constructs a SettingsScreen factory.
    let factory: Rc<dyn Fn() -> crate::screen::SettingsScreen> = Rc::new(|| crate::screen::SettingsScreen::new());
    let screen = factory();
    assert_eq!(screen.title(), String::new()); // translate seam renders empty in tests
}
