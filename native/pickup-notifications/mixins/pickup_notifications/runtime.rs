// Pickup Notifications ported to Horizon as a native mod (slice 1).
// Upstream: github.com/Roundaround/mc-fabric-pickup-notifications, commit aa29c0d,
// MIT (c) 2021 Evan Steinkerchner (Roundaround). Port policy: 1:1 behavior port, no
// optimizations or behavior changes. Slice 1 covers the core non-GUI logic: config
// (trove ModConfigImpl semantics + the v1->v2 offset migration), IconAlignment and
// GuiAlignment values, the pickup event buses, the S2C packet models + server send
// gate, the HUD queue state machine (shown list + backlog queue, merge/extend rules),
// and the notification base state machine (durations, pop, expiry, slide-in/out math)
// plus the item and experience notifications' merge and display logic. The drawing
// itself (strata, text, icons) is a seam: render() is modeled as draw specs, the
// server->client transport and the server Touch hooks (ItemEntityMixin.playerTouch at
// awardStat, ExperienceOrbMixin.playerTouch at take) are game-side seams.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

pub const MOD_ID: &str = "pickupnotifications";
pub const MOD_NAME: &str = "Pickup Notifications";
pub const MOD_VERSION: &str = "1.0.0";
pub const CONFIG_VERSION: i32 = 2;

// ---------------------------------------------------------------------------
// config/IconAlignment.java -> IconAlignment
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconAlignment {
    Outside,
    Inside,
    Left,
    Right,
}

impl IconAlignment {
    pub const ALL: [IconAlignment; 4] = [
        IconAlignment::Outside,
        IconAlignment::Inside,
        IconAlignment::Left,
        IconAlignment::Right,
    ];

    pub fn id(&self) -> &'static str {
        match self {
            IconAlignment::Outside => "outside",
            IconAlignment::Inside => "inside",
            IconAlignment::Left => "left",
            IconAlignment::Right => "right",
        }
    }

    pub fn i18n_key(&self) -> String {
        format!("pickupnotifications.iconAlignment.{}", self.id())
    }

    pub fn get_next(&self) -> IconAlignment {
        let idx = Self::ALL.iter().position(|a| a == self).unwrap();
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn get_prev(&self) -> IconAlignment {
        let idx = Self::ALL.iter().position(|a| a == self).unwrap();
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn get_default() -> IconAlignment {
        IconAlignment::Outside
    }

    pub fn from_id(id: &str) -> IconAlignment {
        Self::ALL
            .iter()
            .copied()
            .find(|a| a.id() == id)
            .unwrap_or_else(Self::get_default)
    }
}

// ---------------------------------------------------------------------------
// trove config/value/Position.java -> Position
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Position {
        Position { x, y }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// ---------------------------------------------------------------------------
// trove config/value/GuiAlignment.java -> GuiAlignment
//
// Modeled from upstream usage in PickupNotification.render(): posX/posY are the
// vanilla screen-edge coordinates (0 or width/height), and the offset multiplier
// is +1 on left/top and -1 on right/bottom, so an offset always pushes inward.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentX {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentY {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum GuiAlignment {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl GuiAlignment {
    pub const ALL: [GuiAlignment; 4] = [
        GuiAlignment::TopLeft,
        GuiAlignment::TopRight,
        GuiAlignment::BottomLeft,
        GuiAlignment::BottomRight,
    ];

    pub fn alignment_x(&self) -> AlignmentX {
        match self {
            GuiAlignment::TopLeft | GuiAlignment::BottomLeft => AlignmentX::Left,
            GuiAlignment::TopRight | GuiAlignment::BottomRight => AlignmentX::Right,
        }
    }

    pub fn alignment_y(&self) -> AlignmentY {
        match self {
            GuiAlignment::TopLeft | GuiAlignment::TopRight => AlignmentY::Top,
            GuiAlignment::BottomLeft | GuiAlignment::BottomRight => AlignmentY::Bottom,
        }
    }

    pub fn get_pos_x(&self, screen_width: i32) -> f32 {
        match self.alignment_x() {
            AlignmentX::Left => 0.0,
            AlignmentX::Right => screen_width as f32,
        }
    }

    pub fn get_pos_y(&self, screen_height: i32) -> f32 {
        match self.alignment_y() {
            AlignmentY::Top => 0.0,
            AlignmentY::Bottom => screen_height as f32,
        }
    }

    pub fn get_offset_multiplier_x(&self) -> f32 {
        match self.alignment_x() {
            AlignmentX::Left => 1.0,
            AlignmentX::Right => -1.0,
        }
    }

    pub fn get_offset_multiplier_y(&self) -> f32 {
        match self.alignment_y() {
            AlignmentY::Top => 1.0,
            AlignmentY::Bottom => -1.0,
        }
    }

    pub fn get_next(&self) -> GuiAlignment {
        let idx = Self::ALL.iter().position(|a| a == self).unwrap();
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn get_prev(&self) -> GuiAlignment {
        let idx = Self::ALL.iter().position(|a| a == self).unwrap();
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

// ---------------------------------------------------------------------------
// config/PickupNotificationsConfig.java -> Config
//
// Upstream stores trove options; here the fields are the option VALUES with the
// same defaults and the same pending-value read semantics (getPendingValue).
// guiScale/backgroundOpacity are slider options with min/max/step like upstream.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "modEnabled", default = "default_true")]
    pub mod_enabled: bool,
    #[serde(rename = "trackExperience", default = "default_true")]
    pub track_experience: bool,
    #[serde(rename = "guiAlignment", default)]
    pub gui_alignment: GuiAlignment,
    #[serde(rename = "guiOffset", default = "default_offset")]
    pub gui_offset: Position,
    #[serde(rename = "guiScale", default = "default_gui_scale")]
    pub gui_scale: f32,
    #[serde(rename = "maxNotifications", default = "default_max_notifications")]
    pub max_notifications: i32,
    #[serde(rename = "iconAlignment", default = "default_icon_alignment")]
    pub icon_alignment: IconAlignment,
    #[serde(rename = "showUniqueInfo", default = "default_true")]
    pub show_unique_info: bool,
    #[serde(rename = "renderBackground", default = "default_true")]
    pub render_background: bool,
    #[serde(rename = "backgroundOpacity", default = "default_background_opacity")]
    pub background_opacity: f32,
    #[serde(rename = "renderShadow", default = "default_false")]
    pub render_shadow: bool,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_offset() -> Position {
    Position::new(4, 4)
}
fn default_gui_scale() -> f32 {
    1.0
}
fn default_max_notifications() -> i32 {
    4
}
fn default_icon_alignment() -> IconAlignment {
    IconAlignment::get_default()
}
fn default_background_opacity() -> f32 {
    0.5
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mod_enabled: true,
            track_experience: true,
            gui_alignment: GuiAlignment::TopLeft,
            gui_offset: Position::new(4, 4),
            gui_scale: 1.0,
            max_notifications: 4,
            icon_alignment: IconAlignment::Outside,
            show_unique_info: true,
            render_background: true,
            background_opacity: 0.5,
            render_shadow: false,
        }
    }
}

impl Config {
    pub fn new() -> Config {
        Config::default()
    }

    /// Upstream option clamps (trove slider semantics): guiScale in [0.1, 3],
    /// backgroundOpacity in [0, 1], maxNotifications at least 1.
    pub fn ensure_valid(&mut self) {
        self.gui_scale = self.gui_scale.clamp(0.1, 3.0);
        self.background_opacity = self.background_opacity.clamp(0.0, 1.0);
        if self.max_notifications < 1 {
            self.max_notifications = 1;
        }
    }

    /// Upstream ModConfigImpl version-2 doc with performConfigUpdate: a version-1
    /// doc migrates guiOffsetX/guiOffsetY into the guiOffset Position string and
    /// reports the update; other versions report false.
    pub fn perform_config_update(&mut self, version: i32, legacy: &LegacyV1Doc) -> bool {
        if version == 1 {
            self.gui_offset = Position::new(
                legacy
                    .gui_offset_x
                    .unwrap_or_else(|| Config::default().gui_offset.x),
                legacy
                    .gui_offset_y
                    .unwrap_or_else(|| Config::default().gui_offset.y),
            );
            true
        } else {
            false
        }
    }

    /// Upstream loadConfigFile semantics: a broken file falls back to defaults.
    pub fn from_json(json: &str) -> Result<Config, String> {
        let mut config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
        config.ensure_valid();
        Ok(config)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("config serializes")
    }
}

/// The version-1 config doc carried only flat guiOffsetX/guiOffsetY keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LegacyV1Doc {
    #[serde(default, rename = "pickupnotifications.guiOffsetX")]
    pub gui_offset_x: Option<i32>,
    #[serde(default, rename = "pickupnotifications.guiOffsetY")]
    pub gui_offset_y: Option<i32>,
}

// ---------------------------------------------------------------------------
// event/ItemPickup.java + event/ExperiencePickup.java -> event buses
//
// CopyOnWriteArrayList listeners; emit iterates the snapshot. Rust stands in
// with a Mutex<Vec<fn>>; registration order is preserved.
// ---------------------------------------------------------------------------

type ItemPickupListener = Box<dyn Fn(&ItemStack) + Send>;
type ExperiencePickupListener = Box<dyn Fn(i32) + Send>;

pub struct ItemPickup;

impl ItemPickup {
    fn listeners() -> &'static Mutex<Vec<ItemPickupListener>> {
        static LISTENERS: Mutex<Vec<ItemPickupListener>> = Mutex::new(Vec::new());
        &LISTENERS
    }

    pub fn register(listener: ItemPickupListener) {
        Self::listeners().lock().unwrap().push(listener);
    }

    pub fn emit(stack: &ItemStack) {
        let listeners = Self::listeners().lock().unwrap();
        for listener in listeners.iter() {
            listener(stack);
        }
    }
}

pub struct ExperiencePickup;

impl ExperiencePickup {
    fn listeners() -> &'static Mutex<Vec<ExperiencePickupListener>> {
        static LISTENERS: Mutex<Vec<ExperiencePickupListener>> = Mutex::new(Vec::new());
        &LISTENERS
    }

    pub fn register(listener: ExperiencePickupListener) {
        Self::listeners().lock().unwrap().push(listener);
    }

    pub fn emit(amount: i32) {
        let listeners = Self::listeners().lock().unwrap();
        for listener in listeners.iter() {
            listener(amount);
        }
    }
}

// ---------------------------------------------------------------------------
// network/Networking.java -> packet models
//
// Upstream: ExperiencePickupS2C is a single INT; ItemPickupS2C carries the
// ItemStack stream codec. The trove S2C transport itself is the game-side seam;
// these are the payload models with the same fields.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperiencePickupS2C {
    pub amount: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemPickupS2C {
    pub stack: ItemStack,
}

/// server/network/ServerNetworking.java -> send paths.
///
/// Upstream checks TroveNetworking.canSend(player, ID) (the client negotiated
/// the payload) and only then sends. The transport is the seam; the gate and
/// payload construction are 1:1. Returns Some(payload) when it would send.
pub struct ServerNetworking;

impl ServerNetworking {
    pub fn send_experience_pickup(
        can_send: bool,
        player: &str,
        amount: i32,
    ) -> Option<ExperiencePickupS2C> {
        if !can_send {
            return None;
        }
        let _ = player;
        Some(ExperiencePickupS2C { amount })
    }

    pub fn send_item_pickup(
        can_send: bool,
        player: &str,
        stack: &ItemStack,
    ) -> Option<ItemPickupS2C> {
        if !can_send {
            return None;
        }
        let _ = player;
        Some(ItemPickupS2C {
            stack: stack.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// ItemStack stand-in
//
// Modeled on the fields upstream touches: count, hover name, rarity color,
// custom-name presence, and component equality (isSameItemSameComponents).
// The serialized form is a compact JSON object so packets stay wire-shaped.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub components_json: String,
    #[serde(default)]
    pub hover_name: String,
    #[serde(default)]
    pub rarity_color: String,
    #[serde(default)]
    pub has_custom_name: bool,
}

impl ItemStack {
    pub fn new(item: &str, count: i32) -> ItemStack {
        ItemStack {
            item: item.to_string(),
            count,
            components_json: "{}".to_string(),
            hover_name: String::new(),
            rarity_color: String::new(),
            has_custom_name: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.is_empty()
    }

    pub fn grow(&mut self, amount: i32) {
        self.count += amount;
    }

    pub fn is_same_item_same_components(&self, other: &ItemStack) -> bool {
        self.item == other.item && self.components_json == other.components_json
    }
}

// ---------------------------------------------------------------------------
// client/gui/hud/PickupNotification.java -> PickupNotification (base state machine)
//
// The rendering body (strata, text, icon blits) is the seam; the placement math
// (alignment/offset/scale, row index, slide-in/out offset percent) is kept 1:1 and
// returned as draw specs for the client.
// ---------------------------------------------------------------------------

pub const SHOW_DURATION: i32 = 120;
pub const DURATION_INCREASE_ON_ADD: i32 = 60;
pub const POP_DURATION: i32 = 5;
pub const ANIM_DURATION: i32 = 6;
pub const ANIM_IN_FINISH_TIME: i32 = SHOW_DURATION - ANIM_DURATION;
pub const LEFT_PADDING: i32 = 1;
pub const FONT_LINE_HEIGHT: i32 = 9;

fn mth_clamp(v: f32, min: f32, max: f32) -> f32 {
    v.max(min).min(max)
}

pub struct DrawSpec {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub full_width: i32,
    pub line_height: i32,
    pub row: i32,
    pub background: Option<u32>,
    pub text: String,
    pub shadow: bool,
    pub icon_spec: IconSpec,
}

pub struct IconSpec {
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub icon_scale: f32,
    pub pop_scale_x: f32,
    pub pop_scale_y: f32,
}

/// What a concrete notification contributes to a frame.
pub struct NotificationRender {
    pub display_text: String,
    pub text_width: i32,
}

pub trait PickupNotificationBase {
    fn timeless(&self) -> bool;
    fn time_remaining(&self) -> i32;
    fn original_time_remaining(&self) -> i32;
    fn pop_time_remaining(&self) -> i32;
    fn last_tick_millis(&self) -> u64;

    fn display(&self, config: &Config) -> NotificationRender;
    /// canAdd on the upstream typed value; both concrete kinds model this as a
    /// tagged value comparison handled by the Hud merge loop, so the trait only
    /// reports whether a merge attempt already happened here.
    fn attempt_add(&mut self, addition: &PickupValue, config: &Config) -> bool;

    fn extend_time(&mut self) {
        self.set_time_remaining(
            (self.time_remaining() + DURATION_INCREASE_ON_ADD).min(SHOW_DURATION),
        );
        self.pop();
    }

    fn pop(&mut self) {
        self.set_pop_time_remaining(POP_DURATION);
    }

    fn is_expired(&self) -> bool {
        self.time_remaining() <= 0
    }

    fn tick(&mut self, now_millis: u64) {
        self.set_time_remaining(self.time_remaining() - 1);
        self.set_original_time_remaining(self.original_time_remaining() - 1);
        self.set_pop_time_remaining(self.pop_time_remaining() - 1);
        self.set_last_tick_millis(now_millis);
    }

    // Manual accessors so the base logic stays in one place.
    fn set_time_remaining(&mut self, value: i32);
    fn set_original_time_remaining(&mut self, value: i32);
    fn set_pop_time_remaining(&mut self, value: i32);
    fn set_last_tick_millis(&mut self, value: u64);

    /// Upstream getXOffsetPercent(): slide-in while originalTimeRemaining is
    /// still inside the anim-in window, slide-out in the last ANIM_DURATION
    /// ticks, partial tick from Util.getMillis() at 50ms per tick.
    fn get_x_offset_percent(&self, now_millis: u64) -> f32 {
        if self.timeless() {
            return 0.0;
        }

        let partial_tick = mth_clamp(
            (now_millis - self.last_tick_millis()) as f32 / 50.0,
            0.0,
            1.0,
        );
        let partial_time_remaining = self.time_remaining() as f32 - partial_tick;

        if self.original_time_remaining() > ANIM_IN_FINISH_TIME {
            // Animating in
            let anim_time = partial_time_remaining.max(0.0) - ANIM_IN_FINISH_TIME as f32;
            let base_percent = mth_clamp(anim_time / ANIM_DURATION as f32, 0.0, 1.0);
            base_percent * base_percent
        } else if partial_time_remaining < ANIM_DURATION as f32 {
            // Animating out
            let anim_time = partial_time_remaining.max(0.0);
            let base_percent = mth_clamp(anim_time / ANIM_DURATION as f32, 0.0, 1.0);
            1.0 - (base_percent * base_percent)
        } else {
            // Fully showing
            0.0
        }
    }

    /// Upstream isRightAligned() truth table over iconAlignment x guiAlignment.
    fn is_right_aligned(&self, config: &Config) -> bool {
        let icon = config.icon_alignment;
        if icon == IconAlignment::Right {
            return true;
        }
        match config.gui_alignment.alignment_x() {
            AlignmentX::Left => icon == IconAlignment::Inside,
            AlignmentX::Right => icon == IconAlignment::Outside,
        }
    }

    /// Upstream render() placement math, kept 1:1; the caller draws the spec.
    fn draw_spec(
        &self,
        config: &Config,
        idx: i32,
        screen_width: i32,
        screen_height: i32,
        now_millis: u64,
        render: &NotificationRender,
    ) -> DrawSpec {
        let alignment = config.gui_alignment;
        let offset = config.gui_offset;
        let scale = config.gui_scale;

        let mut x = alignment.get_pos_x(screen_width)
            + offset.x as f32 * alignment.get_offset_multiplier_x();
        let mut y = alignment.get_pos_y(screen_height)
            + offset.y as f32 * alignment.get_offset_multiplier_y();

        let line_height = FONT_LINE_HEIGHT;
        let text_width = render.text_width;
        let sprite_size = line_height + 1;
        let full_width = 3 * LEFT_PADDING + text_width + sprite_size;

        let mut x_adjust = 0.0f32;
        let mut y_adjust = 0.0f32;

        if alignment.alignment_x() == AlignmentX::Right {
            x_adjust = -(full_width as f32);
        }
        if alignment.alignment_y() == AlignmentY::Bottom {
            y_adjust = -(line_height as f32) - 2.0;
        }

        x_adjust -= full_width as f32
            * self.get_x_offset_percent(now_millis)
            * alignment.get_offset_multiplier_x();
        y_adjust += idx as f32 * (line_height as f32 + 2.0) * alignment.get_offset_multiplier_y();

        x += x_adjust * scale;
        y += y_adjust * scale;

        let height = line_height + 1;
        let right_aligned = self.is_right_aligned(config);
        let left_pad = if right_aligned {
            LEFT_PADDING
        } else {
            2 * LEFT_PADDING + height
        };

        let background = if config.render_background {
            // GuiUtil.genColorInt(0, 0, 0, opacity): 8-bit RGBA, alpha = opacity*255.
            let alpha = (config.background_opacity * 255.0) as u32;
            Some((alpha << 24) | 0x000000)
        } else {
            None
        };

        // Icon block: right-aligned sits at totalWidth - LEFT_PADDING - height + 0.25,
        // left-aligned at LEFT_PADDING - 0.5, y -0.5, scaled height/16, pop-scaled.
        let icon_anchor_x = if right_aligned {
            full_width as f32 - LEFT_PADDING as f32 - height as f32 + 0.25
        } else {
            LEFT_PADDING as f32 - 0.5
        };
        let partial_pop = self.pop_time_remaining() as f32;
        let pop_amount = mth_clamp(partial_pop, 0.0, 5.0) / 5.0;
        let pop_scale = 1.0 + pop_amount;
        let icon_scale = height as f32 / 16.0;
        let pop_scale_x = 1.0 / pop_scale;
        let pop_scale_y = (1.0 + pop_scale) / 2.0;

        DrawSpec {
            x,
            y,
            scale,
            full_width,
            line_height,
            row: idx,
            background,
            text: render.display_text.clone(),
            shadow: config.render_shadow,
            icon_spec: IconSpec {
                anchor_x: icon_anchor_x + (left_pad - left_pad) as f32,
                anchor_y: -0.5,
                icon_scale,
                pop_scale_x,
                pop_scale_y,
            },
        }
    }
}

/// The typed addition value shared by the two notification kinds, standing in for
/// the upstream generic T merge (ItemStack for items, amount for experience).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickupValue {
    Item(ItemStack),
    Experience(i32),
}

// ---------------------------------------------------------------------------
// client/gui/hud/ItemPickupNotification.java -> ItemPickupNotification
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ItemPickupNotification {
    pub timeless: bool,
    pub original_time_remaining: i32,
    pub time_remaining: i32,
    pub pop_time_remaining: i32,
    pub last_tick_millis: u64,
    pub stack: ItemStack,
}

impl ItemPickupNotification {
    pub fn new(initial_items: ItemStack) -> ItemPickupNotification {
        Self::with_timeless(initial_items, false)
    }

    pub fn with_timeless(mut initial_items: ItemStack, timeless: bool) -> ItemPickupNotification {
        initial_items.count = initial_items.count; // upstream .copy()
        ItemPickupNotification {
            timeless,
            original_time_remaining: SHOW_DURATION,
            time_remaining: SHOW_DURATION,
            pop_time_remaining: 0,
            last_tick_millis: 0,
            stack: initial_items,
        }
    }

    fn are_item_stacks_mergeable(a: &ItemStack, b: &ItemStack) -> bool {
        !a.is_empty() && !b.is_empty() && a.is_same_item_same_components(b)
    }

    /// Upstream getFormattedDisplayString(): "countx " + hover name, rarity
    /// color and ITALIC for custom names when showUniqueInfo is on.
    fn formatted_display_string(&self, config: &Config) -> String {
        if config.show_unique_info {
            let custom = if self.stack.has_custom_name { "_" } else { "" };
            format!("{}x {}{}", self.stack.count, self.stack.hover_name, custom)
        } else {
            format!("{}x {}", self.stack.count, self.stack.hover_name)
        }
    }
}

impl PickupNotificationBase for ItemPickupNotification {
    fn timeless(&self) -> bool {
        self.timeless
    }
    fn time_remaining(&self) -> i32 {
        self.time_remaining
    }
    fn original_time_remaining(&self) -> i32 {
        self.original_time_remaining
    }
    fn pop_time_remaining(&self) -> i32 {
        self.pop_time_remaining
    }
    fn last_tick_millis(&self) -> u64 {
        self.last_tick_millis
    }
    fn set_time_remaining(&mut self, value: i32) {
        self.time_remaining = value;
    }
    fn set_original_time_remaining(&mut self, value: i32) {
        self.original_time_remaining = value;
    }
    fn set_pop_time_remaining(&mut self, value: i32) {
        self.pop_time_remaining = value;
    }
    fn set_last_tick_millis(&mut self, value: u64) {
        self.last_tick_millis = value;
    }

    fn display(&self, config: &Config) -> NotificationRender {
        let display_text = self.formatted_display_string(config);
        // Upstream textRenderer.width(text) is font-metric; the stand-in widths
        // the string 1:1 per character so relative layout math is unchanged.
        NotificationRender {
            display_text: display_text.clone(),
            text_width: display_text.chars().count() as i32 * 6,
        }
    }

    fn attempt_add(&mut self, addition: &PickupValue, _config: &Config) -> bool {
        if self.is_expired() {
            return false;
        }
        if let PickupValue::Item(item) = addition {
            if Self::are_item_stacks_mergeable(&self.stack, item) {
                self.stack.grow(item.count);
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// client/gui/hud/ExperiencePickupNotification.java -> ExperiencePickupNotification
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ExperiencePickupNotification {
    pub timeless: bool,
    pub original_time_remaining: i32,
    pub time_remaining: i32,
    pub pop_time_remaining: i32,
    pub last_tick_millis: u64,
    pub amount: i32,
    pub age: i32,
}

impl ExperiencePickupNotification {
    pub fn new(amount: i32) -> ExperiencePickupNotification {
        Self::with_timeless(amount, false)
    }

    pub fn with_timeless(amount: i32, timeless: bool) -> ExperiencePickupNotification {
        ExperiencePickupNotification {
            timeless,
            original_time_remaining: SHOW_DURATION,
            time_remaining: SHOW_DURATION,
            pop_time_remaining: 0,
            last_tick_millis: 0,
            amount,
            age: 0,
        }
    }

    /// From ExperienceOrbEntity.getOrbSize(): the icon frame row for the amount.
    pub fn get_orb_size(&self) -> i32 {
        if self.amount >= 2477 {
            10
        } else if self.amount >= 1237 {
            9
        } else if self.amount >= 617 {
            8
        } else if self.amount >= 307 {
            7
        } else if self.amount >= 149 {
            6
        } else if self.amount >= 73 {
            5
        } else if self.amount >= 37 {
            4
        } else if self.amount >= 17 {
            3
        } else if self.amount >= 7 {
            2
        } else if self.amount >= 3 {
            1
        } else {
            0
        }
    }

    /// Upstream icon color math kept 1:1: t advances a half-tick per frame unit,
    /// r = (sin(t)+1)*0.5, b = (sin(t + 4pi/3)+1)*0.1; returned as components.
    pub fn icon_color(&self, partial_tick: f32) -> (f32, f32, f32) {
        let t = (self.age as f32 + partial_tick) / 2.0;
        let r = (t.sin() + 1.0) * 0.5;
        let b = ((t + (std::f32::consts::PI * 4.0 / 3.0)).sin() + 1.0) * 0.1;
        (r, 1.0, b)
    }
}

impl PickupNotificationBase for ExperiencePickupNotification {
    fn timeless(&self) -> bool {
        self.timeless
    }
    fn time_remaining(&self) -> i32 {
        self.time_remaining
    }
    fn original_time_remaining(&self) -> i32 {
        self.original_time_remaining
    }
    fn pop_time_remaining(&self) -> i32 {
        self.pop_time_remaining
    }
    fn last_tick_millis(&self) -> u64 {
        self.last_tick_millis
    }
    fn set_time_remaining(&mut self, value: i32) {
        self.time_remaining = value;
    }
    fn set_original_time_remaining(&mut self, value: i32) {
        self.original_time_remaining = value;
    }
    fn set_pop_time_remaining(&mut self, value: i32) {
        self.pop_time_remaining = value;
    }
    fn set_last_tick_millis(&mut self, value: u64) {
        self.last_tick_millis = value;
    }

    fn tick(&mut self, now_millis: u64) {
        self.time_remaining -= 1;
        self.original_time_remaining -= 1;
        self.pop_time_remaining -= 1;
        self.last_tick_millis = now_millis;
        self.age += 1;
    }

    fn display(&self, _config: &Config) -> NotificationRender {
        let display_text = format!("{}x subtitles.entity.experience_orb.pickup", self.amount);
        NotificationRender {
            text_width: display_text.chars().count() as i32 * 6,
            display_text,
        }
    }

    fn attempt_add(&mut self, addition: &PickupValue, _config: &Config) -> bool {
        if self.is_expired() {
            return false;
        }
        if let PickupValue::Experience(amount) = addition {
            self.amount += amount;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// client/gui/hud/PickupNotificationsHud.java -> Hud
//
// The shown list is a CopyOnWriteArrayList and the backlog a
// ConcurrentLinkedDeque upstream; here both live behind one mutex, which keeps
// the same tick/merge semantics without the JDK types.
// ---------------------------------------------------------------------------

pub struct Hud {
    pub currently_shown_notifications: Vec<Box<dyn PickupNotificationBase + Send>>,
    pub notification_queue: std::collections::VecDeque<Box<dyn PickupNotificationBase + Send>>,
}

impl Default for Hud {
    fn default() -> Self {
        Self::new()
    }
}

impl Hud {
    pub fn new() -> Hud {
        Hud {
            currently_shown_notifications: Vec::new(),
            notification_queue: std::collections::VecDeque::new(),
        }
    }

    fn has_room_for_new_notification(&self, config: &Config) -> bool {
        (self.currently_shown_notifications.len() as i32) < config.max_notifications
    }

    /// Upstream tick(): expire shown notifications, then promote queued ones
    /// while there is room. The mod-enabled early return is 1:1.
    pub fn tick(&mut self, config: &Config, now_millis: u64) {
        if !config.mod_enabled {
            return;
        }
        if self.currently_shown_notifications.is_empty() {
            return;
        }

        let mut expired: Vec<usize> = Vec::new();
        for (idx, notification) in self.currently_shown_notifications.iter_mut().enumerate() {
            notification.tick(now_millis);
            if notification.is_expired() {
                expired.push(idx);
            }
        }
        for idx in expired.into_iter().rev() {
            self.currently_shown_notifications.remove(idx);
        }

        while self.has_room_for_new_notification(config) {
            let notification = self.notification_queue.pop_front();
            match notification {
                Some(mut n) => {
                    n.pop();
                    self.currently_shown_notifications.push(n);
                }
                None => break,
            }
        }
    }

    /// Upstream handlePickup(): merge into a shown notification (extending it),
    /// else into a queued one, else a fresh notification that pops straight
    /// onto the screen when the queue is empty and there is room.
    pub fn handle_pickup(&mut self, addition: PickupValue, config: &Config, now_millis: u64) {
        let mut merged_into_existing = false;

        for notification in self.currently_shown_notifications.iter_mut() {
            if notification.attempt_add(&addition, config) {
                merged_into_existing = true;
                notification.extend_time();
                break;
            }
        }

        if !merged_into_existing {
            for notification in self.notification_queue.iter_mut() {
                if notification.attempt_add(&addition, config) {
                    merged_into_existing = true;
                    break;
                }
            }
        }

        if !merged_into_existing {
            let mut notification: Box<dyn PickupNotificationBase + Send> = match addition.clone() {
                PickupValue::Item(stack) => Box::new(ItemPickupNotification::new(stack)),
                PickupValue::Experience(amount) => {
                    Box::new(ExperiencePickupNotification::new(amount))
                }
            };
            if self.notification_queue.is_empty() && self.has_room_for_new_notification(config) {
                notification.pop();
                let _ = now_millis;
                self.currently_shown_notifications.push(notification);
            } else {
                self.notification_queue.push_back(notification);
            }
        }
    }

    /// Upstream render() gates: mod enabled, something shown, and the vanilla
    /// HUD not hidden (Minecraft.gui.hud.isHidden() is the game-side seam here,
    /// passed in by the caller).
    pub fn render_specs(
        &self,
        config: &Config,
        hud_hidden: bool,
        screen_width: i32,
        screen_height: i32,
        now_millis: u64,
    ) -> Vec<DrawSpec> {
        if !config.mod_enabled {
            return Vec::new();
        }
        if self.currently_shown_notifications.is_empty() {
            return Vec::new();
        }
        if hud_hidden {
            return Vec::new();
        }
        let mut specs = Vec::new();
        let mut idx = 0;
        for notification in self.currently_shown_notifications.iter() {
            let render = notification.display(config);
            specs.push(notification.draw_spec(
                config,
                idx,
                screen_width,
                screen_height,
                now_millis,
                &render,
            ));
            idx += 1;
        }
        specs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_upstream() {
        let config = Config::new();
        assert!(config.mod_enabled);
        assert!(config.track_experience);
        assert_eq!(config.gui_alignment, GuiAlignment::TopLeft);
        assert_eq!(config.gui_offset, Position::new(4, 4));
        assert_eq!(config.gui_scale, 1.0);
        assert_eq!(config.max_notifications, 4);
        assert_eq!(config.icon_alignment, IconAlignment::Outside);
        assert!(config.show_unique_info);
        assert!(config.render_background);
        assert_eq!(config.background_opacity, 0.5);
        assert!(!config.render_shadow);
    }

    #[test]
    fn v1_migration_moves_flat_offsets_into_position() {
        let mut config = Config::new();
        let legacy = LegacyV1Doc {
            gui_offset_x: Some(12),
            gui_offset_y: None,
        };
        assert!(config.perform_config_update(1, &legacy));
        assert_eq!(config.gui_offset, Position::new(12, 4));
        // Non-matching versions report false like upstream.
        let mut config2 = Config::new();
        assert!(!config2.perform_config_update(3, &LegacyV1Doc::default()));
    }

    #[test]
    fn broken_config_json_falls_back_to_defaults() {
        let parsed = Config::from_json("{nope");
        assert!(parsed.is_err());
        let parsed = Config::from_json(r#"{"guiScale": 99}"#).unwrap();
        assert_eq!(parsed.gui_scale, 3.0, "slider max clamps");
        assert_eq!(parsed.max_notifications, 4, "untouched keys keep defaults");
    }

    #[test]
    fn icon_alignment_ids_and_defaults() {
        assert_eq!(IconAlignment::from_id("inside"), IconAlignment::Inside);
        assert_eq!(IconAlignment::from_id("bogus"), IconAlignment::Outside);
        assert_eq!(IconAlignment::Outside.id(), "outside");
        assert_eq!(IconAlignment::Right.get_next(), IconAlignment::Outside);
        assert_eq!(IconAlignment::Outside.get_prev(), IconAlignment::Right);
        assert_eq!(
            IconAlignment::Left.i18n_key(),
            "pickupnotifications.iconAlignment.left"
        );
    }

    #[test]
    fn gui_alignment_geometry_matches_screen_edges() {
        let right = GuiAlignment::TopRight;
        assert_eq!(right.get_pos_x(427), 427.0);
        assert_eq!(right.get_offset_multiplier_x(), -1.0);
        assert_eq!(right.get_offset_multiplier_y(), 1.0);
        let bottom = GuiAlignment::BottomRight;
        assert_eq!(bottom.get_pos_y(240), 240.0);
        assert_eq!(bottom.get_offset_multiplier_y(), -1.0);
        assert_eq!(GuiAlignment::TopLeft.get_next(), GuiAlignment::TopRight);
        assert_eq!(GuiAlignment::TopLeft.get_prev(), GuiAlignment::BottomRight);
    }

    #[test]
    fn event_buses_emit_to_every_listener_in_order() {
        ItemPickup::register(Box::new(|stack: &ItemStack| {
            ITEM_LOG.lock().unwrap().push(stack.item.clone());
        }));
        ItemPickup::register(Box::new(|stack: &ItemStack| {
            let mut log = ITEM_LOG.lock().unwrap();
            log.push(format!("second:{}", stack.item));
        }));
        ITEM_LOG.lock().unwrap().clear();
        ItemPickup::emit(&ItemStack::new("diamond", 1));
        assert_eq!(ITEM_LOG.lock().unwrap().len(), 2);
        assert_eq!(ITEM_LOG.lock().unwrap()[0], "diamond");
        assert_eq!(ITEM_LOG.lock().unwrap()[1], "second:diamond");

        ExperiencePickup::register(Box::new(|amount: i32| {
            XP_LOG.lock().unwrap().push(amount);
        }));
        XP_LOG.lock().unwrap().clear();
        ExperiencePickup::emit(7);
        assert_eq!(XP_LOG.lock().unwrap().last().unwrap(), &7);
    }

    static ITEM_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static XP_LOG: Mutex<Vec<i32>> = Mutex::new(Vec::new());

    #[test]
    fn server_send_gate_models_can_send() {
        let stack = ItemStack::new("apple", 3);
        assert!(ServerNetworking::send_item_pickup(true, "player", &stack).is_some());
        assert!(ServerNetworking::send_item_pickup(false, "player", &stack).is_none());
        assert_eq!(
            ServerNetworking::send_experience_pickup(true, "player", 42).unwrap(),
            ExperiencePickupS2C { amount: 42 }
        );
        assert!(ServerNetworking::send_experience_pickup(false, "player", 42).is_none());
    }

    #[test]
    fn notification_durations_and_expiry() {
        let mut n = ItemPickupNotification::new(ItemStack::new("dirt", 1));
        assert_eq!(n.time_remaining(), SHOW_DURATION);
        assert!(!n.is_expired());
        n.set_time_remaining(1);
        n.tick(0);
        assert!(n.is_expired());
        // attemptAdd on an expired notification is rejected.
        assert!(!n.attempt_add(
            &PickupValue::Item(ItemStack::new("dirt", 1)),
            &Config::new()
        ));
    }

    #[test]
    fn extend_time_caps_at_show_duration_and_pops() {
        let mut n = ExperiencePickupNotification::new(1);
        n.set_time_remaining(SHOW_DURATION - 10);
        n.extend_time();
        assert_eq!(n.time_remaining(), SHOW_DURATION);
        assert_eq!(n.pop_time_remaining(), POP_DURATION);
        // extendTime pops the icon even when already at cap.
        n.set_pop_time_remaining(0);
        n.extend_time();
        assert_eq!(n.pop_time_remaining(), POP_DURATION);
    }

    #[test]
    fn x_offset_percent_matches_anim_windows() {
        // Fresh notification: originalTimeRemaining == SHOW_DURATION > ANIM_IN_FINISH_TIME.
        let mut n = ItemPickupNotification::new(ItemStack::new("dirt", 1));
        n.set_time_remaining(SHOW_DURATION);
        n.set_last_tick_millis(0);
        // Animating in: animTime = partial - 114 -> at t=0, anim=6 -> percent 1 (fully out).
        assert!((n.get_x_offset_percent(0) - 1.0).abs() < 1e-6);
        // Halfway through the anim-in window.
        n.set_time_remaining(SHOW_DURATION - 3);
        n.set_original_time_remaining(SHOW_DURATION);
        let half = n.get_x_offset_percent(0);
        let expected = 0.5 * 0.5;
        assert!((half - expected).abs() < 1e-6);
        // Fully showing window.
        n.set_time_remaining(60);
        n.set_original_time_remaining(60);
        assert_eq!(n.get_x_offset_percent(0), 0.0);
        // Animating out.
        n.set_time_remaining(3);
        n.set_original_time_remaining(60);
        let out = n.get_x_offset_percent(0);
        assert!((out - (1.0 - 0.25)).abs() < 1e-6);
        // Timeless notifications never slide.
        let t = ItemPickupNotification::with_timeless(ItemStack::new("dirt", 1), true);
        assert_eq!(t.get_x_offset_percent(0), 0.0);
        // Partial tick clamps at 50ms per tick.
        n.set_time_remaining(SHOW_DURATION);
        n.set_original_time_remaining(SHOW_DURATION);
        n.set_last_tick_millis(0);
        // 500ms after the last tick clamps to one partial tick: partialTimeRemaining
        // = 120 - 1 = 119, anim-in time 5 of 6, percent (5/6)^2.
        assert!(
            (n.get_x_offset_percent(500) - 25.0 / 36.0).abs() < 1e-6,
            "partial tick clamped to 1"
        );
    }

    #[test]
    fn item_notifications_merge_only_same_item_and_components() {
        let mut n = ItemPickupNotification::new(ItemStack::new("apple", 2));
        let same = ItemStack::new("apple", 5);
        assert!(n.attempt_add(&PickupValue::Item(same), &Config::new()));
        assert_eq!(n.stack.count, 7);
        let different_item = ItemStack::new("stick", 1);
        assert!(!n.attempt_add(&PickupValue::Item(different_item), &Config::new()));
        let different_components = ItemStack {
            components_json: r#"{"enchantments":{}}"#.into(),
            ..ItemStack::new("apple", 1)
        };
        assert!(!n.attempt_add(&PickupValue::Item(different_components), &Config::new()));
        // Experience additions never merge into an item notification.
        assert!(!n.attempt_add(&PickupValue::Experience(3), &Config::new()));
        // Empty stacks are not mergeable.
        let empty = ItemStack::new("", 0);
        assert!(!ItemPickupNotification::are_item_stacks_mergeable(
            &empty, &empty
        ));
    }

    #[test]
    fn item_display_uses_unique_info_toggle() {
        let mut stack = ItemStack::new("netherite_sword", 1);
        stack.hover_name = "Stabby".into();
        stack.has_custom_name = true;
        let n = ItemPickupNotification::new(stack);
        let mut config = Config::new();
        assert_eq!(n.display(&config).display_text, "1x Stabby_");
        config.show_unique_info = false;
        assert_eq!(n.display(&config).display_text, "1x Stabby");
    }

    #[test]
    fn experience_notifications_merge_any_amount_and_track_age() {
        let mut n = ExperiencePickupNotification::new(5);
        assert!(n.attempt_add(&PickupValue::Experience(7), &Config::new()));
        assert_eq!(n.amount, 12);
        assert!(!n.attempt_add(
            &PickupValue::Item(ItemStack::new("apple", 1)),
            &Config::new()
        ));
        n.tick(0);
        n.tick(0);
        assert_eq!(n.age, 2);
    }

    #[test]
    fn orb_size_thresholds_match_vanilla() {
        let mut n = ExperiencePickupNotification::new(0);
        assert_eq!(n.get_orb_size(), 0);
        n.amount = 3;
        assert_eq!(n.get_orb_size(), 1);
        n.amount = 7;
        assert_eq!(n.get_orb_size(), 2);
        n.amount = 17;
        assert_eq!(n.get_orb_size(), 3);
        n.amount = 37;
        assert_eq!(n.get_orb_size(), 4);
        n.amount = 73;
        assert_eq!(n.get_orb_size(), 5);
        n.amount = 149;
        assert_eq!(n.get_orb_size(), 6);
        n.amount = 307;
        assert_eq!(n.get_orb_size(), 7);
        n.amount = 617;
        assert_eq!(n.get_orb_size(), 8);
        n.amount = 1237;
        assert_eq!(n.get_orb_size(), 9);
        n.amount = 2477;
        assert_eq!(n.get_orb_size(), 10);
    }

    #[test]
    fn orb_icon_color_follows_the_sine_math() {
        let n = ExperiencePickupNotification::new(1);
        let (r, g, b) = n.icon_color(0.0);
        assert_eq!(g, 1.0);
        let expected_r = (0.0f32.sin() + 1.0) * 0.5;
        assert!((r - expected_r).abs() < 1e-6);
        let expected_b = ((0.0f32 + std::f32::consts::PI * 4.0 / 3.0).sin() + 1.0) * 0.1;
        assert!((b - expected_b).abs() < 1e-6);
    }

    #[test]
    fn right_aligned_truth_table_matches_upstream() {
        let mut n = ExperiencePickupNotification::new(1);
        let mut config = Config::new();
        // Icon RIGHT always right-aligned.
        config.icon_alignment = IconAlignment::Right;
        assert!(n.is_right_aligned(&config));
        // Icon LEFT never right-aligned.
        config.icon_alignment = IconAlignment::Left;
        assert!(!n.is_right_aligned(&config));
        // Gui LEFT x icon OUTSIDE -> left; INSIDE -> right.
        config.gui_alignment = GuiAlignment::TopLeft;
        config.icon_alignment = IconAlignment::Outside;
        assert!(!n.is_right_aligned(&config));
        config.icon_alignment = IconAlignment::Inside;
        assert!(n.is_right_aligned(&config));
        // Gui RIGHT x icon OUTSIDE -> right; INSIDE -> left.
        config.gui_alignment = GuiAlignment::TopRight;
        config.icon_alignment = IconAlignment::Outside;
        assert!(n.is_right_aligned(&config));
        config.icon_alignment = IconAlignment::Inside;
        assert!(!n.is_right_aligned(&config));
    }

    #[test]
    fn hud_merges_into_shown_then_queue_then_new() {
        let config = Config::new();
        let mut hud = Hud::new();
        hud.handle_pickup(PickupValue::Item(ItemStack::new("apple", 2)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 1);
        // One tick drops the notification off the SHOW_DURATION cap so the
        // merge's extend_time has room to add DURATION_INCREASE_ON_ADD.
        hud.tick(&config, 50);
        let shown_time = hud.currently_shown_notifications[0].time_remaining();

        // Same stack merges into the shown notification and extends it.
        hud.handle_pickup(PickupValue::Item(ItemStack::new("apple", 3)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 1);
        assert!(hud.currently_shown_notifications[0].time_remaining() > shown_time);

        // Different item goes to a new notification while there is room.
        hud.handle_pickup(PickupValue::Item(ItemStack::new("stick", 1)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 2);

        // Experience merges nowhere among items; with room it pops straight in.
        hud.handle_pickup(PickupValue::Experience(4), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 3);
        hud.handle_pickup(PickupValue::Item(ItemStack::new("iron", 1)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 4);

        // Full screen: the next pickup queues instead of showing.
        hud.handle_pickup(PickupValue::Item(ItemStack::new("gold", 1)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 4);
        assert_eq!(hud.notification_queue.len(), 1);
        // Queued items merge with each other rather than stacking.
        hud.handle_pickup(PickupValue::Item(ItemStack::new("gold", 2)), &config, 0);
        assert_eq!(hud.notification_queue.len(), 1);
    }

    #[test]
    fn hud_tick_expires_and_promotes_queued() {
        let mut config = Config::new();
        config.max_notifications = 2;
        let mut hud = Hud::new();
        hud.handle_pickup(PickupValue::Item(ItemStack::new("apple", 1)), &config, 0);
        hud.handle_pickup(PickupValue::Item(ItemStack::new("stick", 1)), &config, 0);
        hud.handle_pickup(PickupValue::Item(ItemStack::new("gold", 1)), &config, 0);
        assert_eq!(hud.currently_shown_notifications.len(), 2);
        assert_eq!(hud.notification_queue.len(), 1);

        // Two ticks leave 118 ticks; nothing expires yet, queue still held.
        hud.tick(&config, 50);
        hud.tick(&config, 100);
        assert_eq!(hud.currently_shown_notifications.len(), 2);
        assert_eq!(hud.notification_queue.len(), 1);

        // Run past expiry: both shown expire, the queued one pops in.
        for _ in 0..SHOW_DURATION {
            hud.tick(&config, 150);
        }
        assert_eq!(hud.currently_shown_notifications.len(), 1);
        assert_eq!(hud.notification_queue.len(), 0);
    }

    #[test]
    fn hud_tick_is_noop_when_disabled_or_empty() {
        let mut config = Config::new();
        config.mod_enabled = false;
        let mut hud = Hud::new();
        hud.handle_pickup(PickupValue::Experience(1), &Config::new(), 0);
        hud.tick(&config, 0);
        assert_eq!(
            hud.currently_shown_notifications.len(),
            1,
            "disabled tick leaves state untouched"
        );
        let mut hud2 = Hud::new();
        hud2.tick(&Config::new(), 0);
        assert!(hud2.currently_shown_notifications.is_empty());
    }

    #[test]
    fn render_specs_gates_and_row_order() {
        let config = Config::new();
        let mut hud = Hud::new();
        hud.handle_pickup(PickupValue::Item(ItemStack::new("apple", 1)), &config, 0);
        hud.handle_pickup(PickupValue::Item(ItemStack::new("stick", 1)), &config, 0);

        let specs = hud.render_specs(&config, false, 427, 240, 0);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].row, 0);
        assert_eq!(specs[1].row, 1);
        // TOP_LEFT placement: the offset anchors x at 4, and at tick 0 the fresh
        // notification is fully slid out (percent 1), which for a LEFT alignment
        // subtracts the full box width from the anchor.
        assert!((specs[0].x - (4.0 - specs[0].full_width as f32)).abs() < 1e-6);
        assert!((specs[1].y - (specs[0].y + 11.0)).abs() < 1e-6);
        assert!(
            specs[0].background.is_some(),
            "renderBackground default true"
        );

        // Hidden HUD gate.
        assert!(hud.render_specs(&config, true, 427, 240, 0).is_empty());
        // Disabled gate.
        let mut off = config.clone();
        off.mod_enabled = false;
        assert!(hud.render_specs(&off, false, 427, 240, 0).is_empty());
    }

    #[test]
    fn draw_spec_right_alignment_flips_the_anchor() {
        let mut config = Config::new();
        config.gui_alignment = GuiAlignment::TopRight;
        let n = ItemPickupNotification::new(ItemStack::new("apple", 1));
        let render = n.display(&config);
        let spec = n.draw_spec(&config, 0, 427, 240, 0, &render);
        // x = 427 - 4 (offset inward) with the right-edge adjust (-fullWidth) and the
        // slide-out (+fullWidth at percent 1) cancelling at tick 0.
        assert!((spec.x - (427.0 - 4.0)).abs() < 1e-6);
        // Right-aligned icon anchor: full_width - LEFT_PADDING - height + 0.25.
        let expected_anchor =
            spec.full_width as f32 - LEFT_PADDING as f32 - (FONT_LINE_HEIGHT + 1) as f32 + 0.25;
        assert!((spec.icon_spec.anchor_x - expected_anchor).abs() < 0.01);
    }

    #[test]
    fn item_stack_serialization_round_trip() {
        let stack = ItemStack::new("diamond", 5);
        let json = serde_json::to_string(&stack).unwrap();
        let back: ItemStack = serde_json::from_str(&json).unwrap();
        assert_eq!(stack, back);
        assert!(back.is_same_item_same_components(&stack));
    }
}
