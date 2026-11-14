//! Upstream: common/render/ (1:1). DrawContext, OverlayRenderer,
//! DirectionIndicatorRenderer, PingLocationRenderer.
use crate::config::{PlayerInfoMode, TeamColorMode};
use crate::core::{client_config_snapshot, GameContext, PingManager, PingView};
use crate::math::MathUtils;
use crate::resource::{self, ARROW_TEXTURE_ID, PING_TEXTURE_ID};
use crate::resource::ResourceLocationToken;
use crate::seams::{self, GuiEnv, ItemStackToken, PlayerInfoHandle, PoseStack, Vec2};
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

        if crate::resource::ResourceReloadListener::has_custom_texture() {
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
        crate::resource::NUM_CUSTOM_TEXTURES.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(!crate::resource::ResourceReloadListener::has_custom_texture());
        crate::resource::NUM_CUSTOM_TEXTURES.store(1, std::sync::atomic::Ordering::SeqCst);
        assert!(!crate::resource::ResourceReloadListener::has_custom_texture());
        crate::resource::NUM_CUSTOM_TEXTURES.store(2, std::sync::atomic::Ordering::SeqCst);
        assert!(crate::resource::ResourceReloadListener::has_custom_texture());
        crate::resource::NUM_CUSTOM_TEXTURES.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// checkResources flips the deprecation notice only for the three legacy ids.
    #[test]
    fn check_resources_flags_exactly_the_legacy_textures() {
        let calls = std::cell::RefCell::new(Vec::new());
        crate::compat::check_resources(&|ns: &str, path: &str| {
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
        assert!(crate::compat::notify_deprecated_resource_pack(), "texture probe must flag the notice");
        // A resource pack without any legacy texture must clear it again.
        crate::compat::check_resources(&|_ns: &str, _path: &str| false);
        assert!(!crate::compat::notify_deprecated_resource_pack());
    }

    /// DirectionIndicatorRenderer.prepare_safe_zone stores the four screen values.
    #[test]
    fn prepare_safe_zone_computes_the_safe_rect() {
        struct G;
        impl crate::seams::MinecraftClient for G {
            fn level(&self) -> Option<Rc<dyn crate::seams::LevelHandle>> { None }
            fn camera_entity(&self) -> Option<Rc<dyn crate::seams::EntityHandle>> { None }
            fn gui_scaled_size(&self) -> (f32, f32) { (427.0, 240.0) }
        }
        let _g = crate::tests::install();
        crate::seams::set_game(Some(Rc::new(G) as Rc<dyn crate::seams::MinecraftClient>));

        {
            let cfg = crate::config::client_handler().config_mut();
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
            let cfg = crate::config::client_handler().config_mut();
            cfg.safe_zone_left = crate::config::ClientConfig::default().safe_zone_left;
            cfg.safe_zone_top = crate::config::ClientConfig::default().safe_zone_top;
            cfg.safe_zone_right = crate::config::ClientConfig::default().safe_zone_right;
            cfg.safe_zone_bottom = crate::config::ClientConfig::default().safe_zone_bottom;
        }
    }
}
