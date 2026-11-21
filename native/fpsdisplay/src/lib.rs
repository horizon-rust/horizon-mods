//! FPS Display for Horizon: the mod API wiring of the FPS readout.
//!
//! The mod registers its settings on Horizon's Options > Mods page (the
//! original's config screen's fields, same defaults), adds its own HUD
//! layer above all others, and draws the readout there once a frame. The
//! samples come from the frame interval the draw call measures between two
//! of its own calls; the API exposes no client fps read (README seams).
use mod_api::{
    self, ClientHost, DrawContext, GuiLayer, GuiLayerDrawEvent, GuiLayerPlacement, HudMode,
    RegistrationContext, RegisterGuiLayersEvent, Registrar, SettingSpec, SettingValue,
};

mod state;

use state::{Config, KeyMode};

#[derive(Default)]
struct Mod;

/// The mod's id: its settings' namespace and its HUD layer's prefix.
const MOD_ID: &str = "fpsdisplay";
/// The HUD layer the readout draws on.
const LAYER: &str = "fpsdisplay:overlay";

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    /// Register the original's config fields as settings, same defaults.
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        let _ = registrar.register_setting(SettingSpec::toggle(
            "enabled",
            "setting.fpsdisplay.enabled",
            true,
        ));
        let _ = registrar.register_setting(SettingSpec::toggle(
            "advanced_stats",
            "setting.fpsdisplay.advanced_stats",
            false,
        ));
        let _ = registrar.register_setting(SettingSpec::range(
            "text_size",
            "setting.fpsdisplay.text_size",
            0.5,
            4.0,
            0.25,
            1.0,
        ));
        let _ = registrar.register_setting(SettingSpec::color(
            "text_color",
            "setting.fpsdisplay.text_color",
            0xEEEEEE,
        ));
        let _ = registrar.register_setting(SettingSpec::range(
            "text_alpha",
            "setting.fpsdisplay.text_alpha",
            0.0,
            255.0,
            1.0,
            230.0,
        ));
        let _ = registrar.register_setting(SettingSpec::toggle(
            "text_shadows",
            "setting.fpsdisplay.text_shadows",
            false,
        ));
        let _ = registrar.register_setting(SettingSpec::range(
            "offset_top",
            "setting.fpsdisplay.offset_top",
            0.0,
            2000.0,
            1.0,
            4.0,
        ));
        let _ = registrar.register_setting(SettingSpec::range(
            "offset_left",
            "setting.fpsdisplay.offset_left",
            0.0,
            2000.0,
            1.0,
            4.0,
        ));
        let _ = registrar.register_setting(SettingSpec::choice(
            "keybind_mode",
            "setting.fpsdisplay.keybind_mode",
            [
                "setting.fpsdisplay.keybind_mode.toggle",
                "setting.fpsdisplay.keybind_mode.push_to_show",
            ],
            0,
        ));
    }

    /// The HUD layer the readout draws on, above all others.
    fn register_gui_layers(&self, _context: &RegistrationContext, event: &mut RegisterGuiLayersEvent) {
        event.layers.push(GuiLayer {
            id: LAYER.to_string(),
            placement: GuiLayerPlacement::AboveAll,
        });
    }

    /// Once a frame on our layer: sample the fps from the frame interval,
    /// read the settings, and draw the readout the original draws.
    fn gui_layer_draw(&self, host: &dyn ClientHost, event: &mut GuiLayerDrawEvent) {
        if event.layer != LAYER {
            return;
        }
        // The HUD's visibility: the original skips the hidden HUD and the
        // debug overlay; the API answers the hud mode only (README seams).
        if matches!(host.hud_mode(), HudMode::NoHud | HudMode::AllHidden) {
            return;
        }
        let fps = state::sample_frame();
        let cfg = read_config(host);
        if !cfg.enabled || !state::show_overlay() {
            return;
        }
        let text = if !cfg.advanced_stats {
            format!("{fps} FPS")
        } else {
            let (min, avg, max) = state::history_min_avg_max();
            format!("{fps} FPS ({min} min | {avg} avg | {max} max)")
        };
        // The original divides the offsets by the gui scale; the draw
        // context already works in GUI pixels, so the offsets apply as they
        // are and the clamps keep the text on screen.
        let size = event.draw.size();
        let max_x = size[0] - event.draw.text_width(&text);
        let max_y = size[1] - 9.0;
        let x = cfg.offset_left as f32;
        let y = cfg.offset_top as f32;
        let at = [x.min(max_x.max(0.0)), y.min(max_y.max(0.0))];
        let color = ((cfg.text_alpha & 0xFF) << 24) | (cfg.text_color & 0xFFFFFF);
        event.draw.text(&text, at, color, cfg.text_shadows);
    }
}

/// The settings read into the config shape, defaults filling any gap.
fn read_config(host: &dyn ClientHost) -> Config {
    let mut cfg = Config::default();
    if let Some(enabled) = host.setting_bool("enabled") {
        cfg.enabled = enabled;
    }
    if let Some(value) = host.setting_bool("advanced_stats") {
        cfg.advanced_stats = value;
    }
    if let Some(value) = host.setting_f64("text_size") {
        cfg.text_size = value as f32;
    }
    if let Some(value) = host.setting_color("text_color") {
        cfg.text_color = value;
    }
    if let Some(value) = host.setting_f64("text_alpha") {
        cfg.text_alpha = value as u32;
    }
    if let Some(value) = host.setting_bool("text_shadows") {
        cfg.text_shadows = value;
    }
    if let Some(value) = host.setting_f64("offset_top") {
        cfg.offset_top = value as i32;
    }
    if let Some(value) = host.setting_f64("offset_left") {
        cfg.offset_left = value as i32;
    }
    if let Some(index) = host.setting_choice("keybind_mode") {
        cfg.keybind_mode = KeyMode::from_index(index as usize);
    }
    cfg
}
