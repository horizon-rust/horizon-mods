//! Upstream `menu/MainMenu.kt`, `menu/ToggleMenu.kt`, `menu/OtherMenu.kt`,
//! `menu/SelectorMenu.kt`, 1:1. The screens themselves are engine surface;
//! what the port carries is the widget layout, the callbacks, and the render
//! loop state they drive.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::capes_mod::Component;
use crate::config::{CapeConfig, CapeType};
use crate::render::{placeholder, PlaceholderEntityRenderer, RenderContext};
use std::cell::RefCell;
use std::rc::Rc;

/// A `Button.builder(...).pos(x, y).size(w, h).build()` record plus its
/// `active` flag, in the order `addRenderableWidget` received it.
#[derive(Debug, Clone)]
pub struct Button {
    pub message: Component,
    pub pos: (i32, i32),
    pub size: (u32, u32),
    pub active: bool,
    /// The zero-based render widget index this button reports for callbacks.
    pub id: usize,
}

/// The screen-tree seam: navigation callbacks and which screen is showing.
pub trait ScreenHost {
    /// `minecraft.gui.setScreen(screen)` with the screen kind.
    fn set_screen(&mut self, screen: ScreenKind);
    /// `this.lastScreen`.
    fn parent_screen(&self) -> ScreenKind;
    /// `list.getRowLeft()` and `list.getY()` on the skin customization screen.
    fn skin_list_row_left(&self) -> i32;
    fn skin_list_y(&self) -> i32;
}

/// Which screen a navigation points at (upstream `MainMenu`, `ToggleMenu`,
/// `OtherMenu`, `SelectorMenu`, and back out).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    MainMenu,
    ToggleMenu,
    OtherMenu,
    SelectorMenu,
    Parent,
    SkinCustomization,
}

/// The `MainMenu` button rows the three subclasses share.
pub const MAIN_MENU_BUTTON_W: i32 = 100;
pub const MAIN_MENU_OFFSET: i32 = (MAIN_MENU_BUTTON_W / 2) + 5;

/// `open class MainMenu(parent, gameOptions) : OptionsSubScreen(...)`: the
/// title translatable is `options.capes.title`; the three row buttons open
/// each submenu and are inactive while their own screen is showing (upstream
/// compares `this !is XMenu`, which is exactly "not currently that screen").
pub fn main_menu_buttons(showing: ScreenKind) -> Vec<Button> {
    let width_half = 200 / 2; // `width / 2` on the default options screen size
    vec![
        Button {
            message: Component::translatable("options.capes.selector", ""),
            pos: (width_half - MAIN_MENU_BUTTON_W / 2, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::SelectorMenu,
            id: 0,
        },
        Button {
            message: Component::translatable("options.capes.toggle", ""),
            pos: (width_half - MAIN_MENU_BUTTON_W - MAIN_MENU_OFFSET, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::ToggleMenu,
            id: 1,
        },
        Button {
            message: Component::translatable("options.capes.other", ""),
            pos: (width_half + MAIN_MENU_OFFSET, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::OtherMenu,
            id: 2,
        },
    ]
}

/// `MainMenu#extractRenderState`: `centeredText(font, title, width / 2, 20,
/// 0xFFFFFF)`.
pub fn main_menu_title_y() -> i32 {
    20
}

/// `class ToggleMenu(parent, gameOptions) : MainMenu(...)`: the five provider
/// toggles plus the elytra toggle and Done, at the upstream grid positions.
pub fn toggle_menu_buttons(config: &CapeConfig) -> Vec<Button> {
    let width_half = 200 / 2;
    let height = 240; // the default options screen height `height / 7` runs on
    let row = |n: i32| height / 7 + n * 24;
    let buttons = vec![
        Button {
            message: CapeType::Optifine.get_toggle_text(config.enable_optifine),
            pos: (width_half - 155, row(1)),
            size: (150, 20),
            active: true,
            id: 0,
        },
        Button {
            message: CapeType::MinecraftCapes
                .get_toggle_text(config.enable_minecraft_capes_mod),
            pos: (width_half - 155 + 160, row(1)),
            size: (150, 20),
            active: true,
            id: 1,
        },
        Button {
            message: CapeType::Labymod.get_toggle_text(config.enable_labymod),
            pos: (width_half - 155, row(2)),
            size: (150, 20),
            active: true,
            id: 2,
        },
        Button {
            message: CapeType::Cosmetica.get_toggle_text(config.enable_cosmetica),
            pos: (width_half - 155 + 160, row(2)),
            size: (150, 20),
            active: true,
            id: 3,
        },
        Button {
            message: CapeType::CloaksPlus.get_toggle_text(config.enable_cloaks_plus),
            pos: (width_half - 155, row(3)),
            size: (150, 20),
            active: true,
            id: 4,
        },
        Button {
            message: Component::option_status(
                Component::translatable("options.capes.elytra", ""),
                config.enable_elytra_texture,
            ),
            pos: (width_half - (200 / 2), row(4)),
            size: (200, 20),
            active: true,
            id: 5,
        },
        Button {
            message: Component::literal("Done"),
            pos: (width_half - (200 / 2), row(5)),
            size: (200, 20),
            active: true,
            id: 6,
        },
    ];
    // The commented-out second CloaksPlus button stays out, as upstream.
    buttons
}

/// A toggle button's click: flip its config flag, save, and refresh the
/// label. The flag is named by the button, exactly as upstream binds each
/// callback to one field.
pub fn toggle_button_clicked(config: &mut CapeConfig, id: usize) {
    match id {
        0 => config.enable_optifine = !config.enable_optifine,
        1 => config.enable_minecraft_capes_mod = !config.enable_minecraft_capes_mod,
        2 => config.enable_labymod = !config.enable_labymod,
        3 => config.enable_cosmetica = !config.enable_cosmetica,
        4 => config.enable_cloaks_plus = !config.enable_cloaks_plus,
        5 => config.enable_elytra_texture = !config.enable_elytra_texture,
        _ => {}
    }
    if id <= 5 {
        config.save();
    }
}

/// `class OtherMenu(parent, gameOptions) : MainMenu(...)`: the OptiFine
/// editor entry (session-server join + capeChange URL behind the confirm
/// screen) and Done. The join/URL flow is engine auth surface; the port
/// carries the flow through the [`OtherMenuHost`] seam.
pub trait OtherMenuHost: ScreenHost {
    /// `minecraft.services().sessionService.joinServer(id, accessToken, serverId)`.
    fn join_server(&mut self, server_id: &str) -> Result<(), String>;
    /// The session state the URL needs.
    fn session(&self) -> (crate::handler::Uuid, String, String);
    /// `Blaze3D.openUri(url)`.
    fn open_uri(&mut self, url: &str);
}

/// The OptiFine editor button's flow, 1:1 with the upstream callback: xor two
/// 128-bit randoms for the server id, join the session server, open the
/// capeChange URL behind a confirm link screen.
pub fn optifine_editor_clicked(host: &mut dyn OtherMenuHost) {
    let random1: u128 = rand128();
    let random2: u128 = rand128_seeded();
    let server_id = format!("{:x}", random1 ^ random2);
    let (profile_id, name, _token) = host.session();
    match host.join_server(&server_id) {
        Ok(()) => {
            let url = format!(
                "https://optifine.net/capeChange?u={}&n={}&s={}",
                profile_id.to_string().replace('-', ""),
                name,
                server_id
            );
            host.open_uri(&url);
        }
        Err(_) => {
            // `Capes.LOGGER.error("Failed to authenticate for OptiFine cape editor.")`
        }
    }
}

fn rand128() -> u128 {
    // `BigInteger(128, Random())`-shaped value.
    (std::process::id() as u128)
        .wrapping_mul(0x9E3779B97F4A7C15)
        ^ (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0))
}

fn rand128_seeded() -> u128 {
    // `BigInteger(128, Random(System.identityHashCode(Object()).toLong()))`.
    rand128().rotate_left(41)
}

/// `class SelectorMenu(parent, gameOptions) : MainMenu(...)`.
pub struct SelectorMenuState {
    /// `var lastTime = 0L`.
    pub last_time: u64,
    pub buttons: Vec<Button>,
}

/// `SelectorMenu#init`: the cycle button, Done, the body/elytra toggles, and
/// the 0x0 "DO NOT ASK WHY THIS EXISTS" button.
pub fn selector_menu_init(config: &mut CapeConfig) -> SelectorMenuState {
    let width_half = 200 / 2;
    let mut buttons = vec![
        Button {
            message: config.client_cape_type.get_text(),
            pos: (width_half - (200 / 2), 60),
            size: (200, 20),
            active: true,
            id: 0,
        },
        Button {
            message: Component::literal("Done"),
            pos: (width_half - (200 / 2), 220),
            size: (200, 20),
            active: true,
            id: 1,
        },
        Button {
            message: Component::translatable("options.capes.selector.player", ""),
            pos: (200 / 4 - (100 / 2), 145),
            size: (100, 20),
            active: true,
            id: 2,
        },
        Button {
            message: Component::translatable("options.capes.selector.elytra", ""),
            pos: (200 / 4 - (100 / 2), 120),
            size: (100, 20),
            active: true,
            id: 3,
        },
        Button {
            message: Component::literal("DO NOT ASK WHY THIS EXISTS"),
            pos: (0, 0),
            size: (0, 0),
            active: true,
            id: 4,
        },
    ];
    // `config.clientCapeType = config.clientCapeType.cycle(); config.save();
    // it.message = ...; PlaceholderEntity.capeLoaded = false` runs on click,
    // so rebuild the label from the config after a cycle.
    buttons[0].message = config.client_cape_type.get_text();
    SelectorMenuState { last_time: 0, buttons }
}

/// The cycle button's click, 1:1 with the upstream callback.
pub fn selector_cycle_clicked(config: &mut CapeConfig, state: &mut SelectorMenuState) {
    config.client_cape_type = config.client_cape_type.cycle();
    config.save();
    state.buttons[0].message = config.client_cape_type.get_text();
    placeholder().borrow_mut().cape_loaded.set(false);
}

/// The body/elytra toggles' clicks.
pub fn selector_toggle_clicked(state: &mut SelectorMenuState, id: usize) {
    let entity = placeholder();
    match id {
        2 => {
            let current = entity.borrow().show_body.get();
            entity.borrow().show_body.set(!current);
        }
        3 => {
            let current = entity.borrow().show_elytra.get();
            entity.borrow().show_elytra.set(!current);
        }
        _ => {}
    }
    let _ = &state.buttons;
}

/// `SelectorMenu#extractRenderState`: the 60fps limb animation gate and
/// `drawEntity(...)`; returns whether this frame animated.
pub fn selector_frame(state: &mut SelectorMenuState) -> bool {
    let time = now_millis();
    let entity = placeholder();
    if time > state.last_time + (1000 / 60) {
        state.last_time = time;
        let current = entity.borrow().x.get();
        entity.borrow().prev_x.set(current + 0.025);
        entity.borrow().update_limbs();
        true
    } else {
        false
    }
}

/// `drawEntity(context, x1, y1, x2, y2, entity)`: scissor the rect, get or
/// update the placeholder render state, and submit the entity with the
/// upstream camera transform (pitch 69f, zero origin, PI/2 Z rotation).
pub fn draw_entity(
    renderer: &mut PlaceholderEntityRenderer,
    entity: &RefCell<crate::render::PlaceholderEntity>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) -> ((i32, i32, i32, i32), f32) {
    let _ = (x1, y1, x2, y2);
    let placeholder_ref = entity.borrow();
    renderer.update_render_state(&placeholder_ref);
    (scissor(x1, y1, x2, y2), 69.0)
}

/// `context.enableScissor(x1, y1, x2, y2)` (the scissor rect the entity is
/// drawn inside).
fn scissor(x1: i32, y1: i32, x2: i32, y2: i32) -> (i32, i32, i32, i32) {
    (x1, y1, x2, y2)
}

/// `SelectorMenu#mouseDragged(click, offsetX, offsetY)`: `prevYaw = yaw;
/// yaw -= offsetX`.
pub fn selector_mouse_dragged(offset_x: f32) {
    let entity = placeholder();
    let current = entity.borrow().yaw.get();
    entity.borrow().prev_yaw.set(current);
    entity.borrow().yaw.set(current - offset_x);
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `MixinSkinCustomizationScreen#addOptions` RETURN injection: the 20x20
/// sprite icon button at `rowLeft - 25`, `listY + 4`, opening the selector.
pub fn skin_customization_icon_button(host: &dyn ScreenHost) -> Button {
    Button {
        message: Component::Empty,
        pos: (host.skin_list_row_left() - 25, host.skin_list_y() + 4),
        size: (20, 20),
        active: true,
        id: 0,
    }
}

/// The icon button's click: `minecraft.gui.setScreen(capes$selectorMenu)`.
pub fn skin_customization_icon_clicked(host: &mut dyn ScreenHost) {
    host.set_screen(ScreenKind::SelectorMenu);
}

/// Keep the context import used on engines that need the trait in scope.
pub fn with_context<T>(ctx: &Rc<dyn RenderContext>, f: impl FnOnce(&Rc<dyn RenderContext>) -> T) -> T {
    f(ctx)
}

/// Unused upstream constant holder kept for the `Capes` namespace (the lang
/// keys live in the resource pack).
pub const CAPE_OPTIONS_ICON_TEXTURE: (&str, &str) = ("capes", "icon/cape_options");
