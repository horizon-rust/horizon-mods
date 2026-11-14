//! Upstream: common/screen/ (1:1). OptionUtils + SettingsScreen.
//! ProgressOption / CycleOption / CycleButton / OptionsList / EditBox / Button are
//! host GUI types, so the option objects carry their getters, setters, formatters
//! and tooltips and the widget rendering goes through the screen seam. The
//! formatting branches (OPTION_OFF / VALUE_INFINITE / VALUE_HIDDEN / the percent
//! and second units) are exact.

use std::rc::Rc;

use crate::config::{client_handler, ClientConfig, PlayerInfoMode, TeamColorMode,
                    MAX_CHANNEL_LENGTH, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION};
use crate::resource::LanguageUtils;
use crate::{integration, resource, seams};

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
            &crate::compat::Component::empty(),
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

    pub fn render(&self, m: &crate::seams::PoseStack, mouse_x: i32, mouse_y: i32, delta: f32) {
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
            return crate::compat::Component::empty();
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
                    crate::compat::Component::translatable(&seams::game().map(|g| g.key_player_list_name()).unwrap_or_default(), &[]);
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
