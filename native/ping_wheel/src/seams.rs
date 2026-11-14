//! Host seams: every Minecraft / platform type upstream touches directly is translated behind these.
//! The Java classes stay host-side; the Rust side sees tokens and the exact operations upstream performs.
use std::cell::RefCell;
use crate::resource::ResourceLocationToken;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Vector / math types (upstream uses net.minecraft.world.phys.Vec2 / Vec3 and Matrix4f)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Vec3 {
    fn default() -> Self {
        Vec3::ZERO
    }
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    pub fn scale(self, f: f64) -> Vec3 {
        Vec3::new(self.x * f, self.y * f, self.z * f)
    }

    pub fn reverse(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }

    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn distance_to(self, o: Vec3) -> f64 {
        (self - o).length()
    }

    pub fn distance_to_sqr(self, o: Vec3) -> f64 {
        let d = self - o;
        d.x * d.x + d.y * d.y + d.z * d.z
    }

    /// net.minecraft.world.phys.Vec3#vectorTo: normalized vector from self to o.
    pub fn vector_to(self, o: Vec3) -> Vec3 {
        let d = o - self;
        let len = d.length();
        if len == 0.0 {
            Vec3::ZERO
        } else {
            d.scale(1.0 / len)
        }
    }

    pub fn x(self) -> f64 {
        self.x
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

/// Matrix4f stays opaque to the port; the seam provides the one operation upstream uses on it
/// (4x4 transform, row/column order identical to com.mojang.math.Matrix4f#transform).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4f(pub [[f32; 4]; 4]);

impl Matrix4f {
    pub fn identity() -> Self {
        Self([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
}

/// com.mojang.math.Vector4f#transform(Matrix4f) semantics.
pub fn transform4(mat: &Matrix4f, x: f32, y: f32, z: f32, w: f32) -> (f32, f32, f32, f32) {
    let m = &mat.0;
    (
        m[0][0] * x + m[0][1] * y + m[0][2] * z + m[0][3] * w,
        m[1][0] * x + m[1][1] * y + m[1][2] * z + m[1][3] * w,
        m[2][0] * x + m[2][1] * y + m[2][2] * z + m[2][3] * w,
        m[3][0] * x + m[3][1] * y + m[3][2] * z + m[3][3] * w,
    )
}

// ---------------------------------------------------------------------------
// Logging (upstream: Log4j Logger with the MOD_PREFIX FormattedMessageFactory)
// ---------------------------------------------------------------------------

fn log_with_prefix(level: &str, msg: &str) {
    let full = format!("{}{}", crate::global::MOD_PREFIX, msg);
    LOG_SINK_ON.with(|on| {
        if *on.borrow() {
            LOG_SINK.with(|sink| sink.borrow_mut().push((level.to_string(), full)));
        }
    });
}

pub fn log_info(msg: &str) {
    log_with_prefix("INFO", msg);
}

pub fn log_warn(msg: &str) {
    log_with_prefix("WARN", msg);
}

pub fn log_error(msg: &str) {
    log_with_prefix("ERROR", msg);
}

// Host logging sink: records lines for tests, prints otherwise.
thread_local! {
    static LOG_SINK: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static LOG_SINK_ON: RefCell<bool> = const { RefCell::new(false) };
}

pub fn set_log_sink(on: bool) {
    LOG_SINK_ON.with(|f| *f.borrow_mut() = on);
}

pub fn drain_log_sink() -> Vec<(String, String)> {
    LOG_SINK.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

// ---------------------------------------------------------------------------
// Minecraft client seam (upstream static `Game` = Minecraft.getInstance())
// ---------------------------------------------------------------------------

/// Entity token + operations upstream uses. `type_item` mirrors EntityType.ITEM checks.
pub trait EntityHandle {
    fn uuid(&self) -> u128;
    /// Entity instanceof Player && player.getTeam() != null
    fn team_present(&self) -> bool {
        false
    }
    fn is_type_item(&self) -> bool {
        false
    }
    fn item_entity_item_copy(&self) -> Option<crate::seams::ItemStackToken> {
        None
    }
    fn position(&self, tick_delta: f32) -> Vec3;
    fn bounding_box_ysize(&self) -> f64;
    fn is_spectator(&self) -> bool {
        false
    }
    fn pick_radius(&self) -> f64 {
        0.0
    }
    fn bounding_box_clip(&self, _min: Vec3, _max: Vec3) -> Option<Vec3> {
        None
    }
    fn get_entities(&self, _box: &AabbToken, _predicate: &dyn Fn(&dyn EntityHandle) -> bool) -> Vec<Rc<dyn EntityHandle>> {
        Vec::new()
    }
    fn level_token(&self) -> u64;
    /// Entity#getViewVector(partialTicks)
    fn view_vector(&self, _tick_delta: f32) -> Vec3 {
        Vec3::ZERO
    }
    /// Entity#isCrouching (the hitTranslucent branch of Raycast.traceDirectional)
    fn is_crouching(&self) -> bool {
        false
    }
    /// getBoundingBox().expandTowards(v).inflate(1.0, 1.0, 1.0)
    fn bounding_box_expanded_towards_inflated(&self, _towards: Vec3, _inflate: f64) -> AabbToken {
        AabbToken(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStackToken(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AabbToken(pub u64);

/// Level operations upstream uses.
pub trait LevelHandle {
    fn token(&self) -> u64;
    fn game_time(&self) -> i64;
    fn entities_for_rendering(&self) -> Vec<Rc<dyn EntityHandle>>;
    /// level.dimension().location().toString()
    fn dimension_location(&self) -> String {
        String::new()
    }
    /// level.clip(ClipContext) — host-side ray march; the port passes the exact ClipContext shape.
    fn clip(&self, ctx: &ClipContext) -> Option<crate::seams::BlockHitResult>;
    /// level.getEntities(excluded, aabb, predicate)
    fn get_entities(&self, _excluded: &dyn EntityHandle, _box: &AabbToken, _predicate: &dyn Fn(&dyn EntityHandle) -> bool) -> Vec<Rc<dyn EntityHandle>> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClipContext {
    pub from: Vec3,
    pub to: Vec3,
    pub block: ClipContextBlock,
    pub fluid: ClipContextFluid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipContextBlock {
    Outline,
    Visual,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipContextFluid {
    None,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitResultType {
    Miss,
    Block,
    Entity,
}

#[derive(Clone, Copy, Debug)]
pub struct BlockHitResult {
    pub location: Vec3,
    pub block_pos: (i32, i32, i32),
    /// Direction.UP for the DH raycast, other directions from the host clip.
    pub direction_up: bool,
    pub miss_inside: bool,
    pub hit_type: HitResultType,
}

#[derive(Clone, Copy, Debug)]
pub struct EntityHitResult {
    pub entity_token: u64,
    pub location: Vec3,
}

/// PlayerInfo (connection.getPlayerInfo) operations upstream uses.
pub trait PlayerInfoHandle {
    /// playerInfo.getTeam() != null; the upstream double-null check keeps both halves.
    fn has_team(&self) -> bool {
        false
    }
    /// playerInfo.getTeam().getColor() integer; None = no color (upstream returns WHITE then).
    fn team_color(&self) -> Option<i32>;
    /// playerInfo.getSkinLocation() (skin/atlas location the head renders from).
    fn skin_location(&self) -> String {
        String::new()
    }
    /// playerInfo.getProfile().getName()
    fn profile_name(&self) -> String {
        String::new()
    }
    /// playerInfo.getTeam() name/color-code string for PlayerTeam.formatNameForTeam.
    fn team_name(&self) -> Option<String> {
        None
    }
}

pub trait ConnectionHandle {
    fn get_player_info(&self, author: u128) -> Option<Rc<dyn PlayerInfoHandle>>;
}

/// Minecraft client operations the port reads (upstream static Game accessors).
pub trait MinecraftClient {
    fn level(&self) -> Option<Rc<dyn LevelHandle>>;
    fn camera_entity(&self) -> Option<Rc<dyn EntityHandle>>;
    fn player(&self) -> Option<Rc<dyn EntityHandle>> {
        None
    }
    fn connection(&self) -> Option<Rc<dyn ConnectionHandle>> {
        None
    }
    fn gui_scaled_size(&self) -> (f32, f32);
    /// Game.execute(Runnable) — host main-thread executor.
    fn execute(&self, _task: Rc<dyn Fn()>) {}
    /// gameRenderer.getMainCamera().getPosition()
    fn camera_position(&self) -> Vec3 {
        Vec3::ZERO
    }
    /// Game.getCurrentServer().ip
    fn current_server_ip(&self) -> Option<String> {
        None
    }
    /// Game.level.getGameTime() convenience used through the level seam elsewhere.
    fn sound_manager_play(&self, _instance: &crate::core::DirectionalSoundInstance) {}
    /// Game.font
    fn font(&self) -> Option<std::rc::Rc<dyn FontHandle>> {
        None
    }
    /// RenderSystem/GuiComponent/ItemRenderer environment.
    fn gui_env(&self) -> Option<std::rc::Rc<dyn GuiEnv>> {
        None
    }
    /// Game.options.keyPlayerList.isDown()
    fn key_player_list_down(&self) -> bool {
        false
    }
    /// Game.options.keyPlayerList.getName()
    fn key_player_list_name(&self) -> String {
        String::new()
    }
    /// Game.options.keyPlayerList.getTranslatedKeyMessage()
    fn key_player_list_translated(&self) -> String {
        String::new()
    }
    /// Game.options.save()
    fn save_options(&self) {}
    /// Game.player.sendMessage(component, uuid) (system/sender path)
    fn send_player_message(&self, _component: &str) {}
    /// Game.setScreen(screen)
    /// Game.setScreen(screen): the argument constructs the screen (upstream:
    /// new SettingsScreen()); the host drives its lifecycle through IScreenEnv.
    fn set_screen(&self, _new_screen: Rc<dyn Fn() -> crate::screen::SettingsScreen>) {}
}

pub fn game() -> Option<Rc<dyn MinecraftClient>> {
    GAME.with(|g| g.borrow().clone())
}

pub fn set_game(g: Option<Rc<dyn MinecraftClient>>) {
    GAME.with(|slot| *slot.borrow_mut() = g);
}

thread_local! {
    static GAME: RefCell<Option<Rc<dyn MinecraftClient>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Platform service seams (upstream: IPlatform*Service.INSTANCE)
// ---------------------------------------------------------------------------

pub trait IPlatformContextService {
    fn get_self_mod_version(&self) -> String;
    fn resolve_config_dir(&self, file_name: &str) -> String;
    fn resolve_game_dir(&self, file_name: &str) -> String {
        self.resolve_config_dir(file_name)
    }
    fn register_key_mapping(&self, _binding: &crate::util::KeyBindingToken) {}
    fn current_server_ip(&self) -> Option<String> {
        None
    }
    /// ModContext.indexMods(): FabricLoader.isModLoaded(id)
    fn is_mod_loaded(&self, _id: &str) -> bool {
        false
    }
    /// DhApi.getApiMajor/Minor/PatchVersion (integration/ModContext log line)
    fn dh_api_version(&self) -> Option<(i32, i32, i32)> {
        None
    }
}

pub trait IPlatformNetworkService {
    /// sendToServer(IPacket) — serializes the packet via its write() into a FriendlyByteBuf.
    fn send_to_server(&self, packet: &dyn crate::network::IPacket);
    fn send_to_client(&self, packet: &dyn crate::network::IPacket, player: u128);
}

pub trait IPlatformClientEventService {
    fn register_tick_start_event(&self, _cb: Rc<dyn Fn()>);
    fn register_join_server_event(&self, _cb: Rc<dyn Fn()>);
    fn register_leave_server_event(&self, _cb: Rc<dyn Fn()>);
    fn register_render_world_event(&self, _cb: Rc<dyn Fn(&crate::core::WorldRenderContext)>);
    fn register_render_gui_event(&self, _cb: Rc<dyn Fn(PoseStack, f32)>);
}

pub trait IPlatformServerEventService {
    fn register_player_logout_event(&self, _cb: Rc<dyn Fn(u128)>);
}

struct NoContextService;
impl IPlatformContextService for NoContextService {
    fn get_self_mod_version(&self) -> String {
        String::new()
    }
    fn resolve_config_dir(&self, _name: &str) -> String {
        String::new()
    }
}

pub fn client_event_service() -> Rc<dyn IPlatformClientEventService> {
    CLIENT_EVENT_SERVICE.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoClientEventService))
    })
}

pub fn set_client_event_service(s: Rc<dyn IPlatformClientEventService>) {
    CLIENT_EVENT_SERVICE.with(|c| *c.borrow_mut() = Some(s));
}

struct NoClientEventService;

impl IPlatformClientEventService for NoClientEventService {
    fn register_tick_start_event(&self, _cb: Rc<dyn Fn()>) {}
    fn register_join_server_event(&self, _cb: Rc<dyn Fn()>) {}
    fn register_leave_server_event(&self, _cb: Rc<dyn Fn()>) {}
    fn register_render_world_event(&self, _cb: Rc<dyn Fn(&crate::core::WorldRenderContext)>) {}
    fn register_render_gui_event(&self, _cb: Rc<dyn Fn(crate::seams::PoseStack, f32)>) {}
}

struct NoNetworkService;
impl IPlatformNetworkService for NoNetworkService {
    fn send_to_server(&self, _packet: &dyn crate::network::IPacket) {}
    fn send_to_client(&self, _packet: &dyn crate::network::IPacket, _player: u128) {}
}

pub fn context_service() -> Rc<dyn IPlatformContextService> {
    CONTEXT_SERVICE.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoContextService) as Rc<dyn IPlatformContextService>)
    })
}

pub fn network_service() -> Rc<dyn IPlatformNetworkService> {
    NETWORK_SERVICE.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoNetworkService) as Rc<dyn IPlatformNetworkService>)
    })
}

pub fn set_context_service(s: Rc<dyn IPlatformContextService>) {
    CONTEXT_SERVICE.with(|c| *c.borrow_mut() = Some(s));
}

pub fn set_network_service(s: Rc<dyn IPlatformNetworkService>) {
    NETWORK_SERVICE.with(|c| *c.borrow_mut() = Some(s));
}

thread_local! {
    static CONTEXT_SERVICE: RefCell<Option<Rc<dyn IPlatformContextService>>> = const { RefCell::new(None) };
    static CLIENT_EVENT_SERVICE: RefCell<Option<Rc<dyn IPlatformClientEventService>>> = const { RefCell::new(None) };
    static NETWORK_SERVICE: RefCell<Option<Rc<dyn IPlatformNetworkService>>> = const { RefCell::new(None) };
}

/// Global.java logger needs a host sink for tests; production wiring attaches the real logger.
pub fn host_logger(_level: &str, _msg: &str) {
    LOG_SINK_ON.with(|on| {
        if *on.borrow() {
            let level = _level.to_string();
            let msg = _msg.to_string();
            LOG_SINK.with(|s| s.borrow_mut().push((level, msg)));
        }
    });
}

/// Java String#hashCode, used by GameContext.updateDimension (dimension().location().hashCode()).
pub fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for ch in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(ch as i32);
    }
    h
}

/// ModContext.HasDistantHorizons — set true by the integration phase when the mod is present.
pub static HAS_DISTANT_HORIZONS: Mutex<bool> = Mutex::new(false);

// ---------------------------------------------------------------------------
// Distant Horizons API seam (upstream: com.seibel.distanthorizons.api.DhApi)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct DhRaycastResult {
    pub success: bool,
    pub pos: Option<Vec3>,
}

pub trait DhApiSeam {
    /// DhApi.Delayed.worldProxy == null / getSinglePlayerLevel()
    fn single_player_level(&self) -> Option<u64> {
        None
    }
    /// DhApi.Delayed.terrainRepo.raycast(levelWrapper, sx, sy, sz, dx, dy, dz, 4096, cache)
    fn raycast(&self, _level: u64, _sx: f64, _sy: f64, _sz: f64, _dx: f32, _dy: f32, _dz: f32, _max: f32, _cache: u64) -> DhRaycastResult {
        DhRaycastResult { success: false, pos: None }
    }
    fn create_soft_cache(&self) -> u64 {
        0
    }
}

pub fn dh_api() -> Rc<dyn DhApiSeam> {
    DH_API.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoDhApi) as Rc<dyn DhApiSeam>)
    })
}

pub fn set_dh_api(s: Rc<dyn DhApiSeam>) {
    DH_API.with(|c| *c.borrow_mut() = Some(s));
}

struct NoDhApi;
impl DhApiSeam for NoDhApi {}

thread_local! {
    static DH_API: RefCell<Option<Rc<dyn DhApiSeam>>> = const { RefCell::new(None) };
}

/// Upstream keeps `Instant lastCacheLoad = Instant.EPOCH` and refreshes the cache when > 10s old.
pub fn now_instant() -> Instant {
    Instant::now()
}

// ---------------------------------------------------------------------------
// Additional host seams used by util / math / core / network
// ---------------------------------------------------------------------------

/// net.minecraft.network.chat.Component translation (compat/Component translatable).
pub fn translate_component(_key: &str, _args: &[&str]) -> String {
    String::new()
}

/// KeyMapping#consumeClick host seam.
pub fn consume_click_key(_binding: &crate::util::KeyBindingToken) -> bool {
    false
}

/// PoseStack#mulPose(Vector3f.ZP.rotation(theta)) host seam.

impl Vec3 {
    /// Vec3#normalize
    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / len)
        }
    }
}

/// FriendlyByteBuf seam with the exact operations the packets call.
pub struct FriendlyByteBuf {
    pub data: Vec<u8>,
    pub reader_index: usize,
}

impl FriendlyByteBuf {
    pub fn new() -> Self {
        Self { data: Vec::new(), reader_index: 0 }
    }

    fn read_bytes(&mut self, n: usize) -> Vec<u8> {
        let end = (self.reader_index + n).min(self.data.len());
        let out = self.data[self.reader_index..end].to_vec();
        self.reader_index = end;
        out
    }

    pub fn read_utf(&mut self, max_len: usize) -> String {
        let len = u16::from_le_bytes(self.read_bytes(2).try_into().unwrap_or([0; 2])) as usize;
        let bytes = self.read_bytes(len);
        let s = String::from_utf8(bytes).unwrap_or_default();
        s.chars().take(max_len).collect()
    }

    pub fn read_double(&mut self) -> f64 {
        f64::from_le_bytes(self.read_bytes(8).try_into().unwrap_or([0; 8]))
    }

    pub fn read_boolean(&mut self) -> bool {
        self.read_bytes(1).first().map(|b| *b != 0).unwrap_or(false)
    }

    pub fn read_int(&mut self) -> i32 {
        i32::from_le_bytes(self.read_bytes(4).try_into().unwrap_or([0; 4]))
    }

    pub fn read_uuid(&mut self) -> u128 {
        u128::from_le_bytes(self.read_bytes(16).try_into().unwrap_or([0; 16]))
    }

    pub fn write_utf(&mut self, s: Option<&str>, max_len: usize) {
        // Upstream writeUtf would throw on null; only non-corrupt packets are written.
        let s = s.unwrap_or("");
        let bytes: Vec<u8> = s.chars().take(max_len).collect::<String>().into_bytes();
        let len = (bytes.len().min(u16::MAX as usize)) as u16;
        self.data.extend_from_slice(&len.to_le_bytes());
        self.data.extend_from_slice(&bytes[..len as usize]);
    }

    pub fn write_double(&mut self, v: f64) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_boolean(&mut self, v: bool) {
        self.data.push(v as u8);
    }

    pub fn write_int(&mut self, v: i32) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_uuid(&mut self, v: u128) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn readable_bytes(&self) -> usize {
        self.data.len().saturating_sub(self.reader_index)
    }

    pub fn skip_remaining(&mut self) {
        self.reader_index = self.data.len();
    }
}

impl Default for FriendlyByteBuf {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Server seam: ServerPlayer operations + PlayerList access used by ServerCore.
// ---------------------------------------------------------------------------

pub trait ServerPlayerHandle {
    fn uuid(&self) -> u128;
    fn game_profile_name(&self) -> String;
    /// player.displayClientMessage(component, actionBar=false)
    fn display_client_message(&self, _component: &str, _action_bar: bool) {}
}

pub trait ServerEnv {
    /// server.getPlayerList().getPlayers()
    fn players(&self) -> Vec<u128>;
    /// playerList.getPlayer(uuid) != null
    fn has_player(&self, _uuid: u128) -> bool {
        false
    }
    fn player(&self, _uuid: u128) -> Option<Rc<dyn ServerPlayerHandle>> {
        None
    }
}

pub fn server_env() -> Rc<dyn ServerEnv> {
    SERVER_ENV.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoServerEnv) as Rc<dyn ServerEnv>)
    })
}

pub fn set_server_env(s: Rc<dyn ServerEnv>) {
    SERVER_ENV.with(|c| *c.borrow_mut() = Some(s));
}

struct NoServerEnv;
impl ServerEnv for NoServerEnv {
    fn players(&self) -> Vec<u128> {
        Vec::new()
    }
}

thread_local! {
    static SERVER_ENV: RefCell<Option<Rc<dyn ServerEnv>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// TeamContextHandler seam (upstream integration/TeamContextHandler; the real
// translation follows with the integration phase).
// ---------------------------------------------------------------------------

pub trait TeamContextSeam {
    fn has_team(&self, _player: u128) -> bool {
        false
    }
    fn in_same_context(&self, _a: u128, _b: u128) -> bool {
        false
    }
}

pub fn team_context() -> Rc<dyn TeamContextSeam> {
    TEAM_CONTEXT.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoTeamContext) as Rc<dyn TeamContextSeam>)
    })
}

pub fn set_team_context(s: Rc<dyn TeamContextSeam>) {
    TEAM_CONTEXT.with(|c| *c.borrow_mut() = Some(s));
}

struct NoTeamContext;
impl TeamContextSeam for NoTeamContext {}

thread_local! {
    static TEAM_CONTEXT: RefCell<Option<Rc<dyn TeamContextSeam>>> = const { RefCell::new(None) };
}

/// Async executor seam for CompletableFuture.supplyAsync (Raycast.traceDistantAsync).
pub fn run_async(task: Box<dyn FnOnce() + Send>) {
    std::thread::spawn(task);
}

// ---------------------------------------------------------------------------
// Seam additions surfaced while translating core/math (level lookup by token,
// camera view vector, expanded AABB, dimension location, server IP).
// ---------------------------------------------------------------------------

pub fn level_by_token(token: u64) -> Rc<dyn LevelHandle> {
    LEVEL_BY_TOKEN.with(|m| {
        m.borrow()
            .get(&token)
            .cloned()
            .unwrap_or_else(|| Rc::new(NoLevel) as Rc<dyn LevelHandle>)
    })
}

pub fn register_level(handle: Rc<dyn LevelHandle>) {
    LEVEL_BY_TOKEN.with(|m| m.borrow_mut().insert(handle.token(), handle));
}

thread_local! {
    static LEVEL_BY_TOKEN: RefCell<std::collections::HashMap<u64, Rc<dyn LevelHandle>>> = RefCell::new(std::collections::HashMap::new());
}

struct NoLevel;
impl LevelHandle for NoLevel {
    fn token(&self) -> u64 {
        0
    }
    fn game_time(&self) -> i64 {
        0
    }
    fn entities_for_rendering(&self) -> Vec<Rc<dyn EntityHandle>> {
        Vec::new()
    }
    fn clip(&self, _ctx: &ClipContext) -> Option<BlockHitResult> {
        None
    }
}

impl LevelHandle for Box<dyn LevelHandle> {
    fn token(&self) -> u64 {
        (**self).token()
    }
    fn game_time(&self) -> i64 {
        (**self).game_time()
    }
    fn entities_for_rendering(&self) -> Vec<Rc<dyn EntityHandle>> {
        (**self).entities_for_rendering()
    }
    fn clip(&self, ctx: &ClipContext) -> Option<BlockHitResult> {
        (**self).clip(ctx)
    }
    fn get_entities(&self, excluded: &dyn EntityHandle, box_: &AabbToken, predicate: &dyn Fn(&dyn EntityHandle) -> bool) -> Vec<Rc<dyn EntityHandle>> {
        (**self).get_entities(excluded, box_, predicate)
    }
}


// ---------------------------------------------------------------------------
// Render seams (upstream: com.mojang.blaze3d PoseStack / RenderSystem, GuiComponent,
// Font, ItemRenderer). Added with the render phase.
// ---------------------------------------------------------------------------

/// net.minecraft.world.phys.Vec2.scale / add (used by the render label math).
impl Vec2 {
    pub fn scale(self, f: f32) -> Vec2 {
        Vec2::new(self.x * f, self.y * f)
    }

    pub fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

/// Upstream PoseStack: a stack of matrices; vertex transform is column-vector
/// (matches seams::transform4), so translate/scale/rotate right-multiply the top pose.
#[derive(Clone, Debug)]
pub struct PoseStack {
    stack: Vec<Matrix4f>,
}

impl Default for PoseStack {
    fn default() -> Self {
        Self::new()
    }
}

impl PoseStack {
    pub fn new() -> Self {
        Self { stack: vec![Matrix4f::identity()] }
    }

    fn last(&self) -> &Matrix4f {
        self.stack.last().expect("pose stack underflow")
    }

    fn last_mut(&mut self) -> &mut Matrix4f {
        self.stack.last_mut().expect("pose stack underflow")
    }

    fn compose(&mut self, t: &Matrix4f) {
        let a = self.last().clone();
        let mut r = Matrix4f::identity();
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += a.0[i][k] * t.0[k][j];
                }
                r.0[i][j] = s;
            }
        }
        *self.last_mut() = r;
    }

    pub fn push_pose(&mut self) {
        let top = self.last().clone();
        self.stack.push(top);
    }

    pub fn pop_pose(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        let mut t = Matrix4f::identity();
        t.0[0][3] = x;
        t.0[1][3] = y;
        t.0[2][3] = z;
        self.compose(&t);
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        let mut t = Matrix4f::identity();
        t.0[0][0] = x;
        t.0[1][1] = y;
        t.0[2][2] = z;
        self.compose(&t);
    }

    /// Vector3f.ZP.rotation(theta): rotation about +Z, column-vector convention.
    pub fn rotate_z(&mut self, theta: f32) {
        let c = theta.cos();
        let s = theta.sin();
        let mut t = Matrix4f::identity();
        t.0[0][0] = c;
        t.0[0][1] = -s;
        t.0[1][0] = s;
        t.0[1][1] = c;
        self.compose(&t);
    }

    pub fn top(&self) -> &Matrix4f {
        self.last()
    }
}

/// net.minecraft.client.gui.Font the port draws with.
pub trait FontHandle {
    fn width(&self, text: &str) -> f32;
    fn line_height(&self) -> f32;
    /// Font.split(text, lineLength) -> FormattedCharSequence lines
    fn split(&self, _text: &str, _max: i32) -> Vec<String> {
        Vec::new()
    }
    /// Font.draw(matrices, text, x, y, color)
    fn draw(&self, _m: &PoseStack, _text: &str, _x: f32, _y: f32, _color: i32) {}
}

/// RenderSystem + GuiComponent + ItemRenderer ops, in the exact call order
/// DrawContext issues them. Default impls are no-ops (a headless host); the
/// test suite drives a recording implementation.
pub trait GuiEnv {
    /// GuiComponent.fill(matrices, minX, minY, maxX, maxY, color)
    fn fill(&self, _m: &PoseStack, _min_x: i32, _min_y: i32, _max_x: i32, _max_y: i32, _color: i32) {}
    /// GuiComponent.blit(matrices, x, y, u, v, width, height, textureWidth, textureHeight)
    fn blit(&self, _m: &PoseStack, _x: i32, _y: i32, _z: i32, _u: i32, _v: i32, _w: i32, _h: i32, _tw: i32, _th: i32) {}
    /// RenderSystem.setShaderTexture(0, location)
    fn set_shader_texture(&self, _namespace: &str, _path: &str) {}
    /// RenderSystem.setShaderColor(r, g, b, a)
    fn set_shader_color(&self, _r: f32, _g: f32, _b: f32, _a: f32) {}
    fn enable_blend(&self) {}
    fn disable_blend(&self) {}
    fn enable_depth_test(&self) {}
    /// RenderSystem.blendFunc(SourceFactor.SRC_ALPHA, DestFactor.ONE_MINUS_SRC_ALPHA)
    fn blend_func_src_alpha_one_minus(&self) {}
    /// Lighting.setupForFlatItems() / setupFor3DItems()
    fn setup_for_flat_items(&self) {}
    fn setup_for_3d_items(&self) {}
    /// TextureAtlas.LOCATION_BLOCKS
    fn blocks_atlas(&self) -> ResourceLocationToken;
    /// TextureManager.getTexture(atlas).setFilter(blur, mipmap)
    fn set_texture_filter(&self, _res: &ResourceLocationToken, _blur: bool, _mipmap: bool) {}
    /// ItemRenderer.getModel(stack, null, null, 0) -> (model token, model.usesBlockLight())
    fn item_model(&self, _stack: &ItemStackToken) -> (u64, bool);
    /// ItemRenderer.render(..., TransformType.GUI, false, dummy, immediate, LIGHT_VALUE_MAX,
    /// OverlayTexture.NO_OVERLAY, model) then immediate.endBatch()
    fn render_item_gui_and_end_batch(&self, _stack: &ItemStackToken, _model: u64) {}
    /// RenderSystem model-view stack: pushPose, mulPoseMatrix(pose), translate, scale, applyModelViewMatrix
    fn mv_push(&self) {}
    fn mv_mul_pose(&self, _pose: &Matrix4f) {}
    fn mv_translate(&self, _x: f32, _y: f32, _z: f32) {}
    fn mv_scale(&self, _x: f32, _y: f32, _z: f32) {}
    fn mv_apply(&self) {}
    /// RenderSystem model-view stack: popPose
    fn mv_pop(&self) {}
}

/// PlayerTeam.formatNameForTeam(team, text): None team copies the text; a team wraps
/// it in the team's colored formatting (the seam takes the team's color code string).
pub fn player_team_format_name(team: Option<&str>, name: &str) -> String {
    match team {
        None => name.to_string(),
        Some(color_code) => format!("\u{a7}{}{}", color_code, name),
    }
}

/// InputConstants.getKey(keyString) (LegacyMigrationHandler migration path).
pub fn input_constants_get_key(_key_string: &str) -> i32 {
    -1
}


// ---------------------------------------------------------------------------
// Integration service seam: the optional third-party APIs behind
// integration/VoiceChatWrapper + FTBTeamsWrapper, plus the vanilla team lookup
// TeamContextHandler needs. All answers default to "integration absent".
// ---------------------------------------------------------------------------

pub trait IIntegrationService {
    /// VoiceChatIntegration.serverApi.getConnectionOf(uuid).getGroup().getId()
    fn voice_chat_group_id(&self, _player: u128) -> Option<u128> {
        None
    }
    /// VoiceChatIntegration.clientApi.getGroup().getId()
    fn voice_chat_self_group_id(&self) -> Option<u128> {
        None
    }
    /// FTBTeamsAPI.getManager().getPlayerTeam(uuid).getId() when not a player team
    fn ftb_team_id(&self, _player: u128) -> Option<u128> {
        None
    }
    /// FTBTeamsAPI.getClientManager().selfTeam.getId() when not a player team
    fn ftb_self_team_id(&self) -> Option<u128> {
        None
    }
    /// player.getTeam() != null
    fn player_team_present(&self, _player: u128) -> bool {
        false
    }
    /// player.getTeam() as an identity token; None is "no team"
    fn player_team_uuid(&self, _player: u128) -> Option<u128> {
        None
    }
}

pub fn integration_service() -> Rc<dyn IIntegrationService> {
    INTEGRATION_SERVICE.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoIntegrationService) as Rc<dyn IIntegrationService>)
    })
}

pub fn set_integration_service(s: Rc<dyn IIntegrationService>) {
    INTEGRATION_SERVICE.with(|c| *c.borrow_mut() = Some(s));
}

pub(crate) struct NoIntegrationService;

thread_local! {
    static INTEGRATION_SERVICE: RefCell<Option<Rc<dyn IIntegrationService>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// SettingsScreen host seam: the widget types (OptionsList, EditBox, Button) and
// the Screen base-class calls are host surfaces. Defaults are inert (a headless
// host); the test suite drives a recording implementation.
// ---------------------------------------------------------------------------

pub trait OptionsListHost {
    /// OptionsList.addSmall(optionA, optionB); a null second half is None.
    fn add_small(&self, _a: crate::screen::OptionEntry, _b: Option<crate::screen::OptionEntry>) {}
    /// this.list.children().size()
    fn children_len(&self) -> usize {
        0
    }
    /// list.getMouseOver(mouseX, mouseY)
    fn get_mouse_over(&self, _mouse_x: i32, _mouse_y: i32) -> Option<usize> {
        None
    }
    /// ((TooltipAccessor) entry).getTooltip()
    fn tooltip_of(&self, _index: usize) -> Vec<String> {
        Vec::new()
    }
    fn render(&self, _m: &PoseStack, _mouse_x: i32, _mouse_y: i32, _delta: f32) {}
}

pub trait EditBoxHost {
    fn set_max_length(&self, _length: i32) {}
    fn set_value(&self, _value: &str) {}
    fn value(&self) -> String {
        String::new()
    }
    fn set_responder(&self, _responder: Rc<dyn Fn(&str)>) {}
    fn tick(&self) {}
    fn render(&self, _m: &PoseStack, _mouse_x: i32, _mouse_y: i32, _delta: f32) {}
    fn is_hovered_or_focused(&self) -> bool {
        false
    }
    fn is_focused(&self) -> bool {
        false
    }
    /// the edit box's y field
    fn y(&self) -> i32 {
        0
    }
}

pub trait IScreenEnv {
    fn width(&self) -> i32 {
        0
    }
    fn height(&self) -> i32 {
        0
    }
    /// new OptionsList(minecraft, width, height, top, bottom, itemHeight)
    fn new_options_list(&self, _w: i32, _h: i32, _top: i32, _bottom: i32, _item_height: i32) -> Rc<dyn OptionsListHost> {
        Rc::new(NoOptionsList)
    }
    /// new EditBox(font, x, y, width, height, Component.empty())
    fn new_edit_box(&self, _x: i32, _y: i32, _w: i32, _h: i32, _placeholder: &str) -> Rc<dyn EditBoxHost> {
        Rc::new(NoEditBox)
    }
    fn add_widget_edit(&self, _edit: Rc<dyn EditBoxHost>) {}
    fn add_widget_list(&self, _list: Rc<dyn OptionsListHost>) {}
    /// addRenderableWidget(new Button(x, y, w, h, label, onClick))
    fn add_renderable_button(&self, _x: i32, _y: i32, _w: i32, _h: i32, _label: &str, _on_click: Rc<dyn Fn()>) {}
    fn render_background(&self, _m: &PoseStack) {}
    fn draw_centered_string(&self, _m: &PoseStack, _text: &str, _center_x: i32, _y: i32, _color: i32) {}
    fn draw_string(&self, _m: &PoseStack, _text: &str, _x: i32, _y: i32, _color: i32) {}
    fn font_split(&self, _text: &str, _max: i32) -> Vec<String> {
        Vec::new()
    }
    fn render_tooltip(&self, _m: &PoseStack, _lines: &[String], _mouse_x: i32, _mouse_y: i32) {}
    /// Screen.render(matrices, mouseX, mouseY, delta)
    fn super_render(&self, _m: &PoseStack, _mouse_x: i32, _mouse_y: i32, _delta: f32) {}
    /// Screen.onClose() base behavior
    fn super_on_close(&self) {}
    /// this.minecraft != null
    fn mc_present(&self) -> bool {
        false
    }
}

pub fn screen_env() -> Rc<dyn IScreenEnv> {
    SCREEN_ENV.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(NoScreenEnv) as Rc<dyn IScreenEnv>)
    })
}

pub fn set_screen_env(s: Rc<dyn IScreenEnv>) {
    SCREEN_ENV.with(|c| *c.borrow_mut() = Some(s));
}

pub struct NoScreenEnv;
struct NoOptionsList;
struct NoEditBox;

thread_local! {
    static SCREEN_ENV: RefCell<Option<Rc<dyn IScreenEnv>>> = const { RefCell::new(None) };
}


impl IIntegrationService for NoIntegrationService {}

impl OptionsListHost for NoOptionsList {}

impl EditBoxHost for NoEditBox {}

impl IScreenEnv for NoScreenEnv {}
