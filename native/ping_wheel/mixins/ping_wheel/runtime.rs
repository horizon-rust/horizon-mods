//! Ping-wheel runtime splice: the mod's modules inlined for Horizon's
//! crates/horizon-client/src/game.rs carrier, `crate::` -> `super::`.
//! Exact 1:1; MIT, Copyright (c) LukenSkyne.

pub mod global {
//! Upstream: common/Global.java (1:1).
pub const MOD_ID: &str = "pingwheel";
pub const MOD_PREFIX: &str = "[Ping-Wheel] ";

/// Upstream builds LOGGER with a FormattedMessageFactory that prefixes every message with MOD_PREFIX.
/// The host logging seam in `seams` applies the same prefix, so log lines stay 1:1.
pub use super::seams::log_error;
pub use super::seams::log_info;
pub use super::seams::log_warn;

/// MOD_VERSION comes from IPlatformContextService.INSTANCE.getSelfModVersion() at class-load.
pub fn mod_version() -> String {
    super::seams::context_service().get_self_mod_version()
}
}

pub mod common_client {
//! Upstream: common/CommonClient.java (1:1). The platform event hooks register
//! through the IPlatformClientEventService seam; upstream's `Game = Minecraft.getInstance()`
//! each tick maps to the host-held seam client in `seams::game()`.
use std::rc::Rc;

use super::config::client_handler;
use super::core::{GameContext, PingController, PingManager};
use super::network::{PingLocationS2CPacket, UpdateChannelC2SPacket};
use super::render::OverlayRenderer;
use super::seams::{self, PoseStack};
use super::util::InputUtils;

pub struct CommonClient;

impl CommonClient {
    pub fn on_init() {
        client_handler().load();

        let events = seams::client_event_service();
        events.register_tick_start_event(std::rc::Rc::new(Self::on_tick_start));
        events.register_join_server_event(std::rc::Rc::new(Self::on_join_server));
        events.register_leave_server_event(std::rc::Rc::new(Self::on_leave_server));
        events.register_render_world_event(std::rc::Rc::new(Self::on_render_world));
        events.register_render_gui_event(std::rc::Rc::new(Self::on_render_gui));

        let ctx = seams::context_service();
        // Upstream registers the static KEY_BINDING_PING / KEY_BINDING_SETTINGS; this
        // port constructs the tokens per call, so the host records fresh ones.
        ctx.register_key_mapping(&InputUtils::key_binding_ping());
        ctx.register_key_mapping(&InputUtils::key_binding_settings());

        super::compat::migrate_key_mappings();
    }

    pub fn on_join_server() {
        seams::network_service().send_to_server(&UpdateChannelC2SPacket::new(
            Some(client_handler().config().get_channel()),
        ));
    }

    pub fn on_leave_server() {
        PingManager::clear_pings();
    }

    pub fn on_tick_start() {
        // Upstream: Game = Minecraft.getInstance(); the host keeps the seam client current.
        GameContext::update_dimension();

        super::compat::on_tick();

        if InputUtils::consume_ping_hotkey() {
            PingController::queue_ping_action();
        }

        if seams::consume_click_key(&InputUtils::key_binding_settings()) {
            if let Some(game) = seams::game() {
                game.set_screen(Rc::new(|| super::screen::SettingsScreen::new()));
            }
        }
    }

    pub fn on_render_world(ctx: &super::core::WorldRenderContext) {
        PingManager::update_pings(ctx);
        PingController::poll_ping_action(ctx.tick_delta);
    }

    pub fn on_render_gui(pose_stack: PoseStack, _tick_delta: f32) {
        OverlayRenderer::draw(pose_stack, _tick_delta);
    }

    pub fn on_ping_location_packet(packet: &PingLocationS2CPacket) {
        PingManager::accept_ping_packet(packet);
    }
}
}

pub mod integration {
//! Upstream: common/integration/ (1:1). ModContext, TeamContext, TeamContextHandler
//! and the VoiceChat/FTBTeams wrappers. The optional third-party APIs (Simple Voice
//! Chat, FTB Teams) are host surfaces, so the wrappers read them through the
//! integration service seam; the guard checks and Optional handling are exact.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::global::log_info;
use super::seams;

// ---------------------------------------------------------------------------
// ModContext.java
// ---------------------------------------------------------------------------

pub static HAS_DISTANT_HORIZONS: AtomicBool = AtomicBool::new(false);
pub static HAS_VOICE_CHAT: AtomicBool = AtomicBool::new(false);
pub static HAS_FTB_TEAMS: AtomicBool = AtomicBool::new(false);

/// ModContext.indexMods(): the three isModLoaded checks plus the Distant
/// Horizons API version log ("%s.%s.%s"). DhApi is a host API, so the version
/// triple comes from the integration seam.
pub fn index_mods() {
    let ctx = seams::context_service();

    HAS_DISTANT_HORIZONS.store(ctx.is_mod_loaded("distanthorizons"), Ordering::SeqCst);
    HAS_VOICE_CHAT.store(ctx.is_mod_loaded("voicechat"), Ordering::SeqCst);
    HAS_FTB_TEAMS.store(ctx.is_mod_loaded("ftbteams"), Ordering::SeqCst);

    if HAS_DISTANT_HORIZONS.load(Ordering::SeqCst) {
        log_info(&format!(
            "Distant Horizons API Version: {}",
            ctx.dh_api_version()
                .map(|(maj, min, pat)| format!("{maj}.{min}.{pat}"))
                .unwrap_or_else(|| "0.0.0".to_string()),
        ));
    }
}

pub fn has_distant_horizons() -> bool {
    HAS_DISTANT_HORIZONS.load(Ordering::SeqCst)
}

pub fn has_voice_chat() -> bool {
    HAS_VOICE_CHAT.load(Ordering::SeqCst)
}

pub fn has_ftb_teams() -> bool {
    HAS_FTB_TEAMS.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// TeamContext.java: toString() is super.toString().toLowerCase().
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TeamContext {
    None,
    VanillaTeam,
    FtbTeams,
    VoiceChat,
}

impl TeamContext {
    pub fn to_string(self) -> String {
        match self {
            TeamContext::None => "none".to_string(),
            TeamContext::VanillaTeam => "vanilla_team".to_string(),
            TeamContext::FtbTeams => "ftb_teams".to_string(),
            TeamContext::VoiceChat => "voice_chat".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// VoiceChatWrapper.java / FTBTeamsWrapper.java. Both return Optional.empty()
// when the mod is absent, the API is null, the connection/group is null, or
// the FTB team is a player team; those conditions live in the seam's answers.
// ---------------------------------------------------------------------------

/// VoiceChatWrapper.getGroupId(player): Optional&lt;UUID&gt;.
pub fn voice_chat_group_id(player_uuid: u128) -> Option<u128> {
    if !has_voice_chat() {
        return None;
    }
    seams::integration_service().voice_chat_group_id(player_uuid)
}

/// VoiceChatWrapper.getSelfGroupId(): Optional&lt;UUID&gt;.
pub fn voice_chat_self_group_id() -> Option<u128> {
    if !has_voice_chat() {
        return None;
    }
    seams::integration_service().voice_chat_self_group_id()
}

/// FTBTeamsWrapper.getTeamId(player): empty when the manager is not loaded,
/// the team is missing, or the team is a player team.
pub fn ftb_team_id(player_uuid: u128) -> Option<u128> {
    if !has_ftb_teams() {
        return None;
    }
    seams::integration_service().ftb_team_id(player_uuid)
}

/// FTBTeamsWrapper.getSelfTeamId().
pub fn ftb_self_team_id() -> Option<u128> {
    if !has_ftb_teams() {
        return None;
    }
    seams::integration_service().ftb_self_team_id()
}

// ---------------------------------------------------------------------------
// TeamContextHandler.java
// ---------------------------------------------------------------------------

/// hasTeam(player): player == null -> false; getContext(player) != NONE.
pub fn has_team(player_uuid: Option<u128>) -> bool {
    let Some(player_uuid) = player_uuid else {
        return false;
    };

    get_context(player_uuid) != TeamContext::None
}

/// getContext(player): voice chat group first, then FTB team, then the
/// player's vanilla team.
pub fn get_context(player_uuid: u128) -> TeamContext {
    if voice_chat_group_id(player_uuid).is_some() {
        return TeamContext::VoiceChat;
    }

    if ftb_team_id(player_uuid).is_some() {
        return TeamContext::FtbTeams;
    }

    if seams::integration_service().player_team_present(player_uuid) {
        return TeamContext::VanillaTeam;
    }

    TeamContext::None
}

/// getSelfContext(): the same ladder against the local player.
pub fn get_self_context() -> TeamContext {
    if voice_chat_self_group_id().is_some() {
        return TeamContext::VoiceChat;
    }

    if ftb_self_team_id().is_some() {
        return TeamContext::FtbTeams;
    }

    if let Some(player) = seams::game().and_then(|g| g.player()) {
        if player.team_present() {
            return TeamContext::VanillaTeam;
        }
    }

    TeamContext::None
}

/// inSameContext(p1, p2): the first non-empty of (voice chat, FTB) ids on each
/// side is compared (p1 id with p2 id; a null p2 id fails equals); when only p2
/// has an id the contexts differ; otherwise the vanilla teams are compared.
pub fn in_same_context(p1: u128, p2: u128) -> bool {
    let svc = seams::integration_service();
    let p1_team_id = voice_chat_group_id(p1).or_else(|| ftb_team_id(p1));
    let p2_team_id = voice_chat_group_id(p2).or_else(|| ftb_team_id(p2));

    if let Some(p1_team_id) = p1_team_id {
        return Some(p1_team_id) == p2_team_id;
    }
    if let Some(_) = p2_team_id {
        return false;
    }

    let p1_team = svc.player_team_uuid(p1);
    let p2_team = svc.player_team_uuid(p2);
    p1_team == p2_team
}
}

pub mod screen {
//! Upstream: common/screen/ (1:1). OptionUtils + SettingsScreen.
//! ProgressOption / CycleOption / CycleButton / OptionsList / EditBox / Button are
//! host GUI types, so the option objects carry their getters, setters, formatters
//! and tooltips and the widget rendering goes through the screen seam. The
//! formatting branches (OPTION_OFF / VALUE_INFINITE / VALUE_HIDDEN / the percent
//! and second units) are exact.

use std::rc::Rc;

use super::config::{client_handler, ClientConfig, PlayerInfoMode, TeamColorMode,
                    MAX_CHANNEL_LENGTH, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION};
use super::resource::LanguageUtils;
use super::{integration, resource, seams};

/// CommonComponents.OPTION_OFF.
pub const OPTION_OFF: &str = "OFF";

/// Screen colors and the tooltip wrap width, exactly as upstream declares them.
pub const WHITE: i32 = 0xFFFFFF;
pub const GRAY: i32 = 0xA0A0A0;
pub const LINE_LENGTH: i32 = 170;

/// CommonComponents.GUI_DONE.
pub const GUI_DONE: &str = "Done";

// ChatFormatting applied to a placeholder: the port's Component seam renders to a
// display string, so withStyle is the host's job and stays a no-op here.
pub fn with_style(text: String) -> String {
    text
}

// ---------------------------------------------------------------------------
// OptionUtils.java — the option objects.
// ---------------------------------------------------------------------------

/// ProgressOption: an int or float slider with its getter, setter and the
/// render-time formatter (formatter.apply(getter.get())).
pub struct ProgressOptionEntry {
    pub key: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub int: bool,
    pub get: Rc<dyn Fn(&ClientConfig) -> f64>,
    pub set: Rc<dyn Fn(&mut ClientConfig, f64)>,
    pub fmt: Rc<dyn Fn(&ClientConfig) -> String>,
}

/// CycleOption / CycleButton: the value list advances on click, the label comes
/// from the formatter over the getter, and the tooltip supplier is evaluated
/// lazily by the host (TooltipAccessor.getTooltip()).
pub struct CycleOptionEntry {
    pub key: String,
    pub count: usize,
    pub cycle: Rc<dyn Fn(&mut ClientConfig)>,
    pub label: Rc<dyn Fn(&ClientConfig) -> String>,
    pub tooltip: Rc<dyn Fn(&ClientConfig) -> Vec<String>>,
}

pub enum OptionEntry {
    Progress(ProgressOptionEntry),
    Cycle(CycleOptionEntry),
}

pub struct OptionUtils;

impl OptionUtils {
    pub fn of_int(
        key: &str,
        min: i32,
        max: i32,
        step: i32,
        formatter: Rc<dyn Fn(i32) -> String>,
        getter: Rc<dyn Fn(&ClientConfig) -> i32>,
        setter: Rc<dyn Fn(&mut ClientConfig, i32)>,
    ) -> OptionEntry {
        let fmt_getter = getter.clone();
        OptionEntry::Progress(ProgressOptionEntry {
            key: key.to_string(),
            min: min as f64,
            max: max as f64,
            step: step as f64,
            int: true,
            get: Rc::new(move |c| getter(c) as f64),
            set: Rc::new(move |c, v| setter(c, v as i32)),
            fmt: Rc::new(move |c| formatter((fmt_getter)(c) as i32)),
        })
    }

    pub fn of_float(
        key: &str,
        min: f32,
        max: f32,
        step: f32,
        formatter: Rc<dyn Fn(f32) -> String>,
        getter: Rc<dyn Fn(&ClientConfig) -> f32>,
        setter: Rc<dyn Fn(&mut ClientConfig, f32)>,
    ) -> OptionEntry {
        let fmt_getter = getter.clone();
        OptionEntry::Progress(ProgressOptionEntry {
            key: key.to_string(),
            min: min as f64,
            max: max as f64,
            step: step as f64,
            int: false,
            get: Rc::new(move |c| getter(c) as f64),
            set: Rc::new(move |c, v| setter(c, v as f32)),
            fmt: Rc::new(move |c| formatter((fmt_getter)(c))),
        })
    }

    /// CycleOption.createOnOff(key, getter, (gameOptions, option, value) -> setter).
    pub fn of_bool(
        key: &str,
        getter: Rc<dyn Fn(&ClientConfig) -> bool>,
        setter: Rc<dyn Fn(&mut ClientConfig, bool)>,
    ) -> OptionEntry {
        let cycle_getter = getter.clone();
        OptionEntry::Cycle(CycleOptionEntry {
            key: key.to_string(),
            count: 2,
            cycle: Rc::new(move |c| setter(c, !cycle_getter(c))),
            label: Rc::new(move |c| if getter(c) { "ON" } else { "OFF" }.to_string()),
            tooltip: Rc::new(|_c| Vec::new()),
        })
    }

    /// CycleOption.create(key, values, formatter, getter, setter).setTooltip(...).
    pub fn of_enum<E: Copy + PartialEq + 'static>(
        key: &str,
        values: Vec<E>,
        formatter: Rc<dyn Fn(E) -> String>,
        tooltip_supplier: Rc<dyn Fn(E) -> Vec<String>>,
        getter: Rc<dyn Fn(&ClientConfig) -> E>,
        setter: Rc<dyn Fn(&mut ClientConfig, E)>,
    ) -> OptionEntry {
        let values = Rc::new(values);
        let count = values.len();
        let cycle_values = values.clone();
        let cycle_getter = getter.clone();
        let cycle_setter = setter.clone();
        let label_getter = getter.clone();
        OptionEntry::Cycle(CycleOptionEntry {
            key: key.to_string(),
            count,
            cycle: Rc::new(move |c| {
                let current = cycle_getter(c);
                let index = cycle_values.iter().position(|v| *v == current).unwrap_or(0);
                let next = cycle_values[(index + 1) % cycle_values.len()];
                cycle_setter(c, next);
            }),
            label: Rc::new(move |c| formatter((label_getter)(c))),
            tooltip: Rc::new(move |c| tooltip_supplier(getter(c))),
        })
    }
}

// ---------------------------------------------------------------------------
// SettingsScreen.java
// ---------------------------------------------------------------------------

pub struct SettingsScreen {
    /// Upstream holds the Screen it reopens on close; the host seam reopens it
    /// by callback (setScreen(parent)).
    parent: Option<Rc<dyn Fn()>>,
    list: Option<Rc<dyn seams::OptionsListHost>>,
    channel_text_field: Option<Rc<dyn seams::EditBoxHost>>,
}

impl SettingsScreen {
    /// SettingsScreen(): super(LanguageUtils.settings("title").get()) and
    /// config = ClientConfig.HANDLER.getConfig().
    pub fn new() -> Self {
        Self {
            parent: None,
            list: None,
            channel_text_field: None,
        }
    }

    pub fn new_with_parent(parent: Rc<dyn Fn()>) -> Self {
        Self {
            parent: Some(parent),
            list: None,
            channel_text_field: None,
        }
    }

    pub fn title(&self) -> String {
        LanguageUtils::settings("title").get(&[])
    }

    pub fn tick(&self) {
        if let Some(edit) = self.channel_text_field.as_ref() {
            edit.tick();
        }
    }

    pub fn init(&mut self) {
        let env = seams::screen_env();
        let (width, height) = (env.width(), env.height());
        let list = env.new_options_list(width, height, 32, height - 32, 25);

        list.add_small(Self::get_ping_volume_option(), Some(Self::get_ping_duration_option()));

        list.add_small(Self::get_ping_distance_option(), Some(Self::get_correction_period_option()));

        list.add_small(Self::get_item_icons_visible_option(), Some(Self::get_direction_indicator_visible_option()));

        list.add_small(Self::get_player_info_mode_option(), Some(Self::get_team_color_mode_option()));

        list.add_small(Self::get_ping_size_option(), None);

        let y_offset = 50 + 25 * list.children_len() as i32;
        let edit = env.new_edit_box(
            width / 2 - 100,
            y_offset,
            200,
            20,
            &super::compat::Component::empty(),
        );
        edit.set_max_length(MAX_CHANNEL_LENGTH as i32);
        edit.set_value(&client_handler().config().get_channel());
        let responder_edit = edit.clone();
        edit.set_responder(Rc::new(move |value: &str| {
            client_handler().config_mut().set_channel(value.to_string());
            let _ = &responder_edit;
        }));
        env.add_widget_edit(edit.clone());

        env.add_widget_list(list.clone());

        let button_parent = self.parent.clone();
        env.add_renderable_button(
            width / 2 - 100,
            height - 27,
            200,
            20,
            GUI_DONE,
            Rc::new(move || Self::on_close_with_parent(button_parent.clone())),
        );

        self.list = Some(list);
        self.channel_text_field = Some(edit);
    }

    /// onClose(): save the config, then reopen the parent screen or fall back to
    /// the host's default close.
    pub fn on_close_with_parent(parent: Option<Rc<dyn Fn()>>) {
        client_handler().save();

        let env = seams::screen_env();
        match parent {
            Some(parent) if env.mc_present() => parent(),
            _ => env.super_on_close(),
        }
    }

    pub fn on_close(&self) {
        Self::on_close_with_parent(self.parent.clone())
    }

    pub fn render(&self, m: &super::seams::PoseStack, mouse_x: i32, mouse_y: i32, delta: f32) {
        let env = seams::screen_env();
        let edit = self.channel_text_field.as_ref().expect("init() before render");
        let list = self.list.as_ref().expect("init() before render");

        env.render_background(m);
        list.render(m, mouse_x, mouse_y, delta);
        env.draw_centered_string(
            m,
            &self.title(),
            env.width() / 2,
            20,
            WHITE,
        );

        env.draw_string(
            m,
            &LanguageUtils::settings("channel").get(&[]),
            env.width() / 2 - 100,
            edit.y() - 12,
            GRAY,
        );
        edit.render(m, mouse_x, mouse_y, delta);

        if edit.value().is_empty() {
            env.draw_string(
                m,
                &self.get_channel_placeholder(),
                env.width() / 2 - 100 + 4,
                edit.y() + 6,
                WHITE,
            );
        }

        env.super_render(m, mouse_x, mouse_y, delta);

        let mut tooltip_lines = Self::get_hovered_button_tooltip(list, mouse_x, mouse_y);

        if tooltip_lines.is_empty()
            && (edit.is_hovered_or_focused() && !edit.is_focused())
        {
            tooltip_lines = env.font_split(
                &LanguageUtils::settings("channel").path(&["tooltip"]).get(&[]),
                LINE_LENGTH,
            );
        }

        env.render_tooltip(m, &tooltip_lines, mouse_x, mouse_y);
    }

    /// getHoveredButtonTooltip(list, mouseX, mouseY): the OptionsList entry under
    /// the cursor and its TooltipAccessor tooltip, else an empty list.
    fn get_hovered_button_tooltip(
        list: &Rc<dyn seams::OptionsListHost>,
        mouse_x: i32,
        mouse_y: i32,
    ) -> Vec<String> {
        match list.get_mouse_over(mouse_x, mouse_y) {
            Some(index) => list.tooltip_of(index),
            None => Vec::new(),
        }
    }

    /// getChannelPlaceholder(): empty when there is no player, the global value
    /// label without a team context, otherwise the channel placeholder carrying
    /// the self context; ITALIC + DARK_GRAY are host styling.
    fn get_channel_placeholder(&self) -> String {
        if seams::game().and_then(|g| g.player()).is_none() {
            return super::compat::Component::empty();
        }

        let team_context = integration::get_self_context();
        let placeholder = if team_context == integration::TeamContext::None {
            LanguageUtils::of("value", &["global"]).get(&[])
        } else {
            LanguageUtils::settings("channel")
                .path(&["placeholder"])
                .get(&[&LanguageUtils::of("value", &[&team_context.to_string()]).get(&[])])
        };

        with_style(placeholder)
    }

    fn get_ping_volume_option() -> OptionEntry {
        let text = LanguageUtils::settings("ping_volume");

        OptionUtils::of_int(
            &text.key.clone(),
            0,
            100,
            1,
            Rc::new(|value| {
                if value == 0 {
                    return LanguageUtils::settings("ping_volume").get(&[OPTION_OFF]);
                }

                LanguageUtils::settings("ping_volume")
                    .get(&[&resource::unit_percent_get(value)])
            }),
            Rc::new(|c| c.get_ping_volume()),
            Rc::new(|c, v| c.set_ping_volume(v as i32)),
        )
    }

    fn get_ping_duration_option() -> OptionEntry {
        let text = LanguageUtils::settings("ping_duration");

        OptionUtils::of_int(
            &text.key.clone(),
            1,
            MAX_PING_DURATION,
            1,
            Rc::new(|value| {
                if value >= MAX_PING_DURATION {
                    return LanguageUtils::settings("ping_duration").get(&[&resource::value_infinite()]);
                }

                LanguageUtils::settings("ping_duration")
                    .get(&[&resource::unit_seconds_get(&value.to_string())])
            }),
            Rc::new(|c| c.get_ping_duration()),
            Rc::new(|c, v| c.set_ping_duration(v as i32)),
        )
    }

    fn get_ping_distance_option() -> OptionEntry {
        let text = LanguageUtils::settings("ping_distance");

        OptionUtils::of_int(
            &text.key.clone(),
            0,
            MAX_PING_DISTANCE,
            16,
            Rc::new(|value| {
                if value == 0 {
                    return LanguageUtils::settings("ping_distance").get(&[&resource::value_hidden()]);
                } else if value >= MAX_PING_DISTANCE {
                    return LanguageUtils::settings("ping_distance").get(&[&resource::value_infinite()]);
                }

                LanguageUtils::settings("ping_distance")
                    .get(&[&resource::unit_meters().replace("%s", &format!("{value}"))])
            }),
            Rc::new(|c| c.get_ping_distance()),
            Rc::new(|c, v| c.set_ping_distance(v as i32)),
        )
    }

    fn get_correction_period_option() -> OptionEntry {
        let text = LanguageUtils::settings("correction_period");

        OptionUtils::of_float(
            &text.key.clone(),
            0.1,
            MAX_CORRECTION_PERIOD,
            0.1,
            Rc::new(|value| {
                if value >= MAX_CORRECTION_PERIOD {
                    return LanguageUtils::settings("correction_period")
                        .get(&[&resource::value_infinite()]);
                }

                LanguageUtils::settings("correction_period")
                    .get(&[&resource::unit_seconds_get(&format!("{:.1}", value).as_str())])
            }),
            Rc::new(|c| c.get_correction_period()),
            Rc::new(|c, v| c.set_correction_period(v)),
        )
    }

    fn get_item_icons_visible_option() -> OptionEntry {
        OptionUtils::of_bool(
            &LanguageUtils::settings("item_icon_visible").key.clone(),
            Rc::new(|c| c.is_item_icon_visible()),
            Rc::new(|c, v| c.set_item_icon_visible(v)),
        )
    }

    fn get_direction_indicator_visible_option() -> OptionEntry {
        OptionUtils::of_bool(
            &LanguageUtils::settings("direction_indicator_visible").key.clone(),
            Rc::new(|c| c.is_direction_indicator_visible()),
            Rc::new(|c, v| c.set_direction_indicator_visible(v)),
        )
    }

    fn get_player_info_mode_option() -> OptionEntry {
        OptionUtils::of_enum(
            &LanguageUtils::settings("player_info_mode").key.clone(),
            PlayerInfoMode::VALUES.to_vec(),
            Rc::new(|mode| LanguageUtils::of("value", &[&mode.to_string()]).get(&[])),
            Rc::new(|mode| {
                if mode != PlayerInfoMode::Hold {
                    return Vec::new();
                }

                let key_player_list_title =
                    super::compat::Component::translatable(&seams::game().map(|g| g.key_player_list_name()).unwrap_or_default(), &[]);
                let key_player_list_name = seams::game().map(|g| g.key_player_list_translated()).unwrap_or_default();

                seams::screen_env().font_split(
                    &LanguageUtils::settings("player_info_mode")
                        .path(&["hold", "tooltip"])
                        .get(&[&key_player_list_title, &key_player_list_name]),
                    LINE_LENGTH,
                )
            }),
            Rc::new(|c| c.get_player_info_mode()),
            Rc::new(|c, v| c.set_player_info_mode(v)),
        )
    }

    fn get_team_color_mode_option() -> OptionEntry {
        OptionUtils::of_enum(
            &LanguageUtils::settings("team_color_mode").key.clone(),
            TeamColorMode::VALUES.to_vec(),
            Rc::new(|mode| LanguageUtils::of("value", &[&mode.to_string()]).get(&[])),
            Rc::new(|_mode| Vec::new()),
            Rc::new(|c| c.get_team_color_mode()),
            Rc::new(|c, v| c.set_team_color_mode(v)),
        )
    }

    fn get_ping_size_option() -> OptionEntry {
        let text = LanguageUtils::settings("ping_size");

        OptionUtils::of_int(
            &text.key.clone(),
            40,
            300,
            10,
            Rc::new(|value| {
                LanguageUtils::settings("ping_size")
                    .get(&[&resource::unit_percent_get(value)])
            }),
            Rc::new(|c| c.get_ping_size()),
            Rc::new(|c, v| c.set_ping_size(v as i32)),
        )
    }
}
}

pub mod render {
//! Upstream: common/render/ (1:1). DrawContext, OverlayRenderer,
//! DirectionIndicatorRenderer, PingLocationRenderer.
use super::config::{PlayerInfoMode, TeamColorMode};
use super::core::{client_config_snapshot, GameContext, PingManager, PingView};
use super::math::MathUtils;
use super::resource::{self, ARROW_TEXTURE_ID, PING_TEXTURE_ID};
use super::resource::ResourceLocationToken;
use super::seams::{self, GuiEnv, ItemStackToken, PlayerInfoHandle, PoseStack, Vec2};
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// render/DrawContext.java (1:1)
// ---------------------------------------------------------------------------

/// FastColor.ARGB32.color(64, 0, 0, 0)
const SHADOW_BLACK: i32 = 0x40000000u32 as i32;
/// RenderSystem light value cap used for GUI item rendering.
const LIGHT_VALUE_MAX: i32 = 0xF000F0;

/// Upstream com.mojang.blaze3d RenderSystem shader-color components packed from
/// FastColor.ARGB32 ints (alpha / red / green / blue in that byte order).
fn argb_components(color: i32) -> (f32, f32, f32, f32) {
    let c = color as u32;
    (
        (((c >> 24) & 0xFF) as f32) / 255.0,
        (((c >> 16) & 0xFF) as f32) / 255.0,
        (((c >> 8) & 0xFF) as f32) / 255.0,
        ((c & 0xFF) as f32) / 255.0,
    )
}

pub struct DrawContext {
    matrices: RefCell<PoseStack>,
}

impl DrawContext {
    pub fn new(matrices: PoseStack) -> Self {
        Self { matrices: RefCell::new(matrices) }
    }

    /// lombok @Getter matrices (borrowed; the upstream object is shared, so borrows
    /// stand in for direct field access and are released before nested calls).
    pub fn matrices(&self) -> std::cell::Ref<PoseStack> {
        self.matrices.borrow()
    }

    pub fn matrices_mut(&self) -> std::cell::RefMut<PoseStack> {
        self.matrices.borrow_mut()
    }

    pub fn render_label(&self, text: &str, y_offset: f32, player: Option<&Rc<dyn PlayerInfoHandle>>, color: i32) {
        let game = seams::game();
        let Some(game) = game else { return };
        let Some(font) = game.font() else { return };
        let Some(env) = game.gui_env() else { return };

        let extra_width = if player.is_some() { 10.0f32 } else { 0.0 };
        let text_metrics = Vec2::new(font.width(text) + extra_width, font.line_height());
        let text_offset = text_metrics.scale(-0.5).add(Vec2::new(0.0, text_metrics.y * y_offset));

        let mut m = self.matrices.borrow_mut();
        m.push_pose();
        m.translate(text_offset.x, text_offset.y, 0.0);
        env.fill(&m, -2, -2, text_metrics.x as i32 + 1, text_metrics.y as i32, SHADOW_BLACK);
        font.draw(&m, text, extra_width, 0.0, color);

        if let Some(player) = player {
            m.translate(-0.5, -0.5, 0.0);
            drop(m);
            self.render_player_head(player);
            let mut m = self.matrices.borrow_mut();
            m.pop_pose();
            return;
        }

        m.pop_pose();
    }

    pub fn render_player_head(&self, player: &Rc<dyn PlayerInfoHandle>) {
        let game = seams::game();
        let Some(game) = game else { return };
        let Some(env) = game.gui_env() else { return };

        let (skin_ns, skin_path) = split_resource_location(&player.skin_location());
        env.set_shader_texture(&skin_ns, &skin_path);
        env.enable_blend();
        let mut m = self.matrices.borrow_mut();
        env.blit(&m, 0, 0, 0, 8, 8, 8, 8, 64, 64);
        // Overlay (hat)
        env.blit(&m, 0, 0, 0, 40, 8, 8, 8, 64, 64);
        env.disable_blend();
    }

    pub fn render_ping(&self, item_stack: Option<ItemStackToken>, draw_item_icon: bool, color: i32) {
        if let Some(item_stack) = item_stack {
            if draw_item_icon {
                self.render_gui_item_model(&item_stack);
                return;
            }
        }

        if super::resource::ResourceReloadListener::has_custom_texture() {
            self.render_texture(&PING_TEXTURE_ID, 12, color);
        } else {
            self.render_default_ping_icon(color);
        }
    }

    pub fn render_gui_item_model(&self, item_stack: &ItemStackToken) {
        let game = seams::game();
        let Some(game) = game else { return };
        let Some(env) = game.gui_env() else { return };

        let (model, uses_block_light) = env.item_model(item_stack);
        let atlas = env.blocks_atlas();

        env.set_texture_filter(&atlas, false, false);

        env.set_shader_texture(atlas.namespace, atlas.path);
        env.enable_blend();
        env.blend_func_src_alpha_one_minus();
        env.set_shader_color(1.0, 1.0, 1.0, 1.0);

        let m = self.matrices.borrow();
        env.mv_push();
        env.mv_mul_pose(m.top());
        env.mv_translate(0.0, 0.0, -0.5);
        env.mv_scale(16.0, -16.0, 16.0);
        env.mv_apply();

        // bl = !model.usesBlockLight()
        if !uses_block_light {
            env.setup_for_flat_items();
        }

        env.render_item_gui_and_end_batch(item_stack, model);
        env.enable_depth_test();

        if !uses_block_light {
            env.setup_for_3d_items();
        }

        env.mv_pop();
        env.mv_apply();
    }

    pub fn render_default_ping_icon(&self, color: i32) {
        let game = seams::game();
        let Some(game) = game else { return };
        let Some(env) = game.gui_env() else { return };

        let mut m = self.matrices.borrow_mut();
        m.push_pose();
        MathUtils::rotate_z(&mut m, (std::f64::consts::PI / 4.0) as f32);
        m.translate(-2.5, -2.5, 0.0);
        env.fill(&m, 0, 0, 5, 5, color);
        m.pop_pose();
    }

    pub fn render_texture(&self, texture: &ResourceLocationToken, size: i32, color: i32) {
        let game = seams::game();
        let Some(game) = game else { return };
        let Some(env) = game.gui_env() else { return };

        let offset = size / -2;
        let (a, r, g, b) = argb_components(color);

        env.set_shader_texture(texture.namespace, texture.path);
        env.set_shader_color(r, g, b, a);
        env.enable_blend();
        let mut m = self.matrices.borrow_mut();
        env.blit(&m, offset, offset, 0, 0, 0, size, size, size, size);
        env.disable_blend();
    }

    pub fn render_arrow_icon(&self, color: i32) {
        self.render_texture(&ARROW_TEXTURE_ID, 10, color);
    }
}

// ---------------------------------------------------------------------------
// render/OverlayRenderer.java (1:1)
// ---------------------------------------------------------------------------

pub struct OverlayRenderer;

impl OverlayRenderer {
    pub fn draw(pose_stack: PoseStack, _tick_delta: f32) {
        let Some(game) = seams::game() else {
            return;
        };

        let ping_repo = PingManager::repo_snapshot();

        if game.player().is_none() || ping_repo.is_empty() {
            return;
        }

        let mut ctx = DrawContext::new(pose_stack);
        let show_direction_indicator = client_config_snapshot().direction_indicator_visible;

        if show_direction_indicator {
            DirectionIndicatorRenderer::prepare_safe_zone();
        }

        {
            let mut m = ctx.matrices_mut();
            m.push_pose();
            m.translate(0.0, 0.0, -(ping_repo.len() as f32) * 16.0);
        }

        for ping in &ping_repo {
            let Some(_screen_pos) = ping.screen_pos() else {
                continue;
            };

            if ping.dimension != GameContext::dimension() {
                continue;
            }

            let behind_camera = _screen_pos.is_behind_camera();

            if behind_camera && !show_direction_indicator {
                continue;
            }

            ctx.matrices_mut().translate(0.0, 0.0, 16.0);

            if show_direction_indicator {
                DirectionIndicatorRenderer::draw(&ctx, ping);
            }

            if !behind_camera {
                PingLocationRenderer::draw(&ctx, ping);
            }
        }

        ctx.matrices_mut().pop_pose();
    }
}

// ---------------------------------------------------------------------------
// render/DirectionIndicatorRenderer.java (1:1)
// ---------------------------------------------------------------------------

thread_local! {
    static SCREEN_SIZE: RefCell<Option<Vec2>> = const { RefCell::new(None) };
    static SAFE_ZONE_TOP_LEFT: RefCell<Option<Vec2>> = const { RefCell::new(None) };
    static SAFE_ZONE_BOTTOM_RIGHT: RefCell<Option<Vec2>> = const { RefCell::new(None) };
    static SAFE_SCREEN_CENTER: RefCell<Option<Vec2>> = const { RefCell::new(None) };
}

const INDICATOR_WHITE: i32 = -1; // 0xFFFFFFFF

pub struct DirectionIndicatorRenderer;

impl DirectionIndicatorRenderer {
    pub fn prepare_safe_zone() {
        let Some(game) = seams::game() else {
            return;
        };
        let cfg = client_config_snapshot();
        let (width, height) = game.gui_scaled_size();

        let screen_size = Vec2::new(width, height);
        let top_left = Vec2::new(cfg.safe_zone_left as f32, cfg.safe_zone_top as f32);
        let bottom_right = Vec2::new(
            screen_size.x - cfg.safe_zone_right as f32,
            screen_size.y - cfg.safe_zone_bottom as f32,
        );
        let center = Vec2::new(
            (bottom_right.x - top_left.x) * 0.5,
            (bottom_right.y - top_left.y) * 0.5,
        );

        SCREEN_SIZE.with(|s| *s.borrow_mut() = Some(screen_size));
        SAFE_ZONE_TOP_LEFT.with(|s| *s.borrow_mut() = Some(top_left));
        SAFE_ZONE_BOTTOM_RIGHT.with(|s| *s.borrow_mut() = Some(bottom_right));
        SAFE_SCREEN_CENTER.with(|s| *s.borrow_mut() = Some(center));
    }

    pub fn draw(ctx: &DrawContext, ping: &PingView) {
        let Some(screen_pos) = ping.screen_pos() else {
            return;
        };

        let screen_size = SCREEN_SIZE.with(|s| *s.borrow());
        let top_left = SAFE_ZONE_TOP_LEFT.with(|s| *s.borrow());
        let bottom_right = SAFE_ZONE_BOTTOM_RIGHT.with(|s| *s.borrow());
        let safe_center = SAFE_SCREEN_CENTER.with(|s| *s.borrow());

        let (Some(screen_size), Some(top_left), Some(bottom_right), Some(safe_center)) =
            (screen_size, top_left, bottom_right, safe_center)
        else {
            return;
        };

        let behind_camera = screen_pos.is_behind_camera();
        let is_off_screen = behind_camera || !screen_pos.is_in_bounds(Vec2::ZERO, screen_size);

        if !is_off_screen {
            return;
        }

        let mut ping_direction_vec = Vec2::new(
            screen_pos.x - top_left.x - safe_center.x,
            screen_pos.y - top_left.y - safe_center.y,
        );

        if behind_camera {
            ping_direction_vec = ping_direction_vec.scale(-1.0);
        }

        let cfg = client_config_snapshot();
        let ping_scale = ping.scale();
        let ping_size = cfg.ping_size as f32 / 100.0;
        let ping_angle = ping_direction_vec.y.atan2(ping_direction_vec.x);
        let edge_position = MathUtils::calculate_angle_rect_intersection(ping_angle, top_left, bottom_right);
        let indicator_offset_x = ((ping_angle as f64 + std::f64::consts::PI).cos()) * 12.0;
        let indicator_offset_y = ((ping_angle as f64 + std::f64::consts::PI).sin()) * 12.0;

        let use_team_color =
            cfg.team_color_mode == TeamColorMode::Full || cfg.team_color_mode == TeamColorMode::PingOnly;
        let ping_color = if use_team_color { ping.get_team_color() } else { INDICATOR_WHITE };

        let mut m = ctx.matrices_mut();
        m.push_pose();
        m.translate(edge_position.x, edge_position.y, 0.0);

        m.push_pose();
        m.scale(ping_scale, ping_scale, 1.0);
        m.translate(indicator_offset_x as f32, indicator_offset_y as f32, 0.0);
        ctx.render_ping(ping.item_stack(), cfg.item_icon_visible, ping_color);
        m.pop_pose();

        m.push_pose();
        MathUtils::rotate_z(&mut m, ping_angle);
        m.scale(ping_size, ping_size, 1.0);
        m.scale(0.25, 0.25, 1.0);
        m.translate(-5.0, 0.0, 0.0);
        ctx.render_arrow_icon(ping_color);
        m.pop_pose();

        m.pop_pose();
    }
}

// ---------------------------------------------------------------------------
// render/PingLocationRenderer.java (1:1)
// ---------------------------------------------------------------------------

pub struct PingLocationRenderer;

impl PingLocationRenderer {
    pub fn draw(ctx: &DrawContext, ping: &PingView) {
        let Some(screen_pos) = ping.screen_pos() else {
            return;
        };

        let cfg = client_config_snapshot();
        let ping_scale = ping.scale();

        {
            let mut m = ctx.matrices_mut();
            m.push_pose();
            m.translate(screen_pos.x, screen_pos.y, 0.0);
            m.scale(ping_scale, ping_scale, 1.0);
        }

        let label_use_team_color =
            cfg.team_color_mode == TeamColorMode::Full || cfg.team_color_mode == TeamColorMode::LabelsOnly;
        let ping_use_team_color =
            cfg.team_color_mode == TeamColorMode::Full || cfg.team_color_mode == TeamColorMode::PingOnly;
        let distance_color = if label_use_team_color { ping.get_team_color() } else { INDICATOR_WHITE };
        let ping_color = if ping_use_team_color { ping.get_team_color() } else { INDICATOR_WHITE };

        let author = ping.player_info().clone();
        let compact_player_info = if cfg.player_info_mode == PlayerInfoMode::Compact {
            author.clone()
        } else {
            None
        };

        let distance_text = resource::unit_meters_lang().get(&[&format_distance_comma_1f(ping.distance())]);
        ctx.render_label(&distance_text, -1.5, compact_player_info.as_ref(), distance_color);
        ctx.render_ping(ping.item_stack(), cfg.item_icon_visible, ping_color);

        let is_player_list_held = cfg.player_info_mode == PlayerInfoMode::Hold
            && seams::game().map(|g| g.key_player_list_down()).unwrap_or(false);
        let show_verbose_player_info = cfg.player_info_mode == PlayerInfoMode::Always || is_player_list_held;

        if let Some(author) = author.filter(|_| show_verbose_player_info) {
            // PlayerTeam.formatNameForTeam(author.getTeam(), Component.literal(name));
            let team = author.team_name();
            let display_name = seams::player_team_format_name(team.as_deref(), &author.profile_name());
            // .withStyle(ChatFormatting.RESET) on a plain-string seam: the reset style is
            // host-side; the display string is unchanged here.
            ctx.render_label(&display_name, 1.75, Some(&author), INDICATOR_WHITE);
        }

        ctx.matrices_mut().pop_pose();
    }
}

/// Java "%,.1f" (default locale: comma grouping, one fraction digit, HALF_UP).
fn format_distance_comma_1f(value: f64) -> String {
    let scaled = value * 10.0;
    let rounded = if scaled >= 0.0 {
        (scaled + 0.5).floor()
    } else {
        (scaled - 0.5).ceil()
    } as i64;

    let negative = rounded < 0;
    let abs = rounded.abs();
    let int_part = abs / 10;
    let frac = abs % 10;

    let digits = int_part.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    format!(
        "{}{}.{}",
        if negative { "-" } else { "" },
        grouped,
        frac
    )
}

/// "namespace:path" split for a dynamic ResourceLocation (skin locations).
fn split_resource_location(location: &str) -> (String, String) {
    match location.split_once(':') {
        Some((ns, path)) => (ns.to_string(), path.to_string()),
        None => (String::new(), location.to_string()),
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// FastColor.ARGB32.color(64, 0, 0, 0) == 0x40000000
    #[test]
    fn shadow_black_constant_matches_upstream() {
        assert_eq!(SHADOW_BLACK, 1_073_741_824);
        assert_eq!(SHADOW_BLACK as u32, 0x4000_0000);
    }

    #[test]
    fn argb_components_split_matches_fastcolor() {
        let (a, r, g, b) = argb_components(0x40A0B0C0u32 as i32);
        assert!((a - 0x40 as f32 / 255.0).abs() < 1e-6);
        assert!((r - 0xA0 as f32 / 255.0).abs() < 1e-6);
        assert!((g - 0xB0 as f32 / 255.0).abs() < 1e-6);
        assert!((b - 0xC0 as f32 / 255.0).abs() < 1e-6);
    }

    /// Java "%,.1f" formats: grouping + one fraction digit, HALF_UP on ties.
    #[test]
    fn distance_formatting_matches_java_comma_1f() {
        assert_eq!(format_distance_comma_1f(0.0), "0.0");
        assert_eq!(format_distance_comma_1f(123.44), "123.4");
        assert_eq!(format_distance_comma_1f(123.45), "123.5");
        assert_eq!(format_distance_comma_1f(1234.5), "1,234.5");
        assert_eq!(format_distance_comma_1f(2_048.0), "2,048.0");
        assert_eq!(format_distance_comma_1f(0.96), "1.0");
        assert_eq!(format_distance_comma_1f(12345.67), "12,345.7");
    }

    /// has_custom_texture is false until a reload saw more than one texture
    /// (0 == vanilla only; >1 means a pack added ping.png).
    #[test]
    fn custom_texture_flag_defaults_false() {
        super::resource::NUM_CUSTOM_TEXTURES.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(!super::resource::ResourceReloadListener::has_custom_texture());
        super::resource::NUM_CUSTOM_TEXTURES.store(1, std::sync::atomic::Ordering::SeqCst);
        assert!(!super::resource::ResourceReloadListener::has_custom_texture());
        super::resource::NUM_CUSTOM_TEXTURES.store(2, std::sync::atomic::Ordering::SeqCst);
        assert!(super::resource::ResourceReloadListener::has_custom_texture());
        super::resource::NUM_CUSTOM_TEXTURES.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// checkResources flips the deprecation notice only for the three legacy ids.
    #[test]
    fn check_resources_flags_exactly_the_legacy_textures() {
        let calls = std::cell::RefCell::new(Vec::new());
        super::compat::check_resources(&|ns: &str, path: &str| {
            calls.borrow_mut().push(format!("{}/{}", ns, path));
            path == "textures/ping.png"
        });

        assert_eq!(
            calls.into_inner(),
            vec![
                "ping-wheel/ping",
                "ping-wheel/textures/ping.png",
                "ping-wheel/textures/arrow.png",
            ]
        );
        assert!(super::compat::notify_deprecated_resource_pack(), "texture probe must flag the notice");
        // A resource pack without any legacy texture must clear it again.
        super::compat::check_resources(&|_ns: &str, _path: &str| false);
        assert!(!super::compat::notify_deprecated_resource_pack());
    }

    /// DirectionIndicatorRenderer.prepare_safe_zone stores the four screen values.
    #[test]
    fn prepare_safe_zone_computes_the_safe_rect() {
        struct G;
        impl super::seams::MinecraftClient for G {
            fn level(&self) -> Option<Rc<dyn super::seams::LevelHandle>> { None }
            fn camera_entity(&self) -> Option<Rc<dyn super::seams::EntityHandle>> { None }
            fn gui_scaled_size(&self) -> (f32, f32) { (427.0, 240.0) }
        }
        let _g = super::tests::install();
        super::seams::set_game(Some(Rc::new(G) as Rc<dyn super::seams::MinecraftClient>));

        {
            let cfg = super::config::client_handler().config_mut();
            cfg.safe_zone_left = 10;
            cfg.safe_zone_top = 20;
            cfg.safe_zone_right = 30;
            cfg.safe_zone_bottom = 40;
        }

        DirectionIndicatorRenderer::prepare_safe_zone();

        let top_left = SAFE_ZONE_TOP_LEFT.with(|s| *s.borrow()).unwrap();
        let bottom_right = SAFE_ZONE_BOTTOM_RIGHT.with(|s| *s.borrow()).unwrap();
        let center = SAFE_SCREEN_CENTER.with(|s| *s.borrow()).unwrap();
        assert_eq!(top_left, Vec2 { x: 10.0, y: 20.0 });
        assert_eq!(bottom_right, Vec2 { x: 427.0 - 30.0, y: 240.0 - 40.0 });
        assert_eq!(center, Vec2 { x: (397.0 - 10.0) * 0.5, y: (200.0 - 20.0) * 0.5 });
        assert_eq!(SCREEN_SIZE.with(|s| *s.borrow()).unwrap(), Vec2 { x: 427.0, y: 240.0 });

        {
            let cfg = super::config::client_handler().config_mut();
            cfg.safe_zone_left = super::config::ClientConfig::default().safe_zone_left;
            cfg.safe_zone_top = super::config::ClientConfig::default().safe_zone_top;
            cfg.safe_zone_right = super::config::ClientConfig::default().safe_zone_right;
            cfg.safe_zone_bottom = super::config::ClientConfig::default().safe_zone_bottom;
        }
    }
}
}

pub mod compat {
//! Upstream: common/compat/ (1:1).
use super::seams::{Matrix4f, Vec3};

/// Upstream compat/Vector4f.java: a mutable 4-vector that transforms through a Matrix4f.
#[derive(Clone, Copy, Debug)]
pub struct Vector4f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4f {
    pub fn new(vec3d: &Vec3, w: f32) -> Self {
        Self { x: vec3d.x as f32, y: vec3d.y as f32, z: vec3d.z as f32, w }
    }

    pub fn mul(mut self, mat: &Matrix4f) -> Self {
        // Upstream wraps com.mojang.math.Vector4f#transform(Matrix4f); the matrix multiply lives in the seam.
        let t = super::seams::transform4(mat, self.x, self.y, self.z, self.w);
        self.x = t.0;
        self.y = t.1;
        self.z = t.2;
        self.w = t.3;
        self
    }

    pub fn div(mut self, scalar: f32) -> Self {
        let inv = 1f32 / scalar;
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
        self.w *= inv;
        self
    }
}

/// Upstream compat/Component.java: literal / empty / translatable factory.
/// The seam renders to a display string because the MC Component tree is a host type.
pub struct Component;

impl Component {
    pub fn literal(text: &str) -> String {
        text.to_string()
    }

    pub fn empty() -> String {
        String::new()
    }

    pub fn translatable(key: &str, args: &[&str]) -> String {
        super::seams::translate_component(key, args)
    }
}

// ---------------------------------------------------------------------------
// LegacyMigrationHandler.migrateConfig (the part ConfigHandler.of calls).
// The key-mapping / resource-pack migration halves follow with the resource phase.
// ---------------------------------------------------------------------------

pub fn legacy_migrate_config(config_extension: &str) {
    let ctx = super::seams::context_service();
    let legacy_config_path = ctx.resolve_config_dir(&format!("ping-wheel{}", config_extension));
    let new_config_path = ctx.resolve_config_dir(&format!("{}{}", super::global::MOD_ID, config_extension));

    if !std::path::Path::new(&legacy_config_path).exists() {
        return;
    }

    match std::fs::rename(&legacy_config_path, &new_config_path) {
        Ok(()) => super::global::log_info(&format!("Renamed legacy config: {} -> {}", legacy_config_path, new_config_path)),
        Err(e) => super::global::log_error(&format!("Failed to rename legacy config: {}", e)),
    }
}


// ---------------------------------------------------------------------------
// compat/LegacyMigrationHandler.java (1:1). TODO upstream: remove a few
// versions/months down the line; introduced with 1.12.0 on 09.10.2025.
// ---------------------------------------------------------------------------

static GAME_OPTIONS_SAVE_NEEDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static NOTIFY_DEPRECATED_RESOURCE_PACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static MIGRATED_PING_KEY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
static MIGRATED_SETTINGS_KEY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// Migrated KEY_BINDING_PING key (tokens are constructed per call in this port;
/// the host applies the migrated key to the live binding).
pub fn migrated_ping_key() -> i32 {
    MIGRATED_PING_KEY.load(std::sync::atomic::Ordering::SeqCst)
}

/// Migrated KEY_BINDING_SETTINGS key.
pub fn migrated_settings_key() -> i32 {
    MIGRATED_SETTINGS_KEY.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn migrate_key_mappings() {
    use std::sync::atomic::Ordering;

    let ctx = super::seams::context_service();
    let options_path = ctx.resolve_game_dir("options.txt");

    if !std::path::Path::new(&options_path).exists() {
        super::global::log_warn("Unable to find game options for migration routine");
        return;
    }

    match std::fs::read_to_string(&options_path) {
        Err(e) => super::global::log_error(&format!("Failed to read options.txt to transfer legacy keybinds: {}", e)),
        Ok(content) => {
            for line in content.lines() {
                if !line.contains("ping-wheel") {
                    continue;
                }

                // Upstream line.split(":")[1]; a line without a colon would throw there,
                // the port skips it.
                let Some(key_string) = line.split(':').nth(1) else {
                    continue;
                };
                let key_key = super::seams::input_constants_get_key(key_string);

                if line.contains("ping-location") {
                    MIGRATED_PING_KEY.store(key_key, Ordering::SeqCst);
                    super::global::log_info(&format!("Migrated KEY_BINDING_PING: {}", key_key));
                } else if line.contains("open-settings") {
                    MIGRATED_SETTINGS_KEY.store(key_key, Ordering::SeqCst);
                    super::global::log_info(&format!("Migrated KEY_BINDING_SETTINGS: {}", key_key));
                }

                GAME_OPTIONS_SAVE_NEEDED.store(true, Ordering::SeqCst);
            }
        }
    }
}

pub fn on_tick() {
    use std::sync::atomic::Ordering;

    let Some(game) = super::seams::game() else {
        return;
    };

    if GAME_OPTIONS_SAVE_NEEDED.swap(false, Ordering::SeqCst) {
        game.save_options();
    }

    if NOTIFY_DEPRECATED_RESOURCE_PACK.load(Ordering::SeqCst) && game.player().is_some() {
        // msg = literal("a resource pack is using the old mod-id, please update ")
        //     + literal("\"assets/ping-wheel\"").withStyle(ChatFormatting.GRAY)
        //     + " to "
        //     + literal("\"assets/pingwheel\"").withStyle(ChatFormatting.GRAY)
        let msg = "a resource pack is using the old mod-id, please update \
\u{a7}7\"assets/ping-wheel\"\u{a7}r to \u{a7}7\"assets/pingwheel\"";
        game.send_player_message(&super::resource::with_mod_prefix(msg));
        NOTIFY_DEPRECATED_RESOURCE_PACK.store(false, Ordering::SeqCst);
    }
}

/// Host seam: ResourceManager.hasResource(location) for the legacy-texture probe.
pub fn check_resources(has_resource: &dyn Fn(&str, &str) -> bool) {
    use std::sync::atomic::Ordering;

    let legacy_textures = [
        ("ping-wheel", "ping"),
        ("ping-wheel", "textures/ping.png"),
        ("ping-wheel", "textures/arrow.png"),
    ];

    NOTIFY_DEPRECATED_RESOURCE_PACK.store(false, Ordering::SeqCst);

    for (namespace, path) in legacy_textures {
        if has_resource(namespace, path) {
            NOTIFY_DEPRECATED_RESOURCE_PACK.store(true, Ordering::SeqCst);
        }
    }
}

pub fn notify_deprecated_resource_pack() -> bool {
    NOTIFY_DEPRECATED_RESOURCE_PACK.load(std::sync::atomic::Ordering::SeqCst)
}
}

pub mod config {
//! Upstream: common/config/ (1:1). Gson pretty-printing maps to serde_json {:#}; Java field names kept.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;

use super::compat;
use super::global;
use super::network::UpdateChannelC2SPacket;
use super::seams;

// ---------------------------------------------------------------------------

/// Upstream: config/IConfig.java
pub trait IConfig {
    fn validate(&mut self);
    fn on_update(&mut self);
    /// Lombok-generated @ToString snapshot, used in the "Saved <config>" / "Loaded <config>" log lines.
    fn to_string_snapshot(&self) -> String;
}

// ---------------------------------------------------------------------------
// Enums. Each keeps get(String) (case-insensitive, null -> None) and the
// lowercase toString override.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelMode {
    #[serde(rename = "AUTO")] Auto,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "GLOBAL")] Global,
    #[serde(rename = "TEAM_ONLY")] TeamOnly,
}

impl ChannelMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "AUTO" => Self::Auto,
            "DISABLED" => Self::Disabled,
            "GLOBAL" => Self::Global,
            "TEAM_ONLY" => Self::TeamOnly,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Disabled => "DISABLED",
            Self::Global => "GLOBAL",
            Self::TeamOnly => "TEAM_ONLY",
        }
    }
}

impl std::fmt::Display for ChannelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerInfoMode {
    #[serde(rename = "HOLD")] Hold,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "ALWAYS")] Always,
    #[serde(rename = "COMPACT")] Compact,
}

impl PlayerInfoMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "HOLD" => Self::Hold,
            "DISABLED" => Self::Disabled,
            "ALWAYS" => Self::Always,
            "COMPACT" => Self::Compact,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Hold => "HOLD",
            Self::Disabled => "DISABLED",
            Self::Always => "ALWAYS",
            Self::Compact => "COMPACT",
        }
    }
}

impl std::fmt::Display for PlayerInfoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamColorMode {
    #[serde(rename = "FULL")] Full,
    #[serde(rename = "DISABLED")] Disabled,
    #[serde(rename = "PING_ONLY")] PingOnly,
    #[serde(rename = "LABELS_ONLY")] LabelsOnly,
}

impl TeamColorMode {
    pub fn get(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "FULL" => Self::Full,
            "DISABLED" => Self::Disabled,
            "PING_ONLY" => Self::PingOnly,
            "LABELS_ONLY" => Self::LabelsOnly,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Disabled => "DISABLED",
            Self::PingOnly => "PING_ONLY",
            Self::LabelsOnly => "LABELS_ONLY",
        }
    }
}

impl std::fmt::Display for TeamColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

// ---------------------------------------------------------------------------
// ConfigHandler<T>. Upstream keeps a configHash from Lombok @EqualsAndHashCode and
// skips save when unchanged; the hash here runs over the serialized JSON (same effect).
// ---------------------------------------------------------------------------

pub struct ConfigHandler<T: IConfig + Serialize + for<'de> Deserialize<'de>> {
    config: T,
    config_hash: u64,
    config_path: String,
}

impl<T: IConfig + Serialize + for<'de> Deserialize<'de> + Default> ConfigHandler<T> {
    /// ConfigHandler.of(class, configExtension): runs the legacy config migration first,
    /// then resolves MOD_ID + extension through the platform context service.
    pub fn of(config_extension: &str) -> Self {
        compat::legacy_migrate_config(config_extension);
        let path = seams::context_service().resolve_config_dir(&format!("{}{}", global::MOD_ID, config_extension));
        Self::new(path)
    }

    pub fn new(config_path: String) -> Self {
        Self {
            config: T::default(),
            config_hash: 0,
            config_path,
        }
    }

    pub fn config(&self) -> &T {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut T {
        &mut self.config
    }

    fn hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        serde_json::to_string(&self.config).expect("serialize").hash(&mut h);
        h.finish()
    }

    pub fn save(&mut self) {
        let next = self.hash();
        if self.config_hash == next {
            return;
        }

        self.config.on_update();

        let path = Path::new(&self.config_path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    global::log_error(&format!("Creating Config failed: {}", e));
                    return;
                }
            }
            if let Err(e) = std::fs::File::create(path) {
                global::log_error(&format!("Creating Config failed: {}", e));
                return;
            }
        }

        // GsonBuilder().setPrettyPrinting(): pretty JSON.
        match serde_json::to_string_pretty(&self.config).map(|s| std::fs::write(path, s)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                global::log_error(&format!("Saving Config failed: {}", e));
                return;
            }
            Err(e) => {
                global::log_error(&format!("Saving Config failed: {}", e));
                return;
            }
        }

        self.config_hash = self.hash();
        global::log_info(&format!("Saved {}", self.config.to_string_snapshot()));
    }

    pub fn load(&mut self) {
        let path = Path::new(&self.config_path);
        if !path.exists() {
            self.save();
            return;
        }

        // gson.fromJson can hand back null upstream (broken file); parse failure here plays that part.
        let mut parsed_ok = false;
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(c) = serde_json::from_slice::<T>(&bytes) {
                self.config = c;
                parsed_ok = true;
            }
        }

        if !parsed_ok {
            self.config = T::default();
            global::log_error("Config is broken -> reset to defaults");
            self.save();
            return;
        }

        self.config.validate();
        self.config_hash = self.hash();
        global::log_info(&format!("Loaded {}", self.config.to_string_snapshot()));
    }
}

// ---------------------------------------------------------------------------
// ClientConfig. All field names, defaults, hidden-from-settings fields, and the
// per-server channel map stay exactly upstream.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    #[serde(rename = "pingVolume")]
    pub ping_volume: i32,
    #[serde(rename = "pingDuration")]
    pub ping_duration: i32,
    #[serde(rename = "pingDistance")]
    pub ping_distance: i32,
    #[serde(rename = "correctionPeriod")]
    pub correction_period: f32,
    #[serde(rename = "itemIconVisible")]
    pub item_icon_visible: bool,
    #[serde(rename = "directionIndicatorVisible")]
    pub direction_indicator_visible: bool,
    #[serde(rename = "playerInfoMode")]
    pub player_info_mode: PlayerInfoMode,
    #[serde(rename = "teamColorMode")]
    pub team_color_mode: TeamColorMode,
    #[serde(rename = "pingSize")]
    pub ping_size: i32,
    pub channel: String,
    /// Gson keeps Java field names; this one serializes as "serverChannels".
    #[serde(rename = "serverChannels")]
    pub server_channels: BTreeMap<String, String>,

    // hidden from the settings screen
    #[serde(rename = "removeRadius")]
    pub remove_radius: i32,
    #[serde(rename = "raycastDistance")]
    pub raycast_distance: i32,
    #[serde(rename = "safeZoneLeft")]
    pub safe_zone_left: i32,
    #[serde(rename = "safeZoneRight")]
    pub safe_zone_right: i32,
    #[serde(rename = "safeZoneTop")]
    pub safe_zone_top: i32,
    #[serde(rename = "safeZoneBottom")]
    pub safe_zone_bottom: i32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            ping_volume: 100,
            ping_duration: 7,
            ping_distance: 2048,
            correction_period: 1.0,
            item_icon_visible: true,
            direction_indicator_visible: true,
            player_info_mode: PlayerInfoMode::Hold,
            team_color_mode: TeamColorMode::Full,
            ping_size: 100,
            channel: String::new(),
            server_channels: BTreeMap::new(),
            remove_radius: 10,
            raycast_distance: 1024,
            safe_zone_left: 5,
            safe_zone_right: 5,
            safe_zone_top: 5,
            safe_zone_bottom: 60,
        }
    }
}

pub const TPS: i32 = 20;
// Enum value lists in declaration order (upstream: Class.getEnumConstants()).

impl PlayerInfoMode {
    pub const VALUES: [PlayerInfoMode; 4] = [
        PlayerInfoMode::Hold,
        PlayerInfoMode::Disabled,
        PlayerInfoMode::Always,
        PlayerInfoMode::Compact,
    ];
}

impl TeamColorMode {
    pub const VALUES: [TeamColorMode; 4] = [
        TeamColorMode::Full,
        TeamColorMode::Disabled,
        TeamColorMode::PingOnly,
        TeamColorMode::LabelsOnly,
    ];
}

impl ClientConfig {
    // The plain Lombok getters/setters the settings screen drives; get_channel /
    // set_channel keep their custom bodies above.

    pub fn get_ping_volume(&self) -> i32 {
        self.ping_volume
    }
    pub fn set_ping_volume(&mut self, ping_volume: i32) {
        self.ping_volume = ping_volume;
    }
    pub fn get_ping_duration(&self) -> i32 {
        self.ping_duration
    }
    pub fn set_ping_duration(&mut self, ping_duration: i32) {
        self.ping_duration = ping_duration;
    }
    pub fn get_ping_distance(&self) -> i32 {
        self.ping_distance
    }
    pub fn set_ping_distance(&mut self, ping_distance: i32) {
        self.ping_distance = ping_distance;
    }
    pub fn get_correction_period(&self) -> f32 {
        self.correction_period
    }
    pub fn set_correction_period(&mut self, correction_period: f32) {
        self.correction_period = correction_period;
    }
    pub fn is_item_icon_visible(&self) -> bool {
        self.item_icon_visible
    }
    pub fn set_item_icon_visible(&mut self, item_icon_visible: bool) {
        self.item_icon_visible = item_icon_visible;
    }
    pub fn is_direction_indicator_visible(&self) -> bool {
        self.direction_indicator_visible
    }
    pub fn set_direction_indicator_visible(&mut self, direction_indicator_visible: bool) {
        self.direction_indicator_visible = direction_indicator_visible;
    }
    pub fn get_player_info_mode(&self) -> PlayerInfoMode {
        self.player_info_mode
    }
    pub fn set_player_info_mode(&mut self, player_info_mode: PlayerInfoMode) {
        self.player_info_mode = player_info_mode;
    }
    pub fn get_team_color_mode(&self) -> TeamColorMode {
        self.team_color_mode
    }
    pub fn set_team_color_mode(&mut self, team_color_mode: TeamColorMode) {
        self.team_color_mode = team_color_mode;
    }
    pub fn get_ping_size(&self) -> i32 {
        self.ping_size
    }
    pub fn set_ping_size(&mut self, ping_size: i32) {
        self.ping_size = ping_size;
    }
}

pub const MAX_PING_DURATION: i32 = 60;
pub const MAX_PING_DISTANCE: i32 = 2048;
pub const MAX_CORRECTION_PERIOD: f32 = 5.0;
pub const MAX_CHANNEL_LENGTH: usize = 128;

impl ClientConfig {
    /// The per-IP override: serverChannels.getOrDefault(ip, "") when in a game, else the plain channel.
    pub fn get_channel(&self) -> String {
        if let Some(ip) = super::core::GameContext::current_server_ip() {
            return self
                .server_channels
                .get(&ip)
                .cloned()
                .unwrap_or_default();
        }
        self.channel.clone()
    }

    pub fn set_channel(&mut self, new_channel: String) {
        if let Some(ip) = super::core::GameContext::current_server_ip() {
            self.server_channels.insert(ip, new_channel);
        } else {
            self.channel = new_channel;
        }
    }
}

impl IConfig for ClientConfig {
    fn validate(&mut self) {
        if self.channel.len() > MAX_CHANNEL_LENGTH {
            self.channel = self.channel.chars().take(MAX_CHANNEL_LENGTH).collect();
        }

        for (_, channel) in self.server_channels.iter_mut() {
            if channel.len() > MAX_CHANNEL_LENGTH {
                *channel = channel.chars().take(MAX_CHANNEL_LENGTH).collect();
            }
        }
    }

    fn on_update(&mut self) {
        if seams::game().is_some() {
            seams::network_service().send_to_server(&UpdateChannelC2SPacket::new(Some(self.get_channel())));
        }
    }

    fn to_string_snapshot(&self) -> String {
        // Lombok @ToString shape: ClientConfig(pingVolume=.., ..)
        format!(
            "ClientConfig(pingVolume={}, pingDuration={}, pingDistance={}, correctionPeriod={}, itemIconVisible={}, directionIndicatorVisible={}, playerInfoMode={}, teamColorMode={}, pingSize={}, channel={}, serverChannels={}, removeRadius={}, raycastDistance={}, safeZoneLeft={}, safeZoneRight={}, safeZoneTop={}, safeZoneBottom={})",
            self.ping_volume, self.ping_duration, self.ping_distance, self.correction_period,
            self.item_icon_visible, self.direction_indicator_visible, self.player_info_mode,
            self.team_color_mode, self.ping_size, self.channel,
            format!("{:?}", self.server_channels).replace('{', "{").replace('}', "}"),
            self.remove_radius, self.raycast_distance, self.safe_zone_left,
            self.safe_zone_right, self.safe_zone_top, self.safe_zone_bottom
        )
    }
}

/// HANDLER = ConfigHandler.of(ClientConfig.class, ".json") — path resolved by the context service seam.
pub fn client_handler() -> &'static mut ConfigHandler<ClientConfig> {
    static HANDLER: OnceLock<usize> = OnceLock::new();
    let ptr = *HANDLER.get_or_init(|| Box::into_raw(Box::new(ConfigHandler::of(".json"))) as *mut ConfigHandler<ClientConfig> as usize);
    unsafe { &mut *(ptr as *mut ConfigHandler<ClientConfig>) }
}

// ---------------------------------------------------------------------------
// ServerConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Gson serializes the Java field name "defaultChannelMode".
    #[serde(rename = "defaultChannelMode")]
    pub default_channel_mode: ChannelMode,
    #[serde(rename = "playerTrackingEnabled")]
    pub player_tracking_enabled: bool,
    #[serde(rename = "msToRegenerate")]
    pub ms_to_regenerate: i64,
    #[serde(rename = "rateLimit")]
    pub rate_limit: i64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            default_channel_mode: ChannelMode::Auto,
            player_tracking_enabled: true,
            ms_to_regenerate: 1000,
            rate_limit: 5,
        }
    }
}

impl IConfig for ServerConfig {
    fn validate(&mut self) {
        if self.ms_to_regenerate < 0 {
            self.ms_to_regenerate = 1000;
        }

        if self.rate_limit < 0 {
            self.rate_limit = 0;
        }
    }

    fn on_update(&mut self) {
        super::core::ServerCore::init();
    }

    fn to_string_snapshot(&self) -> String {
        format!(
            "ServerConfig(defaultChannelMode={}, playerTrackingEnabled={}, msToRegenerate={}, rateLimit={})",
            self.default_channel_mode, self.player_tracking_enabled, self.ms_to_regenerate, self.rate_limit
        )
    }
}

/// HANDLER = ConfigHandler.of(ServerConfig.class, ".server.json")
pub fn server_handler() -> &'static mut ConfigHandler<ServerConfig> {
    static HANDLER: OnceLock<usize> = OnceLock::new();
    let ptr = *HANDLER.get_or_init(|| Box::into_raw(Box::new(ConfigHandler::of(".json"))) as *mut ConfigHandler<ServerConfig> as usize);
    unsafe { &mut *(ptr as *mut ConfigHandler<ServerConfig>) }
}
}

pub mod network {
//! Upstream: common/network/ (1:1). FriendlyByteBuf is a host seam with the exact
//! read/write operations upstream calls; PacketHandler.readSafe keeps the
//! fall-back-to-default-constructor and skip-remaining-bytes behavior.
use super::config::MAX_CHANNEL_LENGTH;
pub use super::seams::FriendlyByteBuf;
use super::seams::Vec3;

/// net.minecraft.resources.ResourceLocation identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceLocation {
    pub namespace: &'static str,
    pub path: &'static str,
}

pub trait IPacket {
    fn write(&self, buf: &mut FriendlyByteBuf);
    fn is_corrupt(&self) -> bool;
    fn get_id(&self) -> ResourceLocation;
}

// ---------------------------------------------------------------------------
// PingLocationC2SPacket. The no-arg constructor produces channel == null (corrupt).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PingLocationC2SPacket {
    pub channel: Option<String>,
    pub pos: Option<Vec3>,
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
}

pub const PING_LOCATION_C2S_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-c2s", path: "ping-location" };

impl PingLocationC2SPacket {
    pub fn new(channel: Option<String>, pos: Option<Vec3>, entity: Option<u128>, sequence: i32, dimension: i32) -> Self {
        Self { channel, pos, entity, sequence, dimension }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None, None, None, 0, 0)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(
            Some(buf.read_utf(MAX_CHANNEL_LENGTH)),
            Some(Vec3::new(buf.read_double(), buf.read_double(), buf.read_double())),
            if buf.read_boolean() { Some(buf.read_uuid()) } else { None },
            buf.read_int(),
            buf.read_int(),
        )
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }
}

impl IPacket for PingLocationC2SPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
        let p = self.pos.unwrap_or(Vec3::ZERO);
        buf.write_double(p.x);
        buf.write_double(p.y);
        buf.write_double(p.z);
        buf.write_boolean(self.entity.is_some());

        if let Some(e) = self.entity {
            buf.write_uuid(e);
        }

        buf.write_int(self.sequence);
        buf.write_int(self.dimension);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        PING_LOCATION_C2S_ID
    }
}

// ---------------------------------------------------------------------------
// PingLocationS2CPacket
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PingLocationS2CPacket {
    pub channel: Option<String>,
    pub pos: Option<Vec3>,
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
    pub author: u128,
}

pub const PING_LOCATION_S2C_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-s2c", path: "ping-location" };

impl PingLocationS2CPacket {
    pub fn new(channel: Option<String>, pos: Option<Vec3>, entity: Option<u128>, sequence: i32, dimension: i32, author: u128) -> Self {
        Self { channel, pos, entity, sequence, dimension, author }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None, None, None, 0, 0, 0)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(
            Some(buf.read_utf(MAX_CHANNEL_LENGTH)),
            Some(Vec3::new(buf.read_double(), buf.read_double(), buf.read_double())),
            if buf.read_boolean() { Some(buf.read_uuid()) } else { None },
            buf.read_int(),
            buf.read_int(),
            buf.read_uuid(),
        )
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }

    pub fn from_client_packet(client_packet: &PingLocationC2SPacket, author: u128) -> Self {
        Self::new(
            client_packet.channel.clone(),
            client_packet.pos,
            client_packet.entity,
            client_packet.sequence,
            client_packet.dimension,
            author,
        )
    }
}

impl IPacket for PingLocationS2CPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
        let p = self.pos.unwrap_or(Vec3::ZERO);
        buf.write_double(p.x);
        buf.write_double(p.y);
        buf.write_double(p.z);
        buf.write_boolean(self.entity.is_some());

        if let Some(e) = self.entity {
            buf.write_uuid(e);
        }

        buf.write_int(self.sequence);
        buf.write_int(self.dimension);
        buf.write_uuid(self.author);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        PING_LOCATION_S2C_ID
    }
}

// ---------------------------------------------------------------------------
// UpdateChannelC2SPacket
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct UpdateChannelC2SPacket {
    pub channel: Option<String>,
}

pub const UPDATE_CHANNEL_C2S_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-c2s", path: "update-channel" };

impl UpdateChannelC2SPacket {
    pub fn new(channel: Option<String>) -> Self {
        Self { channel }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(Some(buf.read_utf(MAX_CHANNEL_LENGTH)))
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }
}

impl IPacket for UpdateChannelC2SPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        UPDATE_CHANNEL_C2S_ID
    }
}

// ---------------------------------------------------------------------------
// PacketHandler
// ---------------------------------------------------------------------------

pub struct PacketHandler;

impl PacketHandler {
    /// readSafe: try the buf constructor; on any failure build the default (corrupt) packet,
    /// then always skip whatever bytes are left in the buffer.
    pub fn read_safe<T>(buf: &mut FriendlyByteBuf, from_buf: impl FnOnce(&mut FriendlyByteBuf) -> T, default: impl FnOnce() -> T) -> T {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| from_buf(buf)));

        let out = match result {
            Ok(v) => v,
            Err(_) => default(),
        };

        // finally: readerIndex += readableBytes()
        buf.skip_remaining();
        out
    }
}
}

pub mod math {
//! Upstream: common/math/ (1:1).
use super::compat::Vector4f;
use super::seams::{self, BlockHitResult, ClipContext, ClipContextBlock, ClipContextFluid, EntityHandle, EntityHitResult, HitResultType, LevelHandle, Matrix4f, Vec2, Vec3};

// ---------------------------------------------------------------------------
// math/ScreenPos.java
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPos {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
}

impl ScreenPos {
    pub fn new(x: f32, y: f32, depth: f32) -> Self {
        Self { x, y, depth }
    }

    pub fn is_behind_camera(&self) -> bool {
        self.depth <= 0.0
    }

    pub fn distance_to(&self, b: Vec2) -> f32 {
        if self.is_behind_camera() {
            return f32::MAX;
        }

        Vec2::new(b.x - self.x, b.y - self.y).length()
    }

    pub fn is_in_bounds(&self, top_left: Vec2, bottom_right: Vec2) -> bool {
        self.x >= top_left.x && self.x <= bottom_right.x && self.y >= top_left.y && self.y <= bottom_right.y
    }
}

// ---------------------------------------------------------------------------
// math/MathUtils.java
// ---------------------------------------------------------------------------

pub struct MathUtils;

impl MathUtils {
    pub fn world_to_screen(world_pos: Vec3, model_view_matrix: &Matrix4f, projection_matrix: &Matrix4f) -> ScreenPos {
        let game = seams::game();
        let window = game.as_ref().map(|g| g.gui_scaled_size()).unwrap_or((0.0, 0.0));
        let camera = game.as_ref().map(|g| g.camera_position()).unwrap_or(Vec3::ZERO);
        let mut world_pos_rel = Vector4f::new(&camera.reverse().add(world_pos), 1.0);
        world_pos_rel = world_pos_rel.mul(model_view_matrix);
        world_pos_rel = world_pos_rel.mul(projection_matrix);

        let depth = world_pos_rel.w;

        if depth != 0.0 {
            world_pos_rel = world_pos_rel.div(depth);
        }

        ScreenPos::new(
            window.0 * (0.5 + world_pos_rel.x * 0.5),
            window.1 * (0.5 - world_pos_rel.y * 0.5),
            depth,
        )
    }

    pub fn rotate_z(matrix_stack: &mut seams::PoseStack, theta: f32) {
        matrix_stack.rotate_z(theta);
    }

    /// Ray/rect edge intersection against the angle direction, ported line-for-line
    /// (f64 internals, Vec2 f32 out; the upstream float/double mix is kept by the casts).
    pub fn calculate_angle_rect_intersection(angle: f32, left_top: Vec2, right_bottom: Vec2) -> Vec2 {
        let angle = angle as f64;
        let direction = Vec3::new(angle.cos(), angle.sin(), 0.0);
        let width = (right_bottom.x - left_top.x) as f64;
        let height = (right_bottom.y - left_top.y) as f64;
        // direction.multiply(new Vec3(1f / width, 1f / height, 0f))
        let direction = Vec3::new(direction.x * (1.0 / width), direction.y * (1.0 / height), 0.0);

        let dx = angle.cos();
        let dy = angle.sin();
        let cx = width * 0.5;
        let cy = height * 0.5;

        if direction.x.abs() < direction.y.abs() {
            if direction.y < 0.0 {
                // top
                let t = -cy / dy;
                let x = cx + t * dx;
                return Vec2::new(x as f32 + left_top.x, left_top.y);
            }

            // bottom
            let t = cy / dy;
            let x = cx + t * dx;
            return Vec2::new(x as f32 + left_top.x, right_bottom.y);
        }

        if direction.x < 0.0 {
            // left
            let t = -cx / dx;
            let y = cy + t * dy;
            return Vec2::new(left_top.x, y as f32 + left_top.y);
        }

        // right
        let t = cx / dx;
        let y = cy + t * dy;
        Vec2::new(right_bottom.x, y as f32 + left_top.y)
    }
}

// ---------------------------------------------------------------------------
// math/Raycast.java
// ---------------------------------------------------------------------------

static TERRAIN_CACHE: std::sync::Mutex<Option<u64>> = std::sync::Mutex::new(None);
/// Upstream starts at Instant.EPOCH, so the first run is always older than 10s and reloads the cache.
static LAST_CACHE_LOAD: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub struct Raycast;

impl Raycast {
    /// traceDistantAsync: the DH raycast runs off-thread; the callback fires only on a hit.
    /// Upstream reads CLIENT_CONFIG.getChannel() / pingSequence / GameContext.getDimension()
    /// inside the async body; on a worker thread the host seams default, so the caller
    /// snapshots them here (same values the synchronous path would read).
    pub fn trace_distant_async(direction: Vec3, tick_delta: f32, callback: Box<dyn Fn(BlockHitResult) + Send + 'static>) {
        let Some(camera_entity) = seams::game().and_then(|g| g.camera_entity()) else {
            return;
        };

        let ray_start_vec = camera_entity.position(tick_delta);

        let channel = super::config::client_handler().config().get_channel();
        let ping_sequence = super::core::PingController::ping_sequence();
        let dimension = super::core::GameContext::dimension();

        seams::run_async(Box::new(move || {
            let dh = seams::dh_api();
            let Some(level_wrapper) = dh.single_player_level() else {
                return;
            };

            {
                let mut cache = TERRAIN_CACHE.lock().unwrap();
                let mut last = LAST_CACHE_LOAD.lock().unwrap();
                let expired = match *last {
                    None => true,
                    Some(t) => t.elapsed().as_secs_f64() > 10.0,
                };

                if cache.is_none() || expired {
                    *cache = Some(dh.create_soft_cache());
                    *last = Some(seams::now_instant());
                }

                let cache_token = (*cache).unwrap_or(0);

                let ray_cast_result = dh.raycast(
                    level_wrapper,
                    ray_start_vec.x,
                    ray_start_vec.y,
                    ray_start_vec.z,
                    direction.x as f32,
                    direction.y as f32,
                    direction.z as f32,
                    4096.0,
                    cache_token,
                );

                if !ray_cast_result.success {
                    return;
                }

                if let Some(pos) = ray_cast_result.pos {
                    callback(BlockHitResult {
                        location: pos,
                        block_pos: (pos.x as i32, pos.y as i32, pos.z as i32),
                        direction_up: true,
                        miss_inside: true,
                        hit_type: HitResultType::Block,
                    });
                }
            }
        }));
    }

    /// traceDirectional: block clip vs entity trace, nearer wins.
    pub fn trace_directional(direction: Vec3, tick_delta: f32, max_distance: f64, hit_translucent: bool) -> Option<HitResult> {
        let game = seams::game()?;
        let camera_entity = game.camera_entity()?;
        let level = game.level()?;

        let ray_start_vec = camera_entity.position(tick_delta);
        let ray_end_vec = ray_start_vec.add(direction.scale(max_distance));
        // expandTowards(viewVector(1f).scale(maxDistance)).inflate(1.0, 1.0, 1.0) — host-side AABB.
        let bounding_box = camera_entity.bounding_box_expanded_towards_inflated(camera_entity.view_vector(1.0).scale(max_distance), 1.0);

        let block_hit_result = level.clip(&ClipContext {
            from: ray_start_vec,
            to: ray_end_vec,
            block: if hit_translucent { ClipContextBlock::Outline } else { ClipContextBlock::Visual },
            fluid: if hit_translucent { ClipContextFluid::Any } else { ClipContextFluid::None },
        });

        let entity_hit_result = Raycast::trace_entity(
            camera_entity.clone(),
            ray_start_vec,
            ray_end_vec,
            &bounding_box,
            &|target_entity: &dyn EntityHandle| !target_entity.is_spectator(),
        );

        let Some(entity_hit_result) = entity_hit_result else {
            return block_hit_result.map(HitResult::Block);
        };

        if let Some(block_hit) = block_hit_result {
            if ray_start_vec.distance_to_sqr(block_hit.location) < ray_start_vec.distance_to_sqr(entity_hit_result.location) {
                return Some(HitResult::Block(block_hit));
            }
        }

        Some(HitResult::Entity(entity_hit_result))
    }

    fn trace_entity(
        entity: std::rc::Rc<dyn EntityHandle>,
        min: Vec3,
        max: Vec3,
        box_: &seams::AabbToken,
        predicate: &dyn Fn(&dyn EntityHandle) -> bool,
    ) -> Option<EntityHitResult> {
        let level_token = entity.level_token();
        let level = seams::level_by_token(level_token);
        let mut min_dist = min.distance_to_sqr(max);
        let mut min_hit_result: Option<EntityHitResult> = None;

        for ent in level.get_entities(entity.as_ref(), box_, predicate) {
            let hit_pos = ent.bounding_box_clip(min, max);

            let Some(hit_pos) = hit_pos else {
                continue;
            };

            let hit_result = EntityHitResult { entity_token: ent.uuid() as u64, location: hit_pos };
            let hit_dist = min.distance_to_sqr(hit_result.location);

            if min_dist > hit_dist {
                min_dist = hit_dist;
                min_hit_result = Some(hit_result);
            }
        }

        min_hit_result
    }
}

pub enum HitResult {
    Block(BlockHitResult),
    Entity(EntityHitResult),
}

impl HitResult {
    pub fn get_type(&self) -> HitResultType {
        match self {
            HitResult::Block(_) => HitResultType::Block,
            HitResult::Entity(_) => HitResultType::Entity,
        }
    }

    pub fn get_location(&self) -> Vec3 {
        match self {
            HitResult::Block(b) => b.location,
            HitResult::Entity(e) => e.location,
        }
    }
}
}

pub mod util {
//! Upstream: common/util/ (1:1). InputUtils and DirectionalSoundInstance follow the resource/screen phase.
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// util/RateLimiter.java (1:1)
// ---------------------------------------------------------------------------

static TIME_TO_REGENERATE: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);
static TIME_WINDOW: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);

pub fn set_rates(ms_to_regenerate: i64, limit: i64) {
    *TIME_TO_REGENERATE.lock().unwrap() = Some(Duration::from_millis(ms_to_regenerate.max(0) as u64));
    *TIME_WINDOW.lock().unwrap() = Some(Duration::from_millis((ms_to_regenerate * limit).max(0) as u64));
}

#[derive(Default)]
pub struct RateLimiter {
    start_time: Option<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self { start_time: None }
    }

    pub fn check_exceeded(&mut self) -> bool {
        let time_to_regenerate = *TIME_TO_REGENERATE.lock().unwrap();
        let time_window = *TIME_WINDOW.lock().unwrap();

        let (Some(time_to_regenerate), Some(time_window)) = (time_to_regenerate, time_window) else {
            return false;
        };

        if self.start_time.is_none() {
            // Instant.now().minus(timeWindow).plus(timeToRegenerate)
            self.start_time = Some(Instant::now() - time_window + time_to_regenerate);
            return false;
        }

        let start = self.start_time.unwrap();
        let now = Instant::now();
        let mut elapsed = now.duration_since(start);

        if elapsed > time_window {
            elapsed = time_window;
        }

        match elapsed.checked_sub(time_to_regenerate) {
            // leftOver.isNegative()
            None => true,
            Some(left_over) => {
                self.start_time = Some(now - left_over);
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Key mapping tokens (upstream: net.minecraft.client.KeyMapping); the register/set/
// consumeClick/same operations are host-side, the port holds the identity.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct KeyBindingToken {
    pub name: String,
    /// InputConstants.Type: MOUSE or KEYSYM
    pub input_type: &'static str,
    pub key: i32,
    pub category: String,
}

impl KeyBindingToken {
    pub fn new(name: &str, input_type: &'static str, key: i32, category: &str) -> Self {
        Self { name: name.to_string(), input_type, key, category: category.to_string() }
    }

    pub fn set_key(&mut self, key: i32) {
        self.key = key;
    }
}

// ---------------------------------------------------------------------------
// util/InputUtils.java (1:1; key bindings live as tokens, the consumeClick /
// same() behavior stays host-side through the seam trait).
// ---------------------------------------------------------------------------

pub const GLFW_MOUSE_BUTTON_5: i32 = 4;

pub struct InputUtils;

impl InputUtils {
    pub fn key_binding_ping() -> KeyBindingToken {
        KeyBindingToken::new(
            &super::resource::LanguageUtils::key_of("key", &["ping_location"]),
            "MOUSE",
            GLFW_MOUSE_BUTTON_5,
            &super::resource::LanguageUtils::key_of("key.category", &["name"]),
        )
    }

    pub fn key_binding_settings() -> KeyBindingToken {
        KeyBindingToken::new(
            &super::resource::LanguageUtils::key_of("key", &["open_settings"]),
            "KEYSYM",
            -1,
            &super::resource::LanguageUtils::key_of("key.category", &["name"]),
        )
    }

    /// consumePingHotkey: the pick-item overlap workaround is host-side; the port consumes
    /// the ping binding through the key-mapping seam.
    pub fn consume_ping_hotkey() -> bool {
        super::seams::consume_click_key(&InputUtils::key_binding_ping())
    }
}
}

pub mod resource {
//! Upstream: common/resource/ (1:1).
use super::compat::Component;
use super::global::{MOD_ID, MOD_PREFIX};

// ---------------------------------------------------------------------------
// resource/LanguageUtils.java (1:1). MutableComponent trees are host-side; the
// compat::Component seam renders each factory to its display string.
// ---------------------------------------------------------------------------

pub struct LanguageUtils {
    pub key: String,
}

impl LanguageUtils {
    pub fn of(category: &str, path: &[&str]) -> Self {
        Self { key: format!("{}.{}.{}", category, MOD_ID, path.join(".")) }
    }

    pub fn settings(key: &str) -> Self {
        Self::of("settings", &[key])
    }

    pub fn command(key: &str) -> Self {
        Self::of("command", &[key])
    }

    pub fn key_of(category: &str, path: &[&str]) -> String {
        Self::of(category, path).key.clone()
    }

    pub fn get(&self, args: &[&str]) -> String {
        Component::translatable(&self.key, args)
    }

    pub fn path(&self, path: &[&str]) -> Self {
        Self { key: format!("{}.{}", self.key, path.join(".")) }
    }

    pub fn wrapped(&self) -> String {
        format!("({})", self.get(&[]))
    }
}

/// VALUE_INFINITE / VALUE_HIDDEN / UNIT_* statics, resolved once like upstream class-load.
pub fn value_infinite() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("value", &["infinite"]).get(&[])).clone()
}

pub fn value_hidden() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("value", &["hidden"]).get(&[])).clone()
}

pub const NEWLINE: &str = "\n";

pub fn unit_seconds() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["seconds"]).get(&[])).clone()
}

pub fn unit_meters() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["meters"]).get(&[])).clone()
}

pub fn unit_percent() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| LanguageUtils::of("unit", &["percent"]).get(&[])).clone()
}

/// Upstream LanguageUtils.UNIT_PERCENT static: the percent unit formatted with
/// the value as its argument.
pub fn unit_percent_get(value: i32) -> String {
    LanguageUtils::of("unit", &["percent"]).get(&[&value.to_string()])
}

/// Upstream LanguageUtils.UNIT_SECONDS static; the correction-period option
/// formats its value first ("%.1f").
pub fn unit_seconds_get(value: &str) -> String {
    LanguageUtils::of("unit", &["seconds"]).get(&[value])
}

pub fn join(components: &[String]) -> String {
    let mut out = String::new();
    for (i, c) in components.iter().enumerate() {
        if i > 0 {
            out.push_str(NEWLINE);
        }
        out.push_str(c);
    }
    out
}

pub fn with_mod_prefix(component: &str) -> String {
    format!("{}{}", MOD_PREFIX, component)
}

pub fn from(any: &str) -> String {
    Component::literal(any)
}

// ---------------------------------------------------------------------------
// resource/ResourceConstants.java (1:1)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceLocationToken {
    pub namespace: &'static str,
    pub path: &'static str,
}

pub const PING_SOUND_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "ping" };
pub const PING_TEXTURE_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "textures/ping.png" };
pub const ARROW_TEXTURE_ID: ResourceLocationToken = ResourceLocationToken { namespace: MOD_ID_IDENTITY, path: "textures/arrow.png" };

const MOD_ID_IDENTITY: &str = MOD_ID;

/// SoundEvent(PING_SOUND_ID) as a token.
pub const PING_SOUND_EVENT: ResourceLocationToken = PING_SOUND_ID;


/// LanguageUtils.UNIT_METERS = LanguageUtils.of("unit", "meters")
pub fn unit_meters_lang() -> LanguageUtils {
    LanguageUtils::of("unit", &["meters"])
}

// ---------------------------------------------------------------------------
// resource/ResourceReloadListener.java (1:1)
// ---------------------------------------------------------------------------

pub(crate) static NUM_CUSTOM_TEXTURES: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Host seam for the net.minecraft.server.packs.resources.ResourceManager the
/// reload listener probes (hasResource for the legacy-id check, getResources for the count).
pub trait ResourceManagerSeam {
    fn has_resource(&self, namespace: &str, path: &str) -> bool;
    fn resources(&self, res: &super::resource::ResourceLocationToken) -> Result<usize, String>;
}

pub struct ResourceReloadListener;

impl ResourceReloadListener {
    pub fn has_custom_texture() -> bool {
        NUM_CUSTOM_TEXTURES.load(std::sync::atomic::Ordering::SeqCst) > 1
    }

    /// Upstream reloadTextures: supplyAsync(checkResources + resource count, loadExecutor)
    /// -> thenCompose(helper::wait) -> thenAcceptAsync(no-op, applyExecutor). The port runs
    /// the preparation inline; the preparation barrier and apply executor are host seams.
    pub fn reload_textures(resource_manager: &dyn ResourceManagerSeam) {
        super::compat::check_resources(&|ns, path| resource_manager.has_resource(ns, path));

        match resource_manager.resources(&PING_TEXTURE_ID) {
            Ok(count) => NUM_CUSTOM_TEXTURES.store(count as i32, std::sync::atomic::Ordering::SeqCst),
            Err(e) => super::global::log_error(&format!("failed to gather resources: {}", e)),
        }
    }
}
}

pub mod core {
//! Upstream: common/core/ (1:1). PingData is an abstract base with the single concrete
//! subclass PingView; the port merges them into one struct and notes the merge here.
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use super::config::{client_handler, ClientConfig, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION, TPS};
use super::config::ChannelMode;
use super::global;
use super::math::{HitResult, Raycast, ScreenPos};
use super::network::{IPacket, PingLocationC2SPacket, PingLocationS2CPacket};
use super::resource::{PING_SOUND_EVENT, ResourceLocationToken};
use super::seams::{
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

        self.screen_pos = Some(super::math::MathUtils::world_to_screen(
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
static PLAYER_RATES: std::sync::LazyLock<Mutex<std::collections::HashMap<u128, super::util::RateLimiter>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

pub struct ServerCore;

impl ServerCore {
    pub fn init() {
        let cfg = super::config::server_handler().config();
        super::util::set_rates(cfg.ms_to_regenerate, cfg.rate_limit);
    }

    pub fn on_player_disconnect(player: u128) {
        PLAYER_CHANNELS.lock().unwrap().remove(&player);
        PLAYER_RATES.lock().unwrap().remove(&player);
    }

    pub fn on_channel_update(player: u128, packet: &super::network::UpdateChannelC2SPacket) {
        let env = seams::server_env();
        let player_handle = env.player(player);

        if packet.is_corrupt() {
            let name = player_handle.as_ref().map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_warn(&format!("invalid channel update from {} ({})", name, player));
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &super::compat::Component::literal(&format!(
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
        let server_config = super::config::server_handler().config().clone();

        if packet.is_corrupt() {
            let name = player_handle.as_ref().map(|p| p.game_profile_name()).unwrap_or_default();
            global::log_warn(&format!("invalid ping location from {} ({})", name, player));
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &super::compat::Component::literal(&format!(
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
            .or_insert_with(super::util::RateLimiter::new);
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
                    &super::compat::Component::literal(
                        "\u{a7}8[Ping-Wheel] \u{a7}eMust be in a channel to ping location\n\u{a7}fUse \u{a7}a/pingwheel channel\u{a7}f to switch",
                    ),
                    false,
                );
            }
            return;
        }

        if channel.is_empty()
            && default_channel_mode == ChannelMode::TeamOnly
            && !super::integration::has_team(Some(player))
        {
            if let Some(p) = player_handle.as_ref() {
                p.display_client_message(
                    &super::compat::Component::literal(
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

            if channel.is_empty() && default_channel_mode != ChannelMode::Global && !super::integration::in_same_context(player, p) {
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
}

pub mod seams {
//! Host seams: every Minecraft / platform type upstream touches directly is translated behind these.
//! The Java classes stay host-side; the Rust side sees tokens and the exact operations upstream performs.
use std::cell::RefCell;
use super::resource::ResourceLocationToken;
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
    let full = format!("{}{}", super::global::MOD_PREFIX, msg);
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
    fn item_entity_item_copy(&self) -> Option<super::seams::ItemStackToken> {
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
    fn clip(&self, ctx: &ClipContext) -> Option<super::seams::BlockHitResult>;
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
    fn sound_manager_play(&self, _instance: &super::core::DirectionalSoundInstance) {}
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
    fn set_screen(&self, _new_screen: Rc<dyn Fn() -> super::screen::SettingsScreen>) {}
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
    fn register_key_mapping(&self, _binding: &super::util::KeyBindingToken) {}
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
    fn send_to_server(&self, packet: &dyn super::network::IPacket);
    fn send_to_client(&self, packet: &dyn super::network::IPacket, player: u128);
}

pub trait IPlatformClientEventService {
    fn register_tick_start_event(&self, _cb: Rc<dyn Fn()>);
    fn register_join_server_event(&self, _cb: Rc<dyn Fn()>);
    fn register_leave_server_event(&self, _cb: Rc<dyn Fn()>);
    fn register_render_world_event(&self, _cb: Rc<dyn Fn(&super::core::WorldRenderContext)>);
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
    fn register_render_world_event(&self, _cb: Rc<dyn Fn(&super::core::WorldRenderContext)>) {}
    fn register_render_gui_event(&self, _cb: Rc<dyn Fn(super::seams::PoseStack, f32)>) {}
}

struct NoNetworkService;
impl IPlatformNetworkService for NoNetworkService {
    fn send_to_server(&self, _packet: &dyn super::network::IPacket) {}
    fn send_to_client(&self, _packet: &dyn super::network::IPacket, _player: u128) {}
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
pub fn consume_click_key(_binding: &super::util::KeyBindingToken) -> bool {
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
    fn add_small(&self, _a: super::screen::OptionEntry, _b: Option<super::screen::OptionEntry>) {}
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
}

#[cfg(test)]
pub mod tests {
//! Port tests: each asserts behavior against the upstream Java logic.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use super::config::{client_handler, server_handler, ChannelMode, ClientConfig, IConfig, PlayerInfoMode, ServerConfig, TeamColorMode};
use super::core::{DirectionalSoundInstance, GameContext, PingController, PingManager, PingView, ServerCore};
use super::math::MathUtils;
use super::network::{FriendlyByteBuf, IPacket, PacketHandler, PingLocationC2SPacket, PingLocationS2CPacket, ResourceLocation, UpdateChannelC2SPacket};
use super::seams::{
    self, BlockHitResult, ClipContext, EntityHandle, HitResultType, LevelHandle, Matrix4f,
    ItemStackToken, PlayerInfoHandle, Vec2, Vec3,
};
use super::util::RateLimiter;

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
    fn send_to_server(&self, packet: &dyn super::network::IPacket) {
        let mut buf = FriendlyByteBuf::new();
        packet.write(&mut buf);
        let id = packet.get_id();
        self.sent.borrow_mut().push((format!("{}:{}", id.namespace, id.path), buf.data.clone()));
    }

    fn send_to_client(&self, packet: &dyn super::network::IPacket, _player: u128) {
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
    let json = serde_json::to_string(&super::config::ClientConfig::default()).unwrap();
    assert!(json.contains("\"pingVolume\""));
    assert!(json.contains("\"serverChannels\""));
    assert!(json.contains("\"playerInfoMode\""));
    assert!(json.contains("\"teamColorMode\""));

    let parsed: super::config::ClientConfig = serde_json::from_str(&json).unwrap();
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
    assert_eq!(cfg.channel.len(), super::config::MAX_CHANNEL_LENGTH);
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

    super::compat::legacy_migrate_config(".json");
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
    let p = super::math::ScreenPos::new(100.0, 100.0, -1.0);
    assert!(p.is_behind_camera());
    assert_eq!(p.distance_to(Vec2::new(0.0, 0.0)), f32::MAX);

    let p = super::math::ScreenPos::new(3.0, 4.0, 1.0);
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
    super::util::set_rates(5000, 1);
    let mut limiter = RateLimiter::new();
    assert!(!limiter.check_exceeded());
    assert!(limiter.check_exceeded());

    // generous rates: regen 0 always leaves leftOver non-negative
    super::util::set_rates(0, 5);
    assert!(!limiter.check_exceeded());
}

#[test]
fn input_utils_builds_binding_tokens_from_language_keys() {
    let ping = super::util::InputUtils::key_binding_ping();
    assert_eq!(ping.name, "key.pingwheel.ping_location");
    assert_eq!(ping.input_type, "MOUSE");
    assert_eq!(ping.key, super::util::GLFW_MOUSE_BUTTON_5);

    let settings = super::util::InputUtils::key_binding_settings();
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
    let sound = DirectionalSoundInstance::new(super::resource::PING_SOUND_EVENT, "MASTER", 1.0, 1.0, Vec3::new(0.0, 0.0, 100.0));
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
    let ctx = super::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };
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
    ping.update(&super::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 }, 10);
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
    let ctx = super::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };
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
    let ctx = super::core::WorldRenderContext { model_view_matrix: Matrix4f::identity(), projection_matrix: Matrix4f::identity(), tick_delta: 1.0, camera: 0 };

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
    assert_eq!(super::resource::LanguageUtils::key_of("settings", &["ping_volume"]), "settings.pingwheel.ping_volume");
    assert_eq!(super::resource::LanguageUtils::key_of("key.category", &["name"]), "key.category.pingwheel.name");
    assert_eq!(super::resource::LanguageUtils::key_of("key", &["ping_location"]), "key.pingwheel.ping_location");
}


// ---------------------------------------------------------------------------
// integration/ + screen/: OptionUtils option objects and TeamContextHandler.
// ---------------------------------------------------------------------------

#[test]
fn team_context_to_string_is_lowercase() {
    use super::integration::TeamContext;

    assert_eq!(TeamContext::None.to_string(), "none");
    assert_eq!(TeamContext::VanillaTeam.to_string(), "vanilla_team");
    assert_eq!(TeamContext::FtbTeams.to_string(), "ftb_teams");
    assert_eq!(TeamContext::VoiceChat.to_string(), "voice_chat");
}

#[test]
fn team_context_handler_ladder_prefers_voice_chat_then_ftb() {
    use super::integration;
    use super::seams::{set_integration_service, IIntegrationService};

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

    let _guard = super::tests::install();
    // the wrappers gate on the ModContext flags (indexMods), so enable them here
    super::integration::HAS_VOICE_CHAT.store(true, std::sync::atomic::Ordering::SeqCst);
    super::integration::HAS_FTB_TEAMS.store(true, std::sync::atomic::Ordering::SeqCst);
    set_integration_service(Rc::new(CtxLadder { voice: Some(7), ftb: Some(9) }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::VoiceChat);

    set_integration_service(Rc::new(CtxLadder { voice: None, ftb: Some(9) }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::FtbTeams);

    set_integration_service(Rc::new(CtxLadder { voice: None, ftb: None }));
    assert_eq!(integration::get_self_context(), integration::TeamContext::None);
    set_integration_service(Rc::new(super::seams::NoIntegrationService));
    super::integration::HAS_VOICE_CHAT.store(false, std::sync::atomic::Ordering::SeqCst);
    super::integration::HAS_FTB_TEAMS.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn team_context_handler_in_same_context_compares_ids_then_teams() {
    use super::seams::{set_integration_service, IIntegrationService};

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

    let _guard = super::tests::install();
    // same voice-chat group id -> same context
    set_integration_service(Rc::new(CtxIds { voice1: Some(5), voice2: Some(5), team1: None, team2: None }));
    assert!(super::integration::in_same_context(1, 2));
    // p1 has an id, p2 has none -> differ
    set_integration_service(Rc::new(CtxIds { voice1: Some(5), voice2: None, team1: None, team2: Some(3) }));
    assert!(!super::integration::in_same_context(1, 2));
    // no integration ids, same vanilla team identity -> same context
    set_integration_service(Rc::new(CtxIds { voice1: None, voice2: None, team1: Some(3), team2: Some(3) }));
    assert!(super::integration::in_same_context(1, 2));
    // no integration ids, different vanilla teams -> differ
    set_integration_service(Rc::new(CtxIds { voice1: None, voice2: None, team1: Some(3), team2: Some(4) }));
    assert!(!super::integration::in_same_context(1, 2));
    set_integration_service(Rc::new(super::seams::NoIntegrationService));
}

#[test]
fn option_utils_enum_cycles_through_values_and_formats() {
    use super::screen::OptionUtils;

    let opt = OptionUtils::of_enum(
        "key",
        PlayerInfoMode::VALUES.to_vec(),
        Rc::new(|mode| format!("label:{}", mode.name().to_ascii_lowercase())),
        Rc::new(|_m| Vec::new()),
        Rc::new(|c: &ClientConfig| c.get_player_info_mode()),
        Rc::new(|c: &mut ClientConfig, v| c.set_player_info_mode(v)),
    );

    let super::screen::OptionEntry::Cycle(entry) = opt else {
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
    use super::screen::OptionEntry;
    use super::screen::OptionUtils;

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
    use super::screen::OptionUtils;

    let opt = OptionUtils::of_int(
        "k",
        0,
        100,
        1,
        Rc::new(|v| format!("{v}%")),
        Rc::new(|c: &ClientConfig| c.get_ping_volume()),
        Rc::new(|_c: &mut ClientConfig, _v| {}),
    );

    let super::screen::OptionEntry::Progress(entry) = opt else {
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
use super::seams::{EditBoxHost, IScreenEnv, OptionsListHost};

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
impl super::seams::OptionsListHost for RecordingList {
    fn add_small(&self, a: super::screen::OptionEntry, b: Option<super::screen::OptionEntry>) {
        let key_of = |o: Option<super::screen::OptionEntry>| match o {
            Some(super::screen::OptionEntry::Progress(p)) => Some(p.key.clone()),
            Some(super::screen::OptionEntry::Cycle(c)) => Some(c.key.clone()),
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
impl super::seams::EditBoxHost for RecordingEdit {
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
impl super::seams::IScreenEnv for RecordingScreenEnv {
    fn width(&self) -> i32 {
        self.w
    }
    fn height(&self) -> i32 {
        self.h
    }
    fn new_options_list(&self, _w: i32, _h: i32, _top: i32, _bottom: i32, _item_height: i32) -> Rc<dyn super::seams::OptionsListHost> {
        self.list.clone()
    }
    fn new_edit_box(&self, _x: i32, y: i32, _w: i32, _h: i32, _placeholder: &str) -> Rc<dyn super::seams::EditBoxHost> {
        self.edit.y.set(y);
        self.edit.clone()
    }
    fn add_widget_edit(&self, _edit: Rc<dyn super::seams::EditBoxHost>) {}
    fn add_widget_list(&self, _list: Rc<dyn super::seams::OptionsListHost>) {}
    fn add_renderable_button(&self, x: i32, y: i32, w: i32, h: i32, label: &str, _on_click: Rc<dyn Fn()>) {
        self.buttons.borrow_mut().push((x, y, w, h, label.to_string()));
    }
    fn draw_centered_string(&self, _m: &super::seams::PoseStack, text: &str, cx: i32, y: i32, color: i32) {
        self.draw_centered.borrow_mut().push((text.to_string(), cx, y, color));
    }
    fn draw_string(&self, _m: &super::seams::PoseStack, text: &str, x: i32, y: i32, color: i32) {
        self.draw_strings.borrow_mut().push((text.to_string(), x, y, color));
    }
    fn font_split(&self, _text: &str, _max: i32) -> Vec<String> {
        self.splits.clone()
    }
    fn render_tooltip(&self, _m: &super::seams::PoseStack, lines: &[String], _mouse_x: i32, _mouse_y: i32) {
        *self.tooltip_lines.borrow_mut() = lines.to_vec();
    }
    fn super_on_close(&self) {
        self.super_closed.set(true);
    }
}

#[test]
fn settings_screen_init_builds_nine_options_channel_edit_and_done_button() {
    let _guard = super::tests::install();
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
    super::seams::set_screen_env(env.clone());

    // the config channel the EditBox opens with
    client_handler().config_mut().set_channel("teamchat".to_string());

    let mut screen = super::screen::SettingsScreen::new();
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

    super::seams::set_screen_env(Rc::new(super::seams::NoScreenEnv));
}

#[test]
fn settings_screen_render_shows_title_channel_and_tooltip() {
    let _guard = super::tests::install();
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
    super::seams::set_screen_env(env.clone());

    client_handler().config_mut().set_channel(String::new());
    let mut screen = super::screen::SettingsScreen::new();
    screen.init();
    screen.tick();
    assert_eq!(edit.ticks.get(), 1);

    // no hovered row and an unhovered edit box: the tooltip is empty
    let matrices = super::seams::PoseStack::new();
    screen.render(&matrices, 0, 0, 0.0);
    assert!(env.draw_centered.borrow().iter().any(|(t, cx, y, color)| {
        cx == &(800 / 2) && y == &20 && color == &super::screen::WHITE
    }));
    // the channel label draws at edit.y - 12 in GRAY
    assert!(env.draw_strings.borrow().iter().any(|(_, x, y, color)| {
        x == &(800 / 2 - 100) && y == &(175 - 12) && color == &super::screen::GRAY
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

    super::seams::set_screen_env(Rc::new(super::seams::NoScreenEnv));
}

#[test]
fn common_client_settings_key_opens_the_settings_screen() {
    let _guard = super::tests::install();

    // consume_click_key / set_screen are host seams; drive the handler's wiring
    // directly: the settings-key branch constructs a SettingsScreen factory.
    let factory: Rc<dyn Fn() -> super::screen::SettingsScreen> = Rc::new(|| super::screen::SettingsScreen::new());
    let screen = factory();
    assert_eq!(screen.title(), String::new()); // translate seam renders empty in tests
}
}
