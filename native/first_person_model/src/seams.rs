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

impl crate::access::LivingEntityRenderStateAccess for LivingEntityRenderState {
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

impl crate::access::LivingEntityRenderStateAccess for AvatarRenderState {
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
