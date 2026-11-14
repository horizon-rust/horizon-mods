//! Host seams: every Minecraft / platform type upstream touches directly is
//! translated here as the exact 1:1 logic plus a thin injectable surface, so
//! the ported code keeps the upstream shape without a live Minecraft client.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use uuid::Uuid;

use crate::config::armor_config::ArmorConfig;

// ---------------------------------------------------------------------------
// net.minecraft.world.entity.EquipmentSlot
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
    Body,
    Saddle,
}

impl EquipmentSlot {
    /// net.minecraft.world.entity.EquipmentSlot#byName (case-insensitive id
    /// match, null when unknown).
    pub fn by_name(name: &str) -> Option<EquipmentSlot> {
        let lowered = name.to_ascii_lowercase();
        match lowered.as_str() {
            "mainhand" => Some(EquipmentSlot::MainHand),
            "offhand" => Some(EquipmentSlot::OffHand),
            "head" => Some(EquipmentSlot::Head),
            "chest" => Some(EquipmentSlot::Chest),
            "legs" => Some(EquipmentSlot::Legs),
            "feet" => Some(EquipmentSlot::Feet),
            "body" => Some(EquipmentSlot::Body),
            "saddle" => Some(EquipmentSlot::Saddle),
            _ => None,
        }
    }

    /// net.minecraft.world.entity.EquipmentSlot#getName
    pub fn get_name(self) -> &'static str {
        match self {
            EquipmentSlot::MainHand => "mainhand",
            EquipmentSlot::OffHand => "offhand",
            EquipmentSlot::Head => "head",
            EquipmentSlot::Chest => "chest",
            EquipmentSlot::Legs => "legs",
            EquipmentSlot::Feet => "feet",
            EquipmentSlot::Body => "body",
            EquipmentSlot::Saddle => "saddle",
        }
    }
}

// ---------------------------------------------------------------------------
// FabricLoader config dir seam (net.fabricmc.loader.api.FabricLoader
// #getConfigDir). Injectable so tests point at a temp dir.
// ---------------------------------------------------------------------------
thread_local! {
    static CONFIG_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn set_config_dir(dir: PathBuf) {
    CONFIG_DIR.with(|c| *c.borrow_mut() = Some(dir));
}

pub fn config_dir() -> PathBuf {
    CONFIG_DIR
        .with(|c| c.borrow().clone())
        .unwrap_or_else(|| PathBuf::from("config"))
}

// ---------------------------------------------------------------------------
// Client-side world/state seam backing ModConfig's Minecraft.getInstance()
// calls: whether the synced server config + level are available, each
// player's ARMOR_CONFIG component value, and isFallFlying.
// ---------------------------------------------------------------------------
pub struct LevelAccess {
    /// ShowMeYourSkinClient.serverConfig != null && client.level != null
    pub server_available: bool,
    /// Minecraft.getInstance().level != null (the client level gate
    /// ModConfig#getApplicable checks separately).
    pub level_available: bool,
    /// level.getPlayerByUUID(uuid) -> the player's
    /// Components.ARMOR_CONFIG.getConfig(), None when the player is absent.
    pub player_configs: Rc<dyn Fn(Uuid) -> Option<ArmorConfig>>,
    /// player.isFallFlying(), None when the player is absent.
    pub player_fall_flying: Rc<dyn Fn(Uuid) -> Option<bool>>,
}

impl LevelAccess {
    pub fn noop() -> LevelAccess {
        LevelAccess {
            server_available: false,
            level_available: false,
            player_configs: Rc::new(|_| None),
            player_fall_flying: Rc::new(|_| None),
        }
    }
}

thread_local! {
    static LEVEL_ACCESS: RefCell<LevelAccess> = RefCell::new(LevelAccess::noop());
    static COMBAT_CHECK: RefCell<Rc<dyn Fn(Uuid) -> bool>> =
        RefCell::new(Rc::new(|_| false));
}

pub fn set_level_access(access: LevelAccess) {
    LEVEL_ACCESS.with(|l| *l.borrow_mut() = access);
}

pub fn with_level_access<R>(f: impl FnOnce(&LevelAccess) -> R) -> R {
    LEVEL_ACCESS.with(|l| f(&l.borrow()))
}

/// nl.enjarai.showmeyourskin.util.CombatLogger.INSTANCE#isInCombat(uuid),
/// seamed until util/CombatLogger is translated.
pub fn set_combat_check(check: Rc<dyn Fn(Uuid) -> bool>) {
    COMBAT_CHECK.with(|c| *c.borrow_mut() = check);
}

pub fn is_in_combat(uuid: Uuid) -> bool {
    COMBAT_CHECK.with(|c| (c.borrow().clone())(uuid))
}

/// Java: a reference to a LivingEntity/Player. Horizon has no shared entity
/// object for mods yet, so the engine glue supplies a lightweight snapshot;
/// replaced by the real engine entity at the mixins stage.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityRef {
    pub uuid: uuid::Uuid,
    pub is_player: bool,
}

impl EntityRef {
    /// Java: ServerPlayNetworking / PlayerEntity construction seam.
    pub fn player(uuid: uuid::Uuid) -> Self {
        EntityRef { uuid, is_player: true }
    }
}

/// Java: ComponentKey registration (Components.ARMOR_CONFIG) plus the CCA
/// factory registration. Horizon has no component registry yet: the seam keeps
/// the key id and the vanilla-values factory, and the engine glue binds it.
pub struct ComponentRegistry {
    pub key_id: String,
}

/// Java: net.minecraft.util.Mth.clamp.
pub fn mth_clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}
