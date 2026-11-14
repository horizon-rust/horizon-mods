//! Exact 1:1 Rust translation of the mixin set (19 mixins, 1661 upstream
//! lines) from tr7zw/FirstPersonModel @6b5859f, MIT.
//!
//! Carried preprocessor variant: >= 1.21.9 / >= 26.1 / >= 26.3 (the Horizon
//! 26.3 tree). Upstream preprocessor-disabled variants stay as comments.
//! Translation by sparticle. Carries the upstream MIT license and credit.
//!
//! Map (upstream mixin -> here):
//!   EntityAccessor            -> entity_accessor_*
//!   AgeableListModelMixin     -> empty on the carried variant
//!   PlayerModelMixin          -> empty on the carried variant
//!   PlayerMixin               -> player_mixin_*
//!   CustomHeadLayerMixin      -> custom_head_layer_submit
//!   StuckInBodyLayerMixin     -> stuck_in_body_layer_submit
//!   HumanoidModelMixin        -> humanoid_model_setup_anim
//!   ElytraLayerMixin          -> elytra_layer_submit
//!   ModelPartMixin            -> ModelPartBase impl + model_part_set_pivot
//!   LivingEntityRenderStateMixin -> the seams LivingEntityRenderStateAccess impls
//!   FeatureRendererMixin      -> disabled on the carried variant (dummy class)
//!   HeldItemFeatureRendererMixin -> held_item_feature_renderer_submit_arm_with_item
//!   ArmorFeatureRendererMixin -> body commented upstream (dummy class)
//!   FishingBobberRendererMixin -> FishingBobberRendererMixin
//!   PlayerRendererMixin       -> AvatarRendererMixin
//!   RenderDispatcherMixin     -> fpm_extract_*/fpm_submit_*
//!   WorldRendererMixin        -> extract_visible_entities
//!   LivingEntityRendererMixin -> living_entity_renderer_check_if_camera_entity
//!   HeldItemRendererMixin     -> held_item_renderer_* (>= 26.3)

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use crate::access::LivingEntityRenderStateAccess;
use crate::first_person_model_core::FirstPersonModelCore;
use crate::inventory_util::InventoryUtil;
use crate::logic_handler::LogicHandler;
use crate::mixinbase::ModelPartBase;
use crate::RUNTIME;
use crate::seams::{
    self, items, CameraRenderState, CameraType, DeltaTracker, Entity, EntityUtil, EquipmentSlot,
    FishingHookRenderState, Frustum, HumanoidArm, HumanoidModel, InteractionHand, ItemStack,
    LivingEntityRenderState, PoseStack, SubmitNodeCollector, Vec3, AvatarRenderState,
    HumanoidRenderState,
    FirstPersonHandsAndItemsRenderState, LevelRenderState, PlayerRenderState,
};

// ---------------------------------------------------------------------------
// EntityAccessor
// ---------------------------------------------------------------------------

// EntityAccessor.java: @Accessor("position") on Entity.
/// The LogicHandler instance accessor all the mixins share.
fn logic_handler() -> Rc<LogicHandler> {
    FirstPersonModelCore::instance()
        .logic_handler
        .borrow()
        .clone()
        .expect("logic handler")
}

pub fn entity_accessor_get_raw_position(entity: &Rc<RefCell<Entity>>) -> Vec3 {
    entity.borrow().position.clone()
}

pub fn entity_accessor_set_raw_position(entity: &Rc<RefCell<Entity>>, position: Vec3) {
    entity.borrow_mut().position = position;
}

// ---------------------------------------------------------------------------
// AgeableListModelMixin
// ---------------------------------------------------------------------------

// >= 1.21.3 variant is an empty class: nothing to translate.

// ---------------------------------------------------------------------------
// PlayerModelMixin
// ---------------------------------------------------------------------------

// >= 1.21.3 variant has no cloak shadow: nothing to translate.

// ---------------------------------------------------------------------------
// PlayerMixin (@Mixin(LivingEntity.class), >= 1.21.5)
// ---------------------------------------------------------------------------

/// getItemBySlot @Inject(HEAD, cancellable). Some(...) = ci.setReturnValue.
pub fn player_mixin_get_item_by_slot(player: &Entity, slot: EquipmentSlot) -> Option<ItemStack> {
    let core = FirstPersonModelCore::instance();
    let rt = RUNTIME.with(|rt| rt.clone());
    let mc_player = rt.client.borrow().get_player();
    if core.is_rendering_player() && rt.loader.same_thread.get() {
        if slot == EquipmentSlot::Head {
            return Some(ItemStack::EMPTY.clone());
        }
        if (slot == EquipmentSlot::Mainhand || slot == EquipmentSlot::Offhand)
            && mc_player
                .as_ref()
                .map_or(false, |mc_player| {
                    let mc_player = mc_player.borrow().clone();
                    logic_handler().hide_arms_and_items_full(
                        &mc_player,
                        InventoryUtil::get_selected(InventoryUtil::get_inventory(player)),
                        InventoryUtil::get_offhand(InventoryUtil::get_inventory(player)),
                    )
                })
        {
            return Some(ItemStack::EMPTY.clone());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// CustomHeadLayerMixin (priority 100, lower prio to run before other mods)
// ---------------------------------------------------------------------------

/// submit @Inject(HEAD, cancellable). true = ci.cancel().
pub fn custom_head_layer_submit(
    living_entity_render_state: &LivingEntityRenderState,
) -> bool {
    living_entity_render_state.is_camera_entity()
}

// ---------------------------------------------------------------------------
// StuckInBodyLayerMixin (@author KxmischesDomi)
// ---------------------------------------------------------------------------

/// submit @Inject(HEAD, cancellable). true = ci.cancel().
pub fn stuck_in_body_layer_submit(avatar_render_state: &AvatarRenderState) -> bool {
    avatar_render_state.base.is_camera_entity()
        && !FirstPersonModelCore::instance().config().render_stuck_features
}

// ---------------------------------------------------------------------------
// HumanoidModelMixin
// ---------------------------------------------------------------------------

/// setupAnim @Inject(TAIL). Shadow fields leftArm/rightArm/head/body.
pub fn humanoid_model_setup_anim(model: &HumanoidModel, render_state: &HumanoidRenderState) {
    // head.visible = !access.isCameraEntity(); (commented upstream, stays)
    if let Some(head) = &model.head {
        if render_state.base.is_camera_entity() {
            head.set_hidden();
        } else {
            head.show_again();
        }
    }
    if render_state.base.is_spectator {
        // Do not touch the body in spectator mode
        return;
    }
    if let Some(left_arm) = &model.left_arm {
        let mut arm = left_arm.borrow_mut();
        arm.hidden = !render_state.base.hide_left_arm();
        arm.x_rot += render_state.base.get_arm_offset();
    }
    if let Some(right_arm) = &model.right_arm {
        let mut arm = right_arm.borrow_mut();
        arm.hidden = !render_state.base.hide_right_arm();
        arm.x_rot += render_state.base.get_arm_offset();
    }
    if let Some(body) = &model.body {
        body.borrow_mut().hidden = !render_state.base.hide_body();
    }
}

// ---------------------------------------------------------------------------
// ElytraLayerMixin (@Mixin(WingsLayer.class) on >= 1.21.3; @author KxmischesDomi)
// ---------------------------------------------------------------------------

/// submit @Inject(HEAD, cancellable). true = ci.cancel().
pub fn elytra_layer_submit(humanoid_render_state: &HumanoidRenderState) -> bool {
    humanoid_render_state.base.is_camera_entity() && humanoid_render_state.base.is_visually_swimming
}

// ---------------------------------------------------------------------------
// ModelPartMixin
// ---------------------------------------------------------------------------

impl ModelPartBase for Rc<RefCell<seams::ModelPart>> {
    fn set_hidden(&self) {
        let mut part = self.borrow_mut();
        if !part.moved {
            part.z_copy = part.z;
        }
        part.z = 5000.0;
        part.moved = true;
        part.hidden = false; // visible = false
    }

    fn show_again(&self) {
        let mut part = self.borrow_mut();
        if part.moved {
            part.z = part.z_copy;
            part.moved = false;
            part.hidden = true; // visible = true
        }
    }

    fn is_hidden(&self) -> bool {
        self.borrow().moved
    }
}

/// setPivot @Inject(RETURN).
pub fn model_part_set_pivot(part: &Rc<RefCell<seams::ModelPart>>, _x: f32, _y: f32, z: f32) {
    let mut p = part.borrow_mut();
    if p.moved {
        p.z_copy = z;
        p.z = 5000.0;
    }
}

// copyTransform @Inject(RETURN) is the < 1.21.9 variant only (commented upstream).

// ---------------------------------------------------------------------------
// FeatureRendererMixin
// ---------------------------------------------------------------------------

// Disabled on >= 1.21.9 (preprocessor turns the class into a Minecraft dummy):
// nothing to translate.

// ---------------------------------------------------------------------------
// HeldItemFeatureRendererMixin (@Mixin(ItemInHandLayer.class))
// ---------------------------------------------------------------------------

/// submitArmWithItem @Inject(HEAD, cancellable). true = ci.cancel().
/// Stops items in the hand from rendering while in first person.
pub fn held_item_feature_renderer_submit_arm_with_item(
    armed_entity_render_state: &seams::ArmedEntityRenderState,
) -> bool {
    let core = FirstPersonModelCore::instance();
    armed_entity_render_state.base.hide_left_arm()
        && armed_entity_render_state.base.hide_right_arm()
        && !logic_handler().looking_down_state(&armed_entity_render_state.base)
}

// ---------------------------------------------------------------------------
// ArmorFeatureRendererMixin
// ---------------------------------------------------------------------------

// The whole body is commented out upstream (the old < 1.21.9 path): the class
// is an empty shell, nothing to translate.

// ---------------------------------------------------------------------------
// FishingBobberRendererMixin (@Mixin(FishingHookRenderer.class))
// ---------------------------------------------------------------------------

pub struct FishingBobberRendererMixin {
    // to not create @Nullable
    pub(crate) offsetvec3d: RefCell<Vec3>,
}

impl FishingBobberRendererMixin {
    pub fn new() -> Self {
        Self { offsetvec3d: RefCell::new(Vec3::ZERO) }
    }

    fn do_correct(&self) -> bool {
        FirstPersonModelCore::instance().is_enabled()
            && RUNTIME
                .with(|rt| rt.client.borrow().camera_type == CameraType::FirstPerson)
            && !logic_handler().hide_arms_and_items()
    }

    /// @ModifyExpressionValue(method = "getPlayerHandPos", getCameraType).
    pub fn redirect(&self, camera_type: CameraType) -> CameraType {
        if self.do_correct() {
            CameraType::ThirdPersonBack
        } else {
            camera_type
        }
    }

    /// submit @Inject(HEAD), >= 26.1 signature.
    pub fn render(
        &self,
        _fishing_hook_render_state: &FishingHookRenderState,
        _pose_stack: &PoseStack,
        _submit_node_collector: &SubmitNodeCollector,
        _camera_render_state: &CameraRenderState,
    ) {
        let core = FirstPersonModelCore::instance();
        if core.is_rendering_player() {
            // getPositionOffset((Player) fishingBobberEntity.getOwner(), matrixStack);
            *self.offsetvec3d.borrow_mut() = logic_handler().get_offset();
        } else {
            *self.offsetvec3d.borrow_mut() = Vec3::new(0.0, 0.0, 0.0);
        }
    }

    /// getPlayerHandPos @Inject(RETURN, cancellable): adds the offset to the
    /// returned hand position.
    pub fn get_player_hand_pos_offset(&self, return_value: Vec3) -> Vec3 {
        let offset = *self.offsetvec3d.borrow();
        Vec3::new(
            return_value.x + offset.x,
            return_value.y + offset.y,
            return_value.z + offset.z,
        )
    }
}

// ---------------------------------------------------------------------------
// PlayerRendererMixin (@Mixin(AvatarRenderer.class, priority = 500) on
// >= 1.21.9; @author tr7zw. "Offset the player behind the camera")
// ---------------------------------------------------------------------------

pub struct AvatarRendererMixin {
    /// this.layers, as the layers' class names (the hiddenLayers lookup).
    pub layers: RefCell<Vec<String>>,
    pub removed_layers: RefCell<Vec<String>>,
    // fpmMcInstance = Minecraft.getInstance(); read through RUNTIME.
}

impl AvatarRendererMixin {
    pub fn new(layers: Vec<String>) -> Self {
        Self {
            layers: RefCell::new(layers),
            removed_layers: RefCell::new(Vec::new()),
        }
    }

    /// extractRenderState @Inject(TAIL) (>= 1.21.9 branch).
    pub fn extract_render_state(
        &self,
        avatar: &Entity,
        avatar_render_state: &mut AvatarRenderState,
        _delta: f32,
    ) {
        let core = FirstPersonModelCore::instance();
        let lh = logic_handler();
        if !avatar_render_state.base.is_camera_entity() {
            return;
        }
        if lh.hide_arms_and_items_living(avatar) {
            avatar_render_state.base.set_hide_arms(true);
        } else if lh.dynamic_hands_enabled() {
            avatar_render_state
                .base
                .set_arm_offset(seams::mth::clamp(-EntityUtil::get_x_rot(avatar) / 20.0 + 2.0, -0.0, 0.7));
            if !lh.looking_down_living(avatar) {
                // TODO DYNAMIC HAND
                if !avatar.off_hand.is_empty()
                    || avatar.main_hand.item.id == items::FILLED_MAP
                {
                    avatar_render_state.base.set_hide_left_arm(true);
                }
                if !avatar.main_hand.is_empty() {
                    avatar_render_state.base.set_hide_right_arm(true);
                }
            }
        }
        if avatar.swimming {
            avatar_render_state.base.set_hide_body(true);
            avatar_render_state.show_cape = false;
        }
        if lh.hide_arms_and_items_full(
            avatar,
            avatar.main_hand.clone(),
            avatar.off_hand.clone(),
        ) {
            avatar_render_state.base.set_hide_arms(true);
        }
        // avatarRenderState.hitboxesRenderState = null; is the < 1.21.11 line
        // only (commented upstream).
    }

    /// getRenderLayers override: the live layers plus the ones removed for
    /// first person.
    pub fn get_render_layers(&self) -> Vec<String> {
        let mut layers = self.layers.borrow().clone();
        layers.extend(self.removed_layers.borrow().iter().cloned());
        layers
    }

    /// updatePartsList override.
    pub fn update_parts_list(&self, thirdperson: bool, hidden_layers: &HashSet<String>) {
        {
            let removed = self.removed_layers.borrow().clone();
            self.layers.borrow_mut().extend(removed);
        }
        self.removed_layers.borrow_mut().clear();
        if thirdperson {
            return;
        }
        let mut removed = Vec::new();
        for layer in self.layers.borrow().iter() {
            if hidden_layers.contains(layer) {
                removed.push(layer.clone());
            }
        }
        for layer in removed {
            self.removed_layers.borrow_mut().push(layer.clone());
            let mut layers = self.layers.borrow_mut();
            if let Some(pos) = layers.iter().position(|l| *l == layer) {
                layers.remove(pos);
            }
        }
    }

}

// ---------------------------------------------------------------------------
// RenderDispatcherMixin (@Mixin(EntityRenderDispatcher.class))
// ---------------------------------------------------------------------------

thread_local! {
    static FPM_SUBMITTING_CAMERA_ENTITY: Cell<bool> = const { Cell::new(false) };
}

/// fpmExtractStart @Inject(HEAD) on extractEntity.
pub fn fpm_extract_start(entity: &Rc<RefCell<Entity>>, _f: f32) {
    RUNTIME.with(|rt| {
        let client = rt.client.borrow();
        let is_camera_entity = client
            .camera_entity
            .as_ref()
            .map_or(false, |camera| Rc::ptr_eq(camera, entity));
        if is_camera_entity
            && client.camera_type == CameraType::FirstPerson
            && logic_handler().should_apply_third_person(false)
        {
            rt.loader.camera_entity_extract.set(true);
        }
    });
}

/// fpmExtractEnd @Inject(TAIL) on extractEntity.
pub fn fpm_extract_end() {
    RUNTIME.with(|rt| rt.loader.camera_entity_extract.set(false));
}

/// fpmSubmitStart @Inject(HEAD) on submit.
pub fn fpm_submit_start(render_state: &LivingEntityRenderState) {
    if render_state.is_camera_entity() {
        FPM_SUBMITTING_CAMERA_ENTITY.with(|c| c.set(true));
        FirstPersonModelCore::instance().set_rendering_player(true);
    }
}

/// fpmSubmitEnd @Inject(TAIL) on submit.
pub fn fpm_submit_end() {
    if FPM_SUBMITTING_CAMERA_ENTITY.with(|c| c.get()) {
        FPM_SUBMITTING_CAMERA_ENTITY.with(|c| c.set(false));
        FirstPersonModelCore::instance().set_rendering_player(false);
    }
}

// The renderHitbox injections are the < 1.21.9 branches only (commented upstream).

// ---------------------------------------------------------------------------
// WorldRendererMixin (@Mixin(LevelExtractor.class) on >= 26.2; "Detects when
// the player is rendered and triggers the correct changes")
// ---------------------------------------------------------------------------

/// The >= 1.21.9 @Shadow extractEntity hook the engine provides.
pub type ExtractEntityFn<'a> = &'a dyn Fn(&Rc<RefCell<Entity>>, f32) -> LivingEntityRenderState;

/// extractVisibleEntities @Inject(HEAD).
pub fn extract_visible_entities(
    camera: &seams::Camera,
    _frustum: &Frustum,
    delta_tracker: &DeltaTracker,
    level_render_state: &LevelRenderState,
    extract_entity: ExtractEntityFn,
) {
    let matrices = PoseStack::new();
    let core = FirstPersonModelCore::instance();
    if camera.is_detached() || !logic_handler().should_apply_third_person(false) {
        core.set_rendering_player(false);
        return;
    }
    let vec3d = camera.position();
    // Store position and apply offset
    let ent = match camera.entity() {
        Some(ent) => ent,
        None => return,
    };
    let pos = ent.borrow().position.clone();
    let (x_o, y_o, z_o, x_old, y_old, z_old) = {
        let e = ent.borrow();
        (e.xo, e.yo, e.zo, e.x_old, e.y_old, e.z_old)
    };
    let tick_delta = delta_tracker.get_game_time_delta_partial_tick(true);
    let offset = {
        let lh = core.logic_handler.borrow().clone().expect("logic handler");
        lh.update_position_offset(&ent, tick_delta);
        lh.get_offset()
    };
    {
        let mut e = ent.borrow_mut();
        e.position = Vec3::new(pos.x + offset.x, pos.y + offset.y, pos.z + offset.z);
        e.xo += offset.x;
        e.yo += offset.y;
        e.zo += offset.z;
        e.x_old += offset.x;
        e.y_old += offset.y;
        e.z_old += offset.z;
    }
    // Trigger render
    let state = Rc::new(RefCell::new(extract_entity(&ent, tick_delta)));
    level_render_state.entity_render_states.borrow_mut().push(state);
    // Restore position
    ent.borrow_mut().position = pos;
    ent.borrow_mut().refresh_dimensions(); // Fix corrupted state caused during rendering by other mods (NEA apparently)
    {
        let mut e = ent.borrow_mut();
        e.xo = x_o;
        e.yo = y_o;
        e.zo = z_o;
        e.x_old = x_old;
        e.y_old = y_old;
        e.z_old = z_old;
    }
    core.set_rendering_player(false);
    core.set_rendering_player_post(false);
}

// ---------------------------------------------------------------------------
// LivingEntityRendererMixin
// ---------------------------------------------------------------------------

/// checkIfCameraEntity @Inject(HEAD) on extractRenderState (>= 1.21.9 branch;
/// the < 1.21.9 render-pull block is commented upstream, as is @Shadow
/// getModel()).
pub fn living_entity_renderer_check_if_camera_entity(
    living_entity_render_state: &mut LivingEntityRenderState,
) {
    // Raw flag: only states extracted by FirstPerson itself are marked, so
    // third party captures (RealCamera, GUIs) keep rendering the head.
    living_entity_render_state
        .set_is_camera_entity(FirstPersonModelCore::instance().is_rendering_player_raw());
}

// ---------------------------------------------------------------------------
// HeldItemRendererMixin (@Mixin(FirstPersonHandsAndItemsRenderer.class) on
// >= 26.3; "Hides the normal first person hands")
// ---------------------------------------------------------------------------

/// The >= 26.3 @Shadow renderPlayerArm hook the engine provides.
pub type RenderPlayerArmFn<'a> = &'a dyn Fn(
    &mut PoseStack,
    &SubmitNodeCollector,
    i32,
    f32,
    f32,
    HumanoidArm,
    &PlayerRenderState,
);

/// submitArmWithItem @Inject(HEAD, cancellable) (>= 26.3 branch). true =
/// ci.cancel().
#[allow(clippy::too_many_arguments)]
pub fn held_item_renderer_render_first_person_item(
    player_state: &PlayerRenderState,
    _state: &FirstPersonHandsAndItemsRenderState,
    _partial_ticks: f32,
    pitch: f32,
    hand: InteractionHand,
    attack: f32,
    item: &ItemStack,
    inverse_arm_height: f32,
    matrices: &mut PoseStack,
    submit_node_collector: &SubmitNodeCollector,
    light_coords: i32,
    render_player_arm: RenderPlayerArmFn,
) -> bool {
    let core = FirstPersonModelCore::instance();
    if !core.is_enabled() {
        return false;
    }
    if !logic_handler().show_vanilla_hands() {
        return true;
    }
    let mut player: Option<Rc<RefCell<Entity>>> = None;
    if player_state.has_player {
        if let Some(p) = &player_state.avatar_render_state.base.transition_entity {
            if p.borrow().is_player {
                player = Some(p.clone());
            }
        }
    }
    let player = match player {
        Some(player) => player,
        None => return false,
    };
    let (main_arm, is_invisible) = {
        let e = player.borrow();
        if core.config().vanilla_hands_skip_swimming && logic_handler().is_swimming(&e) {
            // while actively swimming, dont show the offhand item
            return true;
        }
        // filter out vanilla hands with no item
        if logic_handler().dynamic_hands_enabled() && pitch > 35.0 {
            // item held too low, hide
            return true;
        }
        // double hands
        if core.config().vanilla_hands_mode != crate::config::VanillaHands::AllDouble
            || e.main_hand.item.id == items::FILLED_MAP
            || e.scoping
        {
            return false;
        }
        (e.main_arm, e.is_invisible)
    };
    let bl = hand == InteractionHand::MainHand;
    let arm = if bl {
        main_arm
    } else {
        main_arm.opposite()
    };
    // matrices.pushPose();
    matrices.push_pose();
    if item.is_empty() && !bl && !is_invisible {
        render_player_arm(
            matrices,
            submit_node_collector,
            light_coords,
            inverse_arm_height,
            attack,
            arm,
            player_state,
        );
    }
    matrices.pop_pose();
    false
}

/// modifyInverseArmHeight @ModifyVariable(method = "submitArmWithItem",
/// at = @At("HEAD"), index = 8).
pub fn modify_inverse_arm_height(inverse_arm_height: f32) -> f32 {
    let core = FirstPersonModelCore::instance();
    let lh = logic_handler();
    if core.is_enabled() && lh.show_vanilla_hands() && lh.dynamic_hands_enabled() {
        let local_player = RUNTIME.with(|rt| rt.client.borrow().get_player());
        if let Some(local_player) = local_player {
            let f = EntityUtil::get_x_rot(&local_player.borrow());
            if f > 15.0 {
                if f < 30.0 {
                    return f32::max(inverse_arm_height, 1.0 - (15.0 / f));
                }
                if f < 35.0 {
                    return f32::max(inverse_arm_height, 0.5 + ((f - 30.0) / 5.0) * 0.7);
                }
                return 1.2;
            }
        }
    }
    inverse_arm_height
}

// The < 26.3 submitArmWithItem/tick branches stay commented out upstream.
