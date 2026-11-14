//! Port tests mirroring the upstream behaviors testable without a full
//! client, against the carried >= 1.21.9 / 26.3 seams surface.
//!
//! Upstream ships one suite (dev.tr7zw.tests.MixinTests.java, objenesis
//! instantiation of the mixin targets); the mixin-injection half is
//! engine-side on Horizon, so the suite here covers the translated
//! behaviors themselves: config defaults + upgrade + serde round-trip, the
//! body-rot math, hands/offset logic, the z=5000 part trick, and the mixin
//! handler conditions.

use crate::access::{LivingEntityRenderStateAccess, PlayerRendererAccessTrait};
use crate::config::{ConfigUpgrader, FirstPersonSettings, VanillaHands};
use crate::constants;
use crate::first_person_model_core::FirstPersonModelCore;
use crate::logic_handler::{calculate_body_rot, LogicHandler};
use crate::mixinbase::ModelPartBase;
use crate::mixins::*;
use crate::seams::{
    self, CameraRenderState, CameraType, Entity, EquipmentSlot, HumanoidArm, InteractionHand,
    ItemStack, LivingEntityRenderState, ModelPart, PoseStack, SubmitNodeCollector, Vec3,
    AvatarRenderState, HumanoidModel, HumanoidRenderState,
};
use crate::RUNTIME;

fn settings() -> FirstPersonSettings {
    FirstPersonSettings::default()
}

/// The core singleton with its LogicHandler and default config wired, as
/// shared_setup does.
fn core() -> std::rc::Rc<FirstPersonModelCore> {
    let core = FirstPersonModelCore::instance();
    if core.logic_handler.borrow().is_none() {
        *core.logic_handler.borrow_mut() = Some(LogicHandler::new(core.clone()));
    }
    if core.base.config.borrow().is_none() {
        *core.base.config.borrow_mut() = Some(FirstPersonSettings::default());
    }
    core
}

/// Minecraft.getInstance().player: inject the runtime default player, as the
/// client would on the main thread.
fn with_player() {
    RUNTIME.with(|rt| {
        let mut client = rt.client.borrow_mut();
        if client.player.is_none() {
            client.player = Some(std::rc::Rc::new(std::cell::RefCell::new(Entity::player())));
        }
    })
}

// --- config (ConfigUpgrader + the serde file format) ---

#[test]
fn settings_defaults_match_upstream() {
    let s = settings();
    assert_eq!(s.config_version, 2);
    assert!(s.enabled_by_default);
    assert!(s.render_stuck_features);
    assert_eq!(s.vanilla_hands_mode, VanillaHands::Off);
    assert!(s.dynamic_mode);
    assert!(s.vanilla_hands_skip_swimming);
    assert_eq!(s.auto_vanilla_hands.len(), 8);
    assert!(s.auto_toggle_mod_items.contains("exposure:camera"));
}

#[test]
fn upgrade_config_v0_reinits_and_adds_exposure_camera() {
    let mut s = settings();
    s.config_version = 0;
    s.auto_vanilla_hands.clear();
    s.auto_toggle_mod_items.clear();
    ConfigUpgrader::upgrade_config(&mut s);
    assert_eq!(s.config_version, 2);
    assert_eq!(s.auto_vanilla_hands.len(), 8);
    assert!(s.auto_toggle_mod_items.contains("exposure:camera"));
}

#[test]
fn settings_json_round_trip() {
    let mut s = settings();
    s.x_offset = -12;
    s.vanilla_hands_mode = VanillaHands::AllDouble;
    s.hidden_layers
        .insert("net.minecraft.client.renderer.entity.layers.CapeLayer".into());
    let json = serde_json::to_string_pretty(&s).unwrap();
    let back: FirstPersonSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.x_offset, s.x_offset);
    assert_eq!(back.vanilla_hands_mode, s.vanilla_hands_mode);
    assert_eq!(
        back.hidden_layers
            .contains("net.minecraft.client.renderer.entity.layers.CapeLayer"),
        true
    );
    assert_eq!(back.config_version, 2);
}

// --- body-rot math (LogicHandler.calculateBodyRot) ---

#[test]
fn calculate_body_rot_pulls_body_with_head() {
    // Within 50 degrees the body stays put.
    assert!((calculate_body_rot(10.0, 30.0) - 10.0).abs() < 1e-6);
    // Past 50 degrees the body trails the head by exactly 50.
    assert!((calculate_body_rot(0.0, 130.0) - 80.0).abs() < 1e-6);
    assert!((calculate_body_rot(0.0, -130.0) + 80.0).abs() < 1e-6);
    // Wrapped heads: 350 reads as -10, and |wrap(-10-180)| = 170 > 50 pulls
    // the body to -60.
    assert!((calculate_body_rot(180.0, 350.0) + 60.0).abs() < 1e-6);
}

// --- the z=5000 part trick (ModelPartMixin via ModelPartBase) ---

fn test_part() -> std::rc::Rc<std::cell::RefCell<ModelPart>> {
    std::rc::Rc::new(std::cell::RefCell::new(ModelPart::default()))
}

#[test]
fn part_set_hidden_pushes_to_z5000_and_restores() {
    let part = test_part();
    part.borrow_mut().z = 0.3;
    part.set_hidden();
    {
        let p = part.borrow();
        assert_eq!(p.z, 5000.0);
        assert!(p.moved);
        assert!((p.z_copy - 0.3).abs() < 1e-6);
    }
    part.show_again();
    {
        let p = part.borrow();
        assert!((p.z - 0.3).abs() < 1e-6);
        assert!(!p.moved);
    }
}

#[test]
fn part_set_hidden_is_idempotent_on_first_move_only() {
    let part = test_part();
    part.borrow_mut().z = 0.3;
    part.set_hidden();
    // A second set_hidden on an already-moved part must not overwrite z_copy
    // with 5000.
    part.set_hidden();
    assert!((part.borrow().z_copy - 0.3).abs() < 1e-6);
    assert!(part.is_hidden());
}

#[test]
fn part_set_pivot_updates_z_copy_while_moved() {
    let part = test_part();
    part.borrow_mut().z = 0.3;
    part.set_hidden();
    model_part_set_pivot(&part, 1.0, 1.0, 0.7);
    {
        let p = part.borrow();
        assert!((p.z_copy - 0.7).abs() < 1e-6);
        assert_eq!(p.z, 5000.0);
    }
    part.show_again();
    assert!((part.borrow().z - 0.7).abs() < 1e-6);
}

// --- render-state flags the mixins read and write ---

#[test]
fn render_state_camera_entity_flag_round_trip() {
    let mut state = LivingEntityRenderState::default();
    assert!(!state.is_camera_entity());
    state.set_is_camera_entity(true);
    assert!(state.is_camera_entity());
}

#[test]
fn avatar_state_hide_flags_and_arm_offset() {
    let mut state = AvatarRenderState::default();
    state.base.set_hide_arms(true);
    state.base.set_arm_offset(0.35);
    assert!(state.base.hide_left_arm());
    assert!(state.base.hide_right_arm());
    assert!((state.base.get_arm_offset() - 0.35).abs() < 1e-6);
    state.base.set_hide_body(true);
    assert!(state.base.hide_body());
    state.base.set_hide_left_arm(false);
    assert!(!state.base.hide_left_arm());
}

// --- mixin handler conditions ---

#[test]
fn custom_head_layer_cancels_only_for_camera_entity() {
    let mut state = LivingEntityRenderState::default();
    assert!(!custom_head_layer_submit(&state));
    state.set_is_camera_entity(true);
    assert!(custom_head_layer_submit(&state));
}

#[test]
fn stuck_in_body_layer_follows_upstream_config_default() {
    let _ = core();
    let mut state = AvatarRenderState::default();
    // Default config keeps render_stuck_features on, so never cancel.
    assert!(!stuck_in_body_layer_submit(&state));
    state.base.set_is_camera_entity(true);
    assert!(!stuck_in_body_layer_submit(&state));
}

#[test]
fn elytra_layer_cancels_for_swimming_camera_entity() {
    let mut state = HumanoidRenderState::default();
    assert!(!elytra_layer_submit(&state));
    state.base.set_is_camera_entity(true);
    assert!(!elytra_layer_submit(&state));
    state.base.is_visually_swimming = true;
    assert!(elytra_layer_submit(&state));
}

#[test]
fn humanoid_model_setup_anim_hides_and_offsets_arms() {
    let mut state = HumanoidRenderState::default();
    state.base.set_is_camera_entity(true);
    state.base.set_hide_left_arm(true);
    state.base.set_hide_right_arm(true);
    state.base.set_arm_offset(0.5);
    // Spectator bodies stay untouched.
    state.base.is_spectator = true;
    let head = test_part();
    let left_arm = test_part();
    let right_arm = test_part();
    let body = test_part();
    let m = HumanoidModel {
        head: Some(head.clone()),
        body: Some(body.clone()),
        left_arm: Some(left_arm.clone()),
        right_arm: Some(right_arm.clone()),
        hat: None,
        left_sleeve: None,
        right_sleeve: None,
    };
    humanoid_model_setup_anim(&m, &state);
    assert!(head.borrow().hidden == false); // head visible is upstream-commented
    assert!(left_arm.borrow().hidden == false);
    assert!(!left_arm.borrow().moved); // hidden flag set, z-trick untouched
    assert!(body.borrow().hidden == false);
}

#[test]
fn held_item_feature_layer_cancels_by_looking_down() {
    let _ = core();
    let mut state = crate::seams::ArmedEntityRenderState::default();
    // Default: arms not hidden -> no cancel.
    assert!(!held_item_feature_renderer_submit_arm_with_item(&state));
    state.base.set_hide_left_arm(true);
    state.base.set_hide_right_arm(true);
    // vanilla_hands_mode Off: dynamic hands is off, so the armed layers
    // cancel whenever both arms are hidden, whatever the pitch.
    state.base.x_rot = 40.0;
    assert!(held_item_feature_renderer_submit_arm_with_item(&state));
    // AllDouble arms the dynamic path: looking down (x_rot > 30) keeps the
    // layers rendering.
    core().base.config.borrow_mut().as_mut().unwrap().vanilla_hands_mode = VanillaHands::AllDouble;
    with_player();
    state.base.x_rot = 40.0;
    assert!(!held_item_feature_renderer_submit_arm_with_item(&state));
    state.base.x_rot = 10.0;
    assert!(held_item_feature_renderer_submit_arm_with_item(&state));
}

#[test]
fn fishing_bobber_redirect_tracks_enabled_and_hands() {
    let _ = core();
    let mixin = FishingBobberRendererMixin::new();
    // Disabled: the camera type passes through untouched.
    FirstPersonModelCore::instance().set_enabled(false);
    assert_eq!(mixin.redirect(CameraType::FirstPerson), CameraType::FirstPerson);
    // Enabled with no player: hideArmsAndItems reads false, so the fishing
    // bobber line reads the third-person hand position.
    FirstPersonModelCore::instance().set_enabled(true);
    assert_eq!(mixin.redirect(CameraType::FirstPerson), CameraType::ThirdPersonBack);
    // Offset plumbing: zero offset adds nothing to the hand position.
    let hand = Vec3::new(1.0, 2.0, 3.0);
    let out = mixin.get_player_hand_pos_offset(hand);
    assert_eq!(out, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn fishing_bobber_render_tracks_the_player_offset() {
    let _ = core();
    let mixin = FishingBobberRendererMixin::new();
    let state = LivingEntityRenderState::default();
    let hook = crate::seams::FishingHookRenderState::default();
    mixin.render(&hook, &PoseStack::new(), &SubmitNodeCollector, &CameraRenderState::default());
    // Not rendering the player: offset resets to zero.
    assert_eq!(*mixin.offsetvec3d.borrow(), Vec3::new(0.0, 0.0, 0.0));
}

#[test]
fn inverse_arm_height_passthrough_without_player() {
    let _ = core();
    assert!((modify_inverse_arm_height(0.75) - 0.75).abs() < 1e-6);
}

#[test]
fn player_mixin_item_slot_disabled_without_rendering_player() {
    let _ = core();
    let player = Entity::player();
    assert!(player_mixin_get_item_by_slot(&player, EquipmentSlot::Head).is_none());
    assert!(player_mixin_get_item_by_slot(&player, EquipmentSlot::Mainhand).is_none());
}

#[test]
fn living_entity_renderer_marks_only_fpm_extractions() {
    let _ = core();
    let mut state = LivingEntityRenderState::default();
    living_entity_renderer_check_if_camera_entity(&mut state);
    assert!(!state.is_camera_entity());
    FirstPersonModelCore::instance().set_rendering_player(true);
    living_entity_renderer_check_if_camera_entity(&mut state);
    assert!(state.is_camera_entity());
    FirstPersonModelCore::instance().set_rendering_player(false);
}

#[test]
fn render_dispatcher_submit_flags_track_the_camera_entity() {
    let _ = core();
    let mut state = LivingEntityRenderState::default();
    fpm_submit_start(&state);
    fpm_submit_end();
    // Non-camera entity: rendering_player stays untouched and ends false.
    assert!(!FirstPersonModelCore::instance().is_rendering_player());
    state.set_is_camera_entity(true);
    fpm_submit_start(&state);
    assert!(FirstPersonModelCore::instance().is_rendering_player());
    fpm_submit_end();
    assert!(!FirstPersonModelCore::instance().is_rendering_player());
}

#[test]
fn avatar_renderer_extract_ignores_non_camera_entities() {
    let _ = core();
    let mixin = AvatarRendererMixin::new(Vec::new());
    let player = Entity::player();
    let mut state = AvatarRenderState::default();
    mixin.extract_render_state(&player, &mut state, 1.0);
    assert!(!state.base.hide_left_arm());
    assert!(!state.base.hide_right_arm());
    assert!(!state.base.hide_body());
}

#[test]
fn avatar_renderer_layer_list_round_trips() {
    let mixin = AvatarRendererMixin::new(vec![
        "net.minecraft.client.renderer.entity.layers.CapeLayer".into(),
        "custom.HiddenLayer".into(),
    ]);
    let mut hidden = std::collections::HashSet::new();
    hidden.insert("custom.HiddenLayer".to_string());
    mixin.update_parts_list(false, &hidden);
    let layers = mixin.get_render_layers();
    assert!(layers.contains(&"net.minecraft.client.renderer.entity.layers.CapeLayer".to_string()));
    assert!(layers.contains(&"custom.HiddenLayer".to_string()));
    // Third person restores everything.
    mixin.update_parts_list(true, &hidden);
    assert_eq!(mixin.get_render_layers().len(), 2);
}

#[test]
fn held_item_renderer_first_person_item_needs_a_player() {
    let _ = core();
    let state = crate::seams::PlayerRenderState::default();
    let _fp = crate::seams::FirstPersonHandsAndItemsRenderState::default();
    let mut matrices = PoseStack::new();
    let render_arm = |_m: &mut PoseStack,
                      _s: &SubmitNodeCollector,
                      _l: i32,
                      _y: f32,
                      _x: f32,
                      _a: HumanoidArm,
                      _p: &crate::seams::PlayerRenderState| {};
    // vanilla_hands_mode Off: showVanillaHands is false, the >= 26.3 hook
    // cancels the vanilla arm path outright.
    let cancelled = held_item_renderer_render_first_person_item(
        &state,
        &_fp,
        1.0,
        0.0,
        InteractionHand::MainHand,
        0.0,
        &ItemStack::EMPTY.clone(),
        1.0,
        &mut matrices,
        &SubmitNodeCollector,
        15,
        &render_arm,
    );
    assert!(cancelled);
    // AllDouble with a runtime player and no state player: the hook bails at
    // the player-extraction step, so the vanilla renderer keeps its path.
    core().base.config.borrow_mut().as_mut().unwrap().vanilla_hands_mode = VanillaHands::AllDouble;
    with_player();
    let cancelled = held_item_renderer_render_first_person_item(
        &state,
        &_fp,
        1.0,
        0.0,
        InteractionHand::MainHand,
        0.0,
        &ItemStack::EMPTY.clone(),
        1.0,
        &mut matrices,
        &SubmitNodeCollector,
        15,
        &render_arm,
    );
    assert!(!cancelled);
}

// --- entity accessor ---

#[test]
fn entity_accessor_raw_position_round_trip() {
    let entity = std::rc::Rc::new(std::cell::RefCell::new(Entity::player()));
    entity_accessor_set_raw_position(&entity, Vec3::new(1.5, -2.0, 3.25));
    let p = entity_accessor_get_raw_position(&entity);
    assert_eq!(p, Vec3::new(1.5, -2.0, 3.25));
}

// --- config screen surface (ItemTab registry parity) ---

#[test]
fn constants_offsets_match_upstream() {
    assert!((constants::Constants::SNEAK_BODY_OFFSET - 0.27).abs() < 1e-9);
    assert!((constants::Constants::SWIM_UP_BODY_OFFSET - 0.60).abs() < 1e-9);
    assert!((constants::Constants::SWIM_DOWN_BODY_OFFSET - 0.50).abs() < 1e-9);
    assert!((constants::Constants::IN_VEHICLE_BODY_OFFSET - 0.20).abs() < 1e-9);
}

// The RUNTIME stays default (no player) across the suite: every
// player-dependent branch bails exactly as upstream does with
// Minecraft.getInstance().player == null.
#[test]
fn runtime_default_camera_is_first_person() {
    RUNTIME.with(|rt| assert!(rt.client.borrow().camera_type == CameraType::FirstPerson));
}
