//! Upstream `render/PlaceholderEntity*.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::capes_mod::Capes;
use crate::handler::{
    Identifier, PlayerHandler, Texture, Uuid,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `HumanoidModel.ArmPose` seam (only `EMPTY` is set upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmPose {
    Empty,
}

/// `PlayerModelType` seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerModelType {
    Default,
    Slim,
}

/// The visible-part bits `updateRenderState` copies from the player options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModelPartOptions {
    pub hat: bool,
    pub jacket: bool,
    pub left_pants: bool,
    pub right_pants: bool,
    pub left_sleeve: bool,
    pub right_sleeve: bool,
}

/// The `EntityRenderDispatcher` accessors `AccessorEntityRenderDispatcher`
/// opens (`equipmentAssets`, `blockModelResolver`) plus the other context
/// fields `PlaceholderEntity`'s init passes to the renderer provider.
pub trait RenderContext {
    fn equipment_assets(&self) -> u64;
    fn block_model_resolver(&self) -> u64;
    fn item_model_resolver(&self) -> u64;
    fn map_renderer(&self) -> u64;
    fn resource_manager(&self) -> u64;
    fn entity_models(&self) -> u64;
    fn atlas_manager(&self) -> u64;
    fn font(&self) -> u64;
    fn player_skin_render_cache(&self) -> u64;
    fn paletted_texture_manager(&self) -> u64;
    /// `Minecraft.getInstance().options.isModelPartEnabled(part)`.
    fn is_model_part_enabled(&self, part: &str) -> bool;
    /// `Minecraft.getInstance().skinManager.get(profile)` resolution: the
    /// player's skin once fetched.
    fn skin_for(&self, profile: &crate::handler::GameProfile) -> PlayerSkin;
}

/// `PlayerSkin` seam: body + cape + elytra textures, model, secure flag.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSkin {
    pub body: Texture,
    pub cape: Option<Texture>,
    pub elytra: Texture,
    pub model: PlayerModelType,
    pub secure: bool,
}

/// The render state upstream builds (`AvatarRenderState` fields this mod
/// touches).
#[derive(Debug, Clone)]
pub struct AvatarRenderState {
    pub body_rot: f32,
    pub walk_animation_pos: f32,
    pub walk_animation_speed: f32,
    pub left_arm_pose: ArmPose,
    pub right_arm_pose: ArmPose,
    pub skin: Option<PlayerSkin>,
    pub model_part_options: ModelPartOptions,
    pub show_cape: bool,
    /// `chestEquipment`: `None` is `ItemStack.EMPTY`.
    pub chest_equipment: Option<ItemStack>,
    pub elytra_rot_x: f32,
    pub elytra_rot_z: f32,
}

impl Default for AvatarRenderState {
    fn default() -> Self {
        Self {
            body_rot: 0.0,
            walk_animation_pos: 0.0,
            walk_animation_speed: 0.0,
            left_arm_pose: ArmPose::Empty,
            right_arm_pose: ArmPose::Empty,
            skin: None,
            model_part_options: ModelPartOptions::default(),
            show_cape: false,
            chest_equipment: None,
            elytra_rot_x: 0.0,
            elytra_rot_z: 0.0,
        }
    }
}

/// Upstream's `PlaceholderEntityRenderState : AvatarRenderState` subclass
/// carries no new state; the dispatcher mixin matches on it.
pub type PlaceholderEntityRenderState = AvatarRenderState;

/// `ItemStack` seam: the elytra stack the state fills, or empty.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemStack {
    Empty,
    Elytra,
}

/// The baked player model the renderer drives (`allParts().visible`).
#[derive(Debug, Clone, Default)]
pub struct PlayerModel {
    pub all_parts_visible: bool,
    pub slim: bool,
}

/// `PlaceholderEntityRenderer` seam surface: what the mod sets per frame and
/// what the engine renders (the actual draw is engine-side).
#[derive(Clone)]
pub struct PlaceholderEntityRenderer {
    pub slim: bool,
    pub ctx: Rc<dyn RenderContext>,
    pub model: PlayerModel,
    pub layers: Vec<&'static str>,
    pub state: AvatarRenderState,
    pub texture_location: Option<Identifier>,
    pub scale: f32,
}

impl PlaceholderEntityRenderer {
    /// The constructor: bake `PLAYER` or the `player_slim` layer, shadow
    /// radius `0.5f`, then `addLayer(CapeLayer)`, `addLayer(CustomHeadLayer)`,
    /// `addLayer(WingsLayer)`.
    pub fn new(ctx: Rc<dyn RenderContext>, slim: bool) -> Self {
        Self {
            slim,
            ctx,
            model: PlayerModel {
                all_parts_visible: true,
                slim,
            },
            layers: vec!["CapeLayer", "CustomHeadLayer", "WingsLayer"],
            state: AvatarRenderState::default(),
            texture_location: None,
            scale: 0.9375,
        }
    }

    /// `createRenderState()` + `getAndUpdatePlaceholderRenderState` /
    /// `updateRenderState`: fill the state from the placeholder, including
    /// the elytra item stack and the `elytraRotZ`/`elytraRotX` angles.
    pub fn update_render_state(&mut self, placeholder: &PlaceholderEntity) {
        let options = &placeholder.model_part_options;
        let body = placeholder.get_skin_textures();
        self.texture_location = Some(match &body {
            PlayerSkin { body, .. } => body.texture_path(),
        });
        self.state = AvatarRenderState {
            body_rot: placeholder.yaw.get(),
            walk_animation_pos: placeholder.limb_angle.get(),
            walk_animation_speed: placeholder.limb_distance.get(),
            left_arm_pose: ArmPose::Empty,
            right_arm_pose: ArmPose::Empty,
            skin: Some(body),
            model_part_options: *options,
            show_cape: true,
            chest_equipment: if placeholder.show_elytra.get() {
                Some(ItemStack::Elytra)
            } else {
                None
            },
            elytra_rot_z: -(std::f32::consts::PI / 12.0),
            elytra_rot_x: std::f32::consts::PI / 12.0,
        };
        self.model.all_parts_visible = placeholder.show_body.get();
    }

    /// `getTextureLocation(state)`.
    pub fn get_texture_location(&self) -> Option<Identifier> {
        self.texture_location.clone()
    }

    /// `scale(state, matrixStack)`.
    pub fn scale(&self) -> f32 {
        self.scale
    }
}

/// `object PlaceholderEntity`.
pub struct PlaceholderEntity {
    pub game_profile: crate::handler::GameProfile,
    pub skin: RefCell<PlayerSkin>,
    pub slim: Cell<bool>,
    pub show_body: Cell<bool>,
    pub show_elytra: Cell<bool>,
    pub cape_loaded: Cell<bool>,
    pub limb_distance: Cell<f32>,
    pub last_limb_distance: Cell<f32>,
    pub limb_angle: Cell<f32>,
    pub yaw: Cell<f32>,
    pub prev_yaw: Cell<f32>,
    pub x: Cell<f64>,
    pub prev_x: Cell<f64>,
    /// The options snapshot `updateRenderState` reads from
    /// `Minecraft.getInstance().options`.
    pub model_part_options: ModelPartOptions,
}

thread_local! {
    static CTX: RefCell<Option<Rc<dyn RenderContext>>> = const { RefCell::new(None) };
}

/// The render-context seam swap before the first `placeholder()` call.
pub fn set_render_context(ctx: Rc<dyn RenderContext>) {
    CTX.with(|slot| *slot.borrow_mut() = Some(ctx));
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<RefCell<PlaceholderEntity>>>> = const { RefCell::new(None) };
}

/// `PlaceholderEntity.INSTANCE` accessor with the upstream init.
pub fn placeholder() -> Rc<RefCell<PlaceholderEntity>> {
    INSTANCE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(instance) = &*slot {
            return instance.clone();
        }
        let ctx = CTX.with(|slot| slot.borrow().clone()).unwrap_or_else(|| {
            Rc::new(NullRenderContext) as Rc<dyn RenderContext>
        });
        let profile = ctx_default_profile(&ctx);
        let skin = ctx.skin_for(&profile);
        let slim = skin.model == PlayerModelType::Slim;
        let instance = Rc::new(RefCell::new(PlaceholderEntity {
            game_profile: profile,
            skin: RefCell::new(skin),
            slim: Cell::new(slim),
            show_body: Cell::new(true),
            show_elytra: Cell::new(false),
            cape_loaded: Cell::new(false),
            limb_distance: Cell::new(0.0),
            last_limb_distance: Cell::new(0.0),
            limb_angle: Cell::new(0.0),
            yaw: Cell::new(0.0),
            prev_yaw: Cell::new(0.0),
            x: Cell::new(0.0),
            prev_x: Cell::new(0.0),
            model_part_options: read_model_part_options(&ctx),
        }));
        // `Minecraft.getInstance().skinManager.get(gameProfile).thenAccept { ... }`
        let resolved = ctx.skin_for(&instance.borrow().game_profile);
        let slim = resolved.model == PlayerModelType::Slim;
        instance.borrow_mut().skin = RefCell::new(resolved);
        instance.borrow_mut().slim.set(slim);
        *slot = Some(instance.clone());
        instance
    })
}

/// Test teardown (upstream the object lives once per process).
pub fn reset_placeholder() {
    INSTANCE.with(|slot| *slot.borrow_mut() = None);
}

fn ctx_default_profile(_ctx: &Rc<dyn RenderContext>) -> crate::handler::GameProfile {
    crate::handler::GameProfile {
        id: Uuid(0, 4),
        name: "placeholder".into(),
        properties: Vec::new(),
    }
}

fn read_model_part_options(ctx: &Rc<dyn RenderContext>) -> ModelPartOptions {
    ModelPartOptions {
        hat: ctx.is_model_part_enabled("HAT"),
        jacket: ctx.is_model_part_enabled("JACKET"),
        left_pants: ctx.is_model_part_enabled("LEFT_PANTS_LEG"),
        right_pants: ctx.is_model_part_enabled("RIGHT_PANTS_LEG"),
        left_sleeve: ctx.is_model_part_enabled("LEFT_SLEEVE"),
        right_sleeve: ctx.is_model_part_enabled("RIGHT_SLEEVE"),
    }
}

struct NullRenderContext;

impl RenderContext for NullRenderContext {
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
    fn skin_for(&self, _profile: &crate::handler::GameProfile) -> PlayerSkin {
        PlayerSkin {
            body: Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/player/wide/steve.png",
            )),
            cape: Some(Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/cape.png",
            ))),
            elytra: Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/equipment/wings/elytra.png",
            )),
            model: PlayerModelType::Default,
            secure: false,
        }
    }
}

impl PlaceholderEntity {
    /// `fun updateLimbs()`.
    pub fn update_limbs(&self) {
        self.last_limb_distance.set(self.limb_distance.get());
        let d = self.x.get() - self.prev_x.get();
        let mut g = ((d * d).sqrt()) as f32 * 4.0;
        if g > 1.0 {
            g = 1.0;
        }
        self.limb_distance
            .set(self.limb_distance.get() + (g - self.limb_distance.get()) * 0.4);
        self.limb_angle.set(self.limb_angle.get() + self.limb_distance.get());
    }

    /// `fun getCapeTexture()`: load once, then the handler's cape or the
    /// skin's own.
    pub fn get_cape_texture(&self) -> Option<Texture> {
        if !self.cape_loaded.get() {
            self.cape_loaded.set(true);
            PlayerHandler::on_load_texture(&self.game_profile);
        }
        let handler = PlayerHandler::from_profile(&self.game_profile);
        if handler.borrow().has_cape {
            Some(handler.borrow_mut().get_cape())
        } else {
            self.skin.borrow().cape.clone()
        }
    }

    /// `fun getElytraTexture()`.
    pub fn get_elytra_texture(&self) -> Texture {
        let handler = PlayerHandler::from_profile(&self.game_profile);
        let cape_texture = self.get_cape_texture();
        if handler.borrow().has_elytra_texture
            && Capes::config().enable_elytra_texture
            && cape_texture.is_some()
        {
            cape_texture.unwrap()
        } else {
            Texture::ResourceTexture(Identifier::with_default_namespace(
                "entity/equipment/wings/elytra",
            ))
        }
    }

    /// `fun getSkinTextures()`.
    pub fn get_skin_textures(&self) -> PlayerSkin {
        let skin = self.skin.borrow().clone();
        PlayerSkin {
            cape: self.get_cape_texture(),
            elytra: self.get_elytra_texture(),
            ..skin
        }
    }
}
