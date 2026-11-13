// Dark Loading Screen (Horizon Rust port) - source bundle.
// Exact 1:1 Rust translation of A5b84/dark-loading-screen v1.6.20 (LGPL-3.0-only).
// Module tree inlined from ports/dark-loading-screen/src; every module is nested 1:1.

//! Exact 1:1 Rust translation of Dark Loading Screen by A5b84
//! (github.com/A5b84/dark-loading-screen, v1.6.20, LGPL-3.0-only).
//! Upstream Java package io.github.a5b84.darkloadingscreen maps to the
//! modules below; every Minecraft client type sits behind the host seams
//! in `seams`. Log lines, field names, branch order and quirks are kept
//! as-is.
//!
//! - `dark_loading_screen` <- DarkLoadingScreen.java (entry + statics)
//! - `util`                <- Util.java
//! - `blit_lambda`         <- BlitLambda.java
//! - `config`              <- config/Config.java
//! - `config_serialization` <- config/ConfigSerialization.java
//! - `mixin`               <- mixin/LoadingOverlayMixin.java
//! - `preview`             <- config/FakeReloadInstance.java +
//!                            config/PreviewSplashOverlay.java
//! - `gui`                 <- config/ModMenuApiImpl.java +
//!                            config/ConfigScreenFactoryImpl.java (+ inner
//!                            ConfigEntries) + config/ButtonEntry.java

pub use dark_loading_screen::MOD_ID;
// ---- inlined: src/dark_loading_screen.rs ----
pub mod dark_loading_screen {
//! 1:1 of DarkLoadingScreen.java.

use std::sync::Mutex;
use std::sync::OnceLock;

use crate::config::Config;
use crate::config_serialization;
use crate::seams;

// java: public static final String MOD_ID = "dark-loading-screen";
pub const MOD_ID: &str = "dark-loading-screen";

// java: public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);
// the seams log helpers carry the logger name implicitly.

// java: public static final RenderPipeline MOJANG_LOGO_SHADOWS =
//   RenderPipelines.register(RenderPipeline.builder(RenderPipelines.GUI_TEXTURED_SNIPPET)
//     .withLocation(Identifier.fromNamespaceAndPath(MOD_ID, "pipeline/mojang_logo_shadows"))
//     .withColorTargetState(new ColorTargetState(new BlendFunction(
//         BlendFactor.SRC_ALPHA, BlendFactor.ONE, BlendOp.REVERSE_SUBTRACT)))
//     .build());
// Registered once at first touch, exactly like the java static initializer.
static MOJANG_LOGO_SHADOWS: OnceLock<seams::RenderPipelineRef> = OnceLock::new();

pub fn mojang_logo_shadows() -> seams::RenderPipelineRef {
    *MOJANG_LOGO_SHADOWS.get_or_init(|| {
        seams::register_render_pipeline(seams::PipelineSpec {
            base_snippet: seams::GuiTexturedSnippet,
            location: format!("{MOD_ID}:pipeline/mojang_logo_shadows"),
            blend: seams::BlendFunction {
                src_factor: seams::BlendFactor::SrcAlpha,
                dst_factor: seams::BlendFactor::One,
                op: seams::BlendOp::ReverseSubtract,
            },
        })
    })
}

// java: public static Config config = ConfigSerialization.read();
static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

pub fn config() -> &'static Mutex<Config> {
    CONFIG.get_or_init(|| Mutex::new(config_serialization::read()))
}

// java: DarkLoadingScreen.config = <value> assignments (preview + save).
pub fn set_config(value: Config) {
    *config().lock().unwrap() = value;
}

}
// ---- inlined: src/util.rs ----
pub mod util {
//! 1:1 of Util.java.

pub fn pad_start(s: &str, min_length: usize, padding: char) -> String {
    let len = s.chars().count();
    if len < min_length {
        // java: StringBuilder(minLength) padded with `padding` until the
        // padding length is reached, then `s` appended.
        let mut result = String::new();
        let padding_length = min_length - len;
        while result.chars().count() < padding_length {
            result.push(padding);
        }
        result.push_str(s);
        result
    } else {
        s.to_string()
    }
}

}
// ---- inlined: src/blit_lambda.rs ----
pub mod blit_lambda {
//! 1:1 of BlitLambda.java (@FunctionalInterface). The pipeline, x/y/z args
//! are the r/g/b channel deltas the callers pass; named as upstream.

pub trait BlitLambda {
    fn call(&mut self, pipeline: crate::seams::RenderPipelineRef, r: f32, g: f32, b: f32);
}

}
// ---- inlined: src/config.rs ----
pub mod config {
//! 1:1 of config/Config.java.

use crate::seams;

// java: Factor to convert time from the config (in seconds) to the actual
// number of milliseconds to use. (Dividing by 2 because the game waits
// twice as long)
// private static final float FADE_DURATION_FACTOR = LoadingOverlay.FADE_OUT_TIME / 2f;
fn fade_duration_factor() -> f32 {
    seams::loading_overlay_fade_out_time() / 2.0
}

// java: static final float MAX_FADE_DURATION = 5;
pub const MAX_FADE_DURATION: f32 = 5.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    // Colors
    pub background_color: i32,
    pub bar_color: i32,
    pub bar_background_color: i32,
    pub bar_border_color: i32,
    pub logo_color: i32,

    // RGB channels of some colors
    pub background_red: f32,
    pub background_green: f32,
    pub background_blue: f32,
    pub logo_red: f32,
    pub logo_green: f32,
    pub logo_blue: f32,

    // Fade durations
    pub fade_in_duration: f32,
    pub fade_out_duration: f32,
    pub fade_in_millis: f32,
    pub fade_out_millis: f32,
}

impl Config {
    pub fn default() -> Config {
        // java: new Config(0x14181c, 0xe22837, 0x14181c, 0x303336, 0xffffff,
        //   LoadingOverlay.FADE_IN_TIME / FADE_DURATION_FACTOR,
        //   LoadingOverlay.FADE_OUT_TIME / FADE_DURATION_FACTOR);
        Config::new(
            0x14181c,
            0xe22837,
            0x14181c,
            0x303336,
            0xffffff,
            seams::loading_overlay_fade_in_time() / fade_duration_factor(),
            seams::loading_overlay_fade_out_time() / fade_duration_factor(),
        )
    }

    // java: @param fadeInDuration Fade in time in seconds
    // java: @param fadeOutDuration Fade out time in seconds
    pub fn new(
        background_color: i32,
        bar_color: i32,
        bar_background_color: i32,
        bar_border_color: i32,
        logo_color: i32,
        fade_in_duration: f32,
        fade_out_duration: f32,
    ) -> Config {
        Config {
            background_color,
            bar_color,
            bar_background_color,
            bar_border_color,
            logo_color,
            // java: this.fadeInDuration = Math.min(fadeInDuration, MAX_FADE_DURATION);
            fade_in_duration: fade_in_duration.min(MAX_FADE_DURATION),
            fade_out_duration: fade_out_duration.min(MAX_FADE_DURATION),
            background_red: seams::argb_red_float(background_color),
            background_green: seams::argb_green_float(background_color),
            background_blue: seams::argb_blue_float(background_color),
            logo_red: seams::argb_red_float(logo_color),
            logo_green: seams::argb_green_float(logo_color),
            logo_blue: seams::argb_blue_float(logo_color),
            // java: fadeInMillis = fadeInDuration * FADE_DURATION_FACTOR;
            fade_in_millis: fade_in_duration.min(MAX_FADE_DURATION) * fade_duration_factor(),
            fade_out_millis: fade_out_duration.min(MAX_FADE_DURATION) * fade_duration_factor(),
        }
    }
}

}
// ---- inlined: src/config_serialization.rs ----
pub mod config_serialization {
//! 1:1 of config/ConfigSerialization.java. The Gson DTO (record ConfigDto)
//! is kept as-is: nullable color strings, plain floats, the same JSON field
//! names, Gson's pretty printing and its null-omitting writer behavior.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dark_loading_screen;
use crate::seams;
use crate::util;

// java: private static final Path CONFIG_PATH =
//   FabricLoader.getInstance().getConfigDir().resolve(DarkLoadingScreen.MOD_ID + ".json");
fn config_path() -> PathBuf {
    let mut path = seams::fabric_config_dir();
    path.push(format!("{}.json", dark_loading_screen::MOD_ID));
    path
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "barBackground")]
    bar_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<String>,
    #[serde(default)]
    fade_in: f32,
    #[serde(default)]
    fade_out: f32,
}

impl ConfigDto {
    // java: public ConfigDto(Config config) { this(stringifyColor(...), ..., config.fadeInDuration, config.fadeOutDuration); }
    fn from_config(config: &Config) -> ConfigDto {
        ConfigDto {
            background: Some(Self::stringify_color(config.background_color)),
            bar: Some(Self::stringify_color(config.bar_color)),
            bar_background: Some(Self::stringify_color(config.bar_background_color)),
            border: Some(Self::stringify_color(config.bar_border_color)),
            logo: Some(Self::stringify_color(config.logo_color)),
            fade_in: config.fade_in_duration,
            fade_out: config.fade_out_duration,
        }
    }

    // java: public Config toConfig() { return new Config(parseColor(...), ..., fadeIn, fadeOut); }
    fn to_config(&self) -> Config {
        Config::new(
            Self::parse_color(self.background.as_deref(), Config::default().background_color),
            Self::parse_color(self.bar.as_deref(), Config::default().bar_color),
            Self::parse_color(
                self.bar_background.as_deref(),
                Config::default().bar_background_color,
            ),
            Self::parse_color(self.border.as_deref(), Config::default().bar_border_color),
            Self::parse_color(self.logo.as_deref(), Config::default().logo_color),
            self.fade_in,
            self.fade_out,
        )
    }

    // java: private static String stringifyColor(int color) { return Util.padStart(Integer.toString(color, 16), 6, '0'); }
    fn stringify_color(color: i32) -> String {
        util::pad_start(&format!("{:x}", color), 6, '0')
    }

    // java: private static int parseColor(@Nullable String value, int fallback) {
    //   if (value == null) return fallback;
    //   try { return Integer.parseInt(value, 16); }
    //   catch (NumberFormatException ignored) { LOGGER.warn("[Dark Loading Screen] Failed to parse config color '{}'", value); return fallback; } }
    fn parse_color(value: Option<&str>, fallback: i32) -> i32 {
        match value {
            None => fallback,
            Some(value) => match i32::from_str_radix(value, 16) {
                Ok(parsed) => parsed,
                Err(_) => {
                    seams::log_warn(&format!(
                        "[Dark Loading Screen] Failed to parse config color '{value}'"
                    ));
                    fallback
                }
            },
        }
    }
}

// java: public static Config read() { ... FileNotFoundException -> DEFAULT;
//   IOException | JsonParseException -> error log + DEFAULT; }
pub fn read() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<ConfigDto>(&text) {
            Ok(dto) => dto.to_config(),
            Err(_) => {
                // Gson throws JsonParseException for malformed JSON.
                seams::log_error(&format!(
                    "[Dark Loading Screen] Couldn't read config at {}, using default config instead (bad JSON)",
                    path.display()
                ));
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

// java: public static void write(Config config) { ... IOException | JsonIOException -> error log; }
pub fn write(config: &Config) {
    let path = config_path();
    let dto = ConfigDto::from_config(config);
    match serde_json::to_string_pretty(&dto) {
        Ok(text) => {
            if let Err(_) = std::fs::write(&path, text) {
                seams::log_error(&format!(
                    "[Dark Loading Screen] Couldn't write config to {}",
                    path.display()
                ));
            }
        }
        Err(_) => {
            seams::log_error(&format!(
                "[Dark Loading Screen] Couldn't write config to {}",
                path.display()
            ));
        }
    }
}

}
// ---- inlined: src/mixin.rs ----
pub mod mixin {
//! 1:1 of mixin/LoadingOverlayMixin.java. Every injection point is exposed
//! as a callback the host mixin wires onto net.minecraft.client.gui.screens.
//! LoadingOverlay; the bodies keep the upstream branch order and the
//! java-level quirks (alpha masks, the `> 0` guard, `(Object) this`
//! instanceof, negative fade passthrough).

use crate::blit_lambda::BlitLambda;
use crate::dark_loading_screen;
use crate::preview::PreviewSplashOverlay;
use crate::seams;

/// The BlitLambda closure of `onBlitLogo`, as a named struct: it captures
/// the wrapped operation's fixed blit arguments and the logo alpha, then
/// renders on `call(pipeline, r, g, b)` exactly like the java lambda.
struct BlitImpl<'a> {
    original: &'a mut seams::BlitOperation,
    graphics: &'a seams::GraphicsRef,
    texture: &'a seams::TextureRef,
    x: i32,
    y: i32,
    u: f32,
    v: f32,
    width: i32,
    height: i32,
    src_width: i32,
    src_height: i32,
    texture_width: i32,
    texture_height: i32,
    alpha: i32,
}

impl BlitLambda for BlitImpl<'_> {
    fn call(&mut self, pipeline: seams::RenderPipelineRef, r: f32, g: f32, b: f32) {
        if r > 0.0 || g > 0.0 || b > 0.0 {
            self.original.call(
                self.graphics,
                pipeline,
                self.texture,
                self.x,
                self.y,
                self.u,
                self.v,
                self.width,
                self.height,
                self.src_width,
                self.src_height,
                self.texture_width,
                self.texture_height,
                seams::argb_color(
                    self.alpha,
                    seams::argb_as8_bit_channel(r.max(0.0)),
                    seams::argb_as8_bit_channel(g.max(0.0)),
                    seams::argb_as8_bit_channel(b.max(0.0)),
                ),
            );
        }
    }
}

// java: @Mutable @Shadow private static @Final IntSupplier BRAND_BACKGROUND;
// @Inject(method = "<clinit>", at = @At("RETURN")) -> the host's BRAND_BACKGROUND
// supplier becomes the value this returns.
pub fn modify_background_color() -> i32 {
    dark_loading_screen::config().lock().unwrap().background_color
}

// java: private int modifyBarColorAndFillBackground(int white, GuiGraphicsExtractor
// graphics, int x0, int y0, int x1, int y1, float fade):
//   int alpha = white & (0xff000000u32 as i32);
//   graphics.fill(x0 + 1, y0 + 1, x1 - 1, y1 - 1, config.barBackgroundColor | alpha);
//   return config.barColor | alpha;
pub fn modify_bar_color_and_fill_background(
    white: i32,
    graphics: &seams::GraphicsRef,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fade: f32,
) -> i32 {
    let _ = fade;
    let alpha = white & (0xff000000u32 as i32);
    let cfg = dark_loading_screen::config().lock().unwrap();
    graphics.fill(x0 + 1, y0 + 1, x1 - 1, y1 - 1, cfg.bar_background_color | alpha);
    cfg.bar_color | alpha
}

// java: private int modifyBarBorderColor(int white):
//   return DarkLoadingScreen.config.barBorderColor | white & (0xff000000u32 as i32);
// (java precedence: | binds looser than &, so this is barBorderColor | (white & (0xff000000u32 as i32)))
pub fn modify_bar_border_color(white: i32) -> i32 {
    dark_loading_screen::config().lock().unwrap().bar_border_color | (white & (0xff000000u32 as i32))
}

/// java: Changes the logo color. @WrapOperation on the GuiGraphicsExtractor.blit
/// call inside extractRenderState. The `original` operation is the upstream
/// blit; we call it twice with per-channel color corrections because
/// `RenderSystem.blendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SOURCE_ALPHA)` causes an
/// ugly outline: once for channels brighter than the background, once for
/// those darker.
pub fn on_blit_logo(
    graphics: &seams::GraphicsRef,
    render_pipeline: seams::RenderPipelineRef,
    texture: &seams::TextureRef,
    x: i32,
    y: i32,
    u: f32,
    v: f32,
    width: i32,
    height: i32,
    src_width: i32,
    src_height: i32,
    texture_width: i32,
    texture_height: i32,
    color: i32,
    original: &mut seams::BlitOperation,
) {
    let alpha = seams::argb_alpha(color);

    // java: BlitLambda blit = (pipeline, r, g, b) -> {
    //   if (r > 0 || g > 0 || b > 0) {
    //     original.call(graphics, pipeline, texture, x, y, u, v, width, height,
    //         srcWidth, srcHeight, textureWidth, textureHeight,
    //         ARGB.color(alpha, ARGB.as8BitChannel(Math.max(r, 0)),
    //             ARGB.as8BitChannel(Math.max(g, 0)), ARGB.as8BitChannel(Math.max(b, 0))));
    //   } };
    let mut blit = BlitImpl {
        original,
        graphics,
        texture,
        x,
        y,
        u,
        v,
        width,
        height,
        src_width,
        src_height,
        texture_width,
        texture_height,
        alpha,
    };

    let cfg = dark_loading_screen::config().lock().unwrap();
    let background_red = cfg.background_red;
    let background_green = cfg.background_green;
    let background_blue = cfg.background_blue;
    let logo_red = cfg.logo_red;
    let logo_green = cfg.logo_green;
    let logo_blue = cfg.logo_blue;
    drop(cfg);

    blit.call(
        dark_loading_screen::mojang_logo_shadows(),
        background_red - logo_red,
        background_green - logo_green,
        background_blue - logo_blue,
    );

    blit.call(
        render_pipeline,
        logo_red - background_red,
        logo_green - background_green,
        logo_blue - background_blue,
    );
}

// java: @Inject(method = "extractRenderState", at = @At(value = "INVOKE",
// target = "...Gui;setOverlay(...)V")) -> fires when the overlay is removed.
// if ((Object) this instanceof PreviewSplashOverlay previewScreen) previewScreen.onRemoved();
pub fn on_overlay_removed(this: Option<&PreviewSplashOverlay>) {
    if let Some(preview_screen) = this {
        preview_screen.on_removed();
    }
}

// java: @ModifyVariable(method = "extractRenderState", at = @At(value = "STORE"), name = "fadeInAnim")
pub fn modify_fade_in_anim(fade_in_anim: f32) -> f32 {
    modify_fade_anim(
        fade_in_anim,
        seams::loading_overlay_fade_in_time(),
        dark_loading_screen::config().lock().unwrap().fade_in_millis,
    )
}

// java: @ModifyVariable(method = "extractRenderState", at = @At(value = "STORE"), name = "fadeOutAnim")
pub fn modify_fade_out_anim(fade_out_anim: f32) -> f32 {
    modify_fade_anim(
        fade_out_anim,
        seams::loading_overlay_fade_out_time(),
        dark_loading_screen::config().lock().unwrap().fade_out_millis,
    )
}

// java: @Unique private float modifyFadeAnim(float value, float vanillaValue, float newValue) {
//   return value >= 0 ? value * vanillaValue / newValue : value; }
pub fn modify_fade_anim(value: f32, vanilla_value: f32, new_value: f32) -> f32 {
    if value >= 0.0 {
        value * vanilla_value / new_value
    } else {
        value
    }
}

}
// ---- inlined: src/preview.rs ----
pub mod preview {
//! 1:1 of config/FakeReloadInstance.java and config/PreviewSplashOverlay.java.

use crate::seams;

/// java: {@link ReloadInstance} that automatically completes after some time.
pub struct FakeReloadInstance {
    start: u64,
    duration: u64,
    // java: private final CompletableFuture<Void> future;
    future: seams::FutureHandle,
}

impl FakeReloadInstance {
    pub fn new(duration_ms: u64) -> FakeReloadInstance {
        // java: start = Util.getMillis(); duration = durationMs;
        // future = CompletableFuture.runAsync(() -> {},
        //     CompletableFuture.delayedExecutor(durationMs, TimeUnit.MILLISECONDS));
        FakeReloadInstance {
            start: seams::millis(),
            duration: duration_ms,
            future: seams::delayed_complete_future(duration_ms),
        }
    }

    // java: public CompletableFuture<Void> done() { return future; }
    pub fn done(&self) -> seams::FutureHandle {
        self.future.clone()
    }

    // java: public float getActualProgress() {
    //   float progress = (float) (Util.getMillis() - start) / duration;
    //   return Mth.clamp(progress, 0, 1); }
    pub fn get_actual_progress(&self) -> f32 {
        let progress = (seams::millis() as f64 - self.start as f64) as f32 / self.duration as f32;
        seams::clamp(progress, 0.0, 1.0)
    }
}

/// java: public class PreviewSplashOverlay extends LoadingOverlay.
pub struct PreviewSplashOverlay {
    // java: super(Minecraft.getInstance(), new FakeReloadInstance(durationMs), _ -> {}, true);
    pub base: seams::LoadingOverlayHandle,
    on_removed: Box<dyn Fn()>,
}

impl PreviewSplashOverlay {
    pub fn new(duration_ms: u64, on_removed: Box<dyn Fn()>) -> PreviewSplashOverlay {
        PreviewSplashOverlay {
            base: seams::loading_overlay_new(
                Box::new(FakeReloadInstance::new(duration_ms)) as Box<dyn seams::ReloadInstance>,
                Box::new(|| {}),
                true,
            ),
            on_removed,
        }
    }

    // java: public void onRemoved() { onRemoved.run(); }
    pub fn on_removed(&self) {
        (self.on_removed)()
    }
}

impl seams::ReloadInstance for FakeReloadInstance {
    fn done(&self) -> seams::FutureHandle {
        self.done()
    }

    fn get_actual_progress(&self) -> f32 {
        self.get_actual_progress()
    }
}

}
// ---- inlined: src/gui.rs ----
pub mod gui {
//! 1:1 of config/ModMenuApiImpl.java, config/ConfigScreenFactoryImpl.java
//! (with its inner ConfigEntries) and config/ButtonEntry.java.

use crate::config;
use crate::config_serialization;
use crate::dark_loading_screen;
use crate::seams;

/// java: public class ModMenuApiImpl implements ModMenuApi.
/// The static FACTORY picks the cloth-config screen factory only when
/// cloth-config2 is loaded; otherwise it is the `-> null` factory.
pub struct ModMenuApiImpl;

impl ModMenuApiImpl {
    // java: private static final ConfigScreenFactory<?> FACTORY =
    //   FabricLoader.getInstance().isModLoaded("cloth-config2")
    //       ? new ConfigScreenFactoryImpl() : _ -> null;
    // java: public ConfigScreenFactory<?> getModConfigScreenFactory() { return FACTORY; }
    pub fn get_mod_config_screen_factory(parent: &seams::ScreenHandle) -> Option<seams::ScreenHandle> {
        if seams::is_mod_loaded("cloth-config2") {
            config_screen_factory_create(parent)
        } else {
            None
        }
    }
}

/// java: public class ConfigScreenFactoryImpl implements ConfigScreenFactory<Screen> {
///   public Screen create(Screen parent) { ... } }
pub fn config_screen_factory_create(parent: &seams::ScreenHandle) -> Option<seams::ScreenHandle> {
    // java: ConfigBuilder builder = ConfigBuilder.create()
    //     .setParentScreen(parent)
    //     .setTitle(Component.translatable("darkLoadingScreen.config.title"));
    let builder = seams::ConfigBuilder::create()
        .set_parent_screen(parent)
        .set_title(seams::Component::translatable(
            "darkLoadingScreen.config.title",
        ));

    // java: Config oldConfig = DarkLoadingScreen.config;
    let old_config = dark_loading_screen::config().lock().unwrap().clone();

    // java: ConfigCategory category = builder.getOrCreateCategory(Component.empty());
    let category = builder.get_or_create_category(seams::Component::empty());

    // java: ConfigEntries entries = new ConfigEntries(builder.entryBuilder(), category);
    let entries = ConfigEntries::new(&builder.entry_builder(), &category);

    // java: category.addEntry(new ButtonEntry(
    //     Component.translatable("darkLoadingScreen.config.entry.preview"),
    //     _ -> {
    //       DarkLoadingScreen.config = entries.createConfig();
    //       Minecraft.getInstance().gui.setOverlay(
    //           new PreviewSplashOverlay(500, () -> DarkLoadingScreen.config = oldConfig));
    //     }));
    let preview_on_press: Box<dyn Fn()> = Box::new(move || {
        let config_now = entries.create_config();
        dark_loading_screen::set_config(config_now);
        // java lambda captures oldConfig by reference; the Rust callback owns
        // a clone for the restore run.
        let old_for_restore = old_config.clone();
        let restore: Box<dyn Fn()> = Box::new(move || {
            // java: () -> DarkLoadingScreen.config = oldConfig (reference capture;
            // the Rust closure owns a clone it restores once).
            dark_loading_screen::set_config(old_for_restore.clone())
        });
        let overlay = crate::preview::PreviewSplashOverlay::new(500, restore);
        seams::gui_set_overlay(&overlay.base);
    });
    let button_entry = ButtonEntry::new(
        seams::Component::translatable("darkLoadingScreen.config.entry.preview"),
        preview_on_press,
    );
    category.add_entry(Box::new(button_entry) as Box<dyn seams::AnyConfigEntry>);

    // java: builder.setSavingRunnable(() -> {
    //   Config config = DarkLoadingScreen.config = entries.createConfig();
    //   ConfigSerialization.write(config); });
    let saving: Box<dyn Fn()> = Box::new(move || {
        let config_now = dark_loading_screen::config().lock().unwrap().clone();
        config_serialization::write(&config_now);
    });
    builder.set_saving_runnable(saving);

    // java: return builder.build();
    Some(builder.build())
}

/// java: private static class ConfigEntries { ... } (fields + createConfig +
/// the two create*Field helpers, in upstream order).
struct ConfigEntries {
    background_color_field: seams::ColorEntry,
    bar_color_field: seams::ColorEntry,
    bar_background_color_field: seams::ColorEntry,
    bar_border_color_field: seams::ColorEntry,
    logo_color_field: seams::ColorEntry,
    fade_in_duration_field: seams::FloatListEntry,
    fade_out_duration_field: seams::FloatListEntry,
}

impl ConfigEntries {
    // java: /** Creates all the fields and adds them to {@code category}. */
    // public ConfigEntries(ConfigEntryBuilder builder, ConfigCategory category) { ... }
    fn new(builder: &seams::ConfigEntryBuilder, category: &seams::ConfigCategory) -> ConfigEntries {
        let cfg = dark_loading_screen::config().lock().unwrap();
        ConfigEntries {
            background_color_field: Self::create_color_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.background"),
                cfg.background_color,
                config::Config::default().background_color,
            ),
            bar_color_field: Self::create_color_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.bar"),
                cfg.bar_color,
                config::Config::default().bar_color,
            ),
            bar_background_color_field: Self::create_color_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.barBackground"),
                cfg.bar_background_color,
                config::Config::default().bar_background_color,
            ),
            bar_border_color_field: Self::create_color_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.border"),
                cfg.bar_border_color,
                config::Config::default().bar_border_color,
            ),
            logo_color_field: Self::create_color_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.logo"),
                cfg.logo_color,
                config::Config::default().logo_color,
            ),
            fade_in_duration_field: Self::create_fade_time_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.fadeIn"),
                cfg.fade_in_duration,
                config::Config::default().fade_in_duration,
            ),
            fade_out_duration_field: Self::create_fade_time_field(
                builder,
                category,
                seams::Component::translatable("darkLoadingScreen.config.entry.fadeOut"),
                cfg.fade_out_duration,
                config::Config::default().fade_out_duration,
            ),
        }
    }

    // java: public Config createConfig() { return new Config(...); }
    fn create_config(&self) -> config::Config {
        config::Config::new(
            self.background_color_field.get_value(),
            self.bar_color_field.get_value(),
            self.bar_background_color_field.get_value(),
            self.bar_border_color_field.get_value(),
            self.logo_color_field.get_value(),
            self.fade_in_duration_field.get_value(),
            self.fade_out_duration_field.get_value(),
        )
    }

    // java: private ColorEntry createColorField(Component name, int value, int defaultValue) {
    //   ColorEntry entry = builder.startColorField(name, value).setDefaultValue(defaultValue).build();
    //   category.addEntry(entry); return entry; }
    fn create_color_field(
        builder: &seams::ConfigEntryBuilder,
        category: &seams::ConfigCategory,
        name: seams::Component,
        value: i32,
        default_value: i32,
    ) -> seams::ColorEntry {
        let entry = builder
            .start_color_field(name, value)
            .set_default_value(default_value)
            .build();
        category.add_entry(Box::new(seams::FieldWidget(entry.handle())) as Box<dyn seams::AnyConfigEntry>);
        entry
    }

    // java: private FloatListEntry createFadeTimeField(Component name, float value, float defaultValue) {
    //   FloatListEntry entry = builder.startFloatField(name, value).setDefaultValue(defaultValue)
    //       .setMin(0).setMax(Config.MAX_FADE_DURATION).build();
    //   category.addEntry(entry); return entry; }
    fn create_fade_time_field(
        builder: &seams::ConfigEntryBuilder,
        category: &seams::ConfigCategory,
        name: seams::Component,
        value: f32,
        default_value: f32,
    ) -> seams::FloatListEntry {
        let entry = builder
            .start_float_field(name, value)
            .set_default_value(default_value)
            .set_min(0.0)
            .set_max(config::MAX_FADE_DURATION)
            .build();
        category.add_entry(Box::new(seams::FieldWidget(entry.handle())) as Box<dyn seams::AnyConfigEntry>);
        entry
    }
}

/// java: /** Clickable Cloth Config button entry that doesn't hold any data. */
/// public class ButtonEntry extends AbstractConfigListEntry<Void>.
pub struct ButtonEntry {
    base: seams::AbstractConfigListEntry,
    button: seams::Button,
}

impl ButtonEntry {
    // java: private static final int HORIZONTAL_PADDING = 24;
    const HORIZONTAL_PADDING: i32 = 24;

    // java: public ButtonEntry(Component fieldName, Button.OnPress onPress) {
    //   super(fieldName, false);
    //   int width = Minecraft.getInstance().font.width(fieldName) + HORIZONTAL_PADDING;
    //   button = Button.builder(fieldName, onPress).width(width).build(); }
    pub fn new(field_name: seams::Component, on_press: Box<dyn Fn()>) -> ButtonEntry {
        let width = seams::font_width(&field_name) + Self::HORIZONTAL_PADDING;
        ButtonEntry {
            base: seams::AbstractConfigListEntry::new(field_name.clone(), false),
            button: seams::Button::builder(field_name, on_press).width(width).build(),
        }
    }

    // java: public void extractRenderState(...) { super.extractRenderState(...);
    //   button.setX(x + (entryWidth - button.getWidth()) / 2);
    //   button.setY(y + (entryHeight - button.getHeight()) / 2);
    //   button.extractRenderState(graphics, mouseX, mouseY, delta); }
    pub fn extract_render_state(
        &mut self,
        graphics: &seams::GraphicsRef,
        index: i32,
        y: i32,
        x: i32,
        entry_width: i32,
        entry_height: i32,
        mouse_x: i32,
        mouse_y: i32,
        is_hovered: bool,
        delta: f32,
    ) {
        self.base
            .extract_render_state(graphics, index, y, x, entry_width, entry_height, mouse_x, mouse_y, is_hovered, delta);
        let button_width = self.button.get_width();
        self.button
            .set_x(x + (entry_width - button_width) / 2);
        self.button
            .set_y(y + (entry_height - self.button.get_height()) / 2);
        self.button
            .extract_render_state(graphics, mouse_x, mouse_y, delta);
    }

    // java: public Void getValue() { return null; }
    pub fn get_value(&self) -> Option<()> {
        None
    }

    // java: public Optional<Void> getDefaultValue() { return Optional.empty(); }
    pub fn get_default_value(&self) -> Option<()> {
        None
    }

    // java: public void save() {}
    pub fn save(&self) {}

    // java: public List<? extends GuiEventListener> children() { return List.of(button); }
    pub fn children(&self) -> [seams::Button; 1] {
        [seams::Button::clone_handle(&self.button)]
    }

    // java: public List<? extends NarratableEntry> narratables() { return List.of(button); }
    pub fn narratables(&self) -> [seams::Button; 1] {
        [seams::Button::clone_handle(&self.button)]
    }
}

impl seams::AnyConfigEntry for ButtonEntry {
    fn extract_render_state(
        &mut self,
        graphics: &seams::GraphicsRef,
        index: i32,
        y: i32,
        x: i32,
        entry_width: i32,
        entry_height: i32,
        mouse_x: i32,
        mouse_y: i32,
        is_hovered: bool,
        delta: f32,
    ) {
        // java calls the same method on the class itself (the inherent method).
        self.extract_render_state(
            graphics, index, y, x, entry_width, entry_height, mouse_x, mouse_y, is_hovered, delta,
        );
    }
}

}
// ---- inlined: src/seams.rs ----
pub mod seams {
//! Host seams: every Minecraft client / Fabric / MixinExtras type the port
//! touches sits here, as a thin no-op-default surface the engine binds.
//! Signatures mirror the upstream Java calls one to one.

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPipelineRef(pub u64);

pub struct GuiTexturedSnippet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    SrcAlpha,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOp {
    ReverseSubtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendFunction {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub op: BlendOp,
}

pub struct PipelineSpec {
    pub base_snippet: GuiTexturedSnippet,
    pub location: String,
    pub blend: BlendFunction,
}

pub fn register_render_pipeline(spec: PipelineSpec) -> RenderPipelineRef {
    // RenderPipelines.register(builder(GUI_TEXTURED_SNIPPET).withLocation(...)
    //     .withColorTargetState(...).build()); the engine binds this.
    let _ = spec;
    RenderPipelineRef(0)
}

// java: LoadingOverlay.FADE_IN_TIME / FADE_OUT_TIME (vanilla ms constants).
pub fn loading_overlay_fade_in_time() -> f32 {
    1000.0
}

pub fn loading_overlay_fade_out_time() -> f32 {
    1000.0
}

// java: net.minecraft.util.ARGB helpers, exact bit math.
pub fn argb_red_float(color: i32) -> f32 {
    ((color >> 16) & 0xff) as f32 / 255.0
}

pub fn argb_green_float(color: i32) -> f32 {
    ((color >> 8) & 0xff) as f32 / 255.0
}

pub fn argb_blue_float(color: i32) -> f32 {
    (color & 0xff) as f32 / 255.0
}

pub fn argb_alpha(color: i32) -> i32 {
    (color >> 24) & 0xff
}

// java: ARGB.as8BitChannel(float) -> (int) (value * 255)
pub fn argb_as8_bit_channel(value: f32) -> i32 {
    (value * 255.0) as i32
}

// java: ARGB.color(alpha, red, green, blue)
pub fn argb_color(alpha: i32, red: i32, green: i32, blue: i32) -> i32 {
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

// java: FabricLoader.getInstance().getConfigDir()
pub fn fabric_config_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("config")
}

pub fn log_warn(message: &str) {
    eprintln!("{message}");
}

pub fn log_error(message: &str) {
    eprintln!("{message}");
}

// java: net.minecraft.util.Util.getMillis()
pub fn millis() -> u64 {
    0
}

// java: net.minecraft.util.Mth.clamp(float, min, max)
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

// java: CompletableFuture.runAsync(() -> {}, CompletableFuture.delayedExecutor(ms, MILLISECONDS))
#[derive(Debug, Clone)]
pub struct FutureHandle(pub u64);

pub fn delayed_complete_future(_ms: u64) -> FutureHandle {
    FutureHandle(0)
}

// java: net.minecraft.resources.Identifier texture handle.
pub type TextureRef = String;

/// java: GuiGraphicsExtractor — fill + the blit variant the mixin wraps.
pub struct GraphicsRef;

impl GraphicsRef {
    // java: graphics.fill(int x0, int y0, int x1, int y1, int color)
    pub fn fill(&self, _x0: i32, _y0: i32, _x1: i32, _y1: i32, _color: i32) {}
}

/// java: MixinExtras Operation<Void> for the blit WrapOperation.
pub struct BlitOperation(pub Arc<dyn Fn(&GraphicsRef, RenderPipelineRef, &TextureRef, i32, i32, f32, f32, i32, i32, i32, i32, i32, i32, i32)>);

impl BlitOperation {
    #[allow(clippy::too_many_arguments)]
    pub fn call(
        &self,
        graphics: &GraphicsRef,
        pipeline: RenderPipelineRef,
        texture: &TextureRef,
        x: i32,
        y: i32,
        u: f32,
        v: f32,
        width: i32,
        height: i32,
        src_width: i32,
        src_height: i32,
        texture_width: i32,
        texture_height: i32,
        color: i32,
    ) {
        (self.0)(graphics, pipeline, texture, x, y, u, v, width, height, src_width, src_height, texture_width, texture_height, color);
    }
}

// java: Minecraft.getInstance() / Screen / overlay handles.
pub type ScreenHandle = u64;

pub fn is_mod_loaded(_id: &str) -> bool {
    false
}

pub fn font_width(_text: &Component) -> i32 {
    0
}

pub fn gui_set_overlay(_overlay: &LoadingOverlayHandle) {}

pub type LoadingOverlayHandle = u64;

/// java: net.minecraft.server.packs.resources.ReloadInstance surface used here.
pub trait ReloadInstance {
    fn done(&self) -> FutureHandle;
    fn get_actual_progress(&self) -> f32;
}

// java: LoadingOverlay(Minecraft, ReloadInstance, consumer, bool) constructor.
pub fn loading_overlay_new(
    _reload: Box<dyn ReloadInstance>,
    _on_done: Box<dyn Fn()>,
    _b: bool,
) -> LoadingOverlayHandle {
    0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component(pub String);

impl Component {
    pub fn translatable(key: &str) -> Component {
        Component(key.to_string())
    }

    pub fn empty() -> Component {
        Component(String::new())
    }
}

// java: me.shedaniel.clothconfig2 GUI seams.
pub struct ConfigBuilder(pub u64);

impl ConfigBuilder {
    pub fn create() -> ConfigBuilderBuilder {
        ConfigBuilderBuilder
    }
}

pub struct ConfigBuilderBuilder;

impl ConfigBuilderBuilder {
    pub fn set_parent_screen(self, _parent: &ScreenHandle) -> Self {
        self
    }

    pub fn set_title(self, _title: Component) -> ConfigBuilder {
        ConfigBuilder(0)
    }
}

impl ConfigBuilder {
    pub fn get_or_create_category(&self, _name: Component) -> ConfigCategory {
        ConfigCategory
    }

    pub fn entry_builder(&self) -> ConfigEntryBuilder {
        ConfigEntryBuilder
    }

    pub fn set_saving_runnable(&self, _runnable: Box<dyn Fn()>) {}

    pub fn build(&self) -> ScreenHandle {
        0
    }
}

pub struct ConfigCategory;

/// Any entry row the Cloth category can hold (color/float fields, the
/// ButtonEntry). Mirrors AbstractConfigListEntry's render surface.
pub trait AnyConfigEntry {
    #[allow(clippy::too_many_arguments)]
    fn extract_render_state(
        &mut self,
        _graphics: &GraphicsRef,
        _index: i32,
        _y: i32,
        _x: i32,
        _entry_width: i32,
        _entry_height: i32,
        _mouse_x: i32,
        _mouse_y: i32,
        _is_hovered: bool,
        _delta: f32,
    ) {
    }
}

pub struct FieldWidget<T>(pub T);

impl<T> AnyConfigEntry for FieldWidget<T> {}

impl ConfigCategory {
    pub fn add_entry(&self, _entry: Box<dyn AnyConfigEntry>) {}
}

pub struct ConfigEntryBuilder;

impl ConfigEntryBuilder {
    pub fn start_color_field(&self, _name: Component, value: i32) -> ColorEntryBuilder {
        ColorEntryBuilder(value)
    }

    pub fn start_float_field(&self, _name: Component, value: f32) -> FloatEntryBuilder {
        FloatEntryBuilder(value)
    }
}

pub struct ColorEntryBuilder(i32);

impl ColorEntryBuilder {
    pub fn set_default_value(self, _default: i32) -> Self {
        self
    }

    pub fn build(self) -> ColorEntry {
        ColorEntry { value: self.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorEntry {
    value: i32,
}

impl ColorEntry {
    pub fn get_value(&self) -> i32 {
        self.value
    }

    pub fn handle(&self) -> ColorEntry {
        *self
    }
}

pub struct FloatEntryBuilder(f32);

impl FloatEntryBuilder {
    pub fn set_default_value(self, _default: f32) -> Self {
        self
    }

    pub fn set_min(self, _min: f32) -> Self {
        self
    }

    pub fn set_max(self, _max: f32) -> Self {
        self
    }

    pub fn build(self) -> FloatListEntry {
        FloatListEntry { value: self.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatListEntry {
    value: f32,
}

impl FloatListEntry {
    pub fn get_value(&self) -> f32 {
        self.value
    }

    pub fn handle(&self) -> FloatListEntry {
        *self
    }
}

/// java: AbstractConfigListEntry<Void> base class.
pub struct AbstractConfigListEntry {
    #[allow(dead_code)]
    name: Component,
    #[allow(dead_code)]
    expanded: bool,
}

impl AbstractConfigListEntry {
    pub fn new(name: Component, expanded: bool) -> AbstractConfigListEntry {
        AbstractConfigListEntry { name, expanded }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn extract_render_state(
        &self,
        _graphics: &GraphicsRef,
        _index: i32,
        _y: i32,
        _x: i32,
        _entry_width: i32,
        _entry_height: i32,
        _mouse_x: i32,
        _mouse_y: i32,
        _is_hovered: bool,
        _delta: f32,
    ) {
        // super.extractRenderState: base draws the label; engine binds it.
    }
}

/// java: net.minecraft.client.gui.components.Button.
#[derive(Debug, Clone, Copy)]
pub struct Button {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

pub struct ButtonBuilder {
    width: i32,
    #[allow(dead_code)]
    on_press: Box<dyn Fn()>,
}

impl Button {
    pub fn builder(label: Component, on_press: Box<dyn Fn()>) -> ButtonBuilder {
        let _ = label;
        ButtonBuilder {
            width: 0,
            on_press,
        }
    }

    pub fn clone_handle(&self) -> Button {
        *self
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: i32) {
        self.y = y;
    }

    pub fn extract_render_state(
        &self,
        _graphics: &GraphicsRef,
        _mouse_x: i32,
        _mouse_y: i32,
        _delta: f32,
    ) {
    }
}

impl ButtonBuilder {
    pub fn width(mut self, width: i32) -> Self {
        self.width = width;
        self
    }

    pub fn build(self) -> Button {
        Button {
            width: self.width,
            height: 20,
            x: 0,
            y: 0,
        }
    }
}

}
