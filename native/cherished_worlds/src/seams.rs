//! Host seams: every Minecraft client / SpectreLib / MixinExtras type the
//! port touches sits here, as a thin no-op-default surface the engine binds.
//! Signatures mirror the upstream Java calls one to one.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------- logging

pub struct Logger;

impl Logger {
    pub fn error(&self, msg: &str) {
        eprintln!("[cherishedworlds] ERROR: {msg}");
    }

    pub fn debug(&self, msg: &str) {
        let _ = msg;
    }
}

// -------------------------------------------------------------- platform

// java: IPlatformHelper.getGamePath()
pub fn game_path() -> PathBuf {
    std::env::var("HORIZON_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// java: IPlatformHelper.isModLoaded(String)
pub fn is_mod_loaded(_mod_id: &str) -> bool {
    HOOKS
        .get()
        .map(|h| (h.is_mod_loaded_fn.lock().unwrap())(_mod_id))
        .unwrap_or(false)
}

// ------------------------------------------------------------------- nbt

// java: net.minecraft.nbt.StringTag / ListTag / CompoundTag / NbtIo, reduced
// to the operations FavoritesList performs on them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListTag {
    pub strings: Vec<String>,
}

impl ListTag {
    pub fn new() -> ListTag {
        ListTag { strings: Vec::new() }
    }

    pub fn size(&self) -> usize {
        self.strings.len()
    }

    // java: ListTag.getString(int) -> Optional<String>
    pub fn get_string(&self, i: usize) -> Option<String> {
        self.strings.get(i).cloned()
    }

    // java: ListTag.add(StringTag.valueOf(s))
    pub fn add(&mut self, tag: StringTag) {
        self.strings.push(tag.value);
    }
}

#[derive(Debug)]
pub struct StringTag {
    pub value: String,
}

impl StringTag {
    // java: StringTag.valueOf(String)
    pub fn value_of(value: &str) -> StringTag {
        StringTag { value: value.to_string() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompoundTag {
    pub favorites: Option<ListTag>,
}

impl CompoundTag {
    // java: CompoundTag.getListOrEmpty(String)
    pub fn get_list_or_empty(&self, key: &str) -> ListTag {
        if key == "favorites" {
            self.favorites.clone().unwrap_or_default()
        } else {
            ListTag::new()
        }
    }

    // java: CompoundTag.put(String, ListTag)
    pub fn put(&mut self, key: &str, list: ListTag) {
        if key == "favorites" {
            self.favorites = Some(list);
        }
    }
}

// java: NbtIo.read(Path) -> CompoundTag or null
pub fn nbt_io_read(path: &PathBuf) -> Option<CompoundTag> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    Some(CompoundTag {
        favorites: Some(ListTag {
            strings: text.lines().map(str::to_string).collect(),
        }),
    })
}

// java: NbtIo.write(CompoundTag, Path)
pub fn nbt_io_write(tag: &CompoundTag, path: &PathBuf) -> std::io::Result<()> {
    let strings = tag
        .favorites
        .clone()
        .unwrap_or_default()
        .strings
        .join("\n");
    std::fs::write(path, strings)
}

// java: org.apache.commons.io.FileUtils.forceMkdirParent(File)
pub fn force_mkdir_parent(file: &PathBuf) {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
}

// ------------------------------------------------------------- component

// java: net.minecraft.network.chat.Component / MutableComponent /
// TranslatableContents, reduced to the translatable key the port reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub translation_key: String,
    pub args: Vec<String>,
}

impl Component {
    // java: Component.translatable(String)
    pub fn translatable(key: &str) -> Component {
        Component { translation_key: key.to_string(), args: Vec::new() }
    }

    // java: Component.translatable(String, Object...)
    pub fn translatable_with(key: &str, args: &[&str]) -> Component {
        Component {
            translation_key: key.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    // java: Component.empty()
    pub fn empty() -> Component {
        Component::translatable("")
    }
}

// ----------------------------------------------------------------- gui

// java: net.minecraft.client.gui.screens.Screen
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenKind {
    CreateWorld,
    JoinMultiplayer,
    SelectWorld,
    #[default]
    Other,
}

// java: net.minecraft.client.gui.components.AbstractWidget (the base the
// favorites widgets extend), engine-bound fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WidgetBase {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub active: bool,
    pub focused: bool,
    pub hovered: bool,
}

impl WidgetBase {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> WidgetBase {
        WidgetBase { x, y, width, height, active: true, focused: false, hovered: false }
    }

    // java: AbstractWidget.isActive()
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

// java: net.minecraft.client.gui.components.AbstractWidget.getMessage()
// for the Create World button scan in FavoriteCreateWorldWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildWidget {
    // An AbstractWidget whose message is a translatable component.
    Button { base: WidgetBase, translation_key: Option<String> },
    // A non-widget GuiEventListener child.
    Listener,
}

impl ChildWidget {
    pub fn as_widget(&self) -> Option<(WidgetBase, Option<String>)> {
        match self {
            ChildWidget::Button { base, translation_key } => Some((*base, translation_key.clone())),
            ChildWidget::Listener => None,
        }
    }
}

// java: net.minecraft.client.multiplayer.ServerData
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerData {
    pub name: String,
    pub ip: String,
}

impl ServerData {
    // java: data.name + data.ip (the favorites key)
    pub fn favorite_key(&self) -> String {
        format!("{}{}", self.name, self.ip)
    }
}

// java: net.minecraft.client.server.LanServer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanServer {
    pub address: String,
}

// java: net.minecraft.world.level.storage.LevelSummary + BackupStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupStatus {
    None,
    UpgradeToSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelSummary {
    // java: LevelSummary.getLevelId()
    pub level_id: String,
    // java: levelSummary.levelVersion().minecraftVersion().version()
    pub level_version: i32,
}

// java: net.minecraft.SharedConstants.getCurrentVersion().dataVersion().version()
pub fn current_data_version() -> i32 {
    9999
}

// java: net.minecraft.client.multiplayer.ServerList
#[derive(Debug, Clone, Default)]
pub struct ServerList {
    pub servers: Vec<ServerData>,
}

impl ServerList {
    pub fn size(&self) -> usize {
        self.servers.len()
    }

    // java: ServerList.get(int)
    pub fn get(&self, i: usize) -> Option<&ServerData> {
        self.servers.get(i)
    }

    // java: ServerList.replace(int, ServerData)
    pub fn replace(&mut self, i: usize, data: ServerData) {
        if let Some(slot) = self.servers.get_mut(i) {
            *slot = data;
        }
    }
}

// java: ServerSelectionList entries, reduced to the data the hooks read.
#[derive(Debug, Clone)]
pub enum ServerSelectionEntry {
    Online(ServerData),
    Network(LanServer),
    Other,
}

// java: WorldSelectionList.Entry with its LevelSummary (null upstream too).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSelectionEntry {
    pub summary: Option<LevelSummary>,
}

// java: net.minecraft.client.gui.screens.multiplayer.ServerSelectionList
#[derive(Debug, Clone, Default)]
pub struct ServerSelectionList {
    // java: private final List<ServerSelectionList.OnlineServerEntry> onlineServers
    pub online_servers: Vec<ServerSelectionEntry>,
    // java: private final List<ServerSelectionList.NetworkServerEntry> networkServers
    pub network_servers: Vec<ServerSelectionEntry>,
    pub selected: Option<ServerSelectionEntry>,
    pub scroll_amount: f64,
    pub top: i32,
    pub bottom: i32,
}

impl ServerSelectionList {
    pub fn children(&self) -> Vec<ServerSelectionEntry> {
        let mut all = self.online_servers.clone();
        all.extend(self.network_servers.clone());
        all
    }

    pub fn get_selected(&self) -> Option<ServerSelectionEntry> {
        self.selected.clone()
    }

    pub fn set_scroll_amount(&mut self, amount: f64) {
        self.scroll_amount = amount;
    }

    pub fn scroll_amount(&self) -> f64 {
        self.scroll_amount
    }

    pub fn get_y(&self) -> i32 {
        self.top
    }

    pub fn get_bottom(&self) -> i32 {
        self.bottom
    }

    // java: this.list.getRectangle() -> ScreenRectangle
    pub fn get_rectangle(&self) -> ScreenRectangle {
        ScreenRectangle { left: 0, top: self.top, width: 200, height: self.bottom - self.top }
    }
}

// java: net.minecraft.client.gui.screens.worldselection.WorldSelectionList
#[derive(Debug, Clone, Default)]
pub struct WorldSelectionList {
    // java: AccessorWorldSelectionList.getCurrentlyDisplayedLevels()
    pub currently_displayed_levels: Option<Vec<LevelSummary>>,
    // java: callFillLevels(String, List<LevelSummary>) via the @Invoker
    pub fill_levels_filter: Option<String>,
    pub children: Vec<WorldSelectionEntry>,
    pub selected_index: Option<usize>,
    pub scroll_amount: f64,
    pub top: i32,
    pub bottom: i32,
}

impl WorldSelectionList {
    pub fn children(&self) -> Vec<WorldSelectionEntry> {
        self.children.clone()
    }

    pub fn get_selected(&self) -> Option<WorldSelectionEntry> {
        self.selected_index.and_then(|i| self.children.get(i)).cloned()
    }

    pub fn sort(&mut self, comparator: &dyn Fn(&WorldSelectionEntry, &WorldSelectionEntry) -> std::cmp::Ordering) {
        self.children.sort_by(|a, b| comparator(a, b));
    }

    pub fn set_scroll_amount(&mut self, amount: f64) {
        self.scroll_amount = amount;
    }

    pub fn scroll_amount(&self) -> f64 {
        self.scroll_amount
    }

    pub fn get_y(&self) -> i32 {
        self.top
    }

    pub fn get_bottom(&self) -> i32 {
        self.bottom
    }

    pub fn get_rectangle(&self) -> ScreenRectangle {
        ScreenRectangle { left: 0, top: self.top, width: 200, height: self.bottom - self.top }
    }
}

// java: net.minecraft.client.gui.screens.Screen children as the widget port
// reads them, plus the accessors the mixins bind.
#[derive(Debug, Clone, Default)]
pub struct HostScreen {
    pub kind: ScreenKind,
    pub children: Vec<ChildWidget>,
    // java: AccessorWorldSelectionScreen.getSearchBox()
    pub search_box_value: String,
    // java: AccessorWorldSelectionScreen.getList()
    pub world_list: Option<WorldSelectionList>,
    // java: AccessorJoinMultiplayerScreen.getSelectionList()
    pub server_selection_list: Option<ServerSelectionList>,
    // java: AccessorJoinMultiplayerScreen.getDeleteButton()
    pub delete_button_active: bool,
    // java: JoinMultiplayerScreen.getServers()
    pub servers: ServerList,
    // java: this.list.children() entry geometry for the list widget ports
    pub entry_geometry: Vec<EntryGeometry>,
}

#[derive(Debug, Clone, Copy)]
pub struct EntryGeometry {
    pub x: i32,
    pub y: i32,
    pub content_y: i32,
    pub content_bottom: i32,
    pub content_y_middle: i32,
    pub height: i32,
}

// java: net.minecraft.client.gui.screens.Screen (the base the favorites
// widgets hold). The engine binds which concrete screen it is and supplies
// the children the port scans.
#[derive(Clone, Default)]
pub struct Screen {
    pub host: HostScreen,
    pub base: WidgetBase,
    // java: screen.triggerImmediateNarration(boolean)
    pub narration_requested: std::cell::Cell<bool>,
}

impl Screen {
    pub fn new(kind: ScreenKind) -> Screen {
        Screen {
            host: HostScreen { kind, ..HostScreen::default() },
            base: WidgetBase::new(0, 0, 0, 0),
            narration_requested: std::cell::Cell::new(false),
        }
    }

    pub fn kind(&self) -> ScreenKind {
        self.host.kind
    }

    // java: screen.children()
    pub fn children(&self) -> Vec<ChildWidget> {
        self.host.children.clone()
    }

    // java: screen.triggerImmediateNarration(true)
    pub fn trigger_immediate_narration(&self, flag: bool) {
        self.narration_requested.set(flag);
    }
}

// ------------------------------------------------------------- graphics

// java: net.minecraft.client.gui.render.state.GuiGraphicsExtractor, reduced
// to the operations the favorites widgets perform.
#[derive(Debug, Default)]
pub struct GuiGraphicsExtractor {
    pub blits: Vec<BlitOp>,
    pub mouse_in_scissor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconTexture {
    Favorited,
    Empty,
    HoveredFavorited,
    HoveredEmpty,
    FocusedOutline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlitOp {
    pub icon: IconTexture,
    pub x: i32,
    pub y: i32,
    pub v_start: i32,
    pub width: i32,
    pub height: i32,
}

impl GuiGraphicsExtractor {
    // java: graphics.containsPointInScissor(mouseX, mouseY)
    pub fn contains_point_in_scissor(&self, _mouse_x: i32, _mouse_y: i32) -> bool {
        self.mouse_in_scissor
    }

    // java: graphics.blit(RenderPipelines.GUI_TEXTURED, icon, x, y, 0, vStart,
    //                     TEXTURE_SIZE, height, TEXTURE_SIZE, TEXTURE_SIZE)
    pub fn blit(&mut self, icon: IconTexture, x: i32, y: i32, v_start: i32, width: i32, height: i32) {
        self.blits.push(BlitOp { icon, x, y, v_start, width, height });
    }
}

// java: net.minecraft.client.renderer.RenderPipelines.GUI_TEXTURED (constant)
pub const RENDER_PIPELINES_GUI_TEXTURED: u8 = 0;

// java: net.minecraft.client.gui.components.Tooltip
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tooltip {
    pub text: Component,
}

impl Tooltip {
    // java: Tooltip.create(Component, Style)
    pub fn create(component: Component, _style: Option<()>) -> Tooltip {
        Tooltip { text: component }
    }
}

// java: net.minecraft.client.gui.components.WidgetTooltipHolder
#[derive(Debug, Default)]
pub struct WidgetTooltipHolder {
    pub tooltip: Option<Tooltip>,
    pub delay: Option<u64>,
    pub last_refresh: Option<RefreshCall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshCall {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub hovered: bool,
    pub focused: bool,
    pub rectangle: Option<ScreenRectangle>,
}

impl WidgetTooltipHolder {
    pub fn set(&mut self, tooltip: Option<Tooltip>) {
        self.tooltip = tooltip;
    }

    pub fn set_delay(&mut self, delay: Option<u64>) {
        self.delay = delay;
    }

    // java: refreshTooltipForNextRenderPass(GuiGraphicsExtractor, int, int, boolean, boolean, ScreenRectangle)
    pub fn refresh_tooltip_for_next_render_pass(
        &mut self,
        _graphics: &mut GuiGraphicsExtractor,
        mouse_x: i32,
        mouse_y: i32,
        hovered: bool,
        focused: bool,
        rectangle: Option<ScreenRectangle>,
    ) {
        self.last_refresh = Some(RefreshCall { mouse_x, mouse_y, hovered, focused, rectangle });
    }
}

// java: net.minecraft.client.gui.navigation.ScreenRectangle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScreenRectangle {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl ScreenRectangle {
    pub fn empty() -> ScreenRectangle {
        ScreenRectangle::default()
    }

    pub fn new(left: i32, top: i32, width: i32, height: i32) -> ScreenRectangle {
        ScreenRectangle { left, top, width, height }
    }
}

// java: net.minecraft.client.gui.narration.NarratedElementType + NarrationElementOutput
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarratedElementType {
    Title,
    Usage,
}

#[derive(Debug, Clone, Default)]
pub struct NarrationElementOutput {
    pub entries: Vec<(NarratedElementType, Component)>,
}

impl NarrationElementOutput {
    // java: output.add(NarratedElementType, Component)
    pub fn add(&mut self, element_type: NarratedElementType, component: Component) {
        self.entries.push((element_type, component));
    }
}

// java: net.minecraft.client.gui.navigation.FocusNavigationEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusNavigationEvent {
    Arrow { direction: FocusDirection },
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

// java: net.minecraft.client.gui.navigation.focus.FocusPath / ComponentPath
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentPath {
    Leaf,
}

// java: net.minecraft.client.gui.components.events.GuiEventListener inputs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub is_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseButtonEvent {
    pub button: i32,
}

// java: Minecraft.getInstance().getSoundManager() + AbstractWidget.playDownSound
pub fn play_down_sound() {
    let _ = HOOKS.get().map(|h| (h.play_down_sound_fn.lock().unwrap())());
}

// ----------------------------------------------------------- engine hooks

pub struct EngineHooks {
    // java: Services.PLATFORM.isModLoaded(modId)
    pub is_mod_loaded_fn: Mutex<Box<dyn Fn(&str) -> bool + Send>>,
    // java: AbstractWidget.playDownSound(SoundManager)
    pub play_down_sound_fn: Mutex<Box<dyn Fn() + Send>>,
}

impl Default for EngineHooks {
    fn default() -> Self {
        EngineHooks {
            is_mod_loaded_fn: Mutex::new(Box::new(|_| false)),
            play_down_sound_fn: Mutex::new(Box::new(|| {})),
        }
    }
}

pub static HOOKS: OnceLock<EngineHooks> = OnceLock::new();

pub fn set_hooks(hooks: EngineHooks) {
    let _ = HOOKS.set(hooks);
}
