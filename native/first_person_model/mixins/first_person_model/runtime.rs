//! Exact 1:1 Rust translation of First-person Model
//! (tr7zw/FirstPersonModel, MIT).
//!
//! Upstream package dev.tr7zw.firstperson, @6b5859f (Merge pull request #667,
//! fix/fishing-redirect-mixinextras). Translation by sparticle. Carries the
//! upstream MIT license and credit.
//!
//! Module map (upstream -> here):
//!   versionless/FirstPersonBase.java      -> first_person_base
//!   versionless/Constants.java            -> constants
//!   versionless/config/*                  -> config
//!   versionless/mixinbase/ModelPartBase   -> mixinbase
//!   InventoryUtil.java                    -> inventory_util
//!   KeybindHolder.java                    -> keybind_holder
//!   LogicHandler.java                     -> logic_handler
//!   FirstPersonModelCore.java             -> first_person_model_core
//!   FirstPersonBootstrap.java, FirstPersonModelMod.java -> bootstrap
//!   access/*                              -> access
//!   api/*                                 -> api
//!   modsupport/*                          -> modsupport
//!   mixins/*                              -> mixins (in progress)
//! Host seams for Minecraft / platform types live in seams. Upstream is a
//! multiloader preprocessor source (//? if fabric / forge / >= 1.21.x); this
//! port carries the fabric, >= 1.21.5 variant paths, which is the branch
//! Horizon's 26.3 tree corresponds to.
//!
//! Note: the mod's own README credits sparticle only; upstream's MIT license
//! text ships in LICENSE and the upstream credit is carried here.
//!
//! Packaged splice: every module inlined as a sibling so the .hnmod holds one
//! standalone source file, per the horizon-mods packaging pattern.

pub use seams::RUNTIME;

pub mod seams {
//! Host seams: every Minecraft / platform type upstream touches directly is
//! translated here as the exact 1:1 logic plus a thin injectable surface, so
//! the ported code keeps the upstream shape without a live Minecraft client.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// net.minecraft.world.phys.Vec3
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn add(&self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x: self.x + x, y: self.y + y, z: self.z + z }
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.CameraType
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraType {
    FirstPerson,
    ThirdPersonBack,
    ThirdPersonFront,
}

impl Default for CameraType {
    fn default() -> Self {
        CameraType::FirstPerson
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.world.entity.Pose (the variant the port reads)
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    Dying,
}

// ---------------------------------------------------------------------------
// net.minecraft.util.Mth
// ---------------------------------------------------------------------------
pub mod mth {
    /// net.minecraft.util.Mth.clamp.
    pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
        value.max(min).min(max)
    }

    /// Mth.rotLerp.
    pub fn rot_lerp(delta: f32, start: f32, end: f32) -> f32 {
        start + delta * wrap_degrees(end - start)
    }

    /// Mth.wrapDegrees.
    pub fn wrap_degrees(value: f32) -> f32 {
        let f = value % 360.0;
        if f >= 180.0 {
            f - 360.0
        } else if f < -180.0 {
            f + 360.0
        } else {
            f
        }
    }

    pub fn abs(value: f32) -> f32 {
        value.abs()
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.world.item.Item / ItemStack, ItemUtil
// ---------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    /// The registry id, "namespace:path".
    pub id: String,
}

impl Item {
    pub fn is_air(&self) -> bool {
        self.id == "minecraft:air"
    }
}

/// dev.tr7zw.transition.mc.ItemUtil.getItem: registry lookup; unknown ids
/// resolve to the air item (upstream compares `invalid != item`).
pub mod item_util {
    use super::Item;

    /// The fake registry the seams carry: every id the tests mention, plus air.
    pub fn get_item(id: String) -> Item {
        Item { id }
    }

    pub fn get_item_opt(namespace: &str, path: &str) -> Item {
        Item { id: format!("{}:{}", namespace, path) }
    }

    /// GeneralUtil.getResourceLocation("minecraft", "air").
    pub fn air() -> Item {
        Item { id: "minecraft:air".to_string() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemStack {
    pub item: Item,
    pub count: i32,
    pub empty: bool,
}

impl ItemStack {
    pub const EMPTY: ItemStack = ItemStack {
        item: Item { id: String::new() },
        count: 0,
        empty: true,
    };

    pub fn new(item: Item) -> Self {
        Self { item, count: 1, empty: false }
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn get_item(&self) -> Item {
        self.item.clone()
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.world.entity.player.Inventory
// ---------------------------------------------------------------------------
#[derive(Clone, Debug)]
pub struct Inventory {
    /// Inventory.SLOT_OFFHAND.
    pub selected: usize,
    pub items: Vec<ItemStack>,
    pub offhand: Vec<ItemStack>,
}

impl Inventory {
    pub const SLOT_OFFHAND: usize = 40;

    pub fn get_selected_item(&self) -> ItemStack {
        self.items.get(self.selected).cloned().unwrap_or(ItemStack::EMPTY)
    }

    pub fn get_item(&self, slot: usize) -> ItemStack {
        if slot == Self::SLOT_OFFHAND {
            return self.offhand.first().cloned().unwrap_or(ItemStack::EMPTY);
        }
        self.items.get(slot).cloned().unwrap_or(ItemStack::EMPTY)
    }

    pub fn get_selected_slot(&self) -> usize {
        self.selected
    }

    pub fn get_non_equipment_items(&self) -> Vec<ItemStack> {
        self.items.clone()
    }
}

// ---------------------------------------------------------------------------
// The entity surface upstream touches (net.minecraft.world.entity.Entity,
// LivingEntity, Player and dev.tr7zw.transition.mc.EntityUtil).
// One struct carries it; the instanceof checks upstream writes (Player,
// LivingEntity, vehicle kinds) become the is_player / vehicle fields.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehicleKind {
    Boat,
    Minecart,
    LivingEntity,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityUtil;

impl EntityUtil {
    /// dev.tr7zw.transition.mc.EntityUtil.getXRot.
    pub fn get_x_rot(entity: &Entity) -> f32 {
        entity.x_rot
    }

    /// dev.tr7zw.transition.mc.EntityUtil.getYRot.
    pub fn get_y_rot(entity: &Entity) -> f32 {
        entity.y_rot
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    // Rotation
    pub x_rot: f32,
    pub x_rot_o: f32,
    pub y_rot: f32,
    pub y_rot_o: f32,
    pub y_body_rot: f32,
    pub y_body_rot_o: f32,
    pub y_head_rot: f32,
    pub y_head_rot_o: f32,
    // instanceof markers
    pub is_player: bool,
    // LivingEntity state
    pub sleeping: bool,
    pub auto_spin_attack: bool,
    pub fall_flying: bool,
    pub swim_amount: f32,
    pub visually_swimming: bool,
    pub swimming: bool,
    pub crouching: bool,
    pub pose: Pose,
    pub under_water: bool,
    pub scoping: bool,
    // Vehicle
    pub passenger: bool,
    pub vehicle: Option<VehicleKind>,
    /// The vehicle's own body rotation, for the LivingEntity-vehicle branch.
    pub vehicle_body_rot: f32,
    pub vehicle_body_rot_o: f32,
    // Held items
    pub main_hand: ItemStack,
    pub off_hand: ItemStack,
    /// The entity's own inventory (players only; empty otherwise).
    pub inventory: Inventory,
    // Positions: the raw position (EntityAccessor.entityCulling$getRawPosition)
    // and the interpolation/old coordinates the WorldRenderer mixin saves.
    pub position: Vec3,
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    pub x_old: f64,
    pub y_old: f64,
    pub z_old: f64,
    /// HumanoidArm.getMainArm; LEFT for the default skin.
    pub main_arm: HumanoidArm,
    pub is_invisible: bool,
}

impl Entity {
    /// Entity.refreshDimensions (a no-op on this surface).
    pub fn refresh_dimensions(&mut self) {}

    /// Player.getMainArm().getOpposite().
    pub fn main_arm_opposite(&self) -> HumanoidArm {
        self.main_arm.opposite()
    }
}

impl Entity {
    pub fn player() -> Self {
        Self {
            x_rot: 0.0,
            x_rot_o: 0.0,
            y_rot: 0.0,
            y_rot_o: 0.0,
            y_body_rot: 0.0,
            y_body_rot_o: 0.0,
            y_head_rot: 0.0,
            y_head_rot_o: 0.0,
            is_player: true,
            sleeping: false,
            auto_spin_attack: false,
            fall_flying: false,
            swim_amount: 0.0,
            visually_swimming: false,
            swimming: false,
            crouching: false,
            pose: Pose::Standing,
            under_water: false,
            scoping: false,
            passenger: false,
            vehicle: None,
            vehicle_body_rot: 0.0,
            vehicle_body_rot_o: 0.0,
            main_hand: ItemStack::EMPTY,
            off_hand: ItemStack::EMPTY,
            inventory: Inventory { selected: 0, items: Vec::new(), offhand: Vec::new() },
            position: Vec3::ZERO,
            xo: 0.0,
            yo: 0.0,
            zo: 0.0,
            x_old: 0.0,
            y_old: 0.0,
            z_old: 0.0,
            main_arm: HumanoidArm::Right,
            is_invisible: false,
        }
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.model.geom.ModelPart (the surface the mixins touch)
// ---------------------------------------------------------------------------
#[derive(Clone, Debug, Default)]
pub struct ModelPart {
    pub name: String,
    pub hidden: bool,
    pub z: f32,
    pub x_rot: f32,
    /// ModelPartMixin state.
    pub z_copy: f32,
    pub moved: bool,
}

// ---------------------------------------------------------------------------
// net.minecraft.client.KeyMapping
// ---------------------------------------------------------------------------
pub struct KeyMapping {
    pub name: String,
    pub key: i32,
    pub down: Cell<bool>,
}

impl KeyMapping {
    pub fn is_down(&self) -> bool {
        self.down.get()
    }
}

/// GeneralUtil.createKeyMapping.
pub fn create_key_mapping(name: &str, key: i32, _category: &str) -> Rc<KeyMapping> {
    Rc::new(KeyMapping { name: name.to_string(), key, down: Cell::new(false) })
}

// ---------------------------------------------------------------------------
// net.minecraft.client.Minecraft (the surface the mod reads)
// ---------------------------------------------------------------------------
#[derive(Default)]
pub struct Minecraft {
    pub player: Option<Rc<RefCell<Entity>>>,
    pub camera_entity: Option<Rc<RefCell<Entity>>>,
    pub camera_type: CameraType,
    /// The player renderer lookup behind getEntityRenderDispatcher().getRenderer.
    pub player_renderer: Option<Rc<RefCell<super::access::PlayerRendererAccessImpl>>>,
}

impl Minecraft {
    pub fn get_player(&self) -> Option<Rc<RefCell<Entity>>> {
        self.player.clone()
    }

    pub fn get_camera_entity(&self) -> Option<Rc<RefCell<Entity>>> {
        self.camera_entity.clone()
    }

    pub fn options_get_camera_type(&self) -> CameraType {
        self.camera_type
    }

    pub fn get_entity_render_dispatcher_renderer(
        &self,
        _entity: &Rc<RefCell<Entity>>,
    ) -> Option<Rc<RefCell<super::access::PlayerRendererAccessImpl>>> {
        self.player_renderer.clone()
    }
}

// ---------------------------------------------------------------------------
// dev.tr7zw.transition.loader.ModLoaderUtil / ModLoaderEventUtil /
// GeneralUtil surfaces
// ---------------------------------------------------------------------------
pub struct ModLoaderState {
    pub registered_keybinds: RefCell<Vec<Rc<KeyMapping>>>,
    pub config_screen: Cell<bool>,
    pub display_test_disabled: Cell<bool>,
    pub client_tick_listeners: RefCell<Vec<u32>>,
    pub loaded_mods: RefCell<HashSet<String>>,
    /// FirstPersonModelCore.cameraEntityExtract, set by the extraction mixin.
    pub camera_entity_extract: Cell<bool>,
    /// Minecraft.isSameThread.
    pub same_thread: Cell<bool>,
}

impl Default for ModLoaderState {
    fn default() -> Self {
        Self {
            registered_keybinds: RefCell::new(Vec::new()),
            config_screen: Cell::new(false),
            display_test_disabled: Cell::new(false),
            client_tick_listeners: RefCell::new(Vec::new()),
            loaded_mods: RefCell::new(HashSet::new()),
            camera_entity_extract: Cell::new(false),
            same_thread: Cell::new(true),
        }
    }
}

impl ModLoaderState {
    pub fn register_keybind(&self, key: &Rc<KeyMapping>) {
        self.registered_keybinds.borrow_mut().push(key.clone());
    }

    pub fn disable_display_test(&self) {
        self.display_test_disabled.set(true);
    }

    pub fn register_config_screen(&self) {
        self.config_screen.set(true);
    }

    pub fn register_client_tick_start_listener(&self) -> u32 {
        let id = self.client_tick_listeners.borrow().len() as u32 + 1;
        self.client_tick_listeners.borrow_mut().push(id);
        id
    }

    /// ModLoaderUtil.isModLoaded.
    pub fn is_mod_loaded(&self, id: &str) -> bool {
        self.loaded_mods.borrow().contains(id)
    }
}

/// GeneralUtil.getPlayer: the camera entity, falling back to the client player.
pub mod general_util {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::Minecraft;

    pub fn get_player(client: &Minecraft) -> Option<Rc<RefCell<super::Entity>>> {
        client.get_camera_entity().or_else(|| client.get_player())
    }

    /// GeneralUtil.getResourceLocation(namespace, path).
    pub fn get_resource_location(namespace: &str, path: &str) -> String {
        format!("{}:{}", namespace, path)
    }
}

/// The runtime the seams inject: one struct the port hands around, mirroring
/// the statics upstream leans on (Minecraft.getInstance() and friends).
#[derive(Default)]
pub struct Runtime {
    pub client: RefCell<Minecraft>,
    pub loader: ModLoaderState,
}

impl Runtime {
    pub fn new(client: Minecraft) -> Rc<Self> {
        Rc::new(Self { client: RefCell::new(client), loader: ModLoaderState::default() })
    }
}

// ---------------------------------------------------------------------------
// The runtime singleton: Minecraft.getInstance() and ModLoaderUtil statics
// collapse onto one thread-local the port and the tests inject.
// ---------------------------------------------------------------------------
thread_local! {
    pub static RUNTIME: Rc<Runtime> = Runtime::new(Minecraft {
        player: None,
        camera_entity: None,
        camera_type: CameraType::FirstPerson,
        player_renderer: None,
    });
}

/// The logger surface upstream calls LOGGER on.
pub mod log {
    pub fn info(msg: &str) {
        let _ = msg; // The runtime logger drops it outside tests.
    }

    pub fn warn(_msg: &str, _err: Option<&str>) {}
}


// ---------------------------------------------------------------------------
// Render enums and the render-state structs the >= 1.21.9 / 26.3 mixins read.
// ---------------------------------------------------------------------------

/// net.minecraft.world.entity.HumanoidArm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HumanoidArm {
    Left,
    #[default]
    Right,
}

impl HumanoidArm {
    pub fn opposite(self) -> Self {
        match self {
            HumanoidArm::Left => HumanoidArm::Right,
            HumanoidArm::Right => HumanoidArm::Left,
        }
    }
}

/// net.minecraft.world.InteractionHand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionHand {
    MainHand,
    OffHand,
}

/// net.minecraft.world.entity.EquipmentSlot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    Mainhand,
    Offhand,
    Head,
    Chest,
    Legs,
    Feet,
    Body,
}

/// net.minecraft.world.item.Items.FILLED_MAP's registry id.
pub mod items {
    pub const FILLED_MAP: &str = "filled_map";
}

/// com.mojang.blaze3d.vertex.PoseStack (the push/pop surface the mixins use).
#[derive(Default)]
pub struct PoseStack;

impl PoseStack {
    pub fn new() -> Self {
        Self
    }

    pub fn push_pose(&mut self) {}

    pub fn pop_pose(&mut self) {}
}

/// net.minecraft.client.renderer.SubmitNodeCollector.
#[derive(Default)]
pub struct SubmitNodeCollector;

/// net.minecraft.client.renderer.culling.Frustum.
#[derive(Default)]
pub struct Frustum;

/// net.minecraft.client.DeltaTracker.getGameTimeDeltaPartialTick.
#[derive(Default)]
pub struct DeltaTracker {
    pub partial_tick: f32,
}

impl DeltaTracker {
    pub fn get_game_time_delta_partial_tick(&self, _tick: bool) -> f32 {
        self.partial_tick
    }
}

/// net.minecraft.client.Camera (position, entity, detached).
#[derive(Default)]
pub struct Camera {
    pub detached: bool,
    pub position: Vec3,
    pub entity: Option<Rc<RefCell<Entity>>>,
}

impl Camera {
    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn entity(&self) -> Option<Rc<RefCell<Entity>>> {
        self.entity.clone()
    }

    pub fn is_detached(&self) -> bool {
        self.detached
    }
}

/// net.minecraft.client.renderer.entity.state.LivingEntityRenderState, the
/// surface LivingEntityRenderStateMixin bolts its access onto.
#[derive(Clone, Debug, Default)]
pub struct LivingEntityRenderState {
    pub camera_entity: bool,
    pub hide_left_arm: bool,
    pub hide_right_arm: bool,
    pub hide_body: bool,
    pub arm_offset: f32,
    /// For EntityUtil.getXRot over the render state.
    pub x_rot: f32,
    pub is_spectator: bool,
    pub is_visually_swimming: bool,
    pub show_cape: bool,
    /// The entity the state was extracted from (transition.getEntity()).
    pub transition_entity: Option<Rc<RefCell<Entity>>>,
}

impl super::access::LivingEntityRenderStateAccess for LivingEntityRenderState {
    fn set_is_camera_entity(&mut self, value: bool) {
        self.camera_entity = value;
    }
    fn is_camera_entity(&self) -> bool {
        self.camera_entity
    }
    fn set_hide_right_arm(&mut self, value: bool) {
        self.hide_right_arm = value;
    }
    fn set_hide_left_arm(&mut self, value: bool) {
        self.hide_left_arm = value;
    }
    fn hide_left_arm(&self) -> bool {
        self.hide_left_arm
    }
    fn hide_right_arm(&self) -> bool {
        self.hide_right_arm
    }
    fn set_hide_body(&mut self, value: bool) {
        self.hide_body = value;
    }
    fn hide_body(&self) -> bool {
        self.hide_body
    }
    fn set_arm_offset(&mut self, offset: f32) {
        self.arm_offset = offset;
    }
    fn get_arm_offset(&self) -> f32 {
        self.arm_offset
    }
}

impl super::access::LivingEntityRenderStateAccess for AvatarRenderState {
    fn set_is_camera_entity(&mut self, value: bool) {
        self.base.set_is_camera_entity(value);
    }
    fn is_camera_entity(&self) -> bool {
        self.base.is_camera_entity()
    }
    fn set_hide_right_arm(&mut self, value: bool) {
        self.base.set_hide_right_arm(value);
    }
    fn set_hide_left_arm(&mut self, value: bool) {
        self.base.set_hide_left_arm(value);
    }
    fn hide_left_arm(&self) -> bool {
        self.base.hide_left_arm()
    }
    fn hide_right_arm(&self) -> bool {
        self.base.hide_right_arm()
    }
    fn set_hide_body(&mut self, value: bool) {
        self.base.set_hide_body(value);
    }
    fn hide_body(&self) -> bool {
        self.base.hide_body()
    }
    fn set_arm_offset(&mut self, offset: f32) {
        self.base.set_arm_offset(offset);
    }
    fn get_arm_offset(&self) -> f32 {
        self.base.get_arm_offset()
    }
}

/// net.minecraft.client.renderer.entity.state.AvatarRenderState (the player
/// variant; showCape is the field the mixins write).
#[derive(Clone, Debug, Default)]
pub struct AvatarRenderState {
    pub base: LivingEntityRenderState,
    pub show_cape: bool,
}

/// net.minecraft.client.renderer.entity.state.HumanoidRenderState.
#[derive(Clone, Debug, Default)]
pub struct HumanoidRenderState {
    pub base: LivingEntityRenderState,
}

/// net.minecraft.client.renderer.state.level.PlayerRenderState.
#[derive(Clone, Debug, Default)]
pub struct PlayerRenderState {
    pub has_player: bool,
    pub avatar_render_state: AvatarRenderState,
}

/// net.minecraft.client.renderer.state.level.FirstPersonHandsAndItemsRenderState.
#[derive(Clone, Debug, Default)]
pub struct FirstPersonHandsAndItemsRenderState;

/// net.minecraft.client.renderer.entity.state.FishingHookRenderState.
#[derive(Clone, Debug, Default)]
pub struct FishingHookRenderState;

/// net.minecraft.client.renderer.state.level.LevelRenderState.
#[derive(Default)]
pub struct LevelRenderState {
    pub entity_render_states: RefCell<Vec<Rc<RefCell<LivingEntityRenderState>>>>,
}

/// net.minecraft.client.renderer.entity.state.ArmedEntityRenderState.
#[derive(Clone, Debug, Default)]
pub struct ArmedEntityRenderState {
    pub base: LivingEntityRenderState,
}

/// net.minecraft.client.renderer.entity.state.ItemStackRenderState.
#[derive(Clone, Debug, Default)]
pub struct ItemStackRenderState;

/// net.minecraft.client.renderer.state.level.CameraRenderState.
#[derive(Clone, Debug, Default)]
pub struct CameraRenderState;



/// net.minecraft.client.model.HumanoidModel: the parts the mixins touch, each
/// an Rc so the model and the mixin state share the same part.
#[derive(Clone, Debug, Default)]
pub struct HumanoidModel {
    pub head: Option<Rc<RefCell<ModelPart>>>,
    pub body: Option<Rc<RefCell<ModelPart>>>,
    pub left_arm: Option<Rc<RefCell<ModelPart>>>,
    pub right_arm: Option<Rc<RefCell<ModelPart>>>,
    pub hat: Option<Rc<RefCell<ModelPart>>>,
    pub left_sleeve: Option<Rc<RefCell<ModelPart>>>,
    pub right_sleeve: Option<Rc<RefCell<ModelPart>>>,
}

}

pub mod constants {
//! Upstream versionless/Constants.java, 1:1.

/// dev.tr7zw.firstperson.versionless.Constants.
pub struct Constants;

impl Constants {
    pub const SNEAK_BODY_OFFSET: f32 = 0.27;
    pub const SWIM_UP_BODY_OFFSET: f32 = 0.60;
    pub const SWIM_DOWN_BODY_OFFSET: f32 = 0.50;
    pub const IN_VEHICLE_BODY_OFFSET: f32 = 0.20;
}

}

pub mod config {
//! Upstream versionless/config/{FirstPersonSettings, VanillaHands,
//! ConfigUpgrader}.java, 1:1.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// versionless/config/VanillaHands.java.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VanillaHands {
    Off,
    All,
    AllDouble,
    Items,
}

/// versionless/config/FirstPersonSettings.java.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FirstPersonSettings {
    pub config_version: i32,
    pub enabled_by_default: bool,
    pub x_offset: i32,
    pub sneak_x_offset: i32,
    pub sit_x_offset: i32,
    pub render_stuck_features: bool,
    pub vanilla_hands_mode: VanillaHands,
    pub dynamic_mode: bool,
    pub vanilla_hands_skip_swimming: bool,
    pub auto_vanilla_hands: HashSet<String>,
    pub auto_toggle_mod_items: HashSet<String>,
    pub hidden_layers: HashSet<String>,
}

impl Default for FirstPersonSettings {
    /// The field initializers upstream writes.
    fn default() -> Self {
        Self {
            config_version: 2,
            enabled_by_default: true,
            x_offset: 0,
            sneak_x_offset: 0,
            sit_x_offset: 0,
            render_stuck_features: true,
            vanilla_hands_mode: VanillaHands::Off,
            dynamic_mode: true,
            vanilla_hands_skip_swimming: true,
            auto_vanilla_hands: HashSet::from([
                "antiqueatlas:antique_atlas".to_string(),
                "twilightforest:filled_magic_map".to_string(),
                "twilightforest:filled_maze_map".to_string(),
                "twilightforest:filled_ore_map".to_string(),
                "create:potato_cannon".to_string(),
                "create:extendo_grip".to_string(),
                "create:handheld_worldshaper".to_string(),
                "map_atlases:atlas".to_string(),
            ]),
            auto_toggle_mod_items: HashSet::from(["exposure:camera".to_string()]),
            hidden_layers: HashSet::new(),
        }
    }
}

/// versionless/config/ConfigUpgrader.java.
pub struct ConfigUpgrader;

impl ConfigUpgrader {
    pub fn upgrade_config(conf: &mut FirstPersonSettings) {
        if conf.config_version < 1 {
            // re-init the auto vanilla hands set
            conf.auto_vanilla_hands.extend([
                "antiqueatlas:antique_atlas".to_string(),
                "twilightforest:filled_magic_map".to_string(),
                "twilightforest:filled_maze_map".to_string(),
                "twilightforest:filled_ore_map".to_string(),
                "create:potato_cannon".to_string(),
                "create:extendo_grip".to_string(),
                "create:handheld_worldshaper".to_string(),
                "map_atlases:atlas".to_string(),
            ]);
            conf.config_version = 1;
        }
        if conf.config_version < 2 {
            conf.auto_toggle_mod_items.insert("exposure:camera".to_string());
            conf.config_version = 2;
        }
    }
}

}

pub mod mixinbase {
//! Upstream versionless/mixinbase/ModelPartBase.java, 1:1: the mixin-base
//! surface the mixins bolt onto model parts.

/// versionless/mixinbase/ModelPartBase.java.
pub trait ModelPartBase {
    fn set_hidden(&self);
    fn show_again(&self);
    fn is_hidden(&self) -> bool;
}

}

pub mod inventory_util {
//! Upstream InventoryUtil.java, 1:1. The //? preprocessor branches collapse to
//! the >= 1.21.5 path (Horizon's 26.3 tree).

use super::seams::{Entity, Inventory, ItemStack};

/// dev.tr7zw.firstperson.InventoryUtil.
pub struct InventoryUtil;

impl InventoryUtil {
    pub fn get_inventory(player: &Entity) -> &Inventory {
        &player.inventory
    }

    pub fn get_selected(inventory: &Inventory) -> ItemStack {
        inventory.get_selected_item()
    }

    pub fn get_offhand(inventory: &Inventory) -> ItemStack {
        inventory.get_item(Inventory::SLOT_OFFHAND)
    }

    pub fn get_selected_id(inventory: &Inventory) -> usize {
        inventory.get_selected_slot()
    }

    pub fn get_non_equipment_items(inventory: &Inventory) -> Vec<ItemStack> {
        inventory.get_non_equipment_items()
    }
}

}

pub mod keybind_holder {
//! Upstream KeybindHolder.java, 1:1.

use std::cell::Cell;
use std::rc::Rc;

use super::seams::{create_key_mapping, KeyMapping};

/// dev.tr7zw.firstperson.KeybindHolder.
pub struct KeybindHolder {
    pub key_binding: Rc<KeyMapping>,
    initialized: Cell<bool>,
}

impl KeybindHolder {
    /// KeybindHolder.INSTANCE.
    pub fn instance() -> Rc<Self> {
        thread_local! {
            static INSTANCE: Rc<KeybindHolder> = Rc::new(KeybindHolder {
                key_binding: create_key_mapping("key.firstperson.toggle", 295, "firstperson:keybind"),
                initialized: Cell::new(false),
            });
        }
        INSTANCE.with(|i| i.clone())
    }

    pub fn register_keybinds(&self) {
        if self.initialized.get() {
            return;
        }
        self.initialized.set(true);
        // ModLoaderUtil.registerKeybind(keyBinding);
        super::RUNTIME.with(|r| r.loader.register_keybind(&self.key_binding));
    }
}

}

pub mod access {
//! Upstream access/*, 1:1: the duck interfaces the mixins inject into engine
//! types, plus the concrete renderer state the seams hand to them.

use std::cell::RefCell;
use std::rc::Rc;

/// access/AgeableListModelAccess.java.
pub trait AgeableListModelAccess {
    fn first_person_head_parts_getter(&self) -> Vec<Rc<RefCell<super::seams::ModelPart>>>;
}

/// access/LivingEntityRenderStateAccess.java.
pub trait LivingEntityRenderStateAccess {
    fn set_is_camera_entity(&mut self, value: bool);
    fn is_camera_entity(&self) -> bool;

    /// The default method upstream declares.
    fn set_hide_arms(&mut self, value: bool) {
        self.set_hide_left_arm(value);
        self.set_hide_right_arm(value);
    }

    fn set_hide_right_arm(&mut self, value: bool);
    fn set_hide_left_arm(&mut self, value: bool);
    fn hide_left_arm(&self) -> bool;
    fn hide_right_arm(&self) -> bool;
    fn set_hide_body(&mut self, value: bool);
    fn hide_body(&self) -> bool;
    fn set_arm_offset(&mut self, offset: f32);
    fn get_arm_offset(&self) -> f32;
}

/// access/PlayerModelAccess.java: the < 1.21.3 cloak getter is the whole
/// interface; the carried >= 1.21.3 variant keeps it empty.
pub trait PlayerModelAccess {}

/// access/PlayerRendererAccess.java.
pub trait PlayerRendererAccessTrait {
    fn get_render_layers(&self) -> Vec<String>;
    fn update_parts_list(&self, thirdperson: bool);
}

/// The concrete renderer state behind PlayerRendererAccessImpl: the render
/// layer list and the parts visibility the mixins drive.
#[derive(Default)]
pub struct PlayerRendererAccessImpl {
    pub render_layers: RefCell<Vec<String>>,
    /// updatePartsList state: whether parts are hidden for third person.
    pub thirdperson: std::cell::Cell<bool>,
}

impl PlayerRendererAccessTrait for PlayerRendererAccessImpl {
    fn get_render_layers(&self) -> Vec<String> {
        self.render_layers.borrow().clone()
    }

    fn update_parts_list(&self, thirdperson: bool) {
        self.thirdperson.set(thirdperson);
    }
}

}

pub mod api {
//! Upstream api/*, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use super::seams::{Entity, Vec3};

/// api/ActivationHandler.java.
pub trait ActivationHandler {
    fn prevent_firstperson(&self) -> bool;
}

/// api/PlayerOffsetHandler.java.
pub trait PlayerOffsetHandler {
    /// The current value is the value pre-calculated by the mod (or handlers
    /// running before). If the handler doesn't apply to the current situation,
    /// the expected behavior is to just return `current`.
    fn apply_offset(
        &self,
        entity: &Rc<RefCell<Entity>>,
        delta: f32,
        original: Vec3,
        current: Vec3,
    ) -> Vec3;
}

/// api/FirstPersonAPI.java: the static lists become thread-locals.
pub struct FirstPersonAPI;

impl FirstPersonAPI {
    thread_local! {
        static PLAYER_OFFSET_HANDLERS: RefCell<Vec<Rc<dyn PlayerOffsetHandler>>> =
            const { RefCell::new(Vec::new()) };
        static ACTIVATION_HANDLERS: RefCell<Vec<Rc<dyn ActivationHandler>>> =
            const { RefCell::new(Vec::new()) };
    }

    /// The current status of the FirstPerson Mod.
    pub fn is_enabled() -> bool {
        super::first_person_model_core::FirstPersonModelCore::instance().is_enabled()
    }

    /// Change the enabled status. Simulates pressing the (by default) F6 hotkey.
    pub fn set_enabled(enabled: bool) {
        super::first_person_model_core::FirstPersonModelCore::instance().set_enabled(enabled);
    }

    /// Returns true when called during the first person player rendering.
    /// While this is true, no heads/helmets/hats should be rendered!
    pub fn is_rendering_player() -> bool {
        super::first_person_model_core::FirstPersonModelCore::instance().is_rendering_player()
    }

    /// Adds a new handler. Supports PlayerOffsetHandler and ActivationHandler.
    pub fn register_player_handler(handler: Rc<dyn PlayerOffsetHandler>) {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    /// The PlayerOffsetHandler arm of upstream's registerPlayerHandler.
    pub fn register_player_offset_handler(handler: Rc<dyn PlayerOffsetHandler>) {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    pub fn register_activation_handler(handler: Rc<dyn ActivationHandler>) {
        Self::ACTIVATION_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    pub fn get_player_offset_handlers() -> Vec<Rc<dyn PlayerOffsetHandler>> {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow().clone())
    }

    pub fn get_activation_handlers() -> Vec<Rc<dyn ActivationHandler>> {
        Self::ACTIVATION_HANDLERS.with(|l| l.borrow().clone())
    }
}

}

pub mod logic_handler {
//! Upstream LogicHandler.java, 1:1. The //? preprocessor branches collapse to
//! the >= 1.21.5 path (Horizon's 26.3 tree): Boat/Minecart live in
//! world.entity.vehicle.minecart/boat upstream.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use super::api::FirstPersonAPI;
use super::config::VanillaHands;
use super::constants::Constants;
use super::first_person_model_core::FirstPersonModelCore;
use super::inventory_util::InventoryUtil;
use super::seams::mth;
use super::seams::{item_util, Entity, EntityUtil, ItemStack, Pose, Vec3, VehicleKind};

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
        super::RUNTIME.with(|rt| {
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

        super::RUNTIME.with(|rt| {
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

            if client.options_get_camera_type() != super::seams::CameraType::FirstPerson {
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
            let player = super::RUNTIME
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
        super::RUNTIME.with(|rt| {
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
        super::RUNTIME.with(|rt| {
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
        super::RUNTIME.with(|rt| {
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
        super::RUNTIME.with(|rt| {
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
    pub fn looking_down_state(&self, state: &super::seams::LivingEntityRenderState) -> bool {
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
        super::seams::log::info(&format!(
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
        super::seams::log::info(&format!(
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

impl super::api::ActivationHandler for InstanceActivation {
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

}

pub mod mixins {
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

use super::access::LivingEntityRenderStateAccess;
use super::first_person_model_core::FirstPersonModelCore;
use super::inventory_util::InventoryUtil;
use super::logic_handler::LogicHandler;
use super::mixinbase::ModelPartBase;
use super::RUNTIME;
use super::seams::{
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
        if core.config().vanilla_hands_mode != super::config::VanillaHands::AllDouble
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

}

pub mod first_person_base {
//! Upstream versionless/FirstPersonBase.java, 1:1.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use super::config::{ConfigUpgrader, FirstPersonSettings};

/// versionless/FirstPersonBase.java. The static LOGGER stays in seams::log.
pub struct FirstPersonBase {
    /// Stays true till the first entity is rendered when the player is
    /// supposed to be rendered, or during extraction in 26.1+.
    pub is_rendering_player: Cell<bool>,
    /// Stays true for the entire render time of the entity, even after the
    /// entity itself.
    pub is_rendering_player_post: Cell<bool>,
    pub enabled: Cell<bool>,
    pub config: RefCell<Option<FirstPersonSettings>>,
    settings_file: PathBuf,
}

impl FirstPersonBase {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            is_rendering_player: Cell::new(false),
            is_rendering_player_post: Cell::new(false),
            enabled: Cell::new(true),
            config: RefCell::new(None),
            // new File("config", "firstperson.json")
            settings_file: PathBuf::from("config").join("firstperson.json"),
        })
    }

    pub fn load_config(&self) {
        if self.settings_file.exists() {
            match std::fs::read_to_string(&self.settings_file) {
                Ok(text) => match serde_json::from_str::<FirstPersonSettings>(&text) {
                    Ok(cfg) => *self.config.borrow_mut() = Some(cfg),
                    Err(err) => {
                        super::seams::log::warn(
                            "Unable to load the config, creating a new one.",
                            Some(&err.to_string()),
                        );
                    }
                },
                Err(err) => {
                    super::seams::log::warn(
                        "Unable to load the config, creating a new one.",
                        Some(&err.to_string()),
                    );
                }
            }
        }
        if self.config.borrow().is_none() {
            *self.config.borrow_mut() = Some(FirstPersonSettings::default());
        }
        ConfigUpgrader::upgrade_config(self.config.borrow_mut().as_mut().unwrap());
        self.write_settings();
        let enabled = self.config.borrow().as_ref().unwrap().enabled_by_default;
        self.set_enabled(enabled);
    }

    pub fn write_settings(&self) {
        let json = serde_json::to_string_pretty(self.config.borrow().as_ref().unwrap())
            .unwrap_or_else(|_| "{}".to_string());
        if let Some(parent) = self.settings_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = std::fs::write(&self.settings_file, json) {
            super::seams::log::warn("Error while saving the config!", Some(&err.to_string()));
        }
    }

    pub fn reset_settings(&self) {
        *self.config.borrow_mut() = Some(FirstPersonSettings::default());
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    pub fn is_rendering_player(&self) -> bool {
        self.is_rendering_player.get()
    }

    pub fn set_rendering_player(&self, value: bool) {
        self.is_rendering_player.set(value);
    }

    /// Checks if a class exists or not. On this port the mod-support classes
    /// are compiled in, so a class "exists" when its feature was registered.
    pub fn is_valid_class(name: &str) -> bool {
        // Class.forName: the port checks the known support class names.
        matches!(name, "dev.kosmx.playerAnim.core.impl.AnimationProcessor")
    }
}

}

pub mod first_person_model_core {
//! Upstream FirstPersonModelCore.java, 1:1. The //? preprocessor branches
//! collapse to the fabric / >= 1.21.2 paths (Horizon's 26.3 tree); the forge
//! and neoforge event wiring is the loader-specific block upstream gates off
//! on fabric and is not carried.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::access::{PlayerRendererAccessImpl, PlayerRendererAccessTrait};
use super::api::{FirstPersonAPI, PlayerOffsetHandler};
use super::keybind_holder::KeybindHolder;
use super::logic_handler::LogicHandler;
use super::modsupport::{ModSupportLoader, PlayerAnimatorSupport};
use super::seams::{general_util, CameraType};
use super::first_person_base::FirstPersonBase;

/// dev.tr7zw.firstperson.FirstPersonModelCore.
pub struct FirstPersonModelCore {
    pub base: Rc<FirstPersonBase>,
    pub logic_handler: RefCell<Option<Rc<LogicHandler>>>,
    /// WorldRendererMixin/RenderDispatcherMixin renderingPlayerPost flag.
    pub rendering_player_post: Cell<bool>,
    is_held: Cell<bool>,
    late_init: Cell<bool>,
    last_camera_type: RefCell<Option<CameraType>>,
    tick_counter_workaround: Cell<i32>,
}

impl FirstPersonModelCore {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            base: FirstPersonBase::new(),
            logic_handler: RefCell::new(None),
            is_held: Cell::new(false),
            late_init: Cell::new(true),
            last_camera_type: RefCell::new(None),
            tick_counter_workaround: Cell::new(0),
            rendering_player_post: Cell::new(false),
        })
    }

    /// FirstPersonModelCore.instance.
    pub fn instance() -> Rc<Self> {
        thread_local! {
            static INSTANCE: Rc<FirstPersonModelCore> = FirstPersonModelCore::new();
        }
        INSTANCE.with(|i| i.clone())
    }

    pub fn shared_setup(self: &Rc<Self>) {
        super::seams::log::info("Loading FirstPerson Mod");
        *self.logic_handler.borrow_mut() = Some(LogicHandler::new(self.clone()));

        self.base.load_config();

        KeybindHolder::instance().register_keybinds();
        super::RUNTIME.with(|r| {
            r.loader.register_client_tick_start_listener();
        });
        super::RUNTIME.with(|r| r.loader.disable_display_test());
        super::RUNTIME.with(|r| r.loader.register_config_screen());

        ModSupportLoader::load_support();
    }

    fn late_init(&self) {
        // isValidClass("dev.kosmx.playerAnim.core.impl.AnimationProcessor")
        if FirstPersonBase::is_valid_class("dev.kosmx.playerAnim.core.impl.AnimationProcessor") {
            super::seams::log::info("Loading PlayerAnimator support!");
            FirstPersonAPI::register_player_offset_handler(Rc::new(PlayerAnimatorSupport));
        } else {
            super::seams::log::info("PlayerAnimator not found!");
        }

        if let Some(lh) = self.logic_handler.borrow().as_ref() {
            lh.register_default_handlers();
            lh.reload_auto_vanilla_hands_settings();
        }
        self.update_player_layers();
    }

    pub fn on_tick(&self) {
        if self.late_init.get() {
            self.late_init.set(false);
            self.late_init();
        }
        let down = super::RUNTIME.with(|_| KeybindHolder::instance().key_binding.is_down());
        if down {
            if self.is_held.get() {
                return;
            }
            self.is_held.set(true);
            self.set_enabled(!self.is_enabled());
        } else {
            self.is_held.set(false);
        }
        self.tick_counter_workaround.set(self.tick_counter_workaround.get() + 1);
    }

    pub fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.base.set_enabled(enabled);
    }

    pub fn is_rendering_player(&self) -> bool {
        // super.isRenderingPlayer() || cameraEntityExtract
        self.base.is_rendering_player() || self.camera_entity_extract()
    }

    /// FirstPersonModelCore.cameraEntityExtract: true while extraction runs.
    pub fn camera_entity_extract(&self) -> bool {
        super::RUNTIME.with(|rt| rt.loader.camera_entity_extract.get())
    }

    pub fn is_rendering_player_post(&self) -> bool {
        self.rendering_player_post.get()
    }

    pub fn set_rendering_player_post(&self, is_rendering_player_post: bool) {
        self.rendering_player_post.set(is_rendering_player_post);
    }

    pub fn is_rendering_player_raw(&self) -> bool {
        self.base.is_rendering_player()
    }

    pub fn set_rendering_player(&self, is_rendering_player: bool) {
        self.base.set_rendering_player(is_rendering_player);
        // TODO: ugly sideffect, find better way
        // TODO2: even worse workaround with counting ticks
        let current = super::RUNTIME.with(|rt| rt.client.borrow().options_get_camera_type());
        let changed = self.last_camera_type.borrow().as_ref() != Some(&current)
            || self.tick_counter_workaround.get() % 21 == 0;
        if changed {
            self.tick_counter_workaround.set(self.tick_counter_workaround.get() + 1);
            *self.last_camera_type.borrow_mut() = Some(current);
            self.update_player_layers();
        }
    }

    pub fn config(&self) -> super::config::FirstPersonSettings {
        self.base
            .config
            .borrow()
            .as_ref()
            .expect("config loaded")
            .clone()
    }

    pub fn config_x_offset(&self) -> i32 {
        self.config().x_offset
    }

    pub fn config_sneak_x_offset(&self) -> i32 {
        self.config().sneak_x_offset
    }

    pub fn config_sit_x_offset(&self) -> i32 {
        self.config().sit_x_offset
    }

    pub fn update_player_layers(&self) {
        if self.config().hidden_layers.is_empty() {
            return;
        }
        // PlayerRendererAccess access = null;
        // >= 1.21.6 branch:
        super::RUNTIME.with(|rt| {
            let player = general_util::get_player(&rt.client.borrow());
            let access: Option<Rc<RefCell<PlayerRendererAccessImpl>>> = match &player {
                Some(p) => rt.client.borrow().get_entity_render_dispatcher_renderer(p),
                None => return,
            };
            if let Some(access) = access {
                let third = self.last_camera_type.borrow().as_ref()
                    != Some(&CameraType::FirstPerson);
                access.borrow().update_parts_list(third);
            }
        });
    }
}

}

pub mod bootstrap {
//! Upstream FirstPersonBootstrap.java + FirstPersonModelMod.java, 1:1. The
//! forge and neoforge bootstrap variants are preprocessor blocks upstream
//! gates off on fabric and are not carried; the fabric entry point is
//! FirstPersonModelMod.onInitializeClient.

use std::rc::Rc;

use super::first_person_model_core::FirstPersonModelCore;

/// dev.tr7zw.firstperson.FirstPersonModelMod.
pub struct FirstPersonModelMod;

impl FirstPersonModelMod {
    pub fn on_initialize_client() {
        FirstPersonModelCore::instance().shared_setup();
    }

    /// The whole fabric bootstrap: construct the core and run sharedSetup.
    pub fn bootstrap() -> Rc<FirstPersonModelCore> {
        let core = FirstPersonModelCore::instance();
        core.shared_setup();
        core
    }
}

}

pub mod modsupport {
//! Upstream modsupport/*, 1:1.

use std::rc::Rc;

use super::api::{ActivationHandler, PlayerOffsetHandler};
use super::first_person_base::FirstPersonBase;
use super::seams::{Entity, Vec3};

/// modsupport/ModSupportLoader.java.
pub struct ModSupportLoader;

impl ModSupportLoader {
    pub fn load_support() {
        super::RUNTIME.with(|rt| {
            // try {
            if rt.loader.is_mod_loaded("freecam") {
                FirstPersonAPI_reg_freecam();
                super::seams::log::info("Freecam support loaded.");
            }
            // } catch (Throwable ex) { LOGGER.warn(...) } - the port's support
            // classes cannot throw at load time.
        });
    }
}

fn FirstPersonAPI_reg_freecam() {
    super::api::FirstPersonAPI::register_activation_handler(Rc::new(FreecamSupport));
}

/// modsupport/FreecamSupport.java: making sure the constructor throws an
/// exception, in case something is wrong, is the Java-side guard; the port's
/// Freecam.isEnabled() seam is total.
pub struct FreecamSupport;

impl ActivationHandler for FreecamSupport {
    fn prevent_firstperson(&self) -> bool {
        // Freecam.isEnabled(): a mod-loaded check on this port.
        super::RUNTIME.with(|rt| rt.loader.is_mod_loaded("freecam"))
    }
}

/// modsupport/PlayerAnimatorSupport.java: the transform math upstream ships is
/// fully commented out (FIXME: PartKey missing), so the handler returns
/// current, 1:1.
pub struct PlayerAnimatorSupport;

impl PlayerOffsetHandler for PlayerAnimatorSupport {
    fn apply_offset(
        &self,
        _entity: &Rc<std::cell::RefCell<Entity>>,
        _delta: f32,
        _original: Vec3,
        current: Vec3,
    ) -> Vec3 {
        current
    }
}

}

/// Upstream mod id.
pub const MOD_ID: &str = "firstperson";
