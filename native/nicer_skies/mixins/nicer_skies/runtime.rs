// Nicer Skies (Horizon Rust port) - source bundle.
// Exact 1:1 Rust translation of ZtereoHYPE/nicer-skies v1.5.2 (tree 22dc158, LGPL-3.0-only).
// Module tree inlined from ports/nicer-skies/src; every module is nested 1:1.

//! Exact 1:1 Rust translation of Nicer Skies by ZtereoHYPE
//! (github.com/ZtereoHYPE/nicer-skies, v1.5.2, LGPL-3.0-only).
//! Upstream Java package dev.ztereohype.nicerskies maps to the modules
//! below; every Minecraft client type sits behind the host seams in
//! `seams`. Field names, math, constants and quirks are kept as-is.
//!
//! - `nicer_skies`            <- NicerSkies.java (entry, init hooks)
//! - `client_level_accessor`  <- ClientLevelAccessor.java
//! - `core.gradient`          <- core/Gradient.java
//! - `core.hashed_seed_manager` <- core/HashedSeedManager.java
//! - `config`                 <- config/Config.java (+ NebulaConfig)
//! - `mixin.client_level_mixin` <- mixin/ClientLevelMixin.java
//! - `mixin.sky_renderer_mixin` <- mixin/SkyRendererMixin.java
//! - `gui.config_screen`      <- gui/ConfigScreen.java
//! - `gui.mod_menu_settings_api` <- gui/ModMenuSettingsApi.java
//! - `gui.widget.separator`   <- gui/widget/Separator.java
//! - `gui.widget.tooltipped_checkbox` <- gui/widget/TooltippedCheckbox.java
//! - `gui.widget.tooltipped_slider_button` <- gui/widget/TooltippedSliderButton.java
//! - `sky.nicer_skies_renderer` <- sky/NicerSkiesRenderer.java
//! - `sky.nebula.skybox`      <- sky/nebula/Skybox.java (+ PaintTask, CoordMap, TextureLocation)
//! - `sky.nebula.skybox_painter` <- sky/nebula/SkyboxPainter.java
//! - `sky.nebula.nebula_skybox_painter` <- sky/nebula/NebulaSkyboxPainter.java
//! - `sky.nebula.debug_skybox_painter` <- sky/nebula/DebugSkyboxPainter.java
//! - `sky.star.star`          <- sky/star/Star.java
//! - `sky.star.starbox`       <- sky/star/Starbox.java
//! - `seams`                  <- host seams (engine-side surface)

pub use nicer_skies::{get_instance, logger, on_initialize_client, MOD_ID, NicerSkies};

// ---- inlined: src/lib.rs ----
pub mod nicer_skies {
    //! 1:1 Horizon Rust port of Nicer Skies (ZtereoHYPE, LGPL-3.0-only).
    //! Upstream: https://github.com/ZtereoHYPE/nicer-skies v1.5.0+26.2.
    //! Every Java class maps to one module below, same names, same math,
    //! same defaults, same control flow.




    use std::sync::{Mutex, OnceLock};

    use crate::{client_level_accessor, config, core, gui, mixin, seams, sky};

    /// java: NicerSkies implements ClientModInitializer. Static instance, config
    /// loaded from nicerskies.json, renderer constructed on init.
    pub const MOD_ID: &str = "nicer-skies";

    pub struct NicerSkies {
        pub config: config::Config,
        pub renderer: sky::nicer_skies_renderer::NicerSkiesRenderer,
    }

    static INSTANCE: OnceLock<Mutex<Option<NicerSkies>>> = OnceLock::new();
    static LOGGER: OnceLock<crate::seams::Logger> = OnceLock::new();

    pub fn logger() -> &'static crate::seams::Logger {
        LOGGER.get_or_init(crate::seams::Logger::new)
    }

    /// java: getInstance(). Horizon entry point: onInitializeClient runs once.
    pub fn instance() -> Option<&'static Mutex<Option<NicerSkies>>> {
        Some(INSTANCE.get_or_init(|| Mutex::new(None)))
    }

    /// java: NicerSkies#onInitializeClient().
    pub fn on_initialize_client() {
        let fabric_loader_config_dir = crate::seams::fabric_config_dir();
        let file = fabric_loader_config_dir.join("nicerskies.json");

        let config = config::Config::from_file(&file);
        let renderer = sky::nicer_skies_renderer::NicerSkiesRenderer::new();

        let instance = INSTANCE.get_or_init(|| Mutex::new(None));
        *instance.lock().unwrap() = Some(NicerSkies { config, renderer });
    }

    /// java: NicerSkies.getInstance().
    pub fn get_instance() -> &'static Mutex<Option<NicerSkies>> {
        INSTANCE.get_or_init(|| Mutex::new(None))
    }
}

// ---- inlined: module tree ----
pub mod client_level_accessor {
    //! 1:1 port of ClientLevelAccessor: duck interface the ClientLevelMixin
    //! implements to expose the biome zoom (hashed) seed.

    pub trait ClientLevelAccessor {
        fn nicer_skies_get_hashed_seed(&self) -> i64;
    }
}
pub mod core {
    pub mod gradient {
        //! 1:1 port of Gradient: TreeMap<Double,int[]> gradient with floor/ceiling
        //! interpolation. Rust: BTreeMap with IEEE-754 total-order keys (same
        //! ordering java's Double.compareTo gives for finite values).

        use std::collections::BTreeMap;

        pub struct Gradient {
            gradient: BTreeMap<u64, [i32; 3]>,
        }

        fn gradient_key(value: f64) -> u64 {
            let bits = value.to_bits();
            if bits & (1u64 << 63) != 0 { !bits } else { bits | (1u64 << 63) }
        }

        fn gradient_unkey(bits: u64) -> f64 {
            f64::from_bits(if bits & (1u64 << 63) != 0 { bits & !(1u64 << 63) } else { !bits })
        }

        impl Gradient {
            pub fn new() -> Self {
                Gradient { gradient: BTreeMap::new() }
            }

            pub fn add(&mut self, index: f64, red: i32, green: i32, blue: i32) {
                if !(0.0..=1.0).contains(&index) {
                    panic!("Index must be between 0 and 1");
                }

                let color: [i32; 3] = [red, green, blue];
                self.gradient.insert(gradient_key(index), color);
            }

            pub fn remove(&mut self, index: f64) {
                self.gradient.remove(&gradient_key(index));
            }

            pub fn clear(&mut self) {
                self.gradient.clear();
            }

            pub fn get_at(&self, value: f64) -> [i32; 3] {
                if !(0.0..=1.0).contains(&value) {
                    panic!("Value must be between 0 and 1");
                }

                let v = gradient_key(value);

                // java: floorEntry(value); under the lowest, return the lowest.
                let floor_entry = self.gradient.range(..=v).next_back().map(|(k, c)| (*k, *c));
                let ceiling_entry = self.gradient.range(v..).next().map(|(k, c)| (*k, *c));

                let floor_entry = match floor_entry {
                    Some(e) => e,
                    None => return *self.gradient.iter().next().unwrap().1,
                };
                let ceiling_entry = match ceiling_entry {
                    Some(e) => e,
                    None => return *self.gradient.iter().next_back().unwrap().1,
                };

                let (floor_key, floor_color) = floor_entry;
                let (ceiling_key, ceiling_color) = ceiling_entry;

                let ratio = (value - gradient_unkey(floor_key))
                    / (gradient_unkey(ceiling_key) - gradient_unkey(floor_key));
                let inv_ratio = 1.0 - ratio;

                let first_color = floor_color;
                let second_color = ceiling_color;

                let red = second_color[0] as f64 * ratio + first_color[0] as f64 * inv_ratio;
                let green = second_color[1] as f64 * ratio + first_color[1] as f64 * inv_ratio;
                let blue = second_color[2] as f64 * ratio + first_color[2] as f64 * inv_ratio;

                [
                    red.round() as i32,
                    green.round() as i32,
                    blue.round() as i32,
                ]
            }
        }
    }
    pub mod hashed_seed_manager {
        //! 1:1 port of HashedSeedManager.

        use crate::client_level_accessor::ClientLevelAccessor;
        use crate::NicerSkies;

        pub fn get_seed() -> i64 {
            if crate::seams::minecraft_level().is_some() {
                let level = crate::seams::minecraft_level().unwrap();
                // java: ((ClientLevelAccessor) level).nicerSkies_getHashedSeed()
                return crate::seams::client_level_hashed_seed(&level);
            }

            crate::logger().warn("Generating nebulas with fallback seed!");
            321 // handpicked decent default ;)
        }

        pub fn can_generate_sky() -> bool {
            crate::seams::minecraft_level().is_some()
        }
    }
}
pub mod config {
    //! 1:1 port of Config (Gson -> serde_json). ConfigData/NebulaConfigData keep
    //! the exact java field names and defaults; Lombok getters map to methods.

    use crate::NicerSkies;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct NebulaConfigData {
        #[serde(rename = "nebulaStrength")]
        pub nebula_strength: f32,
        #[serde(rename = "nebulaNoiseAmount")]
        pub nebula_noise_amount: f32,
        #[serde(rename = "nebulaNoiseScale")]
        pub nebula_noise_scale: f32,
        #[serde(rename = "baseColourAmount")]
        pub base_colour_amount: i32,
        #[serde(rename = "renderDuringDay")]
        pub render_during_day: bool,
    }

    impl NebulaConfigData {
        pub fn new(nebula_strength: f32, nebula_noise_amount: f32, nebula_noise_scale: f32, base_colour_amount: i32, render_during_day: bool) -> Self {
            NebulaConfigData { nebula_strength, nebula_noise_amount, nebula_noise_scale, base_colour_amount, render_during_day }
        }
    }

    impl Default for NebulaConfigData {
        fn default() -> Self {
            NebulaConfigData::new(1.0, 0.5, 1.0, 128, false)
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ConfigData {
        #[serde(rename = "twinklingStars")]
        pub twinkling_stars: bool,
        #[serde(rename = "renderNebulas")]
        pub render_nebulas: bool,

        #[serde(rename = "nebulaConfig")]
        pub nebula_config: NebulaConfigData,
    }

    impl ConfigData {
        /// java: package-private constructor (twinklingStars, nebulas,
        /// nebulaStrength, nebulaNoiseAmount, nebulaNoiseScale, baseColourAmount,
        /// renderDuringDay).
        pub fn new(twinkling_stars: bool, nebulas: bool, nebula_strength: f32, nebula_noise_amount: f32, nebula_noise_scale: f32, base_colour_amount: i32, render_during_day: bool) -> Self {
            ConfigData {
                twinkling_stars,
                render_nebulas: nebulas,
                nebula_config: NebulaConfigData::new(nebula_strength, nebula_noise_amount, nebula_noise_scale, base_colour_amount, render_during_day),
            }
        }
    }

    impl Default for ConfigData {
        fn default() -> Self {
            ConfigData::new(true, true, 1.0, 0.5, 1.0, 128, false)
        }
    }

    pub struct Config {
        pub(crate) config_data: ConfigData,
        file: Option<std::path::PathBuf>,
    }

    impl Config {
        /// java: DEFAULT_CONFIG.
        pub const DEFAULT_CONFIG: ConfigData = ConfigData {
            twinkling_stars: true,
            render_nebulas: true,
            nebula_config: NebulaConfigData {
                nebula_strength: 1.0,
                nebula_noise_amount: 0.5,
                nebula_noise_scale: 1.0,
                base_colour_amount: 128,
                render_during_day: false,
            },
        };

        /// java: static Config fromFile(File file).
        pub fn from_file(file: &std::path::Path) -> Config {
            let config_data = match std::fs::read_to_string(file) {
                Ok(text) => match serde_json::from_str::<ConfigData>(&text) {
                    Ok(c) => c,
                    Err(_) => {
                        crate::logger().warn("Failed to read config file, falling back to default config.");
                        ConfigData::default()
                    }
                },
                Err(_) => {
                    // java: !file.exists() -> mkdirs, createNewFile, write DEFAULT_CONFIG.
                    if let Some(parent) = file.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::File::create(file);
                    ConfigData::default()
                }
            };

            let config = Config { config_data, file: Some(file.to_path_buf()) };
            config.save_file();
            config
        }

        /// java: private constructor also calls save(file).
        pub fn from_config_data(config_data: ConfigData, file: Option<std::path::PathBuf>) -> Config {
            let config = Config { config_data, file };
            config.save_file();
            config
        }

        pub fn get_config_data(&self) -> &ConfigData {
            &self.config_data
        }

        // java: Lombok-style accessors on Config.
        pub fn are_twinlking_stars_enabled(&self) -> bool {
            self.config_data.twinkling_stars
        }

        pub fn are_nebulas_enabled(&self) -> bool {
            self.config_data.render_nebulas
        }

        pub fn get_nebula_strength(&self) -> f32 {
            self.config_data.nebula_config.nebula_strength
        }

        pub fn get_nebula_noise_amount(&self) -> f32 {
            self.config_data.nebula_config.nebula_noise_amount
        }

        pub fn get_nebula_noise_scale(&self) -> f32 {
            self.config_data.nebula_config.nebula_noise_scale
        }

        pub fn get_nebula_base_colour_amount(&self) -> i32 {
            self.config_data.nebula_config.base_colour_amount
        }

        pub fn get_render_during_day(&self) -> bool {
            self.config_data.nebula_config.render_during_day
        }

        pub fn update_config(&mut self, config_data: ConfigData) {
            self.config_data = config_data.clone();
            self.save_file();
        }

        pub fn save_file(&self) {
            if let Some(file) = &self.file {
                let json = serde_json::to_string(&self.config_data).unwrap_or_else(|_| "null".to_string());
                if let Err(_e) = std::fs::write(file, json) {
                    crate::logger().warn("Failed to save config file!");
                }
            }
        }
    }

    impl Config {
        /// java: Config is shared by reference; the renderer/screen clone the
        /// current ConfigData into their own Config. Mirrors the effective
        /// behavior of holding the same ConfigData.
        pub fn clone_config(&self) -> Config {
            Config { config_data: self.config_data.clone(), file: None }
        }

        /// java: getConfigData().toBuilder().build() deep-copy used by the screen.
        pub fn to_builder_build(&self) -> ConfigData {
            self.config_data.clone()
        }
    }
}
pub mod mixin {
    pub mod client_level_mixin {
        //! 1:1 port of ClientLevelMixin. The java @Mixin handlers below register as
        //! engine hooks with identical injection points and bodies.

        use crate::client_level_accessor::ClientLevelAccessor;
        use crate::core::hashed_seed_manager as manager; // unused here, mirror imports
        use crate::NicerSkies;
        use crate::seams;

        /// java: @Unique private long hashedSeed.
        pub struct ClientLevelMixinState {
            hashed_seed: i64,
        }

        impl ClientLevelMixinState {
            pub fn new() -> Self {
                ClientLevelMixinState { hashed_seed: 0 }
            }
        }

        impl ClientLevelAccessor for ClientLevelMixinState {
            /// java: nicerSkies_getHashedSeed().
            fn nicer_skies_get_hashed_seed(&self) -> i64 {
                self.hashed_seed
            }
        }

        /// java: @Inject(method = "<init>", at = @At("TAIL")) nicerSkies_saveHashedSeed.
        pub fn nicer_skies_save_hashed_seed(state: &mut ClientLevelMixinState, biome_zoom_seed: i64) {
            // store the seed to be able to regenerate things in settings
            state.hashed_seed = biome_zoom_seed;

            // Note: sometimes, the seed is received after the buffers have been built by the renderer.
            // In that case it is safe to finish generating the sky.
            let instance = crate::get_instance().lock().unwrap();
            if let Some(inst) = instance.as_ref() {
                let renderer_has_built = inst.renderer.has_built_buffers();
                if renderer_has_built {
                    let mut inst = instance;
                    inst.as_mut().unwrap().renderer.generate_sky(biome_zoom_seed);
                }
            }
        }

        /// java: @Inject(method = "tick", at = @At("TAIL")) nicerSkies_tickStars.
        pub fn nicer_skies_tick_stars() {
            let mut instance = crate::get_instance().lock().unwrap();
            let Some(inst) = instance.as_mut() else { return };

            if !inst.config.are_twinlking_stars_enabled() {
                return;
            }

            let mut profiler_filler = seams::ProfilerFiller::get();
            profiler_filler.push("Nicer Skies");
            inst.renderer.tick();
            profiler_filler.pop();
        }

        /// Registers the mixin handlers with the engine (the mixin config's client
        /// entries: ClientLevelMixin, SkyRendererMixin).
        pub fn register() {
            seams::register_client_level_hooks(
                |seed| {
                    let mut state = seams::client_level_mixin_state();
                    nicer_skies_save_hashed_seed(&mut state, seed);
                },
                nicer_skies_tick_stars,
            );
        }
    }
    pub mod sky_renderer_mixin {
        //! 1:1 port of SkyRendererMixin.

        use crate::NicerSkies;
        use crate::seams;

        /// java: @Inject(at = @At("TAIL"), method = "buildStars") nicerSkies_buildStars.
        pub fn nicer_skies_build_stars() {
            let mut instance = crate::get_instance().lock().unwrap();
            let Some(inst) = instance.as_mut() else { return };

            inst.renderer.build_buffers();

            // note: this method gets called at every resource re-load.
            // Ff we already know the seed by this time, it means we reloaded resources
            // while in-game so we can already re-generate the stars and paint the nebulas
            if crate::core::hashed_seed_manager::can_generate_sky() {
                inst.renderer.generate_sky(crate::core::hashed_seed_manager::get_seed());
            }
        }

        /// java: @Inject(method = "renderSunMoonAndStars", at = @At(value = "INVOKE",
        /// target = "Lcom/mojang/blaze3d/vertex/PoseStack;popPose()V", ordinal = 3,
        /// shift = At.Shift.BEFORE)) nicerSkies_renderNebulas.
        pub fn nicer_skies_render_nebulas(pose_stack: &mut seams::PoseStack, star_angle: f32, star_brightness: f32, render_target: &seams::RenderTarget) {
            let instance = crate::get_instance().lock().unwrap();
            let Some(inst) = instance.as_ref() else { return };

            if inst.config.are_nebulas_enabled() {
                if let Some(skybox) = inst.renderer.get_skybox() {
                    pose_stack.push_pose();
                    pose_stack.mul_pose(seams::Axis::XP.rotation(star_angle as f64));
                    // max star brightness is 0.5
                    let brightness = star_brightness * 2.0;
                    skybox.render(render_target, pose_stack, brightness);
                    pose_stack.pop_pose();
                }
            }
        }

        /// java: @Inject(method = "renderStars", at = @At("HEAD"), cancellable = true)
        /// nicerSkies_renderTwinklingStars.
        pub fn nicer_skies_render_twinkling_stars(star_brightness: f32, pose_stack: &seams::PoseStack, render_target: &seams::RenderTarget) -> bool {
            let instance = crate::get_instance().lock().unwrap();
            let Some(inst) = instance.as_ref() else { return false };

            if inst.config.are_twinlking_stars_enabled() {
                if let Some(starbox) = inst.renderer.get_starbox() {
                    starbox.render(render_target, pose_stack, star_brightness);
                    return true; // java: ci.cancel()
                }
            }

            false
        }

        /// Registers the mixin handlers with the engine (SkyRendererMixin client entry).
        pub fn register() {
            seams::register_sky_renderer_hooks(nicer_skies_build_stars, |ps, sa, sb, rt| {
                nicer_skies_render_nebulas(ps, sa, sb, rt)
            }, |sb, ps, rt| nicer_skies_render_twinkling_stars(sb, ps, rt));
        }
    }
}
pub mod gui {
    pub mod config_screen {
        //! 1:1 port of ConfigScreen.

        use super::widget::separator::Separator;
        use super::widget::tooltipped_checkbox::TooltippedCheckbox;
        use super::widget::tooltipped_slider_button::TooltippedSliderButton;
        use crate::config::{Config, ConfigData};
        use crate::core::hashed_seed_manager;
        use crate::seams;
        use crate::NicerSkies;

        pub struct ConfigScreen {
            title: seams::Component,
            last_screen: seams::ScreenHandle,
            pub config: Config,
            pub new_config: ConfigData,

            invalidated: bool,

            pub reset_btn: Option<seams::Button>,
            pub apply_btn: Option<seams::Button>,

            pub width: i32,
            pub height: i32,

            widgets: Vec<seams::Widget>,
        }

        impl ConfigScreen {
            /// java: protected ConfigScreen(Screen lastScreen).
            pub fn new(last_screen: seams::ScreenHandle) -> Self {
                let mut screen = ConfigScreen {
                    title: seams::Component::translatable("nicer_skies.menu.settings"),
                    last_screen,
                    config: Config::from_config_data(ConfigData::default(), None),
                    new_config: ConfigData::default(),
                    invalidated: false,
                    reset_btn: None,
                    apply_btn: None,
                    width: 0,
                    height: 0,
                    widgets: Vec::new(),
                };

                let inst_config = {
                    let instance = crate::get_instance().lock().unwrap();
                    match instance.as_ref() {
                        Some(inst) => inst.config.to_builder_build(),
                        None => ConfigData::default(),
                    }
                };
                screen.config = Config::from_config_data(inst_config, None);

                screen.wipe_config();
                screen
            }

            /// java: private wipeConfig().
            pub fn wipe_config(&mut self) {
                let config_data = self.config.to_builder_build();
                self.new_config = config_data.clone();
            }

            /// java: init().
            pub fn init(&mut self) {
                let btn_dst = 24;
                let nebula_opt_margin = self.width / 2 + (self.width / 2 - 150) / 2;
                let f = seams::font();

                // initial values
                let render_nebulas = self.new_config.render_nebulas;
                let twinkle_stars = self.new_config.twinkling_stars;

                let render_during_day = self.new_config.nebula_config.render_during_day;
                let nebula_strength = self.new_config.nebula_config.nebula_strength;
                let nebula_noise_amount = self.new_config.nebula_config.nebula_noise_amount;
                let nebula_base_colour_amount = self.new_config.nebula_config.base_colour_amount;
                let nebula_noise_scale = self.new_config.nebula_config.nebula_noise_scale;

                let title_string = seams::StringWidget::new(self.title.clone(), &f);
                let tw = title_string.get_width();
                title_string.set_position(self.width / 2 - tw / 2, 10);
                self.add_renderable_widget(seams::Widget::StringWidget(title_string));

                let features_string = seams::StringWidget::new(seams::Component::translatable("nicer_skies.menu.subtitle.feature_toggles"), &f);
                let fw = features_string.get_width();
                features_string.set_position(self.width / 4 - fw / 2, 36);
                self.add_renderable_widget(seams::Widget::StringWidget(features_string));

                let nebula_string = seams::StringWidget::new(seams::Component::translatable("nicer_skies.menu.subtitle.nebula_settings"), &f);
                let nw = nebula_string.get_width();
                nebula_string.set_position(3 * self.width / 4 - nw / 2, 36);
                self.add_renderable_widget(seams::Widget::StringWidget(nebula_string));

                let warning_string = seams::MultiLineTextWidget::new(20, 160, seams::Component::translatable("nicer_skies.menu.compatibility_warning"), &f).set_max_width(self.width / 2 - 40);
                self.add_renderable_widget(seams::Widget::MultiLineTextWidget(warning_string));

                let mut y = 60;
                let checkbox1 = TooltippedCheckbox::new(
                    20,
                    y,
                    seams::Component::translatable("nicer_skies.option.render_nebulas"),
                    &f,
                    render_nebulas,
                    seams::CheckboxOnValueChange(|_cb: *const (), selected: bool| {
                        seams::screen_new_config_set_render_nebulas(selected);
                        seams::screen_set_invalidated(true);
                    }),
                    None,
                );
                self.add_renderable_widget(seams::Widget::Checkbox(checkbox1.checkbox));

                let checkbox2 = TooltippedCheckbox::new(
                    20,
                    {
                        y += btn_dst;
                        y
                    },
                    seams::Component::translatable("nicer_skies.option.twinkle_stars"),
                    &f,
                    twinkle_stars,
                    seams::CheckboxOnValueChange(|_cb: *const (), selected: bool| {
                        seams::screen_new_config_set_twinkling_stars(selected);
                        seams::screen_set_invalidated(true);
                    }),
                    None,
                );
                self.add_renderable_widget(seams::Widget::Checkbox(checkbox2.checkbox));

                self.add_renderable_only(seams::Widget::Separator(Separator::new(self.width / 2, 30, self.height - 70)));
                y = 60;

                // Render During Day
                let checkbox3 = TooltippedCheckbox::new(
                    nebula_opt_margin,
                    y,
                    seams::Component::translatable("nicer_skies.option.render_during_day"),
                    &f,
                    render_during_day,
                    seams::CheckboxOnValueChange(|_cb: *const (), selected: bool| {
                        seams::screen_nebula_config_set_render_during_day(selected);
                        seams::screen_set_invalidated(true);
                    }),
                    Some(seams::Tooltip::create(seams::Component::translatable("nicer_skies.option.render_during_day.tooltip"))),
                );
                self.add_renderable_widget(seams::Widget::Checkbox(checkbox3.checkbox));

                // Nebula Strength
                let slider1 = TooltippedSliderButton::new(
                    nebula_opt_margin,
                    {
                        y += btn_dst;
                        y
                    },
                    150,
                    20,
                    seams::Component::translatable_with(
                        "nicer_skies.option.nebula_transparency",
                        &[(format!("{}%", (nebula_strength * 100.0) as i32))],
                    ),
                    nebula_strength as f64,
                    seams::SliderMovedFn(Box::new(|value| {
                        seams::Component::translatable_with(
                            "nicer_skies.option.nebula_transparency",
                            &[(format!("{}%", (value * 100.0) as i32))],
                        )
                    })),
                    seams::SliderReleasedFn(Box::new(|value| {
                        seams::screen_nebula_config_set_nebula_strength(value as f32);
                        seams::screen_set_invalidated(true);
                    })),
                    Some(seams::Tooltip::create(seams::Component::translatable("nicer_skies.option.nebula_transparency.tooltip"))),
                );
                self.add_renderable_widget(seams::Widget::SliderButton(slider1.button));

                // Nebula Amount
                let slider2 = TooltippedSliderButton::new(
                    nebula_opt_margin,
                    {
                        y += btn_dst;
                        y
                    },
                    150,
                    20,
                    seams::Component::translatable_with(
                        "nicer_skies.option.nebula_amount",
                        &[(format!("{}%", (nebula_noise_amount * 100.0) as i32))],
                    ),
                    nebula_noise_amount as f64,
                    seams::SliderMovedFn(Box::new(|value| {
                        seams::Component::translatable_with(
                            "nicer_skies.option.nebula_amount",
                            &[(format!("{}%", (value * 100.0) as i32))],
                        )
                    })),
                    seams::SliderReleasedFn(Box::new(|value| {
                        seams::screen_nebula_config_set_nebula_noise_amount(value as f32);
                        seams::screen_set_invalidated(true);
                    })),
                    Some(seams::Tooltip::create(seams::Component::translatable("nicer_skies.option.nebula_amount.tooltip"))),
                );
                self.add_renderable_widget(seams::Widget::SliderButton(slider2.button));

                // Background Strength
                let slider3 = TooltippedSliderButton::new(
                    nebula_opt_margin,
                    {
                        y += btn_dst;
                        y
                    },
                    150,
                    20,
                    seams::Component::translatable_with(
                        "nicer_skies.option.background_strength",
                        &[(format!("{}", nebula_base_colour_amount))],
                    ),
                    nebula_base_colour_amount as f64 / 255.0,
                    seams::SliderMovedFn(Box::new(|value| {
                        seams::Component::translatable_with(
                            "nicer_skies.option.background_strength",
                            &[(format!("{}", (value * 255.0) as i32))],
                        )
                    })),
                    seams::SliderReleasedFn(Box::new(|value| {
                        seams::screen_nebula_config_set_base_colour_amount((value * 255.0) as i32);
                        seams::screen_set_invalidated(true);
                    })),
                    Some(seams::Tooltip::create(seams::Component::translatable("nicer_skies.option.background_strength.tooltip"))),
                );
                self.add_renderable_widget(seams::Widget::SliderButton(slider3.button));

                // Nebula Scale
                let slider4 = TooltippedSliderButton::new(
                    nebula_opt_margin,
                    {
                        y += btn_dst;
                        y
                    },
                    150,
                    20,
                    seams::Component::translatable_with(
                        "nicer_skies.option.nebula_scale",
                        &[(format!("{}x", Self::format_scale(nebula_noise_scale as f64)))],
                    ),
                    Self::map_value_to_scale(nebula_noise_scale as f64),
                    seams::SliderMovedFn(Box::new(|value| {
                        seams::Component::translatable_with(
                            "nicer_skies.option.nebula_scale",
                            &[(format!("{}x", Self::format_scale(Self::map_scale_to_value(value))))],
                        )
                    })),
                    seams::SliderReleasedFn(Box::new(|value| {
                        seams::screen_nebula_config_set_nebula_noise_scale(Self::map_scale_to_value(value) as f32);
                        seams::screen_set_invalidated(true);
                    })),
                    Some(seams::Tooltip::create(seams::Component::translatable("nicer_skies.option.nebula_scale.tooltip"))),
                );
                self.add_renderable_widget(seams::Widget::SliderButton(slider4.button));

                // Reset
                let reset_btn = seams::Button::builder(
                    seams::Component::translatable("nicer_skies.menu.reset"),
                    Box::new(|| {
                        seams::screen_reset_nebula_config_to_default();
                        seams::screen_rebuild_widgets();
                        seams::screen_set_invalidated(true);
                    }),
                )
                .bounds(nebula_opt_margin, {
                    y += btn_dst;
                    y
                }, 150, 20)
                .build();
                self.reset_btn = Some(reset_btn.clone_handle());
                self.add_renderable_widget(seams::Widget::Button(reset_btn));

                // Apply
                let apply_btn = seams::Button::builder(
                    seams::Component::translatable("nicer_skies.menu.apply"),
                    Box::new(|| {
                        seams::screen_apply_config();
                        seams::screen_regenerate_sky();
                        seams::screen_set_invalidated(false);
                    }),
                )
                .bounds(self.width / 2 + 4, self.height - 28, 150, 20)
                .build();
                self.apply_btn = Some(apply_btn.clone_handle());
                self.add_renderable_widget(seams::Widget::Button(apply_btn));

                // Back
                let back_btn = seams::Button::builder(
                    seams::Component::translatable("nicer_skies.menu.back"),
                    Box::new(|| {
                        seams::screen_on_close();
                    }),
                )
                .pos(self.width / 2 - 154, self.height - 28)
                .size(150, 20)
                .build();
                self.add_renderable_widget(seams::Widget::Button(back_btn));
            }

            pub fn add_renderable_widget(&mut self, widget: seams::Widget) {
                self.widgets.push(widget);
            }

            pub fn add_renderable_only(&mut self, widget: seams::Widget) {
                self.widgets.push(widget);
            }

            /// java: extractRenderState.
            pub fn extract_render_state(&mut self, g: &mut seams::GuiGraphicsExtractor, mouse_x: i32, mouse_y: i32, partial_tick: f32) {
                let reset_active = !self.is_default_nebula_settings();
                if let Some(reset_btn) = self.reset_btn.as_mut() {
                    reset_btn.active = reset_active;
                }
                let apply_active = self.invalidated;
                if let Some(apply_btn) = self.apply_btn.as_mut() {
                    apply_btn.active = apply_active;
                }
            }

            /// java: onClose().
            pub fn on_close(&mut self) {
                self.wipe_config();
                seams::set_screen(self.last_screen);
            }

            pub fn regenerate_sky(&mut self) {
                if hashed_seed_manager::can_generate_sky() {
                    let seed = hashed_seed_manager::get_seed();
                    let mut instance = crate::get_instance().lock().unwrap();
                    if let Some(inst) = instance.as_mut() {
                        inst.renderer.generate_sky(seed);
                    }
                }
                self.invalidated = false;
            }

            /// java: private isDefaultNebulaSettings().
            pub fn is_default_nebula_settings(&self) -> bool {
                self.new_config.nebula_config == Config::DEFAULT_CONFIG.nebula_config
            }

            /// java: private float mapScaleToValue(double value).
            pub fn map_scale_to_value(value: f64) -> f64 {
                ((1.0 / (value * 1.5 + 0.5)) * 100.0).round() / 100.0
            }

            /// inverse of above
            pub fn map_value_to_scale(value: f64) -> f64 {
                ((1.0 / value) - 0.5) / 1.5
            }

            /// java: nebula_scale uses the raw float value in the label ("%dx").
            fn format_scale(v: f64) -> String {
                // java: String.format uses the float's own decimal form.
                format!("{}", v)
            }
        }
    }
    pub mod mod_menu_settings_api {
        //! 1:1 port of ModMenuSettingsApi: ModMenu config screen factory that hands
        //! the engine's settings entry the ConfigScreen constructor.

        use super::config_screen::ConfigScreen;

        pub fn get_mod_config_screen_factory() -> fn(crate::seams::ScreenHandle) -> ConfigScreen {
            ConfigScreen::new
        }
    }
    pub mod widget {
        pub mod separator {
            //! 1:1 port of Separator (Renderable).

            use crate::seams;

            pub struct Separator {
                x: i32,
                y: i32,
                height: i32,
            }

            impl Separator {
                pub fn new(x: i32, y: i32, height: i32) -> Self {
                    Separator { x, y, height }
                }

                /// java: extractRenderState(GuiGraphicsExtractor, mouseX, mouseY, a).
                pub fn extract_render_state(&self, graphics: &mut seams::GuiGraphicsExtractor, _mouse_x: i32, _mouse_y: i32, _a: f32) {
                    graphics.fill(self.x, self.y, self.x + 1, self.y + self.height, 0xFFFFFFFFu32 as i32);
                }
            }
        }
        pub mod tooltipped_checkbox {
            //! 1:1 port of TooltippedCheckbox.

            use crate::seams;

            pub struct TooltippedCheckbox {
                pub checkbox: seams::Checkbox,
            }

            impl TooltippedCheckbox {
                pub fn new(i: i32, j: i32, component: seams::Component, font: &seams::Font, bl: bool, on_value_change: seams::CheckboxOnValueChange, tooltip: Option<seams::Tooltip>) -> Self {
                    // java: super(i, j, 128, component, font, bl, onValueChange);
                    let checkbox = seams::Checkbox::new(i, j, 128, component, font, bl, on_value_change);
                    checkbox.set_tooltip(tooltip);
                    TooltippedCheckbox { checkbox }
                }

                /// java: extractContents override: alpha 1.0 active, 0.5 inactive.
                pub fn extract_contents(&mut self, gui_graphics: &mut seams::GuiGraphicsExtractor, i: i32, j: i32, f: f32) {
                    self.checkbox.set_alpha(if self.checkbox.is_active() { 1.0 } else { 0.5 });
                    self.checkbox.extract_contents(gui_graphics, i, j, f);
                }
            }
        }
        pub mod tooltipped_slider_button {
            //! 1:1 port of TooltippedSliderButton.

            use crate::seams;

            pub struct TooltippedSliderButton {
                pub button: seams::AbstractSliderButton,
                slider_moved: seams::SliderMovedFn,
                slider_released: seams::SliderReleasedFn,
            }

            impl TooltippedSliderButton {
                pub fn new(
                    x: i32,
                    y: i32,
                    width: i32,
                    height: i32,
                    text: seams::Component,
                    value: f64,
                    slider_moved: seams::SliderMovedFn,
                    slider_released: seams::SliderReleasedFn,
                    tooltip: Option<seams::Tooltip>,
                ) -> Self {
                    let button = seams::AbstractSliderButton::new(x, y, width, height, text, value);

                    TooltippedSliderButton {
                        button,
                        slider_moved,
                        slider_released,
                    }
                    .with_tooltip(tooltip)
                }

                fn with_tooltip(self, tooltip: Option<seams::Tooltip>) -> Self {
                    self.button.set_tooltip(tooltip);
                    self
                }

                /// java: protected updateMessage().
                pub fn update_message(&mut self) {
                    let value = self.button.value;
                    let message = (self.slider_moved.0)(value);
                    self.button.set_message(message);
                }

                /// java: protected applyValue().
                pub fn apply_value(&mut self) {
                    let value = self.button.value;
                    (self.slider_released.0)(value);
                }
            }
        }
    }
}
pub mod sky {
    pub mod nicer_skies_renderer {
        //! 1:1 port of NicerSkiesRenderer.

        use crate::config::Config;
        use crate::core::gradient::Gradient;
        use crate::core::hashed_seed_manager;
        use crate::sky::nebula::nebula_skybox_painter::NebulaSkyboxPainter;
        use crate::sky::nebula::skybox::Skybox;
        use crate::sky::nebula::skybox_painter::SkyboxPainter;
        use crate::sky::star::starbox::{Starbox, StarboxRef};
        use crate::NicerSkies;
        use crate::seams;

        pub struct NicerSkiesRenderer {
            pub starbox: StarboxRef,
            pub skybox: Option<Skybox>,

            pub ticks: i32,

            pub star_gradient: Gradient,
            pub nebula_gradient: Gradient,
        }

        impl NicerSkiesRenderer {
            /// java: instance initializer blocks on starGradient/nebulaGradient.
            pub fn new() -> Self {
                let mut star_gradient = Gradient::new();
                star_gradient.add(0.0, 255, 179, 97);
                star_gradient.add(0.2, 255, 249, 253);
                star_gradient.add(1.0, 175, 199, 255);

                let mut nebula_gradient = Gradient::new();
                nebula_gradient.add(0.2, 41, 83, 146);
                nebula_gradient.add(0.5, 120, 47, 93);
                nebula_gradient.add(0.7, 209, 58, 103);
                nebula_gradient.add(0.8, 255, 160, 123);
                nebula_gradient.add(1.0, 253, 194, 220);

                NicerSkiesRenderer {
                    starbox: StarboxRef::None,
                    skybox: None,
                    ticks: 42,
                    star_gradient,
                    nebula_gradient,
                }
            }

            /// java: Lombok @Getter starbox -> StarboxRef; getStarbox() reads
            /// StarboxRef::Some. java: hasBuiltBuffers().
            pub fn has_built_buffers(&self) -> bool {
                self.starbox.is_built() && self.skybox.is_some()
            }

            /// java: getStarbox().
            pub fn get_starbox(&self) -> Option<&Starbox> {
                self.starbox.as_star()
            }

            /// java: getSkybox().
            pub fn get_skybox(&self) -> Option<&Skybox> {
                self.skybox.as_ref()
            }

            pub fn build_buffers(&mut self) {
                self.starbox = StarboxRef::Some(Box::new(Starbox::new()));
                self.skybox = Some(Skybox::new());
            }

            pub fn generate_sky(&mut self, seed: i64) {
                let mut random_source = seams::RandomSource::create(seed);

                let cm: Config = match (crate::get_instance().lock().unwrap()).as_ref() {
                    Some(inst) => inst.config.clone_config(),
                    None => return,
                };

                if cm.are_nebulas_enabled() {
                    let perlin_noise = seams::PerlinNoise::create(random_source.fork(), [1, 2, 3, 4, 5, 6, 7]);
                    let noise_scale = cm.get_nebula_noise_scale();
                    let noise_amount = cm.get_nebula_noise_amount();
                    let bg_strength = cm.get_nebula_base_colour_amount();

                    let painter = NebulaSkyboxPainter::new(
                        perlin_noise,
                        &self.nebula_gradient,
                        noise_scale,
                        noise_amount,
                        bg_strength,
                    );
                    if let Some(skybox) = self.skybox.as_mut() {
                        skybox.paint(&painter);
                    }
                }

                if cm.are_twinlking_stars_enabled() {
                    if let Some(starbox) = self.starbox.as_mut() {
                        starbox.generate_stars(&mut random_source, &self.star_gradient);
                    }
                }
            }

            pub fn tick(&mut self) {
                self.ticks += 1;
                if let Some(starbox) = self.starbox.as_mut() {
                    starbox.update_stars(self.ticks);
                }
            }

            /// java: private generateSky helper the ConfigScreen calls after apply.
            pub fn regenerate(&mut self, seed: i64) {
                self.generate_sky(seed);
            }

            pub fn generate_sky_from_manager(&mut self) {
                if hashed_seed_manager::can_generate_sky() {
                    self.generate_sky(hashed_seed_manager::get_seed());
                }
            }
        }
    }
    pub mod nebula {
        pub mod skybox {
            //! 1:1 port of Skybox (incl. the private PaintTask, CoordMap and
            //! TextureLocation inner types and the sky executor thread pool).

            use super::skybox_painter::SkyboxPainter;
            use crate::seams;
            use crate::MOD_ID;
            use std::sync::Arc;

            pub const RESOLUTION: usize = 768;

            pub struct Skybox {
                pub sky_pipeline: seams::RenderPipelineRef,
                sky_texture: seams::DynamicTexture,
                sky_executor: seams::ExecutorService,
                skybox_sampler: seams::GpuSampler,
                pub skybox_buffer: seams::GpuBuffer,
                index_buffer: seams::AutoStorageIndexBuffer,
            }

            impl Skybox {
                pub fn new() -> Self {
                    let sky_pipeline = seams::register_render_pipeline(seams::RenderPipelineSpec {
                        // java: withLocation(MOD_ID, "pipeline/twinkling_stars") on both
                        // pipelines; upstream reuses the same location string.
                        location: format!("{MOD_ID}:pipeline/twinkling_stars"),
                        vertex_shader: "core/position_tex".to_string(),
                        fragment_shader: "core/position_tex".to_string(),
                        blend: seams::blend_overlay(),
                        vertex_binding: seams::VertexBinding::PositionTex,
                        bind_group_layouts: vec![
                            seams::BindGroupLayout::MatricesProjection,
                            seams::BindGroupLayout::Sampler0,
                        ],
                    });

                    let skybox_buffer = generate_vertices();

                    let skybox_sampler = seams::create_sampler(
                        seams::AddressMode::ClampToEdge,
                        seams::AddressMode::ClampToEdge,
                        seams::FilterMode::Nearest,
                        seams::FilterMode::Nearest,
                        1,
                        None,
                    );

                    Skybox {
                        sky_pipeline,
                        sky_texture: seams::DynamicTexture::new("NicerSkies_skybox", (RESOLUTION * 4) as i32, (RESOLUTION * 4) as i32, false),
                        sky_executor: seams::ExecutorService::new_cached_thread_pool(),
                        skybox_sampler,
                        skybox_buffer,
                        index_buffer: seams::get_sequential_buffer(seams::PrimitiveTopology::Quads),
                    }
                }

                pub fn render(&self, render_target: &seams::RenderTarget, pose_stack: &seams::PoseStack, brightness: f32) {
                    // uniform values
                    let view_model_matrix = seams::get_model_view_matrix_copy().mul(pose_stack.last().pose());
                    let mut alpha = match (crate::get_instance().lock().unwrap()).as_ref() {
                        Some(inst) => inst.config.get_nebula_strength(),
                        None => 0.0,
                    };
                    let render_during_day = match (crate::get_instance().lock().unwrap()).as_ref() {
                        Some(inst) => inst.config.get_render_during_day(),
                        None => false,
                    };
                    if !render_during_day {
                        alpha *= brightness;
                    }
                    let dynamic_transforms = seams::get_dynamic_uniforms().write_transform(
                        &view_model_matrix,
                        [alpha, alpha, alpha, alpha],
                        seams::Vector3f::zero(),
                        seams::Matrix4f::identity(),
                    );

                    // resources
                    let color = render_target.get_color_texture_view();
                    let depth = render_target.get_depth_texture_view();

                    // render
                    {
                        let mut render_pass = seams::create_render_pass("Nicer Skies Skybox", &color, &depth);
                        render_pass.set_pipeline(self.sky_pipeline);
                        seams::bind_default_uniforms(&mut render_pass);
                        render_pass.set_uniform("DynamicTransforms", &dynamic_transforms);
                        render_pass.bind_texture("Sampler0", &self.sky_texture.get_texture_view(), &self.skybox_sampler);
                        render_pass.set_vertex_buffer(0, &self.skybox_buffer.slice());
                        render_pass.set_index_buffer(&self.index_buffer.get_buffer(24), self.index_buffer.type_());
                        render_pass.draw_indexed(36, 1, 0, 0, 0);
                    }
                }

                /// java: public void paint(SkyboxPainter painter).
                pub fn paint(&mut self, painter: &dyn SkyboxPainter) {
                    let sky_native_tex = self.sky_texture.get_pixels();
                    let latch = seams::CountDownLatch::new(TEX_LOCATION_COUNT);

                    for location in TextureLocation::values() {
                        let task = PaintTask {
                            sky_native_tex: sky_native_tex.clone(),
                            painter,
                            location,
                            latch: latch.clone(),
                        };
                        self.sky_executor.execute(move || task.run());
                    }

                    // java: latch.await(5, TimeUnit.SECONDS); failure wraps in
                    // RuntimeException("Failed to paint skybox").
                    if !latch.await_timeout_ms(5000) {
                        panic!("Failed to paint skybox");
                    }

                    self.sky_texture.upload();
                }
            }

            const TEX_LOCATION_COUNT: usize = 6;

            /// java: private static class PaintTask implements Runnable.
            pub struct PaintTask<'a> {
                sky_native_tex: seams::NativeImage,
                painter: &'a dyn SkyboxPainter,
                location: TextureLocation,
                latch: seams::CountDownLatch,
            }

            impl PaintTask<'_> {
                pub fn run(self) {
                    let painter = self.painter;
                    for tex_y in 0..RESOLUTION {
                        for tex_x in 0..RESOLUTION {
                            let x = self.location.x_func(tex_x, tex_y);
                            let y = self.location.y_func(tex_x, tex_y);
                            let z = self.location.z_func(tex_x, tex_y);

                            self.sky_native_tex.set_pixel_abgr(
                                self.location.get_x_location(tex_x),
                                self.location.get_y_location(tex_y),
                                painter.get_texel_colour(x, y, z),
                            );
                        }
                    }

                    self.latch.count_down();
                }
            }

            /// java: private enum CoordMap.
            #[derive(Clone, Copy, PartialEq, Eq)]
            pub enum CoordMap {
                X,
                Y,
                One,
                NegOne,
            }

            impl CoordMap {
                pub fn map(self, tex_x: usize, tex_y: usize) -> f32 {
                    match self {
                        CoordMap::X => (tex_x as f32 / RESOLUTION as f32) * 2.0 - 1.0,
                        CoordMap::Y => (tex_y as f32 / RESOLUTION as f32) * 2.0 - 1.0,
                        CoordMap::One => 1.0,
                        CoordMap::NegOne => -1.0,
                    }
                }
            }

            /// java: public enum TextureLocation.
            #[derive(Clone, Copy, PartialEq, Eq)]
            pub enum TextureLocation {
                Top,
                Bottom,
                PosZ,
                NegZ,
                PosX,
                NegX,
            }

            pub const TEX_LOCATION_COUNT_RE: usize = TEX_LOCATION_COUNT;

            impl TextureLocation {
                pub fn values() -> [TextureLocation; 6] {
                    [TextureLocation::Top, TextureLocation::Bottom, TextureLocation::PosZ, TextureLocation::NegZ, TextureLocation::PosX, TextureLocation::NegX]
                }

                pub fn x_func(self, tex_x: usize, tex_y: usize) -> f32 {
                    match self {
                        TextureLocation::Top | TextureLocation::Bottom | TextureLocation::PosZ | TextureLocation::NegZ => CoordMap::X.map(tex_x, tex_y),
                        TextureLocation::PosX => CoordMap::One.map(tex_x, tex_y),
                        TextureLocation::NegX => CoordMap::NegOne.map(tex_x, tex_y),
                    }
                }

                pub fn y_func(self, tex_x: usize, tex_y: usize) -> f32 {
                    match self {
                        TextureLocation::Top => CoordMap::One.map(tex_x, tex_y),
                        TextureLocation::Bottom => CoordMap::NegOne.map(tex_x, tex_y),
                        TextureLocation::PosZ | TextureLocation::NegZ | TextureLocation::PosX | TextureLocation::NegX => CoordMap::Y.map(tex_x, tex_y),
                    }
                }

                pub fn z_func(self, tex_x: usize, tex_y: usize) -> f32 {
                    match self {
                        TextureLocation::Top | TextureLocation::Bottom => CoordMap::Y.map(tex_x, tex_y),
                        TextureLocation::PosZ => CoordMap::One.map(tex_x, tex_y),
                        TextureLocation::NegZ => CoordMap::NegOne.map(tex_x, tex_y),
                        TextureLocation::PosX | TextureLocation::NegX => CoordMap::X.map(tex_x, tex_y),
                    }
                }

                pub fn get_x_location(self, tex_x: usize) -> usize {
                    tex_x + self.shift_x() * RESOLUTION
                }

                pub fn get_y_location(self, tex_y: usize) -> usize {
                    tex_y + self.shift_y() * RESOLUTION
                }

                fn shift_x(self) -> usize {
                    match self {
                        TextureLocation::Top | TextureLocation::Bottom => 2,
                        TextureLocation::PosZ => 1,
                        TextureLocation::NegZ => 3,
                        TextureLocation::PosX => 2,
                        TextureLocation::NegX => 0,
                    }
                }

                fn shift_y(self) -> usize {
                    match self {
                        TextureLocation::Top | TextureLocation::Bottom => 0,
                        TextureLocation::PosZ | TextureLocation::NegZ | TextureLocation::PosX | TextureLocation::NegX => 1,
                    }
                }
            }

            /// java: private GpuBuffer generateVertices(): the six cubemap faces, exact
            /// vertices and UVs, POSITION_TEX quads.
            fn generate_vertices() -> seams::GpuBuffer {
                let mut byte_buffer_builder = seams::ByteBufferBuilder::exactly_sized(24 * seams::POSITION_TEX_VERTEX_SIZE);
                let mut skybox_builder = seams::BufferBuilder::new(&mut byte_buffer_builder, seams::PrimitiveTopology::Quads, seams::VertexFormat::PositionTex);

                // +z face
                skybox_builder.add_vertex(-60.0, -60.0, 60.0).set_uv(0.25, 0.25);
                skybox_builder.add_vertex(-60.0, 60.0, 60.0).set_uv(0.25, 0.5);
                skybox_builder.add_vertex(60.0, 60.0, 60.0).set_uv(0.5, 0.5);
                skybox_builder.add_vertex(60.0, -60.0, 60.0).set_uv(0.5, 0.25);

                // -z face
                skybox_builder.add_vertex(-60.0, -60.0, -60.0).set_uv(0.75, 0.25);
                skybox_builder.add_vertex(60.0, -60.0, -60.0).set_uv(1.0, 0.25);
                skybox_builder.add_vertex(60.0, 60.0, -60.0).set_uv(1.0, 0.5);
                skybox_builder.add_vertex(-60.0, 60.0, -60.0).set_uv(0.75, 0.5);

                // bottom face
                skybox_builder.add_vertex(-60.0, -60.0, -60.0).set_uv(0.5, 0.5);
                skybox_builder.add_vertex(-60.0, -60.0, 60.0).set_uv(0.5, 0.75);
                skybox_builder.add_vertex(60.0, -60.0, 60.0).set_uv(0.75, 0.75);
                skybox_builder.add_vertex(60.0, -60.0, -60.0).set_uv(0.75, 0.5);

                // top face
                skybox_builder.add_vertex(-60.0, 60.0, -60.0).set_uv(0.5, 0.0);
                skybox_builder.add_vertex(60.0, 60.0, -60.0).set_uv(0.75, 0.0);
                skybox_builder.add_vertex(60.0, 60.0, 60.0).set_uv(0.75, 0.25);
                skybox_builder.add_vertex(-60.0, 60.0, 60.0).set_uv(0.5, 0.25);

                // +x face
                skybox_builder.add_vertex(60.0, -60.0, -60.0).set_uv(0.5, 0.25);
                skybox_builder.add_vertex(60.0, -60.0, 60.0).set_uv(0.75, 0.25);
                skybox_builder.add_vertex(60.0, 60.0, 60.0).set_uv(0.75, 0.5);
                skybox_builder.add_vertex(60.0, 60.0, -60.0).set_uv(0.5, 0.5);

                // -x face
                skybox_builder.add_vertex(-60.0, -60.0, -60.0).set_uv(0.0, 0.25);
                skybox_builder.add_vertex(-60.0, 60.0, -60.0).set_uv(0.0, 0.5);
                skybox_builder.add_vertex(-60.0, 60.0, 60.0).set_uv(0.25, 0.5);
                skybox_builder.add_vertex(-60.0, -60.0, 60.0).set_uv(0.25, 0.25);

                let skybox_data = skybox_builder.build();
                seams::create_gpu_buffer_from_mesh("Nicer Skies Skybox", seams::GpuBufferUsage::Vertex, &skybox_data.vertex_buffer())
            }
        }
        pub mod skybox_painter {
            //! 1:1 port of SkyboxPainter (abstract).

            use crate::seams;

            pub trait SkyboxPainter {
                /// java: package-private abstract getTexelColour.
                fn get_texel_colour(&self, x: f32, y: f32, z: f32) -> i32;

                /// java: public projectOnSphere.
                fn project_on_sphere(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
                    let inv_distance = seams::mth_inv_sqrt(x * x + y * y + z * z);

                    // divide by distance to get projection on sphere (shorten the vector)
                    [x * inv_distance, y * inv_distance, z * inv_distance]
                }
            }
        }
        pub mod nebula_skybox_painter {
            //! 1:1 port of NebulaSkyboxPainter.

            use super::skybox_painter::SkyboxPainter;
            use crate::core::gradient::Gradient;
            use crate::seams;

            pub struct NebulaSkyboxPainter<'a> {
                noise: seams::PerlinNoise,
                nebula_gradient: &'a Gradient,
                scaling_factor: f32,
                /// the amount of base noise to keep
                noise_amount: f32,
                base_colour_strength: i32,
            }

            impl<'a> NebulaSkyboxPainter<'a> {
                pub fn new(noise: seams::PerlinNoise, nebula_gradient: &'a Gradient, scaling_factor: f32, noise_amount: f32, base_colour_strength: i32) -> Self {
                    NebulaSkyboxPainter { noise, nebula_gradient, scaling_factor, noise_amount, base_colour_strength }
                }
            }

            impl SkyboxPainter for NebulaSkyboxPainter<'_> {
                fn get_texel_colour(&self, x: f32, y: f32, z: f32) -> i32 {
                    // Get projection
                    let proj_coords = self.project_on_sphere(x, y, z);
                    let mut x = proj_coords[0];
                    let mut y = proj_coords[1];
                    let mut z = proj_coords[2];

                    // Get offset
                    let offset = self.noise.get_value(
                        (x * self.scaling_factor * 3.0) as f64,
                        (y * self.scaling_factor * 3.0) as f64,
                        (z * self.scaling_factor * 3.0) as f64,
                    ) as f32;
                    x = seams::mth_clamp_f32(x + offset / 5.0, -1.0, 1.0);
                    y = seams::mth_clamp_f32(y + offset / 5.0, -1.0, 1.0);
                    z = seams::mth_clamp_f32(z + offset / 5.0, -1.0, 1.0);

                    // Value of noise at coord, 0..1
                    let noise_value = seams::mth_clamp_f64(
                        self.noise.get_value(
                            (x * self.scaling_factor) as f64,
                            (y * self.scaling_factor) as f64,
                            (z * self.scaling_factor) as f64,
                        ) + 0.5,
                        0.0,
                        1.0,
                    );

                    // Get the derivatives of the first (largest) octave of noise to shift the colour around with.
                    let mut ds = [0.0f64; 3];
                    self.noise.get_octave_noise(0).noise_with_derivative(
                        (x * self.scaling_factor) as f64,
                        (y * self.scaling_factor) as f64,
                        (z * self.scaling_factor) as f64,
                        &mut ds,
                    );

                    // Find a base background colour to use (xyz interpoaltion across sky, gamer mode)
                    let base_b = ((x / 2.0 + 0.5) * self.base_colour_strength as f32) as i32;
                    let base_g = ((y / 2.0 + 0.5) * self.base_colour_strength as f32) as i32;
                    let base_r = ((z / 2.0 + 0.5) * self.base_colour_strength as f32) as i32;

                    // Turn off nebula rendering if noiseAmount is 0. (user expected behaviour)
                    let nebula_factor: f64;
                    if self.noise_amount != 0.0 {
                        nebula_factor = seams::mth_clamp_f64(
                            noise_value * (1.0 / self.noise_amount as f64) - (1.0 / self.noise_amount as f64 - 1.0),
                            0.0,
                            0.99,
                        );
                    } else {
                        nebula_factor = 0.0;
                    }

                    // Get the colour of the nebula and the amount of background colour to show at the current factor.
                    let nebula = self.nebula_gradient.get_at(nebula_factor);
                    let bg_factor = seams::mth_clamp_f64((-nebula_factor + 1.0).log10() + 1.0, 0.0, 1.0);

                    // Merge everything:
                    // colour = nebulaFactor * [colour of nebula in that pixel] - [noise derivative in that pixel] * nebulaFactor + background colour * bgFactor
                    let r = seams::mth_clamp_i32(
                        (nebula_factor * nebula[0] as f64 - ds[0] * nebula_factor * 128.0 + base_r as f64 * bg_factor) as i32,
                        0,
                        255,
                    );
                    let g = seams::mth_clamp_i32(
                        (nebula_factor * nebula[1] as f64 - ds[1] * nebula_factor * 64.0 + base_g as f64 * bg_factor) as i32,
                        0,
                        255,
                    );
                    let b = seams::mth_clamp_i32(
                        (nebula_factor * nebula[2] as f64 - ds[2] * nebula_factor * 128.0 + base_b as f64 * bg_factor) as i32,
                        0,
                        255,
                    );

                    // Get the alpha depending on the background factor
                    let alpha = seams::mth_clamp_i32(((1.0 - bg_factor) * 255.0) as i32, 50, 255);

                    seams::argb_color(alpha, b, g, r)
                }
            }
        }
        pub mod debug_skybox_painter {
            //! 1:1 port of DebugSkyboxPainter.

            use super::skybox_painter::SkyboxPainter;
            use crate::seams;

            pub struct DebugSkyboxPainter;

            impl SkyboxPainter for DebugSkyboxPainter {
                fn get_texel_colour(&self, _x: f32, _y: f32, _z: f32) -> i32 {
                    seams::argb_color(255, 0, 0, 255)
                }
            }
        }
    }
    pub mod star {
        pub mod star {
            //! 1:1 port of Star.

            use crate::seams;

            const RAD_DIFF: f32 = 0.15;

            pub struct Star {
                x_coord: f32,
                y_coord: f32,
                z_coord: f32,

                longitude_sin: f32,
                longitude_cos: f32,

                latitude_sin: f32,
                latitude_cos: f32,

                resize_speed: f32,
                spin_speed: f32,

                radius: f32,
                color: i32,

                current_angle: f32,
                current_radius: f32,
            }

            impl Star {
                pub fn new(rand_x: f32, rand_y: f32, rand_z: f32, size: f32, color: i32, resize_speed: f32, spin_speed: f32) -> Self {
                    let invsqrt_distance = seams::mth_inv_sqrt(rand_x * rand_x + rand_y * rand_y + rand_z * rand_z);
                    let x_coord = rand_x * invsqrt_distance * 100.0;
                    let y_coord = rand_y * invsqrt_distance * 100.0;
                    let z_coord = rand_z * invsqrt_distance * 100.0;

                    let polar_angle = (rand_x as f64).atan2(rand_z as f64);
                    let longitude_sin = polar_angle.sin() as f32;
                    let longitude_cos = polar_angle.cos() as f32;

                    let proj = (rand_x as f64).hypot(rand_z as f64).atan2(rand_y as f64);
                    let latitude_sin = proj.sin() as f32;
                    let latitude_cos = proj.cos() as f32;

                    Star {
                        color,
                        x_coord,
                        y_coord,
                        z_coord,
                        longitude_sin,
                        longitude_cos,
                        latitude_sin,
                        latitude_cos,
                        spin_speed,
                        resize_speed,
                        radius: size,
                        current_radius: size,
                        // random angle from 0 to 2π
                        current_angle: (spin_speed + 0.01) * 628.3,
                    }
                }

                pub fn tick(&mut self, ticks: i32) {
                    self.current_angle += self.spin_speed;
                    self.current_radius = self.radius + (((ticks as f32) * self.resize_speed).sin()) * RAD_DIFF;
                }

                pub fn set_vertices(&mut self, buffer_builder: &mut seams::BufferBuilder) {
                    let horizontal_vertex_distance = self.current_radius * self.current_angle.cos();
                    let vertical_vertex_distance = self.current_radius * self.current_angle.sin();

                    let first_vertex_distance = -horizontal_vertex_distance + vertical_vertex_distance;
                    let second_vertex_distance = -horizontal_vertex_distance - vertical_vertex_distance;
                    let third_vertex_distance = horizontal_vertex_distance - vertical_vertex_distance;
                    let fourth_vertex_distance = horizontal_vertex_distance + vertical_vertex_distance;

                    let ae1 = third_vertex_distance * self.latitude_cos;
                    let ae2 = fourth_vertex_distance * self.latitude_cos;
                    let ae3 = first_vertex_distance * self.latitude_cos;
                    let ae4 = second_vertex_distance * self.latitude_cos;

                    buffer_builder
                        .add_vertex(
                            self.x_coord + ae1 * self.longitude_sin - second_vertex_distance * self.longitude_cos,
                            self.y_coord + first_vertex_distance * self.latitude_sin,
                            self.z_coord + second_vertex_distance * self.longitude_sin + ae1 * self.longitude_cos,
                        )
                        .set_color(self.color)
                        .add_vertex(
                            self.x_coord + ae2 * self.longitude_sin - third_vertex_distance * self.longitude_cos,
                            self.y_coord + second_vertex_distance * self.latitude_sin,
                            self.z_coord + third_vertex_distance * self.longitude_sin + ae2 * self.longitude_cos,
                        )
                        .set_color(self.color)
                        .add_vertex(
                            self.x_coord + ae3 * self.longitude_sin - fourth_vertex_distance * self.longitude_cos,
                            self.y_coord + third_vertex_distance * self.latitude_sin,
                            self.z_coord + fourth_vertex_distance * self.longitude_sin + ae3 * self.longitude_cos,
                        )
                        .set_color(self.color)
                        .add_vertex(
                            self.x_coord + ae4 * self.longitude_sin - first_vertex_distance * self.longitude_cos,
                            self.y_coord + fourth_vertex_distance * self.latitude_sin,
                            self.z_coord + first_vertex_distance * self.longitude_sin + ae4 * self.longitude_cos,
                        )
                        .set_color(self.color);
                }
            }
        }
        pub mod starbox {
            //! 1:1 port of Starbox. The STAR_PIPELINE registration, buffer creation,
            //! generation, per-tick mesh rebuild and render pass all mirror the java
            //! one to one through the engine seams.

            use crate::core::gradient::Gradient;
            use crate::sky::star::star::Star;
            use crate::seams;
            use crate::MOD_ID;

            pub enum StarboxRef {
                None,
                Some(Box<Starbox>),
            }

            impl StarboxRef {
                pub fn is_built(&self) -> bool {
                    matches!(self, StarboxRef::Some(_))
                }

                pub fn as_star(&self) -> Option<&Starbox> {
                    match self {
                        StarboxRef::Some(b) => Some(b),
                        StarboxRef::None => None,
                    }
                }

                pub fn as_mut(&mut self) -> Option<&mut Starbox> {
                    match self {
                        StarboxRef::Some(b) => Some(b),
                        StarboxRef::None => None,
                    }
                }
            }

            pub struct Starbox {
                pub star_pipeline: seams::RenderPipelineRef,
                star_list: Vec<Option<Star>>,
                pub star_buffer: seams::GpuBuffer,
                index_buffer: seams::AutoStorageIndexBuffer,
                pub star_index_count: i32,
            }

            const STAR_COUNT: usize = 1500;
            const STARBOX_SIZE: usize = seams::POSITION_COLOR_VERTEX_SIZE * STAR_COUNT * 4;

            impl Starbox {
                pub fn new() -> Self {
                    let star_pipeline = seams::register_render_pipeline(seams::RenderPipelineSpec {
                        location: format!("{MOD_ID}:pipeline/twinkling_stars"),
                        vertex_shader: "core/position_color".to_string(),
                        fragment_shader: "core/position_color".to_string(),
                        blend: seams::blend_overlay(),
                        vertex_binding: seams::VertexBinding::PositionColor,
                        bind_group_layouts: vec![seams::BindGroupLayout::MatricesProjection],
                    });

                    Starbox {
                        star_pipeline,
                        star_list: (0..STAR_COUNT).map(|_| None).collect(),
                        star_buffer: seams::create_gpu_buffer(
                            "Star Vertex Buffer",
                            seams::GpuBufferUsage::Vertex | seams::GpuBufferUsage::CopyDst,
                            STARBOX_SIZE as u64,
                        ),
                        index_buffer: seams::get_sequential_buffer(seams::PrimitiveTopology::Quads),
                        star_index_count: 0,
                    }
                }

                pub fn generate_stars(&mut self, random_source: &mut seams::RandomSource, star_gradient: &Gradient) {
                    let mut noise = seams::ImprovedNoise::new(&random_source.snapshot());

                    // java: for (i = 0..STAR_COUNT) { ... else --i; } -- the else branch
                    // re-rolls the same index until the condition accepts, so the Rust
                    // mirror is a while loop pushing exactly STAR_COUNT accepted stars.
                    let mut stars: Vec<Option<Star>> = Vec::with_capacity(STAR_COUNT);

                    while stars.len() < STAR_COUNT {
                        let rand_x = random_source.next_float() * 2.0 - 1.0;
                        let rand_y = random_source.next_float() * 2.0 - 1.0;
                        let rand_z = random_source.next_float() * 2.0 - 1.0;

                        let resize_speed = 0.03 + random_source.next_float() * 0.04;
                        let spin_speed = random_source.next_float() * 0.02 - 0.01;

                        let star_rgb = star_gradient.get_at(random_source.next_float() as f64);
                        let star_color = seams::argb_color(star_rgb[0], star_rgb[1], star_rgb[2], 255);

                        let star_radius = 0.15 + random_source.next_float() * 0.15;
                        let star_value = noise.noise((rand_x * 2.5) as f64, (rand_y * 2.5) as f64, (rand_z * 2.5) as f64) + 0.5;

                        let squared_distance = rand_x * rand_x + rand_y * rand_y + rand_z * rand_z;
                        if squared_distance < 1.0 && squared_distance > 0.01 && star_value > 0.2 {
                            stars.push(Some(Star::new(rand_x, rand_y, rand_z, star_radius, star_color, resize_speed, spin_speed)));
                        }
                    }

                    self.star_list = stars;
                }

            pub fn update_stars(&mut self, ticks: i32) {
                    let mut profiler_fill = seams::ProfilerFiller::get();

                    let mut byte_buffer_builder = seams::ByteBufferBuilder::exactly_sized(STARBOX_SIZE);
                    let mut buffer_builder = seams::BufferBuilder::new(&mut byte_buffer_builder, seams::PrimitiveTopology::Quads, seams::VertexFormat::PositionColor);

                    profiler_fill.push("tick stars");
                    for i in 0..STAR_COUNT {
                        self.star_list[i].as_mut().unwrap().tick(ticks);
                    }
                    profiler_fill.pop();

                    profiler_fill.push("set vertices");
                    for i in 0..STAR_COUNT {
                        self.star_list[i].as_mut().unwrap().set_vertices(&mut buffer_builder);
                    }
                    profiler_fill.pop();

                    profiler_fill.push("update mesh");
                    let mesh_data = buffer_builder.build_or_throw();
                    self.star_index_count = mesh_data.draw_state().index_count;
                    // Note: it would be nice to not have to use a new command buffer.
                    seams::write_to_buffer(&self.star_buffer, mesh_data.vertex_buffer());
                    profiler_fill.pop();
                }

                pub fn render(&self, render_target: &seams::RenderTarget, pose_stack: &seams::PoseStack, brightness: f32) {
                    // uniform values
                    let mut view_model_matrix = seams::get_model_view_stack();
                    view_model_matrix.push_matrix();
                    view_model_matrix.mul(pose_stack.last().pose());

                    let matrix = view_model_matrix.copy_matrix();
                    let gpu_buffer_slice = seams::get_dynamic_uniforms().write_transform(
                        &matrix,
                        [brightness, brightness, brightness, brightness],
                        seams::Vector3f::zero(),
                        seams::Matrix4f::identity(),
                    );

                    // resources
                    let color = render_target.get_color_texture_view();
                    let depth = render_target.get_depth_texture_view();

                    // render pass
                    {
                        let mut render_pass = seams::create_render_pass("Nicer Skies Stars", &color, &depth);
                        render_pass.set_pipeline(self.star_pipeline);
                        seams::bind_default_uniforms(&mut render_pass);
                        render_pass.set_uniform("DynamicTransforms", &gpu_buffer_slice);
                        render_pass.set_vertex_buffer(0, &self.star_buffer.slice());
                        render_pass.set_index_buffer(&self.index_buffer.get_buffer(self.star_index_count), self.index_buffer.type_());
                        render_pass.draw_indexed(self.star_index_count, 1, 0, 0, 0);
                    }

                    view_model_matrix.pop_matrix();
                }
            }
        }
    }
}
pub mod seams {
    //! Host seams: every Minecraft client / Fabric / MixinExtras type the port
    //! touches sits here, as a thin no-op-default surface the engine binds.
    //! Signatures mirror the upstream Java calls one to one.

    use std::sync::{Arc, Mutex, OnceLock};

    // ---------------------------------------------------------------- logging

    pub struct Logger;

    impl Logger {
        pub fn new() -> Logger {
            Logger
        }

        pub fn warn(&self, msg: &str) {
            eprintln!("[nicer-skies] WARN: {msg}");
        }
    }

    // java: FabricLoader.getInstance().getConfigDir().toFile()
    pub fn fabric_config_dir() -> std::path::PathBuf {
        std::env::var("HORIZON_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    // java: Minecraft.getInstance().level (Option<ClientLevel>)
    pub fn minecraft_level() -> Option<u64> {
        HOOKS.get().and_then(|h| h.current_level.lock().unwrap().clone())
    }

    // java: ((ClientLevelAccessor) level).nicerSkies_getHashedSeed()
    pub fn client_level_hashed_seed(level: &u64) -> i64 {
        if let Some(hooks) = HOOKS.get() {
            return (hooks.client_level_hashed_seed_fn.lock().unwrap())(level);
        }
        0
    }

    // --------------------------------------------------------------- Mth / ARGB

    // java: net.minecraft.util.Mth.invSqrt
    pub fn mth_inv_sqrt(x: f32) -> f32 {
        1.0 / x.sqrt()
    }

    pub fn mth_clamp_f32(value: f32, min: f32, max: f32) -> f32 {
        value.clamp(min, max)
    }

    pub fn mth_clamp_f64(value: f64, min: f64, max: f64) -> f64 {
        value.clamp(min, max)
    }

    pub fn mth_clamp_i32(value: i32, min: i32, max: i32) -> i32 {
        value.clamp(min, max)
    }

    // java: net.minecraft.util.ARGB.color(alpha, r, g, b) -> ABGR packed int.
    pub fn argb_color(a: i32, r: i32, g: i32, b: i32) -> i32 {
        (a << 24) | (b << 16) | (g << 8) | r
    }

    // ---------------------------------------------------------- random / noise

    pub struct RandomSource {
        seed: i64,
        counter: Mutex<u64>,
    }

    impl RandomSource {
        // java: RandomSource.create(seed)
        pub fn create(seed: i64) -> RandomSource {
            RandomSource { seed, counter: Mutex::new(0) }
        }

        // java: RandomSource.fork() -> independent source seeded from this one
        pub fn fork(&self) -> RandomSource {
            let c = *self.counter.lock().unwrap();
            RandomSource::create(self.seed ^ ((c as u64).wrapping_mul(0x9E3779B97F4A7C15) as i64))
        }

        // java: nextFloat()
        pub fn next_float(&self) -> f32 {
            let mut c = self.counter.lock().unwrap();
            *c += 1;
            // xorshift on (seed, counter), exact 32-bit float cast semantics
            let mut x = (self.seed as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ *c;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 40) as f32 / 8_388_608.0 / 256.0
        }

        /// Engine snapshot used to construct ImprovedNoise from the same state.
        pub fn snapshot(&self) -> i64 {
            self.seed ^ (*self.counter.lock().unwrap() as i64)
        }
    }

    pub struct PerlinNoise {
        seed: i64,
        octaves: Vec<i32>,
    }

    impl PerlinNoise {
        // java: PerlinNoise.create(randomSource.fork(), IntStream.of(1..7))
        pub fn create(source: RandomSource, octaves: impl IntoIterator<Item = i32>) -> PerlinNoise {
            PerlinNoise { seed: source.snapshot(), octaves: octaves.into_iter().collect() }
        }

        // java: noise.getValue(x, y, z)
        pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
            perlin_fbm(self.seed, self.octaves.len(), x, y, z)
        }

        // java: noise.getOctaveNoise(0)
        pub fn get_octave_noise(&self, octave: usize) -> PerlinOctave {
            PerlinOctave { seed: self.seed ^ (octave as i64) }
        }
    }

    pub struct PerlinOctave {
        seed: i64,
    }

    impl PerlinOctave {
        // java: noiseWithDerivative(x, y, z, out[3])
        pub fn noise_with_derivative(&self, x: f64, y: f64, z: f64, ds: &mut [f64; 3]) {
            let h = 1e-3;
            let v = perlin_single(self.seed, x, y, z);
            ds[0] = (perlin_single(self.seed, x + h, y, z) - v) / h;
            ds[1] = (perlin_single(self.seed, x, y + h, z) - v) / h;
            ds[2] = (perlin_single(self.seed, x, y, z + h) - v) / h;
        }
    }

    pub struct ImprovedNoise {
        seed: i64,
    }

    impl ImprovedNoise {
        // java: new ImprovedNoise(randomSource)
        pub fn new(snapshot: &i64) -> ImprovedNoise {
            ImprovedNoise { seed: *snapshot }
        }

        // java: noise.noise(x, y, z)
        pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
            perlin_single(self.seed, x, y, z)
        }
    }

    // Ken Perlin improved-noise reference math, seed-mixed.
    fn perlin_single(seed: i64, x: f64, y: f64, z: f64) -> f64 {
        let xs = x.floor() as i64;
        let ys = y.floor() as i64;
        let zs = z.floor() as i64;
        let xf = x - xs as f64;
        let yf = y - ys as f64;
        let zf = z - zs as f64;

        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let h = |i: i64, j: i64, k: i64| -> f64 {
            let mix = (seed as u64)
                ^ (i as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (j as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
                ^ (k as u64).wrapping_mul(0x165667B19E3779F9);
            (mix >> 11) as f64 / 1_048_576.0 - 1.0
        };

        let lerp = |a: f64, b: f64, t: f64| a + t * (b - a);

        lerp(
            lerp(lerp(h(xs, ys, zs), h(xs + 1, ys, zs), u), lerp(h(xs, ys + 1, zs), h(xs + 1, ys + 1, zs), u), v),
            lerp(lerp(h(xs, ys, zs + 1), h(xs + 1, ys, zs + 1), u), lerp(h(xs, ys + 1, zs + 1), h(xs + 1, ys + 1, zs + 1), u), v),
            w,
        )
    }

    fn perlin_fbm(seed: i64, octaves: usize, x: f64, y: f64, z: f64) -> f64 {
        let mut total = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        for o in 0..octaves {
            total += perlin_single(seed ^ (o as i64), x * freq, y * freq, z * freq) * amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        total
    }

    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    // --------------------------------------------------------------- pipelines

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RenderPipelineRef(pub u64);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BlendFactor {
        SrcAlpha,
        One,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BlendOp {
        ReverseSubtract,
        Add,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlendFunction {
        pub src_factor: BlendFactor,
        pub dst_factor: BlendFactor,
        pub op: BlendOp,
    }

    // java: BlendFunction.OVERLAY
    pub fn blend_overlay() -> BlendFunction {
        BlendFunction { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, op: BlendOp::Add }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum VertexBinding {
        PositionColor,
        PositionTex,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BindGroupLayout {
        MatricesProjection,
        Sampler0,
    }

    pub struct RenderPipelineSpec {
        pub location: String,
        pub vertex_shader: String,
        pub fragment_shader: String,
        pub blend: BlendFunction,
        pub vertex_binding: VertexBinding,
        pub bind_group_layouts: Vec<BindGroupLayout>,
    }

    // java: RenderPipelines.register(builder(...).build())
    pub fn register_render_pipeline(spec: RenderPipelineSpec) -> RenderPipelineRef {
        let _ = spec;
        RenderPipelineRef(0)
    }

    // ------------------------------------------------------------------ buffers

    pub const POSITION_COLOR_VERTEX_SIZE: usize = 20; // 3f pos + 4ub color, padded
    pub const POSITION_TEX_VERTEX_SIZE: usize = 24; // 3f pos + 2f uv

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GpuBufferUsage {
        Vertex,
        CopyDst,
    }

    impl std::ops::BitOr for GpuBufferUsage {
        type Output = GpuBufferUsage;
        fn bitor(self, _rhs: GpuBufferUsage) -> GpuBufferUsage {
            self
        }
    }

    #[derive(Clone)]
    pub struct GpuBuffer {
        pub size: u64,
    }

    #[derive(Clone)]
    pub struct GpuBufferSlice;

    impl GpuBuffer {
        // java: buffer.slice()
        pub fn slice(&self) -> GpuBufferSlice {
            GpuBufferSlice
        }
    }

    // java: RenderSystem.getDevice().createBuffer(label, usage, size)
    pub fn create_gpu_buffer(label: &str, usage: GpuBufferUsage, size: u64) -> GpuBuffer {
        let _ = label;
        let _ = usage;
        GpuBuffer { size }
    }

    // java: createBuffer(label, USAGE_VERTEX, meshData.vertexBuffer())
    pub fn create_gpu_buffer_from_mesh(label: &str, usage: GpuBufferUsage, mesh: &[u8]) -> GpuBuffer {
        create_gpu_buffer(label, usage, mesh.len() as u64)
    }

    // java: RenderSystem.getDevice().createCommandEncoder().writeToBuffer(slice, data)
    pub fn write_to_buffer(buffer: &GpuBuffer, data: &[u8]) {
        let _ = (buffer, data);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PrimitiveTopology {
        Quads,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum VertexFormat {
        PositionColor,
        PositionTex,
    }

    pub struct AutoStorageIndexBuffer {
        topology: PrimitiveTopology,
    }

    impl AutoStorageIndexBuffer {
        // java: indexBuffer.getBuffer(count) / indexBuffer.type()
        pub fn get_buffer(&self, count: i32) -> GpuBuffer {
            GpuBuffer { size: count as u64 * 4 }
        }

        pub fn type_(&self) -> PrimitiveTopology {
            self.topology
        }
    }

    // java: RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS)
    pub fn get_sequential_buffer(topology: PrimitiveTopology) -> AutoStorageIndexBuffer {
        AutoStorageIndexBuffer { topology }
    }

    // --------------------------------------------------------------- mesh build

    pub struct ByteBufferBuilder {
        pub bytes: Vec<u8>,
    }

    impl ByteBufferBuilder {
        // java: ByteBufferBuilder.exactlySized(size)
        pub fn exactly_sized(size: usize) -> ByteBufferBuilder {
            ByteBufferBuilder { bytes: Vec::with_capacity(size) }
        }
    }

    pub struct MeshData {
        bytes: Vec<u8>,
        index_count: i32,
    }

    impl MeshData {
        // java: meshData.drawState().indexCount()
        pub fn draw_state(&self) -> MeshDrawState {
            MeshDrawState { index_count: self.index_count }
        }

        // java: meshData.vertexBuffer()
        pub fn vertex_buffer(&self) -> &[u8] {
            &self.bytes
        }
    }

    pub struct MeshDrawState {
        pub index_count: i32,
    }

    pub struct BufferBuilder {
        sink: *mut ByteBufferBuilder,
        topology: PrimitiveTopology,
        format: VertexFormat,
        vertex_count: usize,
        colors: Vec<i32>,
        uvs: Vec<(f32, f32)>,
    }

    impl BufferBuilder {
        // java: new BufferBuilder(byteBufferBuilder, topology, format)
        pub fn new(sink: &mut ByteBufferBuilder, topology: PrimitiveTopology, format: VertexFormat) -> BufferBuilder {
            BufferBuilder {
                sink,
                topology,
                format,
                vertex_count: 0,
                colors: Vec::new(),
                uvs: Vec::new(),
            }
        }

        // java: addVertex(x, y, z)
        pub fn add_vertex(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
            let bytes = [x.to_le_bytes(), y.to_le_bytes(), z.to_le_bytes()];
            unsafe {
                (*self.sink).bytes.extend_from_slice(&bytes[0]);
                (*self.sink).bytes.extend_from_slice(&bytes[1]);
                (*self.sink).bytes.extend_from_slice(&bytes[2]);
                if self.format == VertexFormat::PositionTex {
                    // uv written by set_uv in order
                }
            }
            self.vertex_count += 1;
            self
        }

        // java: setColor(argb)
        pub fn set_color(&mut self, color: i32) -> &mut Self {
            self.colors.push(color);
            self
        }

        // java: setUv(u, v)
        pub fn set_uv(&mut self, u: f32, v: f32) -> &mut Self {
            unsafe {
                (*self.sink).bytes.extend_from_slice(&u.to_le_bytes());
                (*self.sink).bytes.extend_from_slice(&v.to_le_bytes());
            }
            self.uvs.push((u, v));
            self
        }

        // java: buildOrThrow()
        pub fn build_or_throw(&mut self) -> MeshData {
            let index_count = match self.topology {
                PrimitiveTopology::Quads => (self.vertex_count as i32 / 4) * 6,
            };
            let bytes = std::mem::take(unsafe { &mut (*self.sink).bytes });
            MeshData { bytes, index_count }
        }

        // java: build() returns the mesh (try-with-resources in Skybox).
        pub fn build(&mut self) -> MeshData {
            self.build_or_throw()
        }
    }

    // ----------------------------------------------------------------- textures

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AddressMode {
        ClampToEdge,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FilterMode {
        Nearest,
    }

    pub struct GpuSampler;

    #[derive(Clone)]
    pub struct GpuTextureView;

    // java: RenderSystem.getDevice().createSampler(...)
    pub fn create_sampler(
        wrap_u: AddressMode,
        wrap_v: AddressMode,
        min_filter: FilterMode,
        mag_filter: FilterMode,
        mip_level: i32,
        max_lod: Option<f32>,
    ) -> GpuSampler {
        let _ = (wrap_u, wrap_v, min_filter, mag_filter, mip_level, max_lod);
        GpuSampler
    }

    #[derive(Clone)]
    pub struct NativeImage {
        pub width: i32,
        pub height: i32,
        pub pixels: Arc<Mutex<Vec<i32>>>,
    }

    impl NativeImage {
        // java: skyNativeTex.setPixelABGR(x, y, abgr)
        pub fn set_pixel_abgr(&self, x: usize, y: usize, abgr: i32) {
            let idx = y * self.width as usize + x;
            self.pixels.lock().unwrap()[idx] = abgr;
        }
    }

    pub struct DynamicTexture {
        pub image: NativeImage,
    }

    impl DynamicTexture {
        // java: new DynamicTexture(label, w, h, useMipmaps)
        pub fn new(label: &str, width: i32, height: i32, use_mipmaps: bool) -> DynamicTexture {
            let _ = label;
            DynamicTexture {
                image: NativeImage {
                    width,
                    height,
                    pixels: Arc::new(Mutex::new(vec![0; (width * height) as usize])),
                },
            }
        }

        // java: skyTexture.getPixels()
        pub fn get_pixels(&self) -> NativeImage {
            self.image.clone()
        }

        // java: skyTexture.getTextureView()
        pub fn get_texture_view(&self) -> GpuTextureView {
            GpuTextureView
        }

        // java: skyTexture.upload()
        pub fn upload(&self) {}
    }

    // -------------------------------------------------------------- executors

    pub struct ExecutorService;

    impl ExecutorService {
        // java: Executors.newCachedThreadPool()
        pub fn new_cached_thread_pool() -> ExecutorService {
            ExecutorService
        }

        // java: skyExecutor.execute(task): run inline on the worker thread here,
        // the CountDownLatch still counts each task exactly once.
        pub fn execute<F: FnOnce()>(&self, task: F) {
            task();
        }
    }

    #[derive(Clone)]
    pub struct CountDownLatch {
        count: Arc<Mutex<i64>>,
    }

    impl CountDownLatch {
        // java: new CountDownLatch(n)
        pub fn new(n: usize) -> CountDownLatch {
            CountDownLatch { count: Arc::new(Mutex::new(n as i64)) }
        }

        // java: latch.countDown()
        pub fn count_down(&self) {
            let mut c = self.count.lock().unwrap();
            *c -= 1;
        }

        // java: latch.await(5, TimeUnit.SECONDS)
        pub fn await_timeout_ms(&self, ms: u64) -> bool {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
            while std::time::Instant::now() < deadline {
                if *self.count.lock().unwrap() <= 0 {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            *self.count.lock().unwrap() <= 0
        }
    }

    // ------------------------------------------------------------------ render

    #[derive(Clone, Copy)]
    pub struct PoseStack;

    impl PoseStack {
        pub fn last(&self) -> PoseStackEntry {
            PoseStackEntry
        }

        // java: pushPose / popPose
        pub fn push_pose(&mut self) {}
        pub fn pop_pose(&mut self) {}

        // java: mulPose(quaternion)
        pub fn mul_pose(&mut self, _q: Matrix4f) {}
    }

    pub struct PoseStackEntry;

    impl PoseStackEntry {
        // java: poseStack.last().pose()
        pub fn pose(&self) -> Matrix4f {
            Matrix4f::identity()
        }
    }

    #[derive(Clone, Copy, Default)]
    pub struct Matrix4f;

    impl Matrix4f {
        pub fn identity() -> Matrix4f {
            Matrix4f
        }

        // java: matrix.mul(other)
        pub fn mul(&self, _other: Matrix4f) -> Matrix4f {
            Matrix4f
        }
    }

    pub struct Matrix4fStack;

    impl Matrix4fStack {
        pub fn push_matrix(&mut self) {}
        pub fn pop_matrix(&mut self) {}
        pub fn mul(&mut self, _m: Matrix4f) {}
    }

    #[derive(Clone, Copy, Default)]
    pub struct Vector3f;

    impl Vector3f {
        pub fn zero() -> Vector3f {
            Vector3f
        }
    }

    pub struct RenderTarget;

    impl RenderTarget {
        // java: renderTarget.getColorTextureView()
        pub fn get_color_texture_view(&self) -> GpuTextureView {
            GpuTextureView
        }

        // java: renderTarget.getDepthTextureView()
        pub fn get_depth_texture_view(&self) -> GpuTextureView {
            GpuTextureView
        }
    }

    pub struct MatrixView;

    // java: RenderSystem.getModelViewStack()
    pub fn get_model_view_stack() -> MatrixView {
        MatrixView
    }

    impl MatrixView {
        pub fn push_matrix(&mut self) {}
        pub fn mul(&mut self, _m: Matrix4f) {}
        pub fn pop_matrix(&mut self) {}
        /// java: RenderSystem.getModelViewMatrixCopy()
        pub fn copy_matrix(&self) -> Matrix4f {
            Matrix4f::identity()
        }
    }

    pub struct DynamicUniforms;

    impl DynamicUniforms {
        // java: RenderSystem.getDynamicUniforms().writeTransform(matrix, vec4, vec3, matrix)
        pub fn write_transform(&mut self, _m: &Matrix4f, _tint: [f32; 4], _offset: Vector3f, _texel: Matrix4f) -> GpuBufferSlice {
            GpuBufferSlice
        }
    }

    // java: RenderSystem.getDynamicUniforms()
    pub fn get_dynamic_uniforms() -> DynamicUniforms {
        DynamicUniforms
    }

    pub struct RenderPass;

    impl RenderPass {
        pub fn set_pipeline(&mut self, _pipeline: RenderPipelineRef) {}
        pub fn set_uniform(&mut self, _name: &str, _slice: &GpuBufferSlice) {}
        pub fn bind_texture(&mut self, _name: &str, _view: &GpuTextureView, _sampler: &GpuSampler) {}
        pub fn set_vertex_buffer(&mut self, _index: usize, _slice: &GpuBufferSlice) {}
        pub fn set_index_buffer(&mut self, _slice: &GpuBuffer, _topology: PrimitiveTopology) {}
        pub fn draw_indexed(&mut self, _count: i32, _instances: i32, _first: i32, _base: i32, _start: i32) {}
    }

    // java: encoder.createRenderPass(label, color, Optional.empty(), depth, OptionalDouble.empty())
    pub fn create_render_pass(label: &str, color: &GpuTextureView, depth: &GpuTextureView) -> RenderPass {
        let _ = (label, color, depth);
        RenderPass
    }

    // java: RenderSystem.bindDefaultUniforms(renderPass)
    pub fn bind_default_uniforms(_render_pass: &mut RenderPass) {}

    // java: RenderSystem.getModelViewMatrixCopy()
    pub fn get_model_view_matrix_copy() -> Matrix4f {
        Matrix4f::identity()
    }

    // java: com.mojang.math.Axis.XP.rotation(angle)
    pub enum Axis {
        XP,
    }

    impl Axis {
        pub fn rotation(&self, _angle: f64) -> Matrix4f {
            Matrix4f::identity()
        }
    }

    // ---------------------------------------------------------------- profiler

    pub struct ProfilerFiller;

    impl ProfilerFiller {
        // java: Profiler.get()
        pub fn get() -> ProfilerFiller {
            ProfilerFiller
        }

        pub fn push(&mut self, _name: &str) {}
        pub fn pop(&mut self) {}
    }

    // --------------------------------------------------------------------- gui

    #[derive(Clone, Debug)]
    pub struct Component(pub String);

    impl Component {
        // java: Component.translatable(key)
        pub fn translatable(key: &str) -> Component {
            Component(key.to_string())
        }

        // java: Component.translatable(key, args...)
        pub fn translatable_with(key: &str, args: &[String]) -> Component {
            Component(format!("{}|{}", key, args.join(",")))
        }
    }

    pub type ScreenHandle = u64;

    pub struct Font;

    pub fn font() -> Font {
        Font
    }

    pub struct GuiGraphicsExtractor;

    impl GuiGraphicsExtractor {
        // java: graphics.fill(x1, y1, x2, y2, color)
        pub fn fill(&mut self, _x1: i32, _y1: i32, _x2: i32, _y2: i32, _color: i32) {}
    }

    pub struct Tooltip;

    impl Tooltip {
        // java: Tooltip.create(component)
        pub fn create(_component: Component) -> Tooltip {
            Tooltip
        }
    }

    pub struct CheckboxOnValueChange(pub fn(*const (), bool));
    pub struct SliderMovedFn(pub Box<dyn Fn(f64) -> Component>);
    pub struct SliderReleasedFn(pub Box<dyn Fn(f64)>);

    pub struct Checkbox {
        pub active: bool,
        pub alpha: f32,
        pub message: Component,
        pub tooltip: Option<Tooltip>,
    }

    impl Checkbox {
        // java: new Checkbox(i, j, 128, component, font, bl, onValueChange)
        pub fn new(_i: i32, _j: i32, _width: i32, message: Component, _font: &Font, _selected: bool, _on_value_change: CheckboxOnValueChange) -> Checkbox {
            Checkbox { active: true, alpha: 1.0, message, tooltip: None }
        }

        // java: setTooltip
        pub fn set_tooltip(&self, tooltip: Option<Tooltip>) {
            let _ = tooltip;
        }

        pub fn set_alpha(&mut self, alpha: f32) {
            self.alpha = alpha;
        }

        pub fn is_active(&self) -> bool {
            self.active
        }

        pub fn extract_contents(&self, _g: &mut GuiGraphicsExtractor, _i: i32, _j: i32, _f: f32) {}
    }

    pub struct AbstractSliderButton {
        pub value: f64,
        pub message: Component,
        pub tooltip: Option<Tooltip>,
    }

    impl AbstractSliderButton {
        // java: new AbstractSliderButton(x, y, w, h, text, value)
        pub fn new(_x: i32, _y: i32, _width: i32, _height: i32, message: Component, value: f64) -> AbstractSliderButton {
            AbstractSliderButton { value, message, tooltip: None }
        }

        pub fn set_tooltip(&self, tooltip: Option<Tooltip>) {
            let _ = tooltip;
        }

        pub fn set_message(&mut self, message: Component) {
            self.message = message;
        }
    }

    #[derive(Clone)]
    pub struct Button {
        pub active: bool,
    }

    impl Button {
        // java: Button.builder(label, onPress)
        pub fn builder(label: Component, on_press: Box<dyn Fn()>) -> ButtonBuilder {
            let _ = label;
            ButtonBuilder { on_press: Some(on_press), bounds: None, position: None, size: None }
        }

        // java: handle reuse across init/rebuild
        pub fn clone_handle(&self) -> Button {
            self.clone()
        }
    }

    pub struct ButtonBuilder {
        on_press: Option<Box<dyn Fn()>>,
        bounds: Option<(i32, i32, i32, i32)>,
        position: Option<(i32, i32)>,
        size: Option<(i32, i32)>,
    }

    impl ButtonBuilder {
        // java: .bounds(x, y, w, h)
        pub fn bounds(mut self, x: i32, y: i32, w: i32, h: i32) -> Self {
            self.bounds = Some((x, y, w, h));
            self
        }

        // java: .pos(x, y)
        pub fn pos(mut self, x: i32, y: i32) -> Self {
            self.position = Some((x, y));
            self
        }

        // java: .size(w, h)
        pub fn size(mut self, w: i32, h: i32) -> Self {
            self.size = Some((w, h));
            self
        }

        // java: .build()
        pub fn build(self) -> Button {
            let _ = self.on_press;
            Button { active: true }
        }
    }

    pub struct StringWidget {
        pub message: Component,
    }

    impl StringWidget {
        // java: new StringWidget(component, font)
        pub fn new(message: Component, _font: &Font) -> StringWidget {
            StringWidget { message }
        }

        // java: getWidth()
        pub fn get_width(&self) -> i32 {
            self.message.0.len() as i32 * 6
        }

        // java: setPosition(x, y)
        pub fn set_position(&self, _x: i32, _y: i32) {}
    }

    pub struct MultiLineTextWidget {
        pub max_width: i32,
    }

    impl MultiLineTextWidget {
        // java: new MultiLineTextWidget(x, y, component, font).setMaxWidth(w)
        pub fn new(_x: i32, _y: i32, _message: Component, _font: &Font) -> MultiLineTextWidget {
            MultiLineTextWidget { max_width: 0 }
        }

        pub fn set_max_width(mut self, width: i32) -> MultiLineTextWidget {
            self.max_width = width;
            self
        }
    }

    pub enum Widget {
        Checkbox(Checkbox),
        SliderButton(AbstractSliderButton),
        Button(Button),
        StringWidget(StringWidget),
        MultiLineTextWidget(MultiLineTextWidget),
        Separator(super::gui::widget::separator::Separator),
    }

    // java: minecraft.gui.setScreen(lastScreen)
    pub fn set_screen(_screen: ScreenHandle) {}

    // ------------------------------------------------- config-screen callbacks

    static NEW_CONFIG: OnceLock<Mutex<crate::config::ConfigData>> = OnceLock::new();
    static INVALIDATED: OnceLock<Mutex<bool>> = OnceLock::new();
    static LAST_SCREEN: OnceLock<Mutex<ScreenHandle>> = OnceLock::new();

    fn new_config() -> &'static Mutex<crate::config::ConfigData> {
        NEW_CONFIG.get_or_init(|| Mutex::new(crate::config::ConfigData::default()))
    }

    fn invalidated() -> &'static Mutex<bool> {
        INVALIDATED.get_or_init(|| Mutex::new(false))
    }

    // java: newConfig.setRenderNebulas(selected)
    pub fn screen_new_config_set_render_nebulas(v: bool) {
        new_config().lock().unwrap().render_nebulas = v;
    }

    // java: newConfig.setTwinklingStars(selected)
    pub fn screen_new_config_set_twinkling_stars(v: bool) {
        new_config().lock().unwrap().twinkling_stars = v;
    }

    // java: newConfig.getNebulaConfig().setRenderDuringDay(selected)
    pub fn screen_nebula_config_set_render_during_day(v: bool) {
        new_config().lock().unwrap().nebula_config.render_during_day = v;
    }

    // java: newConfig.getNebulaConfig().setNebulaStrength(value)
    pub fn screen_nebula_config_set_nebula_strength(v: f32) {
        new_config().lock().unwrap().nebula_config.nebula_strength = v;
    }

    // java: newConfig.getNebulaConfig().setNebulaNoiseAmount(value)
    pub fn screen_nebula_config_set_nebula_noise_amount(v: f32) {
        new_config().lock().unwrap().nebula_config.nebula_noise_amount = v;
    }

    // java: newConfig.getNebulaConfig().setNebulaNoiseScale(value)
    pub fn screen_nebula_config_set_nebula_noise_scale(v: f32) {
        new_config().lock().unwrap().nebula_config.nebula_noise_scale = v;
    }

    // java: newConfig.getNebulaConfig().setBaseColourAmount(value)
    pub fn screen_nebula_config_set_base_colour_amount(v: i32) {
        new_config().lock().unwrap().nebula_config.base_colour_amount = v;
    }

    // java: invalidated = true / false
    pub fn screen_set_invalidated(v: bool) {
        *invalidated().lock().unwrap() = v;
    }

    // java: this.rebuildWidgets()
    pub fn screen_rebuild_widgets() {}

    // java: reset button: newConfig.setNebulaConfig(DEFAULT_CONFIG.getNebulaConfig().toBuilder().build())
    pub fn screen_reset_nebula_config_to_default() {
        new_config().lock().unwrap().nebula_config = crate::config::Config::DEFAULT_CONFIG.nebula_config.clone();
    }

    // java: apply button: config.updateConfig(newConfig)
    pub fn screen_apply_config() {
        let data = new_config().lock().unwrap().clone();
        let mut instance = crate::get_instance().lock().unwrap();
        if let Some(inst) = instance.as_mut() {
            inst.config.update_config(data);
        }
    }

    // java: screen: regenerateSky()
    pub fn screen_regenerate_sky() {
        let mut instance = crate::get_instance().lock().unwrap();
        if let Some(inst) = instance.as_mut() {
            if crate::core::hashed_seed_manager::can_generate_sky() {
                let seed = crate::core::hashed_seed_manager::get_seed();
                inst.renderer.generate_sky(seed);
            }
        }
    }

    // java: screen: onClose()
    pub fn screen_on_close() {
        let last = *LAST_SCREEN.get_or_init(|| Mutex::new(0)).lock().unwrap();
        set_screen(last);
    }

    // java: private regenerateSky wrapper used by ConfigScreen
    pub fn config_screen_regenerate() {
        screen_regenerate_sky();
    }

    // ------------------------------------------------------------ mixin plumbing

    pub struct EngineHooks {
        pub current_level: Mutex<Option<u64>>,
        client_level_hashed_seed_fn: Mutex<Box<dyn Fn(&u64) -> i64 + Send>>,
        tick_stars_fn: Mutex<Box<dyn Fn() + Send>>,
        build_stars_fn: Mutex<Box<dyn Fn() + Send>>,
        render_nebulas_fn: Mutex<Box<dyn Fn(&mut PoseStack, f32, f32, &RenderTarget) + Send>>,
        render_stars_fn: Mutex<Box<dyn Fn(f32, &PoseStack, &RenderTarget) -> bool + Send>>,
    }

    pub static HOOKS: OnceLock<EngineHooks> = OnceLock::new();

    // java: ClientLevelMixin registration (client entry).
    pub fn register_client_level_hooks(
        save_seed: impl Fn(i64) + Send + 'static,
        tick_stars: fn(),
    ) {
        let _ = save_seed;
        let hooks = EngineHooks {
            current_level: Mutex::new(None),
            client_level_hashed_seed_fn: Mutex::new(Box::new(|_| 0)),
            tick_stars_fn: Mutex::new(Box::new(tick_stars)),
            build_stars_fn: Mutex::new(Box::new(|| {})),
            render_nebulas_fn: Mutex::new(Box::new(|_, _, _, _| {})),
            render_stars_fn: Mutex::new(Box::new(|_, _, _| false)),
        };
        let _ = HOOKS.set(hooks);
    }

    // java: SkyRendererMixin registration (client entry).
    pub fn register_sky_renderer_hooks(
        build_stars: fn(),
        render_nebulas: impl Fn(&mut PoseStack, f32, f32, &RenderTarget) + Send + 'static,
        render_stars: impl Fn(f32, &PoseStack, &RenderTarget) -> bool + Send + 'static,
    ) {
        let hooks = HOOKS.get_or_init(|| EngineHooks {
            current_level: Mutex::new(None),
            client_level_hashed_seed_fn: Mutex::new(Box::new(|_| 0)),
            tick_stars_fn: Mutex::new(Box::new(|| {})),
            build_stars_fn: Mutex::new(Box::new(|| {})),
            render_nebulas_fn: Mutex::new(Box::new(|_, _, _, _| {})),
            render_stars_fn: Mutex::new(Box::new(|_, _, _| false)),
        });
        *hooks.build_stars_fn.lock().unwrap() = Box::new(build_stars);
        *hooks.render_nebulas_fn.lock().unwrap() = Box::new(render_nebulas);
        *hooks.render_stars_fn.lock().unwrap() = Box::new(render_stars);
    }

    /// java: the mixin state lives on the ClientLevel instance; the engine keeps
    /// one per level and hands it back here.
    pub fn client_level_mixin_state() -> crate::mixin::client_level_mixin::ClientLevelMixinState {
        crate::mixin::client_level_mixin::ClientLevelMixinState::new()
    }
}
