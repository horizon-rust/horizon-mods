//! Upstream LogicHandler.java, 1:1. The //? preprocessor branches collapse to
//! the >= 1.21.5 path (Horizon's 26.3 tree): Boat/Minecart live in
//! world.entity.vehicle.minecart/boat upstream.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::api::FirstPersonAPI;
use crate::config::VanillaHands;
use crate::constants::Constants;
use crate::first_person_model_core::FirstPersonModelCore;
use crate::inventory_util::InventoryUtil;
use crate::seams::mth;
use crate::seams::{item_util, Entity, EntityUtil, ItemStack, Pose, Vec3, VehicleKind};

/// dev.tr7zw.firstperson.LogicHandler.
pub struct LogicHandler {
    pub fpm: Rc<FirstPersonModelCore>,
    /// Current offset used for rendering.
    pub offset: RefCell<Vec3>,
    auto_vanilla_hand_items: RefCell<HashSet<String>>,
    auto_disable_items: RefCell<HashSet<String>>,
    timeout: RefCell<u64>,
}

impl LogicHandler {
    /// @RequiredArgsConstructor (the client is folded into the RUNTIME seams).
    pub fn new(fpm: Rc<FirstPersonModelCore>) -> Rc<Self> {
        Rc::new(Self {
            fpm,
            offset: RefCell::new(Vec3::ZERO),
            auto_vanilla_hand_items: RefCell::new(HashSet::new()),
            auto_disable_items: RefCell::new(HashSet::new()),
            timeout: RefCell::new(0),
        })
    }

    pub fn register_default_handlers(self: &Rc<Self>) {
        // FirstPersonAPI.registerPlayerHandler((ActivationHandler) () -> { ... }):
        // the lambda closes over this, so the registry entry holds the instance.
        FirstPersonAPI::register_activation_handler(Rc::new(InstanceActivation {
            inner: self.clone(),
        }));
    }

    /// The body of the default activation handler the lambda carries.
    pub fn prevent_firstperson(&self) -> bool {
        crate::RUNTIME.with(|rt| {
            let client = rt.client.borrow();
            let player = match &client.player {
                Some(p) => p.clone(),
                None => return false,
            };
            let p = player.borrow();
            if p.sleeping
                || p.auto_spin_attack
                || p.fall_flying
                || (p.swim_amount != 0.0 && !self.is_crawling_or_swimming(&p))
            {
                *self.timeout.borrow_mut() = now_millis() + 100;
                return true;
            }
            // FIXME: Evil hack to fix weird 1 frame-ish issues when landing
            // with an elytra
            if now_millis() < *self.timeout.borrow() {
                return true;
            }
            if self.auto_disable_items.borrow().contains(&p.main_hand.get_item().id)
                || self
                    .auto_disable_items
                    .borrow()
                    .contains(&p.off_hand.get_item().id)
            {
                return true;
            }

            if p.scoping {
                return true;
            }
            false
        })
    }

    /// Checks whether the mod should render at all.
    pub fn should_apply_third_person(&self, third_person: bool) -> bool {
        if !self.fpm.is_enabled() || third_person {
            return false;
        }
        for handler in FirstPersonAPI::get_activation_handlers() {
            if handler.prevent_firstperson() {
                return false;
            }
        }
        true
    }

    /// Calculates the X/Z offset applied to the player model to get it
    /// relative to the vanilla camera position.
    pub fn update_position_offset(&self, entity: &Rc<RefCell<Entity>>, delta: f32) {
        *self.offset.borrow_mut() = Vec3::ZERO;

        let mut x = 0.0f64;
        let mut y = 0.0f64;
        let mut z = 0.0f64;

        crate::RUNTIME.with(|rt| {
            let client = rt.client.borrow();
            // handle sleeping
            let camera = client.get_camera_entity();
            let same_camera = camera.as_ref().map(|c| Rc::ptr_eq(c, entity)).unwrap_or(false);
            if same_camera {
                if let Some(player_rc) = &client.player {
                    if player_rc.borrow().sleeping {
                        return;
                    }
                }
            }

            if client.options_get_camera_type() != crate::seams::CameraType::FirstPerson {
                return;
            }

            // entity instanceof LivingEntity player
            let mut real_yaw = {
                let snapshot = entity.borrow();
                mth::rot_lerp(delta, snapshot.y_body_rot_o, snapshot.y_body_rot) as f64
            };
            if same_camera {
                // isCrawlingOrSwimming(client.player) reads the client player.
                let crawling = client
                    .player
                    .as_ref()
                    .map(|p| self.is_crawling_or_swimming(&p.borrow()))
                    .unwrap_or(false);

                {
                    let head_rot = entity.borrow().y_head_rot;
                    if crawling {
                        // player.yBodyRot = player.yHeadRot;
                        entity.borrow_mut().y_body_rot = head_rot;
                    }
                }

                let e = entity.borrow();
                let body_offset: f32;
                if crawling {
                    if e.x_rot_o > 0.0 {
                        body_offset = Constants::SWIM_UP_BODY_OFFSET;
                    } else {
                        body_offset = Constants::SWIM_DOWN_BODY_OFFSET;
                    }
                    // some mods seem to break the isCrouching method
                } else if e.crouching || e.pose == Pose::Crouching {
                    body_offset = Constants::SNEAK_BODY_OFFSET
                        + self.fpm.config().sneak_x_offset as f32 / 100.0;
                } else if e.passenger {
                    match e.vehicle {
                        Some(VehicleKind::Boat) | Some(VehicleKind::Minecart) => {
                            real_yaw =
                                mth::rot_lerp(delta, e.y_body_rot_o, e.y_body_rot) as f64;
                        }
                        Some(VehicleKind::LivingEntity) => {
                            real_yaw = calculate_body_rot(
                                mth::rot_lerp(delta, e.vehicle_body_rot_o, e.vehicle_body_rot),
                                EntityUtil::get_y_rot(&e),
                            ) as f64;
                        }
                        _ => {
                            // Non living entities don't use any custom rotation
                        }
                    }
                    body_offset = Constants::IN_VEHICLE_BODY_OFFSET
                        + self.fpm.config().sit_x_offset as f32 / 100.0;
                } else {
                    body_offset = 0.25 + self.fpm.config().x_offset as f32 / 100.0;
                }
                x += body_offset as f64 * real_yaw.to_radians().sin();
                z -= body_offset as f64 * real_yaw.to_radians().cos();
                if crawling {
                    if e.x_rot_o > 0.0 && e.under_water {
                        y += 0.6 * (e.x_rot_o as f64).to_radians().sin();
                    } else {
                        y += 0.01 * -(e.x_rot_o as f64).to_radians().sin();
                    }
                }
            }
        });

        *self.offset.borrow_mut() = Vec3::new(x, y, z);
        for handler in FirstPersonAPI::get_player_offset_handlers() {
            let current = *self.offset.borrow();
            let player = crate::RUNTIME
                .with(|rt| rt.client.borrow().player.clone())
                .expect("client.player");
            let next = handler.apply_offset(&player, delta, Vec3::ZERO, current);
            *self.offset.borrow_mut() = next;
        }
    }

    /// Util method to quicker find where swimming is referenced.
    pub fn is_swimming(&self, player: &Entity) -> bool {
        player.swimming
    }

    /// Util method to quicker find where the crawling/swimming animation is
    /// referenced.
    pub fn is_crawling_or_swimming(&self, player: &Entity) -> bool {
        player.visually_swimming
    }

    pub fn show_vanilla_hands(&self) -> bool {
        crate::RUNTIME.with(|rt| {
            let player = match &rt.client.borrow().player {
                Some(p) => p.clone(),
                None => return false,
            };
            let e = player.borrow();
            // showVanillaHands(client.player) routes through the LivingEntity
            // overload, which checks instanceof Player.
            self.show_vanilla_hands_living(&e)
        })
    }

    pub fn show_vanilla_hands_living(&self, living_entity: &Entity) -> bool {
        if living_entity.is_player {
            self.show_vanilla_hands_items(
                InventoryUtil::get_selected(InventoryUtil::get_inventory(living_entity)),
                InventoryUtil::get_offhand(InventoryUtil::get_inventory(living_entity)),
            )
        } else {
            false
        }
    }

    /// Don't skip the vanilla first person arm rendering.
    pub fn show_vanilla_hands_items(&self, mainhand: ItemStack, offhand: ItemStack) -> bool {
        let cfg = self.fpm.config();
        cfg.vanilla_hands_mode == VanillaHands::All
            || cfg.vanilla_hands_mode == VanillaHands::AllDouble
            || (cfg.vanilla_hands_mode == VanillaHands::Items
                && (!mainhand.is_empty() || !offhand.is_empty()))
            || self
                .auto_vanilla_hand_items
                .borrow()
                .contains(&mainhand.get_item().id)
            || self
                .auto_vanilla_hand_items
                .borrow()
                .contains(&offhand.get_item().id)
            || self.auto_disable_items.borrow().contains(&mainhand.get_item().id)
            || self.auto_disable_items.borrow().contains(&offhand.get_item().id)
    }

    pub fn hide_arms_and_items(&self) -> bool {
        crate::RUNTIME.with(|rt| {
            let player = match &rt.client.borrow().player {
                Some(p) => p.clone(),
                None => return false,
            };
            let e = player.borrow();
            self.hide_arms_and_items_living(&e)
        })
    }

    pub fn hide_arms_and_items_living(&self, living_entity: &Entity) -> bool {
        if living_entity.is_player {
            self.hide_arms_and_items_full(
                living_entity,
                InventoryUtil::get_selected(InventoryUtil::get_inventory(living_entity)),
                InventoryUtil::get_offhand(InventoryUtil::get_inventory(living_entity)),
            )
        } else {
            false
        }
    }

    /// Should the models arms and items not be rendered?
    pub fn hide_arms_and_items_full(
        &self,
        living_entity: &Entity,
        mainhand: ItemStack,
        offhand: ItemStack,
    ) -> bool {
        let cfg = FirstPersonModelCore::instance().config();
        if cfg.vanilla_hands_skip_swimming
            && living_entity.is_player
            && self.is_swimming(living_entity)
        {
            return false;
        }
        if self.looking_down() {
            return false;
        }
        cfg.vanilla_hands_mode != VanillaHands::Off
            || self
                .auto_vanilla_hand_items
                .borrow()
                .contains(&mainhand.get_item().id)
            || self
                .auto_vanilla_hand_items
                .borrow()
                .contains(&offhand.get_item().id)
            || self.auto_disable_items.borrow().contains(&mainhand.get_item().id)
            || self.auto_disable_items.borrow().contains(&offhand.get_item().id)
    }

    pub fn dynamic_hands_enabled(&self) -> bool {
        crate::RUNTIME.with(|rt| {
            let player = match &rt.client.borrow().player {
                Some(p) => p.clone(),
                None => return false,
            };
            let e = player.borrow();
            self.dynamic_hands_enabled_living(&e)
        })
    }

    pub fn dynamic_hands_enabled_living(&self, living_entity: &Entity) -> bool {
        if living_entity.is_player {
            self.dynamic_hands_enabled_full(
                living_entity,
                InventoryUtil::get_selected(InventoryUtil::get_inventory(living_entity)),
                InventoryUtil::get_offhand(InventoryUtil::get_inventory(living_entity)),
            )
        } else {
            false
        }
    }

    /// True is dynamic hands is enabled and could apply at this moment.
    pub fn dynamic_hands_enabled_full(
        &self,
        living_entity: &Entity,
        mainhand: ItemStack,
        offhand: ItemStack,
    ) -> bool {
        let cfg = FirstPersonModelCore::instance().config();
        if cfg.vanilla_hands_skip_swimming
            && living_entity.is_player
            && self.is_swimming(living_entity)
        {
            return false;
        }
        cfg.dynamic_mode
            && cfg.vanilla_hands_mode != VanillaHands::Off
            && !(self
                .auto_vanilla_hand_items
                .borrow()
                .contains(&mainhand.get_item().id)
                || self
                    .auto_vanilla_hand_items
                    .borrow()
                    .contains(&offhand.get_item().id)
                || self.auto_disable_items.borrow().contains(&mainhand.get_item().id)
                || self.auto_disable_items.borrow().contains(&offhand.get_item().id))
    }

    /// Is Dynamic hands enabled and the player looking down?
    pub fn looking_down(&self) -> bool {
        crate::RUNTIME.with(|rt| {
            let player = match &rt.client.borrow().player {
                Some(p) => p.clone(),
                None => return false,
            };
            let e = player.borrow();
            self.looking_down_living(&e)
        })
    }

    pub fn looking_down_living(&self, living_entity: &Entity) -> bool {
        self.dynamic_hands_enabled() && EntityUtil::get_x_rot(living_entity) > 30.0
    }

    /// >= 1.21.9 lookingDown(EntityRenderState) overload: reads the state's
    /// xRot instead of the entity's.
    pub fn looking_down_state(&self, state: &crate::seams::LivingEntityRenderState) -> bool {
        self.dynamic_hands_enabled() && state.x_rot > 30.0
    }

    /// LogicHandler.getOffset.
    pub fn get_offset(&self) -> Vec3 {
        *self.offset.borrow()
    }

    pub fn add_auto_vanilla_hands_item(&self, item: &str) {
        self.auto_vanilla_hand_items.borrow_mut().insert(item.to_string());
    }

    pub fn add_auto_disable_item(&self, item: &str) {
        self.auto_disable_items.borrow_mut().insert(item.to_string());
    }

    pub fn reload_auto_vanilla_hands_settings(&self) {
        self.auto_vanilla_hand_items.borrow_mut().clear();
        self.auto_disable_items.borrow_mut().clear();
        let invalid = item_util::air();
        let cfg = self.fpm.config();
        for item_id in &cfg.auto_vanilla_hands {
            let mut parts = item_id.splitn(2, ':');
            let ns = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            let item = item_util::get_item_opt(ns, path);
            if invalid != item {
                self.add_auto_vanilla_hands_item(&item.id);
            }
        }
        crate::seams::log::info(&format!(
            "Loaded Vanilla Hands items: {:?}",
            self.auto_vanilla_hand_items.borrow()
        ));
        for item_id in &cfg.auto_toggle_mod_items {
            let mut parts = item_id.splitn(2, ':');
            let ns = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            let item = item_util::get_item_opt(ns, path);
            if invalid != item {
                self.add_auto_disable_item(&item.id);
            }
        }
        crate::seams::log::info(&format!(
            "Loaded Auto Disable items: {:?}",
            self.auto_disable_items.borrow()
        ));
    }
}

/// The registry entry carrying the instance, standing in for upstream's lambda
/// that closes over `this`.
struct InstanceActivation {
    inner: Rc<LogicHandler>,
}

impl crate::api::ActivationHandler for InstanceActivation {
    fn prevent_firstperson(&self) -> bool {
        self.inner.prevent_firstperson()
    }
}

/// calculateBodyRot.
pub(crate) fn calculate_body_rot(mut entity_body_rot: f32, rider_head_rot: f32) -> f32 {
    // Wrap the head rotation to the range [-180, 180]
    let wrapped_head_rot = mth::wrap_degrees(rider_head_rot);

    // Calculate the difference between the head and body rotation
    let rot_diff = mth::wrap_degrees(wrapped_head_rot - entity_body_rot);

    // If the difference is more than 50 degrees, adjust the body rotation
    if mth::abs(rot_diff) > 50.0 {
        // Pull the body along with the head
        entity_body_rot = wrapped_head_rot - 50.0 * rot_diff.signum();
    }

    // Ensure the body rotation is wrapped to [-180, 180]
    mth::wrap_degrees(entity_body_rot)
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
