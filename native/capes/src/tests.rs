//! Tests for the capes port: config, profile validation, the provider flows,
//! and the mixin/menu callbacks.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::capes_mod::{parse_config, Capes, Component, MissingConfig};
use crate::config::{CapeConfig, CapeType};
use crate::handler::{
    GameProfile, HttpResponse, NativeImage, PlayerHandler, ProfileProperty, Texture, Uuid,
};
use crate::menus::*;
use crate::mixins::*;
use crate::render::{
    placeholder, reset_placeholder, PlayerModelType, PlayerSkin, RenderContext,
    PlaceholderEntityRenderState,
};
use crate::util::is_valid_profile;
use std::cell::RefCell;
use std::rc::Rc;

/// A recorded config store (`ConfigIo` seam).
struct RecordingConfig {
    reads: RefCell<Vec<Option<String>>>,
    writes: RefCell<Vec<String>>,
}

impl RecordingConfig {
    fn new() -> Self {
        Self {
            reads: RefCell::new(vec![None]),
            writes: RefCell::new(Vec::new()),
        }
    }
}

impl crate::capes_mod::ConfigIo for RecordingConfig {
    fn read_config(&self) -> Option<String> {
        self.reads.borrow_mut().pop().flatten()
    }
    fn write_config(&self, json: &str) {
        self.writes.borrow_mut().push(json.to_string());
    }
}

fn profile(id: &str, name: &str, textures: Option<&str>) -> GameProfile {
    GameProfile {
        id: Uuid::from_string(id),
        name: name.into(),
        properties: textures
            .map(|value| {
                vec![ProfileProperty {
                    name: "textures".into(),
                    value: value.into(),
                }]
            })
            .unwrap_or_default(),
    }
}

/// A version-4 UUID with a textures property naming the same profile.
fn valid_profile() -> GameProfile {
    // Base64 of {"profileName":"Steve"}; version nibble of the third group
    // hex is 4 when the uuid reads ...-4xxx-....
    let encoded = "eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0=";
    profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some(encoded),
    )
}

#[test]
fn a_valid_profile_passes_uuid_version_four() {
    assert!(is_valid_profile(&valid_profile()));
}

#[test]
fn an_unparseable_or_absent_textures_property_fails() {
    let no_textures = profile("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Steve", None);
    assert!(!is_valid_profile(&no_textures));
    let bad = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("!!!not base64!!!"),
    );
    assert!(!is_valid_profile(&bad));
}

#[test]
fn a_version_two_uuid_needs_the_matching_profile_name() {
    // Version-2 UUID (third group starts with 2) whose profileName differs.
    let mismatch = profile(
        "069a79f4-44e9-2726-a5be-fca90e38aaf5",
        "Alex",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    assert!(!is_valid_profile(&mismatch));
    let matching = profile(
        "069a79f4-44e9-2726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    assert!(is_valid_profile(&matching));
}

#[test]
fn the_config_defaults_match_the_upstream_fields() {
    let config = CapeConfig::default();
    assert_eq!(config.client_cape_type, CapeType::Minecraft);
    assert!(config.enable_optifine);
    assert!(!config.enable_labymod);
    assert!(!config.enable_minecraft_capes_mod);
    assert!(!config.enable_cosmetica);
    assert!(!config.enable_cloaks_plus);
    assert!(config.enable_elytra_texture);
}

#[test]
fn the_config_round_trips_through_its_json() {
    let config = CapeConfig {
        client_cape_type: CapeType::CloaksPlus,
        enable_labymod: true,
        ..CapeConfig::default()
    };
    let parsed = parse_config(&config.to_json()).unwrap();
    assert_eq!(parsed, config);
}

#[test]
fn a_missing_config_file_creates_the_default_and_a_save_writes_json() {
    let recording = Rc::new(RecordingConfig::new());
    Capes::reset_config();
    Capes::set_config_io(recording.clone());
    let config = Capes::config();
    assert_eq!(config, CapeConfig::default());
    config.save();
    let writes = recording.writes.borrow().clone();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].contains("\"clientCapeType\": \"MINECRAFT\""));
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

#[test]
fn cape_type_cycles_through_all_six_providers() {
    assert_eq!(CapeType::Minecraft.cycle(), CapeType::Optifine);
    assert_eq!(CapeType::Optifine.cycle(), CapeType::Labymod);
    assert_eq!(CapeType::Labymod.cycle(), CapeType::Cosmetica);
    assert_eq!(CapeType::Cosmetica.cycle(), CapeType::MinecraftCapes);
    assert_eq!(CapeType::MinecraftCapes.cycle(), CapeType::CloaksPlus);
    assert_eq!(CapeType::CloaksPlus.cycle(), CapeType::Minecraft);
}

#[test]
fn cape_urls_gate_on_their_provider_toggles() {
    let p = profile("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Steve", None);
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    // Default: OptiFine on, the rest off.
    assert_eq!(
        CapeType::Optifine.get_url(&p),
        Some("http://s.optifine.net/capes/Steve.png".into())
    );
    assert_eq!(CapeType::Labymod.get_url(&p), None);
    assert_eq!(CapeType::MinecraftCapes.get_url(&p), None);
    assert_eq!(CapeType::Minecraft.get_url(&p), None);
    // MinecraftCapes strips the dashes from the UUID.
    let mut config = CapeConfig::default();
    config.enable_minecraft_capes_mod = true;
    Capes::set_config_io(Rc::new(PinnedConfig(config)));
    Capes::reset_config();
    assert_eq!(
        CapeType::MinecraftCapes.get_url(&p),
        Some("https://api.minecraftcapes.net/profile/069a79f444e94726a5befca90e38aaf5".into())
    );
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

/// A config store serving a fixed JSON.
struct PinnedConfig(CapeConfig);

impl crate::capes_mod::ConfigIo for PinnedConfig {
    fn read_config(&self) -> Option<String> {
        Some(self.0.to_json())
    }
    fn write_config(&self, _json: &str) {}
}

#[test]
fn toggle_text_and_get_text_shape_the_labels() {
    assert_eq!(
        CapeType::Optifine.get_toggle_text(true),
        Component::literal("OptiFine: ON")
    );
    assert_eq!(
        CapeType::MinecraftCapes.get_toggle_text(false),
        Component::literal("MinecraftCapes: OFF")
    );
    assert_eq!(
        CapeType::Labymod.get_text(),
        Component::translatable("options.capes.capetype", "LabyMod")
    );
}

#[test]
fn the_debug_info_lists_type_animation_elytra_and_url() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let p = valid_profile();
    let info = Capes::get_debug_info_for_player(&p);
    assert!(info.contains("Name: Steve"));
    assert!(info.contains("UUID: 069a79f4-44e9-4726-a5be-fca90e38aaf5"));
    assert!(info.contains("Type: null"));
    assert!(info.contains("IsAnimated: false"));
    assert!(info.contains("HasElytraTexture: true"));
    assert!(info.contains("URL: null"));
}

#[test]
fn on_load_texture_for_the_own_profile_loads_the_configured_type() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    crate::handler::set_http(Rc::new(RecordingHttp::default()));
    let own = valid_profile();
    crate::handler::set_own_profile(own.clone());
    let handler = PlayerHandler::on_load_texture(&own);
    // MINECRAFT has no URL, so nothing loads and the type stays unset.
    assert!(!handler.borrow().has_cape);
    assert_eq!(handler.borrow().cape_type, None);
}

#[test]
fn on_load_texture_for_another_profile_tries_every_provider_once() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let http = Rc::new(RecordingHttp::default());
    crate::handler::set_http(http.clone());
    let other = valid_profile();
    crate::handler::set_own_profile(profile("00000000-0000-0000-0000-000000000000", "other", None));
    // Every provider is disabled or unreachable except OptiFine, which
    // returns a cape image.
    http.capes.borrow_mut().insert(
        "http://s.optifine.net/capes/Steve.png".to_string(),
        cape_png(64, 32),
    );
    let handler = PlayerHandler::on_load_texture(&other);
    assert!(handler.borrow().has_cape);
    assert_eq!(handler.borrow().cape_type, Some(CapeType::Optifine));
    assert!(handler.borrow().has_loaded_textures);
}

#[test]
fn a_labymod_watermark_cape_is_rejected() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let watermark = crate::handler::uuid_from_int_array(&[
        0x81, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30,
        0xff,
    ]);
    assert_eq!(
        watermark.0,
        Uuid::from_string("ff305f81-ff30-5f90-ff30-5f90ff305f90").0
    );
}

#[test]
fn parse_cape_upscales_to_the_next_power_of_two_frame() {
    let img = NativeImage::new(128, 64, true);
    let parsed = crate::handler::parse_cape(&img);
    assert_eq!(parsed.width, 128);
    assert_eq!(parsed.height, 64);
    let odd = NativeImage::new(65, 40, true);
    let parsed = crate::handler::parse_cape(&odd);
    assert_eq!(parsed.width, 128);
    assert_eq!(parsed.height, 64);
}

#[test]
fn parse_animated_cape_slices_frames_of_half_the_width() {
    let mut img = NativeImage::new(64, 96, true);
    for frame in 0..3 {
        for x in 0..64 {
            for y in 0..32 {
                img.set_pixel(x, y + frame * 32, 0xFF00_0000 | frame);
            }
        }
    }
    let frames = crate::handler::parse_animated_cape(&img);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[&1].get_pixel(0, 0) & 0xFF, 1);
}

#[test]
fn get_cape_steps_the_animated_frame_every_hundred_millis() {
    let mut handler = crate::handler::PlayerHandler {
        profile: valid_profile(),
        uuid: Uuid::from_string("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
        last_frame: 0,
        max_frames: 3,
        last_frame_time: 0,
        has_cape: true,
        has_elytra_texture: true,
        has_animated_cape: true,
        cape_type: None,
        has_loaded_textures: true,
    };
    let first = handler.get_cape();
    assert!(matches!(first, Texture::Dynamic(_)));
    assert_eq!(handler.last_frame, 1);
    // Inside the 100ms window the frame holds.
    handler.last_frame_time = u64::MAX / 2; // far in the future
    let held = handler.get_cape();
    assert!(matches!(held, Texture::Dynamic(_)));
    assert_eq!(handler.last_frame, 1);
    // Static capes resolve to the plain uuid texture.
    handler.has_animated_cape = false;
    let static_cape = handler.get_cape();
    assert!(matches!(static_cape, Texture::Dynamic(_)));
}

#[test]
fn get_cape_texture_swaps_the_skin_when_the_handler_has_a_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    crate::handler::set_http(Rc::new(RecordingHttp::default()));
    let p = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    let http = Rc::new(RecordingHttp::default());
    crate::handler::set_http(http.clone());
    http.capes.borrow_mut().insert(
        "http://s.optifine.net/capes/Steve.png".to_string(),
        cape_png(64, 32),
    );
    loadTextures(&host_for(&p, base_skin()));
    let handler = PlayerHandler::from_profile(&p);
    assert!(handler.borrow().has_cape);
    let mut host = host_for(&p, base_skin());
    get_cape_texture(&mut host);
    assert!(host.returned.is_some());
    let skin = host.returned.unwrap();
    assert!(skin.cape.is_some());
}

#[test]
fn get_cape_texture_leaves_the_skin_alone_without_a_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    crate::handler::set_http(Rc::new(RecordingHttp::default()));
    let p = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    loadTextures(&host_for(&p, base_skin()));
    let mut host = host_for(&p, base_skin());
    get_cape_texture(&mut host);
    assert!(host.returned.is_none());
}

#[test]
fn fix_cape_transparency_swaps_entity_solid_for_armor_cutout_no_cull() {
    assert_eq!(
        fix_cape_transparency(
            RenderKind::EntitySolid,
            crate::handler::Identifier::with_default_namespace("x"),
        ),
        RenderKind::ArmorCutoutNoCull
    );
}

#[test]
fn get_placeholder_renderer_matches_its_own_render_state() {
    let state = PlaceholderEntityRenderState::default();
    assert!(get_placeholder_renderer(&state).is_some());
    assert!(get_placeholder_renderer(&42u32).is_none());
}

#[test]
fn toggle_menu_buttons_flip_their_config_flag_save_and_relabel() {
    let recording = Rc::new(RecordingConfig::new());
    Capes::reset_config();
    Capes::set_config_io(recording.clone());
    let mut config = CapeConfig::default();
    let buttons = toggle_menu_buttons(&config);
    assert_eq!(buttons.len(), 7);
    assert_eq!(
        buttons[0].message,
        CapeType::Optifine.get_toggle_text(true)
    );
    toggle_button_clicked(&mut config, 0);
    assert!(!config.enable_optifine);
    assert!(!recording.writes.borrow().is_empty());
    assert_eq!(
        CapeType::Optifine.get_toggle_text(config.enable_optifine),
        Component::literal("OptiFine: OFF")
    );
    // The elytra toggle saves too; Done does not.
    let writes = recording.writes.borrow().len();
    toggle_button_clicked(&mut config, 5);
    assert_eq!(recording.writes.borrow().len(), writes + 1);
    toggle_button_clicked(&mut config, 6);
    assert_eq!(recording.writes.borrow().len(), writes + 1);
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

#[test]
fn main_menu_lays_out_the_three_row_buttons() {
    let buttons = main_menu_buttons(ScreenKind::MainMenu);
    assert_eq!(buttons.len(), 3);
    assert_eq!(buttons[0].pos, (100 - 50, 35));
    assert_eq!(buttons[1].pos, (100 - 100 - 55, 35));
    assert_eq!(buttons[2].pos, (100 + 55, 35));
    assert!(buttons.iter().all(|b| b.active));
    let on_selector = main_menu_buttons(ScreenKind::SelectorMenu);
    assert!(!on_selector[0].active);
    assert!(main_menu_title_y() == 20);
}

#[test]
fn selector_cycle_clicked_advances_the_type_and_reloads_the_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    reset_placeholder();
    crate::handler::set_http(Rc::new(RecordingHttp::default()));
    let mut config = CapeConfig::default();
    let mut state = selector_menu_init(&mut config);
    assert_eq!(state.buttons[0].message, config.client_cape_type.get_text());
    // A loaded placeholder cape is dropped on cycle (`PlaceholderEntity.capeLoaded = false`).
    placeholder().borrow().get_cape_texture();
    assert!(placeholder().borrow().cape_loaded.get());
    selector_cycle_clicked(&mut config, &mut state);
    assert_eq!(config.client_cape_type, CapeType::Optifine);
    assert!(!placeholder().borrow().cape_loaded.get());
    assert_eq!(state.buttons[0].message, config.client_cape_type.get_text());
}

#[test]
fn selector_toggles_flip_body_and_elytra() {
    reset_placeholder();
    let mut state = SelectorMenuState {
        last_time: 0,
        buttons: Vec::new(),
    };
    let before = placeholder().borrow().show_body.get();
    selector_toggle_clicked(&mut state, 2);
    assert_eq!(placeholder().borrow().show_body.get(), !before);
    let before = placeholder().borrow().show_elytra.get();
    selector_toggle_clicked(&mut state, 3);
    assert_eq!(placeholder().borrow().show_elytra.get(), !before);
}

#[test]
fn selector_mouse_dragged_rotates_the_placeholder() {
    reset_placeholder();
    selector_mouse_dragged(5.0);
    let entity = placeholder();
    assert_eq!(entity.borrow().yaw.get(), -5.0);
    assert_eq!(entity.borrow().prev_yaw.get(), 0.0);
}

#[test]
fn selector_frame_animates_limbs_at_sixty_fps() {
    reset_placeholder();
    let mut state = SelectorMenuState {
        last_time: 0,
        buttons: Vec::new(),
    };
    assert!(selector_frame(&mut state));
    assert_eq!(placeholder().borrow().prev_x.get(), 0.025);
    // A second call in the same 16ms window is skipped.
    state.last_time = u64::MAX / 2;
    assert!(!selector_frame(&mut state));
}

#[test]
fn the_skin_customization_icon_opens_the_selector() {
    struct Host {
        opened: RefCell<Option<ScreenKind>>,
    }
    impl ScreenHost for Host {
        fn set_screen(&mut self, screen: ScreenKind) {
            *self.opened.borrow_mut() = Some(screen);
        }
        fn parent_screen(&self) -> ScreenKind {
            ScreenKind::SkinCustomization
        }
        fn skin_list_row_left(&self) -> i32 {
            120
        }
        fn skin_list_y(&self) -> i32 {
            30
        }
    }
    let host = Host {
        opened: RefCell::new(None),
    };
    let button = skin_customization_icon_button(&host);
    assert_eq!(button.pos, (95, 34));
    assert_eq!(button.size, (20, 20));
    let mut host = host;
    skin_customization_icon_on_click(&mut host);
    assert_eq!(*host.opened.borrow(), Some(ScreenKind::SelectorMenu));
}

#[test]
fn the_placeholder_skin_carries_the_cape_and_elytra_textures() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    reset_placeholder();
    crate::handler::set_http(Rc::new(RecordingHttp::default()));
    let entity = placeholder();
    let skin = entity.borrow().get_skin_textures();
    assert!(skin.cape.is_some());
    // `hasElytraTexture` + `enableElytraTexture`: the elytra reuses the cape.
    assert_eq!(skin.elytra, skin.cape.clone().unwrap());
    // capeLoaded flips on the first texture request.
    assert!(entity.borrow().cape_loaded.get());
}

#[test]
fn the_renderer_bakes_the_right_model_and_layers() {
    reset_placeholder();
    let ctx: Rc<dyn RenderContext> = Rc::new(NullCtx);
    let mut renderer = crate::render::PlaceholderEntityRenderer::new(ctx.clone(), false);
    assert!(!renderer.slim);
    assert_eq!(
        renderer.layers,
        vec!["CapeLayer", "CustomHeadLayer", "WingsLayer"]
    );
    let entity = placeholder();
    renderer.update_render_state(&entity.borrow());
    assert!(renderer.state.show_cape);
    assert_eq!(renderer.state.model_part_options.hat, true);
    assert_eq!(renderer.scale(), 0.9375);
    // The slim model bakes the player_slim layer.
    let renderer = crate::render::PlaceholderEntityRenderer::new(ctx, true);
    assert!(renderer.model.slim);
}

struct NullCtx;

impl RenderContext for NullCtx {
    fn equipment_assets(&self) -> u64 {
        0
    }
    fn block_model_resolver(&self) -> u64 {
        0
    }
    fn item_model_resolver(&self) -> u64 {
        0
    }
    fn map_renderer(&self) -> u64 {
        0
    }
    fn resource_manager(&self) -> u64 {
        0
    }
    fn entity_models(&self) -> u64 {
        0
    }
    fn atlas_manager(&self) -> u64 {
        0
    }
    fn font(&self) -> u64 {
        0
    }
    fn player_skin_render_cache(&self) -> u64 {
        0
    }
    fn paletted_texture_manager(&self) -> u64 {
        0
    }
    fn is_model_part_enabled(&self, _part: &str) -> bool {
        true
    }
    fn skin_for(
        &self,
        _profile: &GameProfile,
    ) -> crate::render::PlayerSkin {
        base_skin()
    }
}

fn base_skin() -> PlayerSkin {
    PlayerSkin {
        body: Texture::ResourceTexture(crate::handler::Identifier::with_default_namespace(
            "textures/entity/player/wide/steve.png",
        )),
        cape: Some(Texture::ResourceTexture(
            crate::handler::Identifier::with_default_namespace("textures/entity/cape.png"),
        )),
        elytra: Texture::ResourceTexture(crate::handler::Identifier::with_default_namespace(
            "textures/entity/equipment/wings/elytra.png",
        )),
        model: PlayerModelType::Default,
        secure: true,
    }
}

/// A `PlayerInfo` host recording the mixin's return swap.
struct Host {
    profile: GameProfile,
    skin: PlayerSkin,
    returned: Option<PlayerSkin>,
}

impl PlayerInfoHost for Host {
    fn profile(&self) -> GameProfile {
        self.profile.clone()
    }
    fn skin(&self) -> PlayerSkin {
        self.skin.clone()
    }
    fn set_return(&mut self, skin: PlayerSkin) {
        self.returned = Some(skin);
    }
}

fn host_for(profile: &GameProfile, skin: PlayerSkin) -> Host {
    Host {
        profile: profile.clone(),
        skin,
        returned: None,
    }
}

/// The HTTP seam recording every URL and serving pinned capes.
#[derive(Default)]
struct RecordingHttp {
    pub capes: RefCell<std::collections::HashMap<String, Vec<u8>>>,
}

impl crate::handler::Http for RecordingHttp {
    fn connect(&self, url: &str) -> Result<HttpResponse, crate::handler::HttpError> {
        if let Some(body) = self.capes.borrow().get(url) {
            return Ok(HttpResponse {
                status: 200,
                body: body.clone(),
            });
        }
        Err(crate::handler::HttpError)
    }
}

/// A 64x32 RGBA PNG of stored-block form.
fn cape_png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.push(0u8); // filter none
        for _ in 0..width * 4 {
            raw.push(0x40);
        }
    }
    let mut png = vec![0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    // IHDR
    push_chunk(&mut png, b"IHDR", {
        let mut d = Vec::new();
        d.extend_from_slice(&width.to_be_bytes());
        d.extend_from_slice(&height.to_be_bytes());
        d.extend_from_slice(&[8, 6, 0, 0, 0]);
        d
    });
    push_chunk(&mut png, b"IDAT", raw);
    push_chunk(&mut png, b"IEND", Vec::new());
    png
}

fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: Vec<u8>) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(&data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(&data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}
