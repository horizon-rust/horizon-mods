// FPS - Display (Horizon Rust port) - source bundle.
// Exact 1:1 Rust translation of Grayray75/FPS-Display v5.2.0 (LGPL-3.0-only).
// Module tree inlined from ports/fpsdisplay/src; every module is nested 1:1.


//! Exact 1:1 Rust translation of FPS - Display by Grayray75
//! (github.com/Grayray75/FPS-Display, tag v5.2.0, LGPL-3.0-only).
//! Upstream Java package io.grayray75.mods.fpsdisplay maps to the modules
//! below; every Minecraft client type sits behind the host seams in `seams`.
//! Log lines, field names, branch order and quirks are kept as-is.
//!
//! - `fps_history` <- FpsHistory.java
//! - `config`      <- config/ConfigData.java + config/ConfigManager.java
//! - `fps_display` <- FpsDisplayMod.java
//! - `mixin`       <- mixin/HudMixin.java (body ported as the TAIL callback)
//! - `gui`         <- config/ModMenuIntegration.java + gui/* (ModMenu dispatch,
//!                    ClothOptionScreen, YaclOptionScreen, FallbackOptionScreen)

pub use fps_display::MOD_ID;
// ---- inlined: src/fps_history.rs ----
pub mod fps_history {
//! 1:1 of FpsHistory.java.

use std::collections::VecDeque;

pub struct FpsHistory {
    // java: private static final int LIMIT = 420;
    // java list is ArrayList + remove(0); VecDeque with pop_front is the
    // same observable behavior (FIFO cap at LIMIT).
    list: VecDeque<i32>,
}

impl Default for FpsHistory {
    fn default() -> Self {
        Self {
            list: VecDeque::with_capacity(420),
        }
    }
}

impl FpsHistory {
    pub fn add(&mut self, fps: i32) {
        self.list.push_back(fps);
        if self.list.len() > 420 {
            self.list.pop_front();
        }
    }

    // java: Collections.min(this.list); panics on an empty list the same way
    // (NoSuchElementException -> our unwrap on the empty iterator).
    pub fn get_minimum(&self) -> i32 {
        *self.list.iter().min().unwrap()
    }

    // java: Collections.max(this.list).
    pub fn get_maximum(&self) -> i32 {
        *self.list.iter().max().unwrap()
    }

    // java: int sum accumulated in an int, then integer division by size.
    pub fn get_average(&self) -> i32 {
        let mut sum: i64 = 0;
        for fps in &self.list {
            sum += *fps as i64;
        }
        (sum / self.list.len() as i64) as i32
    }
}
}
// ---- inlined: src/config.rs ----
pub mod config {
//! 1:1 of config/ConfigData.java + config/ConfigManager.java.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::seams;

// java: private static ConfigData config; - the shared static, backed by
// ConfigData::new() until load_config() populates it.
static CONFIG: OnceLock<Mutex<ConfigData>> = OnceLock::new();
// java: config == null is the "not loaded" state; the AtomicBool mirrors it.
static CONFIG_NULL: AtomicBool = AtomicBool::new(true);

fn config_cell() -> &'static Mutex<ConfigData> {
    CONFIG.get_or_init(|| Mutex::new(ConfigData::default()))
}

// java: public class ConfigData { ... } - field initializers are the
// constructor defaults, so every serde field is #[serde(default)] with the
// same default (Gson leaves absent fields at the Java defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigData {
    pub enabled: bool,
    pub advanced_stats: bool,
    pub text_size: f32,
    pub text_color: i32,
    pub text_alpha: i32,
    pub text_shadows: bool,
    pub offset_top: i32,
    pub offset_left: i32,
    pub keybind_mode: KeyMode,
}

// Field initializers from ConfigData.java, in declaration order.
impl Default for ConfigData {
    fn default() -> Self {
        ConfigData {
            enabled: true,
            advanced_stats: false,
            text_size: 1.0,
            text_color: 0xEEEEEE,
            text_alpha: 230,
            text_shadows: false,
            offset_top: 4,
            offset_left: 4,
            keybind_mode: KeyMode::Toggle,
        }
    }
}

// java: public enum KeyMode { Toggle, PushToShow } - Gson serializes the
// enum constant name, so the serde renames keep the exact JSON tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyMode {
    #[default]
    #[serde(rename = "Toggle")]
    Toggle,
    #[serde(rename = "PushToShow")]
    PushToShow,
}

pub struct ConfigManager;

impl ConfigManager {
    // java: public static ConfigData getConfig() { return config; }
    pub fn get_config() -> std::sync::MutexGuard<'static, ConfigData> {
        config_cell().lock().unwrap()
    }

    fn get_file_path() -> std::path::PathBuf {
        // java: Paths.get(FabricLoader.getInstance().getConfigDir().toString(),
        //                   FpsDisplayMod.MOD_ID + ".json")
        seams::config_dir().join("fpsdisplay.json")
    }

    pub fn load_config() -> std::sync::MutexGuard<'static, ConfigData> {
        // java: BufferedReader + GSON.fromJson(br, ConfigData.class).
        let mut parsed: Option<ConfigData> = None;
        match std::fs::read_to_string(Self::get_file_path()) {
            Ok(text) => match serde_json::from_str::<ConfigData>(&text) {
                Ok(value) => parsed = Some(value),
                Err(err) => {
                    // java: catch (Exception ex) - log message + stack trace.
                    seams::log_info("[fpsdisplay] Couldn't load configuration file, reverting to defaults!");
                    seams::print_stack_trace(&err.to_string());
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // java: catch (FileNotFoundException ex) - log only, no trace.
                seams::log_info("[fpsdisplay] Couldn't load configuration file, reverting to defaults!");
            }
            Err(err) => {
                // java: catch (Exception ex) branch.
                seams::log_info("[fpsdisplay] Couldn't load configuration file, reverting to defaults!");
                seams::print_stack_trace(&err.to_string());
            }
        }

        // java: if (parsed != null) { config = parsed; }
        if let Some(value) = parsed {
            *config_cell().lock().unwrap() = value;
            CONFIG_NULL.store(false, Ordering::SeqCst);
        }

        // java: if (config == null) { config = new ConfigData(); saveConfig(); }
        if CONFIG_NULL.load(Ordering::SeqCst) {
            *config_cell().lock().unwrap() = ConfigData::default();
            CONFIG_NULL.store(false, Ordering::SeqCst);
            Self::save_config();
        }

        config_cell().lock().unwrap()
    }

    pub fn save_config() {
        // java: GSON.toJson(config) with setPrettyPrinting().
        let json = {
            let guard = config_cell().lock().unwrap();
            serde_json::to_string_pretty(&*guard).unwrap_or_default()
        };
        if let Err(err) = std::fs::write(Self::get_file_path(), json) {
            // java: catch (IOException e) - log + stack trace.
            seams::log_info("[fpsdisplay] Couldn't save configuration file.");
            seams::print_stack_trace(&err.to_string());
        }
    }
}
}
// ---- inlined: src/fps_display.rs ----
pub mod fps_display {
//! 1:1 of FpsDisplayMod.java (ClientModInitializer entry).

use std::sync::atomic::Ordering;

use crate::config::{ConfigData, ConfigManager, KeyMode};
use crate::seams;

// java: public static final String MOD_ID = "fpsdisplay";
pub const MOD_ID: &str = "fpsdisplay";

// java: public static FpsHistory FpsHistory = new FpsHistory(); lives in
// seams::fps_history because java statics cross classes.

pub struct FpsDisplayMod;

impl FpsDisplayMod {
    // java: onInitializeClient().
    pub fn on_initialize_client() {
        // java: ConfigData config = ConfigManager.loadConfig(); the guard
        // field below stands in for the captured local across the three
        // lambdas.
        ConfigManager::load_config();

        // java: KeyMapping.Category keybinCategory =
        //   KeyMapping.Category.register(Identifier.fromNamespaceAndPath(MOD_ID, "category"));
        let keybin_category = seams::register_key_mapping_category("fpsdisplay:category");

        // java: KeyMapping toggleKeybinding = KeyMappingHelper.registerKeyMapping(new KeyMapping(
        //   "key.fpsdisplay.toggleOverlay", InputConstants.Type.KEYBOARD,
        //   InputConstants.UNKNOWN.getValue(), keybinCategory));
        let toggle_keybinding = seams::KeyMappingHandle::new(
            "key.fpsdisplay.toggleOverlay",
            seams::InputType::Keyboard,
            seams::INPUT_UNKNOWN_KEY,
            &keybin_category,
        );

        // java: ClientTickEvents.START_CLIENT_TICK.register(client -> {
        //   int currentFps = client.getFps(); FpsHistory.add(currentFps); });
        seams::register_start_client_tick(Box::new(|client| {
            let current_fps = client.get_fps();
            let mut history = seams::fps_history().lock().unwrap();
            history.add(current_fps);
        }));

        // java: ClientTickEvents.END_CLIENT_TICK.register(client -> {
        //   while (toggleKeybinding.consumeClick()
        //       && config.keybindMode == ConfigData.KeyMode.Toggle) {
        //     config.enabled = !config.enabled; }
        //   if (config.keybindMode == ConfigData.KeyMode.PushToShow) {
        //     ShowOverlay = toggleKeybinding.isDown(); }
        //   else { ShowOverlay = config.enabled; } });
        seams::register_end_client_tick(Box::new(move |_client| {
            let mut config = ConfigManager::get_config();
            while toggle_keybinding.consume_click() && config.keybind_mode == KeyMode::Toggle {
                config.enabled = !config.enabled;
            }
            if config.keybind_mode == KeyMode::PushToShow {
                seams::SHOW_OVERLAY.store(toggle_keybinding.is_down(), Ordering::SeqCst);
            } else {
                seams::SHOW_OVERLAY.store(config.enabled, Ordering::SeqCst);
            }
        }));

        // java: ClientLifecycleEvents.CLIENT_STOPPING.register(client -> {
        //   ConfigManager.saveConfig(); });
        seams::register_client_stopping(Box::new(|_client| {
            ConfigManager::save_config();
        }));

        // ConfigData is named in the java signature; keep the alias honest.
        let _: fn() -> std::sync::MutexGuard<'static, ConfigData> = ConfigManager::load_config;
    }
}
}
// ---- inlined: src/mixin.rs ----
pub mod mixin {
//! 1:1 of mixin/HudMixin.java. The @Mixin carrier (Hud + @Shadow isHidden +
//! the TAIL injection on extractRenderState) is engine-stage; the injected
//! body and the @Unique renderText helper are ported exactly.

use crate::config::ConfigManager;
use crate::seams;
use crate::MOD_ID;

pub struct HudMixin;

impl HudMixin {
    // java: @Inject(at = @At("TAIL"), method = "extractRenderState")
    //   public void render(GuiGraphicsExtractor graphics, DeltaTracker deltaTracker, CallbackInfo ci)
    pub fn render(graphics: &mut seams::GuiGraphicsExtractor, _delta_tracker: ()) {
        let client = seams::Minecraft::get_instance();
        let config = ConfigManager::get_config();

        // java: if (!this.isHidden && !client.debugEntries.isOverlayVisible()
        //   && config.enabled && config.textAlpha > 3 && FpsDisplayMod.ShowOverlay)
        let hud_state = seams::HudRenderState { is_hidden: false };
        if !hud_state.is_hidden
            && !client.debug_overlay_visible()
            && config.enabled
            && config.text_alpha > 3
            && seams::SHOW_OVERLAY.load(std::sync::atomic::Ordering::SeqCst)
        {
            let text: String;
            if !config.advanced_stats {
                // java: text = client.getFps() + " FPS";
                text = format!("{} FPS", client.get_fps());
            } else {
                // java: text = String.format("%d FPS (%d min | %d avg | %d max)", ...);
                let min = seams::fps_history().lock().unwrap().get_minimum();
                let avg = seams::fps_history().lock().unwrap().get_average();
                let max = seams::fps_history().lock().unwrap().get_maximum();
                text = format!(
                    "{} FPS ({} min | {} avg | {} max)",
                    client.get_fps(),
                    min,
                    avg,
                    max
                );
            }

            // java: int textPosX = config.offsetLeft; int textPosY = config.offsetTop;
            let mut text_pos_x = config.offset_left;
            let mut text_pos_y = config.offset_top;

            // java: double guiScale = client.getWindow().getGuiScale();
            let gui_scale = client.window_gui_scale();
            // java: if (guiScale > 0) { textPosX /= guiScale; textPosY /= guiScale; }
            // java compound assignment on an int truncates toward zero.
            if gui_scale > 0.0 {
                text_pos_x = (text_pos_x as f64 / gui_scale) as i32;
                text_pos_y = (text_pos_y as f64 / gui_scale) as i32;
            }

            // java: int maxTextPosX = client.getWindow().getGuiScaledWidth()
            //   - client.font.width(text);
            //   int maxTextPosY = client.getWindow().getGuiScaledHeight()
            //   - client.font.lineHeight;
            let max_text_pos_x = client.window_gui_scaled_width() - client.font.width(&text);
            let max_text_pos_y = client.window_gui_scaled_height() - client.font.line_height();
            // java: Math.min clamps so text stays in screenspace.
            text_pos_x = text_pos_x.min(max_text_pos_x);
            text_pos_y = text_pos_y.min(max_text_pos_y);

            // java: int textColor = ((config.textAlpha & 0xFF) << 24) | (config.textColor & 0xFFFFFF);
            let text_color =
                ((config.text_alpha & 0xFF) << 24) | (config.text_color & 0xFFFFFF);

            Self::render_text(
                graphics,
                &client.font,
                &text,
                text_pos_x,
                text_pos_y,
                text_color,
                config.text_size,
                config.text_shadows,
            );
        }

        let _ = MOD_ID;
    }

    // java: @Unique private void renderText(GuiGraphicsExtractor graphics,
    //   Font textRenderer, String text, int x, int y, int color, float scale,
    //   boolean shadowed)
    fn render_text(
        graphics: &mut seams::GuiGraphicsExtractor,
        text_renderer: &seams::Font,
        text: &str,
        x: i32,
        y: i32,
        color: i32,
        scale: f32,
        shadowed: bool,
    ) {
        if scale != 1.0 {
            // java: Matrix3x2fStack matrixStack = graphics.pose();
            //   matrixStack.pushMatrix(); matrixStack.translate(x, y);
            //   matrixStack.scale(scale, scale); matrixStack.translate(-x, -y);
            //   graphics.text(...); matrixStack.popMatrix();
            let mut matrix_stack = graphics.pose();
            matrix_stack.push_matrix();
            matrix_stack.translate(x as f32, y as f32);
            matrix_stack.scale(scale, scale);
            matrix_stack.translate(-(x as f32), -(y as f32));
            graphics.text(text_renderer, text, x, y, color, shadowed);
            matrix_stack.pop_matrix();
        } else {
            graphics.text(text_renderer, text, x, y, color, shadowed);
        }
    }
}
}
// ---- inlined: src/gui.rs ----
pub mod gui {
//! 1:1 of config/ModMenuIntegration.java + gui/FallbackOptionScreen.java +
//! gui/ClothOptionScreen.java + gui/YaclOptionScreen.java.

use crate::config::{ConfigData, ConfigManager, KeyMode};
use crate::seams;

// ---- ModMenuIntegration.java ----
pub struct ModMenuIntegration;

impl ModMenuIntegration {
    // java: getModConfigScreenFactory() - the mod-loaded probe picks the
    // config-lib backend, with the vanilla fallback screen last.
    pub fn get_mod_config_screen_factory(parent: Option<seams::ScreenHandle>) -> seams::ScreenHandle {
        if seams::is_mod_loaded("yet_another_config_lib_v3") {
            yacl_option_screen::generate_screen(parent)
        } else if seams::is_mod_loaded("cloth-config") {
            cloth_option_screen::generate_screen(parent)
        } else {
            fallback_option_screen::generate_screen(parent)
        }
    }
}

// ---- FallbackOptionScreen.java ----
pub mod fallback_option_screen {
    use super::*;

    // java: FallbackOptionScreen(Screen parent) extends Screen with title
    // Component.translatable("text.fpsdisplay.options.fallback_title").
    pub fn generate_screen(parent: Option<seams::ScreenHandle>) -> seams::ScreenHandle {
        let parent = parent.map(Box::new);
        let mut screen = seams::ScreenHandle { parent, width: 0, height: 0 };

        // java: init() - Button doneButton = Button.builder(CommonComponents.GUI_DONE,
        //   (button) -> { this.onClose(); }).width(200).pos(this.width / 2 - 100,
        //   this.height - 27).build(); this.addRenderableWidget(doneButton);
        let done_button = seams::Button::builder_done(seams::GUI_DONE.to_string())
            .width(200)
            .pos(screen.width / 2 - 100, screen.height - 27)
            .build();
        screen.add_renderable_widget(done_button);

        // java: extractRenderState(...) draws the title at width/2, 20 and
        // the message at height/2, both CommonColors.WHITE, after super.
        screen.extract_render_state_base(0, 0, 0.0);
        let font = screen.font();
        let title = screen.title();
        seams::centered_text(&mut screen_g(), &font, &title, screen.width / 2, 20, seams::COMMON_COLORS_WHITE);
        seams::centered_text(
            &mut screen_g(),
            &font,
            &seams::translatable("text.fpsdisplay.options.fallback_message"),
            screen.width / 2,
            screen.height / 2,
            seams::COMMON_COLORS_WHITE,
        );

        // java: onClose() { this.minecraft.gui.setScreen(this.parent); }
        screen.set_screen_parent();
        screen
    }

    fn screen_g() -> seams::GuiGraphicsExtractor {
        seams::GuiGraphicsExtractor
    }
}

// ---- ClothOptionScreen.java (https://shedaniel.gitbook.io/cloth-config/) ----
pub mod cloth_option_screen {
    use super::*;

    pub fn generate_screen(parent: Option<seams::ScreenHandle>) -> seams::ScreenHandle {
        let mut builder = seams::ClothBuilder::create();
        builder.set_parent_screen(parent);
        // java: builder.setTitle(Component.translatable("text.fpsdisplay.options.title"));
        builder.set_title(seams::translatable("text.fpsdisplay.options.title"));
        // java: builder.setSavingRunnable(() -> { ConfigManager.saveConfig(); });
        builder.set_saving_runnable(Box::new(|| {
            ConfigManager::save_config();
        }));

        // java: final ConfigData configDefaults = new ConfigData();
        let config_defaults = ConfigData::default();
        // java: ConfigData config = ConfigManager.getConfig();
        let config = ConfigManager::get_config();
        let enabled = config.enabled;
        let advanced_stats = config.advanced_stats;
        let text_size = config.text_size;
        let text_color = config.text_color;
        let text_alpha = config.text_alpha;
        let text_shadows = config.text_shadows;
        let offset_top = config.offset_top;
        let offset_left = config.offset_left;
        let keybind_mode = config.keybind_mode;
        drop(config);

        // java: ConfigEntryBuilder entryBuilder = builder.entryBuilder();
        let entry_builder = builder.entry_builder();

        // java: ConfigCategory general = builder.getOrCreateCategory(Component.nullToEmpty("General"));
        let mut general = builder.get_or_create_category(String::new());

        // Entries in upstream declaration order; each setSaveConsumer writes
        // the same config field.
        general.add_entry(entry_builder.start_boolean_toggle(
            seams::translatable("text.fpsdisplay.options.enabled"),
            enabled,
            config_defaults.enabled,
            Box::new(|new_value| ConfigManager::get_config().enabled = new_value),
        ));
        general.add_entry(entry_builder.start_boolean_toggle(
            seams::translatable("text.fpsdisplay.options.advancedStats"),
            advanced_stats,
            config_defaults.advanced_stats,
            Box::new(|new_value| ConfigManager::get_config().advanced_stats = new_value),
        ));
        general.add_entry(entry_builder.start_float_field(
            seams::translatable("text.fpsdisplay.options.textSize"),
            text_size,
            config_defaults.text_size,
            Box::new(|new_value| ConfigManager::get_config().text_size = new_value),
        ));
        general.add_entry(entry_builder.start_color_field(
            seams::translatable("text.fpsdisplay.options.textColor"),
            text_color,
            config_defaults.text_color,
            Box::new(|new_value| ConfigManager::get_config().text_color = new_value),
        ));
        general.add_entry(entry_builder.start_int_slider(
            seams::translatable("text.fpsdisplay.options.textAlpha"),
            text_alpha,
            0,
            255,
            config_defaults.text_alpha,
            Box::new(|new_value| ConfigManager::get_config().text_alpha = new_value),
        ));
        general.add_entry(entry_builder.start_boolean_toggle(
            seams::translatable("text.fpsdisplay.options.textShadows"),
            text_shadows,
            config_defaults.text_shadows,
            Box::new(|new_value| ConfigManager::get_config().text_shadows = new_value),
        ));
        general.add_entry(entry_builder.start_int_field(
            seams::translatable("text.fpsdisplay.options.offsetTop"),
            offset_top,
            config_defaults.offset_top,
            Box::new(|new_value| ConfigManager::get_config().offset_top = new_value),
        ));
        general.add_entry(entry_builder.start_int_field(
            seams::translatable("text.fpsdisplay.options.offsetLeft"),
            offset_left,
            config_defaults.offset_left,
            Box::new(|new_value| ConfigManager::get_config().offset_left = new_value),
        ));
        general.add_entry(entry_builder.start_enum_selector(
            seams::translatable("text.fpsdisplay.options.keybindMode"),
            keybind_mode,
            config_defaults.keybind_mode,
            Box::new(|new_value| ConfigManager::get_config().keybind_mode = new_value),
        ));

        // java: return builder.build();
        builder.build()
    }
}

// ---- YaclOptionScreen.java (https://docs.isxander.dev/yet-another-config-lib/) ----
pub mod yacl_option_screen {
    use super::*;

    pub fn generate_screen(parent: Option<seams::ScreenHandle>) -> seams::ScreenHandle {
        // java: ConfigCategory.Builder categoryBuilder = ConfigCategory.createBuilder();
        let mut category_builder = seams::YaclCategoryBuilder::create_builder();
        // java: categoryBuilder.name(Component.translatable("text.fpsdisplay.options.title"));
        category_builder.name(seams::translatable("text.fpsdisplay.options.title"));

        let config_defaults = ConfigData::default();
        let config = ConfigManager::get_config();
        let _enabled = config.enabled;
        let _advanced_stats = config.advanced_stats;
        let _text_size = config.text_size;
        let _text_color = config.text_color;
        let _text_alpha = config.text_alpha;
        let _text_shadows = config.text_shadows;
        let _offset_top = config.offset_top;
        let _offset_left = config.offset_left;
        let _keybind_mode = config.keybind_mode;
        drop(config);

        // Option rows in upstream order; binding(default, getter, setter)
        // captured from the same config fields the cloth screen uses.
        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Bool(
            config_defaults.enabled,
            Box::new(move || ConfigManager::get_config().enabled),
            Box::new(|new_value| ConfigManager::get_config().enabled = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.enabled"))
        .controller(Box::new(move || create_boolean_controller()))
        .build());
        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Bool(
            config_defaults.advanced_stats,
            Box::new(move || ConfigManager::get_config().advanced_stats),
            Box::new(|new_value| ConfigManager::get_config().advanced_stats = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.advancedStats"))
        .controller(Box::new(move || create_boolean_controller()))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Float(
            config_defaults.text_size,
            Box::new(move || ConfigManager::get_config().text_size),
            Box::new(|new_value| ConfigManager::get_config().text_size = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.textSize"))
        // java: option -> FloatSliderControllerBuilder.create(option).range(0.1f, 3.0f).step(0.1f)
        .controller(Box::new(move || {
            seams::float_slider_controller(0.1, 3.0, 0.1)
        }))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Color(
            config_defaults.text_color,
            Box::new(move || ConfigManager::get_config().text_color),
            // java: newValue -> config.textColor = (newValue.getRGB() & 0xFFFFFF)
            Box::new(|new_value| ConfigManager::get_config().text_color = new_value & 0xFFFFFF),
        ))
        .name(seams::translatable("text.fpsdisplay.options.textColor"))
        // java: option -> ColorControllerBuilder.create(option).allowAlpha(false)
        .controller(Box::new(move || seams::color_controller(false)))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Int(
            config_defaults.text_alpha,
            Box::new(move || ConfigManager::get_config().text_alpha),
            Box::new(|new_value| ConfigManager::get_config().text_alpha = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.textAlpha"))
        // java: IntegerSliderControllerBuilder.create(option).range(0, 255).step(1)
        .controller(Box::new(move || seams::integer_slider_controller(0, 255, 1)))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Bool(
            config_defaults.text_shadows,
            Box::new(move || ConfigManager::get_config().text_shadows),
            Box::new(|new_value| ConfigManager::get_config().text_shadows = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.textShadows"))
        .controller(Box::new(move || create_boolean_controller()))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Int(
            config_defaults.offset_top,
            Box::new(move || ConfigManager::get_config().offset_top),
            Box::new(|new_value| ConfigManager::get_config().offset_top = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.offsetTop"))
        // java: IntegerFieldControllerBuilder::create
        .controller(Box::new(move || seams::integer_field_controller()))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Int(
            config_defaults.offset_left,
            Box::new(move || ConfigManager::get_config().offset_left),
            Box::new(|new_value| ConfigManager::get_config().offset_left = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.offsetLeft"))
        .controller(Box::new(move || seams::integer_field_controller()))
        .build());

        category_builder.option(seams::YaclOption::builder(seams::BindingValue::Enum(
            config_defaults.keybind_mode,
            Box::new(move || ConfigManager::get_config().keybind_mode),
            Box::new(|new_value: KeyMode| ConfigManager::get_config().keybind_mode = new_value),
        ))
        .name(seams::translatable("text.fpsdisplay.options.keybindMode"))
        // java: option -> EnumControllerBuilder.create(option).enumClass(ConfigData.KeyMode.class)
        .controller(Box::new(move || seams::enum_controller()))
        .build());

        // java: YetAnotherConfigLib.Builder builder = YetAnotherConfigLib.createBuilder();
        let mut builder = seams::YaclBuilder::create_builder();
        builder.title(seams::translatable("text.fpsdisplay.options.title"));
        builder.category(category_builder.build());
        // java: builder.save(() -> ConfigManager.saveConfig());
        builder.save(Box::new(|| {
            ConfigManager::save_config();
        }));
        // java: return builder.build().generateScreen(parent);
        builder.build().generate_screen(parent)
    }

    // java: private static BooleanControllerBuilder createBooleanController(Option<Boolean> option) {
    //   return BooleanControllerBuilder.create(option).yesNoFormatter().coloured(true); }
    fn create_boolean_controller() -> seams::ControllerSpec {
        seams::boolean_controller()
    }
}
}
// ---- inlined: src/seams.rs ----
pub mod seams {
//! Host seam surface: every Minecraft/fabric type the port touches, with
//! no-op defaults so the standalone check builds without the engine. The
//! engine stage binds the real implementations (same pattern as
//! ping-wheel/src/seams.rs).

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::config::KeyMode;
use crate::fps_history::FpsHistory;

// ---- logging (java: LogManager.getLogger()) ----
pub fn log_info(message: &str) {
    // Default: drop on the floor like log4j with no appender wired.
    let _ = message;
}

// java: ex.printStackTrace().
pub fn print_stack_trace(_message: &str) {}

// java: FabricLoader.getInstance().getConfigDir().
pub fn config_dir() -> PathBuf {
    PathBuf::from("config")
}

// java: FabricLoader.getInstance().isModLoaded(id).
pub fn is_mod_loaded(_id: &str) -> bool {
    false
}

// ---- client lifecycle (fabric event seams) ----
pub struct ClientTickContext;

impl ClientTickContext {
    // java: client.getFps().
    pub fn get_fps(&self) -> i32 {
        0
    }
}

pub fn register_start_client_tick(_handler: Box<dyn Fn(&ClientTickContext) + Send>) {}
pub fn register_end_client_tick(_handler: Box<dyn Fn(&ClientTickContext) + Send>) {}
pub fn register_client_stopping(_handler: Box<dyn Fn(&ClientTickContext) + Send>) {}

// ---- keymapping (KeyMapping.Category.register + KeyMappingHelper) ----
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Keyboard,
}

// java: InputConstants.UNKNOWN.getValue() - the "unknown" keycode.
pub const INPUT_UNKNOWN_KEY: i32 = -1;

pub struct KeyMappingCategory {
    pub id: String,
}

pub fn register_key_mapping_category(_path: &str) -> KeyMappingCategory {
    KeyMappingCategory { id: String::new() }
}

pub struct KeyMappingHandle;

impl KeyMappingHandle {
    pub fn new(
        _name: &str,
        _kind: InputType,
        _key: i32,
        _category: &KeyMappingCategory,
    ) -> KeyMappingHandle {
        KeyMappingHandle
    }
    // java: toggleKeybinding.consumeClick().
    pub fn consume_click(&self) -> bool {
        false
    }
    // java: toggleKeybinding.isDown().
    pub fn is_down(&self) -> bool {
        false
    }
}

// ---- Hud mixin surface (HudMixin.render inject carrier is engine-stage) ----
pub struct HudRenderState {
    // java: @Shadow private boolean isHidden.
    pub is_hidden: bool,
}

pub struct GuiGraphicsExtractor;

impl GuiGraphicsExtractor {
    // java: graphics.pose() -> Matrix3x2fStack. The default seam keeps no
    // pose state; the engine stage binds the real matrix stack.
    pub fn pose(&mut self) -> Matrix3x2fStack {
        Matrix3x2fStack
    }
    // java: graphics.text(textRenderer, text, x, y, color, shadowed).
    pub fn text(&mut self, _font: &Font, _text: &str, _x: i32, _y: i32, _color: i32, _shadowed: bool) {}
}

pub struct Matrix3x2fStack;

impl Matrix3x2fStack {
    pub fn push_matrix(&mut self) {}
    pub fn translate(&mut self, _x: f32, _y: f32) {}
    pub fn scale(&mut self, _x: f32, _y: f32) {}
    pub fn pop_matrix(&mut self) {}
}

pub struct Font;

impl Font {
    // java: client.font.width(text).
    pub fn width(&self, _text: &str) -> i32 {
        0
    }
    // java: client.font.lineHeight.
    pub fn line_height(&self) -> i32 {
        9
    }
}

pub struct Minecraft {
    pub font: Font,
}

impl Minecraft {
    // java: Minecraft.getInstance().
    pub fn get_instance() -> Minecraft {
        Minecraft { font: Font }
    }
    // java: client.getFps().
    pub fn get_fps(&self) -> i32 {
        0
    }
    // java: client.debugEntries.isOverlayVisible().
    pub fn debug_overlay_visible(&self) -> bool {
        false
    }
    // java: client.getWindow().getGuiScale() (double).
    pub fn window_gui_scale(&self) -> f64 {
        0.0
    }
    // java: client.getWindow().getGuiScaledWidth().
    pub fn window_gui_scaled_width(&self) -> i32 {
        0
    }
    // java: client.getWindow().getGuiScaledHeight().
    pub fn window_gui_scaled_height(&self) -> i32 {
        0
    }
}

// ---- FpsDisplayMod statics live here because java statics cross classes ----
pub static SHOW_OVERLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub static FPS_HISTORY: OnceLock<std::sync::Mutex<FpsHistory>> = OnceLock::new();

pub fn fps_history() -> &'static std::sync::Mutex<FpsHistory> {
    FPS_HISTORY.get_or_init(|| std::sync::Mutex::new(FpsHistory::default()))
}

// ---- gui seams (Screen / Button / Component / config-lib builders) ----
pub fn translatable(_key: &str) -> String {
    String::new()
}

pub const GUI_DONE: &str = "gui.done";
pub const COMMON_COLORS_WHITE: i32 = 0xFFFFFF;

pub struct ScreenHandle {
    pub parent: Option<Box<ScreenHandle>>,
    pub width: i32,
    pub height: i32,
}

impl ScreenHandle {
    pub fn title(&self) -> String {
        String::new()
    }
    pub fn font(&self) -> Font {
        Font
    }
    pub fn add_renderable_widget(&mut self, _button: Button) {}
    pub fn on_close(&mut self) {}
    // java: this.minecraft.gui.setScreen(this.parent).
    pub fn set_screen_parent(&mut self) {}
    pub fn extract_render_state_base(&mut self, _mouse_x: i32, _mouse_y: i32, _delta: f32) {}
}

pub struct Button;

impl Button {
    pub fn builder_done(_label: String) -> ButtonBuilder {
        ButtonBuilder { width: 0, x: 0, y: 0 }
    }
}

pub struct ButtonBuilder {
    width: i32,
    x: i32,
    y: i32,
}

impl ButtonBuilder {
    pub fn width(self, _w: i32) -> ButtonBuilder {
        self
    }
    pub fn pos(self, _x: i32, _y: i32) -> ButtonBuilder {
        self
    }
    pub fn build(self) -> Button {
        Button
    }
}

pub fn centered_text(_g: &mut GuiGraphicsExtractor, _font: &Font, _text: &str, _x: i32, _y: i32, _color: i32) {}

// ---- Cloth Config builder surface ----
pub struct ClothBuilder;
pub struct ClothCategory;
pub struct ClothEntryBuilder;

impl ClothBuilder {
    pub fn create() -> ClothBuilder {
        ClothBuilder
    }
    pub fn set_parent_screen(&mut self, _parent: Option<ScreenHandle>) {}
    pub fn set_title(&mut self, _title: String) {}
    pub fn set_saving_runnable(&mut self, _f: Box<dyn FnMut() + Send>) {}
    pub fn entry_builder(&mut self) -> ClothEntryBuilder {
        ClothEntryBuilder
    }
    pub fn get_or_create_category(&mut self, _title: String) -> ClothCategory {
        ClothCategory
    }
    pub fn build(self) -> ScreenHandle {
        ScreenHandle { parent: None, width: 0, height: 0 }
    }
}

impl ClothEntryBuilder {
    pub fn start_boolean_toggle(
        &self,
        _name: String,
        _value: bool,
        _default: bool,
        _save: Box<dyn FnMut(bool) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
    pub fn start_float_field(
        &self,
        _name: String,
        _value: f32,
        _default: f32,
        _save: Box<dyn FnMut(f32) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
    pub fn start_color_field(
        &self,
        _name: String,
        _value: i32,
        _default: i32,
        _save: Box<dyn FnMut(i32) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
    pub fn start_int_slider(
        &self,
        _name: String,
        _value: i32,
        _min: i32,
        _max: i32,
        _default: i32,
        _save: Box<dyn FnMut(i32) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
    pub fn start_int_field(
        &self,
        _name: String,
        _value: i32,
        _default: i32,
        _save: Box<dyn FnMut(i32) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
    pub fn start_enum_selector(
        &self,
        _name: String,
        _value: KeyMode,
        _default: KeyMode,
        _save: Box<dyn FnMut(KeyMode) + Send>,
    ) -> ClothEntry {
        ClothEntry
    }
}

pub struct ClothEntry;

impl ClothEntry {
    pub fn build(self) {}
}

impl ClothCategory {
    pub fn add_entry(&mut self, _entry: ClothEntry) {}
}

// ---- YACL builder surface ----
pub struct YaclCategoryBuilder;
pub struct YaclBuilder;

impl YaclCategoryBuilder {
    pub fn create_builder() -> YaclCategoryBuilder {
        YaclCategoryBuilder
    }
    pub fn name(&mut self, _name: String) {}
    pub fn option(&mut self, _o: YaclOption) {}
    pub fn build(self) -> YaclCategory {
        YaclCategory
    }
}

pub struct YaclCategory;

impl YaclBuilder {
    pub fn create_builder() -> YaclBuilder {
        YaclBuilder
    }
    pub fn title(&mut self, _title: String) {}
    pub fn category(&mut self, _c: YaclCategory) {}
    pub fn save(&mut self, _f: Box<dyn FnMut() + Send>) {}
    pub fn build(self) -> YaclLib {
        YaclLib
    }
}

pub struct YaclLib;

impl YaclLib {
    pub fn generate_screen(self, _parent: Option<ScreenHandle>) -> ScreenHandle {
        ScreenHandle { parent: None, width: 0, height: 0 }
    }
}

pub struct YaclOption;

pub struct YaclOptionBuilder {
    pub option: YaclOption,
}

pub enum BindingValue {
    Bool(bool, Box<dyn Fn() -> bool + Send>, Box<dyn Fn(bool) + Send>),
    Float(f32, Box<dyn Fn() -> f32 + Send>, Box<dyn Fn(f32) + Send>),
    Color(i32, Box<dyn Fn() -> i32 + Send>, Box<dyn Fn(i32) + Send>),
    Int(i32, Box<dyn Fn() -> i32 + Send>, Box<dyn Fn(i32) + Send>),
    Enum(KeyMode, Box<dyn Fn() -> KeyMode + Send>, Box<dyn Fn(KeyMode) + Send>),
}

impl YaclOption {
    pub fn builder(binding: BindingValue) -> YaclOptionBuilder {
        let _ = binding;
        YaclOptionBuilder { option: YaclOption }
    }
}

impl YaclOptionBuilder {
    pub fn name(self, _name: String) -> YaclOptionBuilder {
        self
    }
    pub fn controller(self, _c: Box<dyn FnMut() -> ControllerSpec + Send>) -> YaclOptionBuilder {
        self
    }
    pub fn build(self) -> YaclOption {
        YaclOption
    }
}

// java: BooleanControllerBuilder / FloatSliderControllerBuilder /
// ColorControllerBuilder / IntegerSliderControllerBuilder /
// IntegerFieldControllerBuilder / EnumControllerBuilder collapse into one
// spec surface; the field set is the union of their builder methods.
pub struct ControllerSpec {
    pub kind: ControllerKind,
    pub range_min: f64,
    pub range_max: f64,
    pub step: f64,
    pub allow_alpha: bool,
    pub yes_no_formatter: bool,
    pub coloured: bool,
}

#[derive(PartialEq)]
pub enum ControllerKind {
    BooleanYesNo,
    FloatSlider,
    Color,
    IntegerSlider,
    IntegerField,
    Enum,
}

pub fn boolean_controller() -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::BooleanYesNo,
        range_min: 0.0,
        range_max: 0.0,
        step: 0.0,
        allow_alpha: false,
        yes_no_formatter: true,
        coloured: true,
    }
}

pub fn float_slider_controller(min: f32, max: f32, step: f32) -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::FloatSlider,
        range_min: min as f64,
        range_max: max as f64,
        step: step as f64,
        allow_alpha: false,
        yes_no_formatter: false,
        coloured: false,
    }
}

pub fn color_controller(allow_alpha: bool) -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::Color,
        range_min: 0.0,
        range_max: 0.0,
        step: 0.0,
        allow_alpha,
        yes_no_formatter: false,
        coloured: false,
    }
}

pub fn integer_slider_controller(min: i32, max: i32, step: i32) -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::IntegerSlider,
        range_min: min as f64,
        range_max: max as f64,
        step: step as f64,
        allow_alpha: false,
        yes_no_formatter: false,
        coloured: false,
    }
}

pub fn integer_field_controller() -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::IntegerField,
        range_min: 0.0,
        range_max: 0.0,
        step: 0.0,
        allow_alpha: false,
        yes_no_formatter: false,
        coloured: false,
    }
}

pub fn enum_controller() -> ControllerSpec {
    ControllerSpec {
        kind: ControllerKind::Enum,
        range_min: 0.0,
        range_max: 0.0,
        step: 0.0,
        allow_alpha: false,
        yes_no_formatter: false,
        coloured: false,
    }
}
}
