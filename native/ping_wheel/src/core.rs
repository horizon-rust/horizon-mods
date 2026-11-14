//! Upstream: common/core/ (1:1). PingData is an abstract base with the single concrete
//! subclass PingView; the port merges them into one struct and notes the merge here.
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::config::{client_handler, ClientConfig, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION, TPS};
use crate::config::ChannelMode;
use crate::global;
use crate::math::{HitResult, Raycast, ScreenPos};
use crate::network::{IPacket, PingLocationC2SPacket, PingLocationS2CPacket};
use crate::resource::{PING_SOUND_EVENT, ResourceLocationToken};
use crate::seams::{
    self, EntityHandle, HitResultType, Matrix4f, PlayerInfoHandle, ItemStackToken, Vec2, Vec3,
};

// ---------------------------------------------------------------------------
// render/WorldRenderContext.java (translated here to keep the core self-contained)
// ---------------------------------------------------------------------------

pub struct WorldRenderContext {
    pub model_view_matrix: Matrix4f,
    pub projection_matrix: Matrix4f,
    pub tick_delta: f32,
    /// Camera token (host side holds the real camera).
    pub camera: u64,
}

// ---------------------------------------------------------------------------
// core/GameContext.java
// ---------------------------------------------------------------------------

static DIMENSION: AtomicI32 = AtomicI32::new(0);
static LAST_WORLD: Mutex<u64> = Mutex::new(0);

pub struct GameContext;

impl GameContext {
    pub fn dimension() -> i32 {
        DIMENSION.load(Ordering::SeqCst)
    }

    pub fn update_dimension() {
        let Some(level) = seams::game().and_then(|g| g.level()) else {
            return;
        };

        let token = level.token();
        if *LAST_WORLD.lock().unwrap() == token {
            return;
        }

        *LAST_WORLD.lock().unwrap() = token;
        DIMENSION.store(seams::java_string_hash(&level.dimension_location()), Ordering::SeqCst);
    }

    pub fn get_entity(uuid: u128) -> Option<std::rc::Rc<dyn EntityHandle>> {
        let level = seams::game()?.level()?;

        for entity in level.entities_for_rendering() {
            if entity.uuid() == uuid {
                return Some(entity);
            }
        }

        None
    }

    /// getCurrentServerIp(): Game null or getCurrentServer() null -> empty.
    pub fn current_server_ip() -> Option<String> {
        seams::game().and_then(|g| g.current_server_ip())
    }
}

// ---------------------------------------------------------------------------
// util/DirectionalSoundInstance.java (AbstractTickableSoundInstance subclass)
// ---------------------------------------------------------------------------

pub struct DirectionalSoundInstance {
    pub sound: ResourceLocationToken,
    /// SoundSource.MASTER
    pub category: &'static str,
    pub volume: f32,
    pub pitch: f32,
    pub pos: Mutex<Vec3>,
    /// this.isStopped, set by tick() when there is no player.
    pub stopped: Mutex<bool>,
}

impl DirectionalSoundInstance {
    pub fn new(sound: ResourceLocationToken, category: &'static str, volume: f32, pitch: f32, pos: Vec3) -> Self {
        let inst = Self {
            sound,
            category,
            volume,
            pitch,
            pos: Mutex::new(pos),
            stopped: Mutex::new(false),
        };
        inst.tick();
        inst
    }

    pub fn tick(&self) {
        let Some(game) = seams::game() else {
            *self.stopped.lock().unwrap() = true;
            return;
        };
        let Some(player) = game.player() else {
            *self.stopped.lock().unwrap() = true;
            return;
        };

        let player_pos = player.position(0.0);
        let vec_between = player_pos.vector_to(*self.pos.lock().unwrap());
        let mapped_distance = vec_between.length().min(64.0) / 64.0 * 14.0;
        let sound_direction = vec_between.normalize().scale(mapped_distance);
        let sound_pos = player_pos.add(sound_direction);

        *self.pos.lock().unwrap() = sound_pos;
    }
}

// ---------------------------------------------------------------------------
// core/PingData.java + core/PingView.java
// ---------------------------------------------------------------------------

const WHITE: i32 = -1; // 0xFFFFFFFF

pub struct PingView {
    /// PingData.pos (lombok @Setter)
    pub pos: Vec3,
    pub entity_id: Option<u128>,
    pub author_id: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,

    spawn_time: i64,
    age: i64,
    player_info: Option<std::rc::Rc<dyn PlayerInfoHandle>>,
    item_stack: Option<ItemStackToken>,
    screen_pos: Option<ScreenPos>,
    distance: f64,
    scale: f32,
}

impl PingView {
    fn new(pos: Vec3, entity_id: Option<u128>, author_id: Option<u128>, sequence: i32, dimension: i32) -> Self {
        Self {
            pos,
            entity_id,
            author_id,
            sequence,
            dimension,
            spawn_time: 0,
            age: 0,
            player_info: None,
            item_stack: None,
            screen_pos: None,
            distance: 0.0,
            scale: 0.0,
        }
    }

    pub fn of(pos: Vec3, entity_id: Option<u128>, author_id: Option<u128>, sequence: i32, dimension: i32) -> Self {
        Self::new(pos, entity_id, author_id, sequence, dimension)
    }

    pub fn from(packet: &PingLocationS2CPacket) -> Self {
        Self::of(
            packet.pos.unwrap_or(Vec3::ZERO),
            packet.entity,
            Some(packet.author),
            packet.sequence,
            packet.dimension,
        )
    }

    pub fn update(&mut self, ctx: &WorldRenderContext, game_time: i64) {
        if self.spawn_time == 0 {
            self.spawn_time = game_time;
        }

        self.age = game_time - self.spawn_time;

        let game = seams::game();

        if let (Some(g), Some(conn)) = (game.as_ref(), game.as_ref().and_then(|g| g.connection())) {
            if let Some(author_id) = self.author_id {
                self.player_info = conn.get_player_info(author_id);
            }
        }

        if let Some(entity_id) = self.entity_id {
            if let Some(ent) = GameContext::get_entity(entity_id) {
                if ent.is_type_item() && client_handler().config().item_icon_visible {
                    self.item_stack = ent.item_entity_item_copy();
                }

                self.pos = ent.position(ctx.tick_delta).add(Vec3::new(0.0, ent.bounding_box_ysize(), 0.0));
            }
        }

        self.screen_pos = Some(crate::math::MathUtils::world_to_screen(
            self.pos,
            &ctx.model_view_matrix,
            &ctx.projection_matrix,
        ));
        self.distance = game.as_ref().map(|g| g.camera_position().distance_to(self.pos)).unwrap_or(0.0);
        self.calculate_scale();
    }

    pub fn play_sound_in_dimension(&self) {
        if self.dimension != GameContext::dimension() {
            return;
        }

        let Some(game) = seams::game() else {
            return;
        };

        game.sound_manager_play(&DirectionalSoundInstance::new(
            PING_SOUND_EVENT,
            "MASTER",
            client_handler().config().ping_volume as f32 / 100.0,
            1.0,
            self.pos,
        ));
    }

    pub fn is_expired(&self) -> bool {
        (self.age as i32) > client_handler().config().ping_duration * TPS && client_handler().config().ping_duration < MAX_PING_DURATION
    }

    pub fn is_removable(&self) -> bool {
        (client_handler().config().correction_period >= MAX_CORRECTION_PERIOD || (self.age as i32) as f32 > client_handler().config().correction_period * TPS as f32)
            && self.distance_to_center() < client_handler().config().remove_radius as f32
    }

    pub fn distance_to_center(&self) -> f32 {
        let Some(screen_pos) = self.screen_pos else {
            return 0.0;
        };

        let Some(game) = seams::game() else {
            return 0.0;
        };

        let (w, h) = game.gui_scaled_size();
        let center = Vec2::new(w * 0.5, h * 0.5);
        screen_pos.distance_to(center)
    }

    pub fn is_closer_to_center(&self, b: Option<&PingView>) -> bool {
        let Some(b) = b else {
            return true;
        };

        self.distance_to_center() < b.distance_to_center()
    }

    /// Upstream has a dead second `playerInfo == null` check after the first; the
    /// has_team/color split keeps the same outcomes.
    pub fn get_team_color(&self) -> i32 {
        let Some(info) = self.player_info.as_ref() else {
            return WHITE;
        };
        if !info.has_team() {
            return WHITE;
        }

        match info.team_color() {
            None => WHITE,
            Some(team_color) => (255 << 24) | team_color,
        }
    }

    fn calculate_scale(&mut self) {
        let scale = 2.0 / self.distance.powf(0.3);
        let ping_size = client_handler().config().ping_size as f32 / 100.0;

        self.scale = (1.0f32.max(scale as f32)) * 0.5 * ping_size;
    }

    pub fn player_info(&self) -> &Option<std::rc::Rc<dyn PlayerInfoHandle>> {
        &self.player_info
    }

    pub fn item_stack(&self) -> Option<ItemStackToken> {
        self.item_stack
    }

    pub fn screen_pos(&self) -> Option<ScreenPos> {
        self.screen_pos
    }

    pub fn distance(&self) -> f64 {
        self.distance
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
}

// ---------------------------------------------------------------------------
// core/PingController.java
// ---------------------------------------------------------------------------

static PING_QUEUED: AtomicBool = AtomicBool::new(false);
static PING_SEQUENCE: AtomicI32 = AtomicI32::new(0);
static LAST_PING: AtomicI32 = AtomicI32::new(0);

pub struct PingController;

impl PingController {
    pub fn is_ping_queued() -> bool {
        PING_QUEUED.load(Ordering::SeqCst)
    }

    pub fn ping_sequence() -> i32 {
        PING_SEQUENCE.load(Ordering::SeqCst)
    }

    pub fn revoke_ping_action() {
        PING_QUEUED.store(false, Ordering::SeqCst);
    }

    pub fn queue_ping_action() {
        PING_QUEUED.store(true, Ordering::SeqCst);
    }

    pub fn poll_ping_action(tick_delta: f32) {
        if !Self::is_ping_queued() {
            return;
        }

        let Some(level) = seams::game().and_then(|g| g.level()) else {
            return;
        };

        let time = level.game_time() as i32;
        let correction_period = client_handler().config().correction_period;

        if correction_period < MAX_CORRECTION_PERIOD && time - LAST_PING.load(Ordering::SeqCst) > (correction_period * TPS as f32) as i32 {
            PING_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        }

        LAST_PING.store(time, Ordering::SeqCst);
        PING_QUEUED.store(false, Ordering::SeqCst);
        Self::perform_ping_action(tick_delta);
    }

    fn perform_ping_action(tick_delta: f32) {
        let Some(game) = seams::game() else {
            return;
        };
        let Some(camera_entity) = game.camera_entity() else {
            return;
        };

        let camera_direction = camera_entity.view_vector(tick_delta);
        let hit_result = Raycast::trace_directional(
            camera_direction,
            tick_delta,
            (client_handler().config().raycast_distance.min(client_handler().config().ping_distance)) as f64,
            camera_entity.is_crouching(),
        );

        let hit_result = match hit_result {
            Some(h) if h.get_type() != HitResultType::Miss => h,
            _ => {
                if *seams::HAS_DISTANT_HORIZONS.lock().unwrap() {
                    let channel = client_handler().config().get_channel();
                    let sequence = PING_SEQUENCE.load(Ordering::SeqCst);
                    let dimension = GameContext::dimension();
                    Raycast::trace_distant_async(
                        camera_direction,
                        tick_delta,
                        Box::new(move |distant_hit_result| {
                            seams::network_service().send_to_server(&PingLocationC2SPacket::new(
                                Some(channel.clone()),
                                Some(distant_hit_result.location),
                                None,
                                sequence,
                                dimension,
                            ));
                        }),
                    );
                }

                return;
            }
        };

        let mut uuid = None;

        if let HitResult::Entity(entity_hit) = &hit_result {
            uuid = Some(entity_hit.entity_token as u128);
        }

        seams::network_service().send_to_server(&PingLocationC2SPacket::new(
            Some(client_handler().config().get_channel()),
            Some(hit_result.get_location()),
            uuid,
            PING_SEQUENCE.load(Ordering::SeqCst),
            GameContext::dimension(),
        ));
    }
}

// ---------------------------------------------------------------------------
// core/PingManager.java
// ---------------------------------------------------------------------------

thread_local! {
    static PING_REPO: RefCell<Vec<PingView>> = const { RefCell::new(Vec::new()) };
}

pub struct PingManager;

impl PingManager {
    pub fn clear_pings() {
        PING_REPO.with(|r| r.borrow_mut().clear());
    }

    pub fn repo_len() -> usize {
        PING_REPO.with(|r| r.borrow().len())
    }

    /// Test/view snapshot of the repo in list order.
    pub fn repo_snapshot() -> Vec<PingView> {
        PING_REPO.with(|r| r.borrow().clone())
    }

    pub fn add_or_replace_ping(new_ping: PingView) {
        PING_REPO.with(|r| {
            let mut repo = r.borrow_mut();
            let mut index = -1i32;

            for (i, entry) in repo.iter().enumerate() {
                if entry.author_id == new_ping.author_id && entry.sequence == new_ping.sequence {
                    index = i as i32;
                    break;
                }
            }

            if index != -1 {
                repo[index as usize] = new_ping;
            } else {
                repo.push(new_ping);
            }
        });
    }

    pub fn update_pings(ctx: &WorldRenderContext) {
        let Some(level) = seams::game().and_then(|g| g.level()) else {
            return;
        };

        PING_REPO.with(|r| {
            let mut repo = r.borrow_mut();
            if repo.is_empty() {
                return;
            }

            let time = level.game_time();
            // (index, keep-or-expire, target candidate); the Java iterator remove and the
            // post-loop target removal both keep the survivors in list order.
            let mut expired: Vec<usize> = Vec::new();
            let mut target: Option<usize> = None;
            let mut target_center: Option<f32> = None;

            for (i, ping) in repo.iter_mut().enumerate() {
                ping.update(ctx, time);

                if ping.is_expired() {
                    expired.push(i);
                } else if PingController::is_ping_queued() && ping.is_removable() {
                    let center = ping.distance_to_center();
                    if target_center.map_or(true, |t| center < t) {
                        target_center = Some(center);
                        target = Some(i);
                    }
                }
            }

            // expired entries leave first, then the revoke target is removed by its
            // shifted index (it can never be expired itself).
            let target_shift = target.map(|t| expired.iter().filter(|&&e| e < t).count());
            for i in expired.into_iter().rev() {
                repo.remove(i);
            }

            if let Some(t) = target {
                repo.remove(t - target_shift.unwrap());
                PingController::revoke_ping_action();
            }

            repo.sort_by(|a, b| b.distance.partial_cmp(&a.distance).unwrap_or(std::cmp::Ordering::Equal));
        });
    }

    pub fn accept_ping_packet(packet: &PingLocationS2CPacket) {
        if packet.is_corrupt() {
            global::log_warn("received invalid ping location from server");
            return;
        }

        let Some(game) = seams::game() else {
            return;
        };

        let connection = game.connection();
        let level = game.level();

        if game.player().is_none() || level.is_none() || connection.is_none() {
            return;
        }

        if packet.channel.as_deref() != Some(client_handler().config().get_channel().as_str()) {
            return;
        }

        if client_handler().config().ping_distance < MAX_PING_DISTANCE {
            let pos = packet.pos.unwrap_or(Vec3::ZERO);
            let vec_to_ping = game.player().unwrap().position(0.0).vector_to(pos);

            if vec_to_ping.length() > client_handler().config().ping_distance as f64 {
                return;
            }
        }

        let packet = packet.clone();
        game.execute(std::rc::Rc::new(move || {
            let new_ping = PingView::from(&packet);

            PingManager::add_or_replace_ping(new_ping.clone());
            new_ping.play_sound_in_dimension();
        }));
    }
}

// PingView is cloned only for the play-sound-after-insert step (upstream reuses the same object).
impl Clone for PingView {
    fn clone(&self) -> Self {
        Self {
            pos: self.pos,
            entity_id: self.entity_id,
            author_id: self.author_id,
            sequence: self.sequence,
            dimension: self.dimension,
            spawn_time: self.spawn_time,
            age: self.age,
            player_info: self.player_info.clone(),
            item_stack: self.item_stack,
            screen_pos: self.screen_pos,
            distance: self.distance,
            scale: self.scale,
        }
    }
}

// ---------------------------------------------------------------------------
// core/ServerCore.java
// ---------------------------------------------------------------------------

static PLAYER_CHANNELS: std::sync::LazyLock<Mutex<std::collections::HashMap<u128, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
static PLAYER_RATES: std::sync::LazyLock<Mutex<std::collections::HashMap<u128, crate::util::RateLimiter>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

pub struct ServerCore;

impl ServerCore {
    pub fn init() {
        let cfg = crate::config::server_handler().config();
        crate::util::set_rates(cfg.ms_to_regenerate, cfg.rate_limit);
    }

    pub fn on_player_disconnect(player: u128) {
        PLAYER_CHANNELS.lock().unwrap().remove(&player);
        PLAYER_RATES.lock().unwrap().remove(&player);
    }

    pub fn on_channel_update(player: u128, packet: &crate::network::UpdateChannelC2SPacket) {
        let env = seams::server_env();
        let player_handle = env.player(player);

        if packet.is_corrupt() {
            let name = player_handle.as_ref().map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_warn(&format!("invalid channel update from {} ({})", name, player));
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &crate::compat::Component::literal(&format!(
                        "\u{a7}8[Ping-Wheel] \u{a7}cChannel couldn't be updated\n\u{a7}fMake sure your version matches the server's version: \u{a7}d{}",
                        global::mod_version()
                    )),
                    false,
                );
            }
            return;
        }

        if let Some(channel) = packet.channel.clone() {
            Self::update_player_channel(player, channel);
        }
    }

    pub fn on_ping_location(player: u128, packet: &PingLocationC2SPacket) {
        let env = seams::server_env();
        let player_handle = env.player(player);
        let server_config = crate::config::server_handler().config().clone();

        if packet.is_corrupt() {
            let name = player_handle.as_ref().map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_warn(&format!("invalid ping location from {} ({})", name, player));
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &crate::compat::Component::literal(&format!(
                        "\u{a7}8[Ping-Wheel] \u{a7}cUnable to send ping\n\u{a7}fMake sure your version matches the server's version: \u{a7}d{}",
                        global::mod_version()
                    )),
                    false,
                );
            }
            return;
        }

        PLAYER_RATES
            .lock()
            .unwrap()
            .entry(player)
            .or_insert_with(crate::util::RateLimiter::new);
        let rate_exceeded = {
            let mut rates = PLAYER_RATES.lock().unwrap();
            rates.get_mut(&player).unwrap().check_exceeded()
        };

        if server_config.rate_limit > 0 && rate_exceeded {
            return;
        }

        let channel = packet.channel.clone().unwrap_or_default();
        let default_channel_mode = server_config.default_channel_mode;

        if channel.is_empty() && default_channel_mode == ChannelMode::Disabled {
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &crate::compat::Component::literal(
                        "\u{a7}8[Ping-Wheel] \u{a7}eMust be in a channel to ping location\n\u{a7}fUse \u{a7}a/pingwheel channel\u{a7}f to switch",
                    ),
                    false,
                );
            }
            return;
        }

        if channel.is_empty()
            && default_channel_mode == ChannelMode::TeamOnly
            && !crate::integration::has_team(Some(player))
        {
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &crate::compat::Component::literal(
                        "\u{a7}8[Ping-Wheel] \u{a7}eMust be in a team or channel to ping location\n\u{a7}fUse \u{a7}a/pingwheel channel\u{a7}f to switch",
                    ),
                    false,
                );
            }
            return;
        }

        let stored = PLAYER_CHANNELS.lock().unwrap().get(&player).cloned().unwrap_or_default();
        if channel != stored {
            Self::update_player_channel(player, channel.clone());
        }

        let packet_out;
        let players = env.players();

        if !server_config.player_tracking_enabled && Self::target_entity_is_player(packet, env.as_ref()) {
            packet_out = PingLocationS2CPacket::new(packet.channel.clone(), packet.pos, None, packet.sequence, packet.dimension, player);
        } else {
            packet_out = PingLocationS2CPacket::from_client_packet(packet, player);
        }

        for p in players {
            let stored = PLAYER_CHANNELS.lock().unwrap().get(&p).cloned().unwrap_or_default();
            if channel != stored {
                continue;
            }

            if channel.is_empty() && default_channel_mode != ChannelMode::Global && !crate::integration::in_same_context(player, p) {
                continue;
            }

            seams::network_service().send_to_client(&packet_out, p);
        }
    }

    fn target_entity_is_player(packet: &PingLocationC2SPacket, env: &dyn seams::ServerEnv) -> bool {
        let Some(player_uuid) = packet.entity else {
            return false;
        };

        env.has_player(player_uuid)
    }

    fn update_player_channel(player: u128, channel: String) {
        if channel.is_empty() {
            PLAYER_CHANNELS.lock().unwrap().remove(&player);
            let name = seams::server_env().player(player).map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_info(&format!("Channel update: {} -> default", name));
        } else {
            PLAYER_CHANNELS.lock().unwrap().insert(player, channel.clone());
            let name = seams::server_env().player(player).map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_info(&format!("Channel update: {} -> \"{}\"", name, channel));
        }
    }
}

// Static snapshot mirrors the Java `private static final ClientConfig CLIENT_CONFIG =
// ClientConfig.HANDLER.getConfig()` field: resolved once on first use.
pub fn client_config_snapshot() -> &'static ClientConfig {
    static SNAPSHOT: OnceLock<usize> = OnceLock::new();
    let ptr = *SNAPSHOT.get_or_init(|| client_handler().config() as *const ClientConfig as usize);
    unsafe { &*(ptr as *const ClientConfig) }
}
