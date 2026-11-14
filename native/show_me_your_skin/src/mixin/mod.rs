//! nl.enjarai.showmeyourskin.mixin (1:1): the mixin set. Injection points and
//! render plumbing are Horizon-mixin seams; every mixin keeps its exact cancel
//! and patch logic, testable through the seams below.

pub mod armor;
pub mod compat;
pub mod hat;
pub mod shield;

use crate::config::armor_config::ArmorConfig;
use crate::config::mod_config;
use crate::seams::EquipmentSlot;
use crate::pond::{ArmorConfigContraband, ClientMannequinContraband};
use crate::seams::EntityRef;

/// Java: RenderStateDataKey<ArmorConfig> ARMOR_CONFIG_KEY - render-state data
/// keyed by entity uuid in the seam store.
pub mod render_state_data {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static DATA: RefCell<std::collections::HashMap<uuid::Uuid, ArmorConfig>> =
            RefCell::new(std::collections::HashMap::new());
    }

    /// Java: renderState.setData(ARMOR_CONFIG_KEY, config).
    pub fn set_data(uuid: uuid::Uuid, config: ArmorConfig) {
        DATA.with(|d| d.borrow_mut().insert(uuid, config));
    }

    /// Java: renderState.getData(ARMOR_CONFIG_KEY) (nullable).
    pub fn get_data(uuid: uuid::Uuid) -> Option<ArmorConfig> {
        DATA.with(|d| d.borrow().get(&uuid).cloned())
    }
}

/// Java: mixin/LivingEntityMixin.showmeyourskin$triggerCombat: on damage, log
/// combat for the hit player.
pub fn living_entity_trigger_combat(damaged_is_client_player: bool, player_uuid: uuid::Uuid) {
    if damaged_is_client_player {
        crate::util::COMBAT_LOGGER.trigger_combat(player_uuid);
    }
}

/// Java: mixin/LivingEntityRendererMixin.showmeyourskin$removeLabel: hide the
/// name tag when the applicable config says so.
pub fn living_entity_remove_label(entity_uuid: uuid::Uuid) -> bool {
    mod_config::with_instance(|c| c.get_applicable(entity_uuid).show_name_tag)
}

/// Java: mixin/LivingEntityRendererMixin.showmeyourskin$updateRenderState:
/// stamp the render state with the applicable config, contraband first.
pub fn living_entity_update_render_state(
    entity_uuid: uuid::Uuid,
    is_avatar_render_state: bool,
    contraband: Option<&dyn ArmorConfigContraband>,
) {
    if !is_avatar_render_state {
        return;
    }
    let mut armor_config = mod_config::with_instance(|c| c.get_applicable(entity_uuid));
    if let Some(contraband) = contraband {
        if let Some(config) = contraband.show_me_your_skin_get_armor_config() {
            armor_config = config;
        }
    }
    render_state_data::set_data(entity_uuid, armor_config);
}

/// Java: mixin/ItemStackRenderStateAccessor (+ the FoilType seam the hat and
/// shield mixins use to strip the enchantment glint).
#[derive(Default)]
pub struct ItemStackRenderStateAccess {
    /// Java: getLayers() + getActiveLayerCount() (foil per active layer).
    pub active_layer_count: usize,
    pub foil_types: Vec<FoilType>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FoilType {
    None,
    Standard,
}

impl ItemStackRenderStateAccess {
    /// Java: layers[i].setFoilType(FoilType.NONE) for every active layer.
    pub fn clear_foil(&mut self) {
        for foil in self.foil_types.iter_mut().take(self.active_layer_count) {
            *foil = FoilType::None;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.active_layer_count == 0
    }
}

/// Java: mixin/EntityRendererMixin.captureEntityContext (the body is commented
/// out upstream; kept for parity).
pub fn capture_entity_context() {}

/// Java: mixin/hat/HudMixin.hidePumpkinOverlay: the pumpkin overlay goes when
/// the applicable config hides hats.
pub fn hud_hide_pumpkin_overlay(equippable_slot_is_head: bool, local_player_uuid: uuid::Uuid) -> bool {
    if !equippable_slot_is_head {
        return true;
    }
    mod_config::with_instance(|c| c.get_applicable(local_player_uuid).hat.base)
}

/// Java: mixin/hat/CustomHeadLayerMixin.hideHat: cancel the head layer when
/// hats are hidden, else strip the glint when the hat glint is off.
pub fn custom_head_layer_hide_hat(
    entity_uuid: uuid::Uuid,
    head_item_empty: bool,
    layers: &mut ItemStackRenderStateAccess,
) -> bool {
    let Some(config) = render_state_data::get_data(entity_uuid) else {
        return false; // no cancel when there is no config stamped
    };
    if !config.hat.base {
        return true; // ci.cancel()
    }
    if !config.hat.glint && !head_item_empty {
        layers.clear_foil();
    }
    false
}

/// Java: mixin/shield/ItemInHandLayerMixin.hideShield (same shape as the hat).
pub fn item_in_hand_layer_hide_shield(
    entity_uuid: uuid::Uuid,
    item_stack_render_state_empty: bool,
    layers: &mut ItemStackRenderStateAccess,
) -> bool {
    let Some(config) = render_state_data::get_data(entity_uuid) else {
        return false;
    };
    if !config.shield.base {
        return true;
    }
    if !config.shield.glint && !item_stack_render_state_empty {
        layers.clear_foil();
    }
    false
}

/// Java: mixin/shield/ItemInHandRendererMixin.hideShield: first-person shield
/// render - glint strip + the submit wrap.
pub fn item_in_hand_renderer_hide_shield(
    item_is_shield: bool,
    entity_uuid: uuid::Uuid,
    layers: &mut ItemStackRenderStateAccess,
) -> bool {
    if !item_is_shield {
        return true;
    }
    let config = mod_config::with_instance(|c| c.get_applicable(entity_uuid));
    if !config.shield.glint {
        layers.clear_foil();
    }
    config.shield.base
}

/// Java: the shared PieceConfig supertype view (Java subclasses
/// ArmorPieceConfig under PieceConfig; Rust carries the trim flag as an
/// Option, None = the PieceConfig base method that always returns true).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PieceView {
    pub base: bool,
    pub glint: bool,
    pub trim: Option<bool>,
}

impl PieceView {
    pub fn from_piece(c: &crate::config::armor_config::PieceConfig) -> PieceView {
        PieceView { base: c.base, glint: c.glint, trim: None }
    }

    pub fn from_armor_piece(c: &crate::config::armor_config::ArmorPieceConfig) -> PieceView {
        PieceView { base: c.base, glint: c.glint, trim: Some(c.trim) }
    }

    pub fn trim(&self) -> bool {
        self.trim.unwrap_or(true)
    }
}

/// Java: mixin/armor/EquipmentLayerRendererMixin.loadConfig: pick the piece
/// config for the rendered equipment.
pub fn equipment_layer_load_config(
    entity_uuid: uuid::Uuid,
    has_equippable: bool,
    item_is_elytra: bool,
    item_is_trimmable_armor: bool,
    equippable_slot: Option<EquipmentSlot>,
) -> Option<PieceView> {
    let config = render_state_data::get_data(entity_uuid)?;
    if !has_equippable {
        return None;
    }

    if item_is_elytra {
        return Some(PieceView::from_piece(&config.elytra));
    }

    if item_is_trimmable_armor {
        return match equippable_slot {
            Some(EquipmentSlot::Feet) => Some(PieceView::from_armor_piece(&config.feet)),
            Some(EquipmentSlot::Legs) => Some(PieceView::from_armor_piece(&config.legs)),
            Some(EquipmentSlot::Chest) => Some(PieceView::from_armor_piece(&config.chest)),
            Some(EquipmentSlot::Head) => Some(PieceView::from_armor_piece(&config.head)),
            _ => None,
        };
    }

    if equippable_slot == Some(EquipmentSlot::Head) {
        return Some(PieceView::from_piece(&config.hat));
    }

    None
}

/// Java: the three hideBase/hideGlint/hideTrim WrapWithConditions.
pub fn equipment_layer_hide_base(armor_config: &Option<PieceView>) -> bool {
    armor_config.as_ref().map_or(true, |c| c.base)
}

pub fn equipment_layer_hide_glint(armor_config: &Option<PieceView>) -> bool {
    armor_config.as_ref().map_or(true, |c| c.glint)
}

pub fn equipment_layer_hide_trim(armor_config: &Option<PieceView>) -> bool {
    armor_config.as_ref().map_or(true, |c| c.trim())
}

/// Java: mixin/compat/wildfiregender/GenderArmorLayerMixin (@Pseudo, wildfire's
/// GenderArmorLayer): the chest cancels.
pub mod wildfiregender {
    use super::*;

    pub fn hide_breast_armor(entity_uuid: uuid::Uuid) -> bool {
        render_state_data::get_data(entity_uuid).map(|c| c.chest.base).map_or(false, |base| !base)
    }

    pub fn hide_armor_trim(entity_uuid: uuid::Uuid) -> bool {
        render_state_data::get_data(entity_uuid).map(|c| c.chest.trim).map_or(false, |trim| !trim)
    }

    pub fn hide_glint(entity_uuid: uuid::Uuid) -> bool {
        render_state_data::get_data(entity_uuid).map(|c| c.chest.glint).map_or(false, |glint| !glint)
    }
}

/// Java: mixin/LivingEntityAccessor (get/set equipment + elytra animation),
/// used by PlayerOption's mannequin construction.
pub struct LivingEntityAccess {
    pub uuid: uuid::Uuid,
    pub equipment: Option<()>,
    pub elytra_animation_state: Option<()>,
}

impl LivingEntityAccess {
    pub fn get_equipment(&self) -> Option<()> {
        self.equipment
    }

    pub fn set_equipment(&mut self, equipment: Option<()>) {
        self.equipment = equipment;
    }

    pub fn get_elytra_animation_state(&self) -> Option<()> {
        self.elytra_animation_state
    }

    pub fn set_elytra_animation_state(&mut self, state: Option<()>) {
        self.elytra_animation_state = state;
    }
}

/// Java: mixin/ItemStackRenderStateAccessor (kept next to its uses).
pub fn item_stack_layers(state: &mut ItemStackRenderStateAccess) -> &mut ItemStackRenderStateAccess {
    state
}

/// Test parity: a contraband carrier like the mannequin mixin creates.
pub fn new_contraband() -> ClientMannequinContraband {
    ClientMannequinContraband::default()
}

// EntityRef is referenced for the mannequin-construction accessor pairing.
#[allow(unused)]
fn _entity_shape(_: &EntityRef) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u64) -> uuid::Uuid {
        uuid::Uuid::from_u64_pair(0, n)
    }

    /// Java flow parity: the render state is stamped with
    /// ModConfig.INSTANCE.getApplicable(entity), then the mixins read it.
    fn stamp(n: u64) -> uuid::Uuid {
        let u = uuid(n);
        let cfg = mod_config::with_instance(|c| c.get_applicable(u));
        render_state_data::set_data(u, cfg);
        u
    }

    #[test]
    fn combat_triggered_for_players_only() {
        living_entity_trigger_combat(true, uuid(1));
        assert!(crate::util::COMBAT_LOGGER.is_in_combat(uuid(1)));
        living_entity_trigger_combat(false, uuid(2));
        assert!(!crate::util::COMBAT_LOGGER.is_in_combat(uuid(2)));
    }

    #[test]
    fn name_tag_hidden_by_applicable_config() {
        mod_config::with_instance_mut(|c| {
            c.global_enabled = true;
            c.overrides.clear();
        });
        assert!(living_entity_remove_label(uuid(5)));
        mod_config::with_instance_mut(|c| c.global.show_name_tag = false);
        assert!(!living_entity_remove_label(uuid(5)));
    }

    #[test]
    fn render_state_prefers_contraband() {
        mod_config::with_instance_mut(|c| c.global.head.base = true);
        let mut mannequin = new_contraband();
        assert_eq!(mannequin.show_me_your_skin_get_armor_config(), None);
        let mut cfg = ArmorConfig::vanilla_values();
        cfg.head.base = false;
        mannequin.show_me_your_skin_set_armor_config(Some(cfg.clone()));
        living_entity_update_render_state(uuid(6), true, Some(&mannequin));
        assert_eq!(render_state_data::get_data(uuid(6)), Some(cfg));
        // non-avatar states get nothing
        living_entity_update_render_state(uuid(7), false, Some(&mannequin));
        assert_eq!(render_state_data::get_data(uuid(7)), None);
    }

    #[test]
    fn foil_stripped_per_active_layer() {
        let mut layers = ItemStackRenderStateAccess { active_layer_count: 2, foil_types: vec![FoilType::Standard; 3] };
        layers.clear_foil();
        assert_eq!(layers.foil_types, vec![FoilType::None, FoilType::None, FoilType::Standard]);
    }

    #[test]
    fn pumpkin_overlay_follows_hat_config() {
        mod_config::with_instance_mut(|c| {
            c.global_enabled = true;
            c.overrides.clear();
        });
        // non-head slots keep the overlay
        assert!(hud_hide_pumpkin_overlay(false, uuid(8)));
        // head slot with hats shown keeps it
        mod_config::with_instance_mut(|c| c.global.hat.base = true);
        assert!(hud_hide_pumpkin_overlay(true, uuid(8)));
        // head slot with hats hidden drops it
        mod_config::with_instance_mut(|c| c.global.hat.base = false);
        assert!(!hud_hide_pumpkin_overlay(true, uuid(8)));
    }

    #[test]
    fn head_layer_cancels_and_strips_glint() {
        mod_config::with_instance_mut(|c| c.overrides.clear());
        let _ = stamp(9);
        // no data stamped -> no cancel
        let mut layers = ItemStackRenderStateAccess { active_layer_count: 1, foil_types: vec![FoilType::Standard] };
        assert!(!custom_head_layer_hide_hat(uuid(10), false, &mut layers));
        // hats hidden -> cancel
        mod_config::with_instance_mut(|c| c.global.hat.base = false);
        let _ = stamp(9);
        assert!(custom_head_layer_hide_hat(uuid(9), false, &mut layers));
        // hats shown, glint off -> strip the foil
        mod_config::with_instance_mut(|c| {
            c.global.hat.base = true;
            c.global.hat.glint = false;
        });
        let _ = stamp(9);
        let mut layers = ItemStackRenderStateAccess { active_layer_count: 1, foil_types: vec![FoilType::Standard] };
        assert!(!custom_head_layer_hide_hat(uuid(9), false, &mut layers));
        assert_eq!(layers.foil_types, vec![FoilType::None]);
    }

    #[test]
    fn shield_layers_cancel_and_strip() {
        mod_config::with_instance_mut(|c| c.overrides.clear());
        let _ = stamp(11);
        // hand layer: shield hidden -> cancel
        mod_config::with_instance_mut(|c| c.global.shield.base = false);
        let _ = stamp(11);
        let mut layers = ItemStackRenderStateAccess { active_layer_count: 1, foil_types: vec![FoilType::Standard] };
        assert!(item_in_hand_layer_hide_shield(uuid(11), false, &mut layers));
        // first-person renderer: not a shield -> no interference
        assert!(item_in_hand_renderer_hide_shield(false, uuid(11), &mut layers));
        // shield with glint off -> strip + hide when base off
        mod_config::with_instance_mut(|c| {
            c.global.shield.glint = false;
        });
        let _ = stamp(11);
        let mut layers = ItemStackRenderStateAccess { active_layer_count: 1, foil_types: vec![FoilType::Standard] };
        assert!(!item_in_hand_renderer_hide_shield(true, uuid(11), &mut layers));
        assert_eq!(layers.foil_types, vec![FoilType::None]);
        mod_config::with_instance_mut(|c| c.global.shield.base = false);
        assert!(!item_in_hand_renderer_hide_shield(true, uuid(11), &mut layers));
    }

    #[test]
    fn equipment_layer_picks_the_right_piece() {
        render_state_data::set_data(uuid(12), ArmorConfig::vanilla_values());

        assert_eq!(
            equipment_layer_load_config(uuid(12), true, true, false, Some(EquipmentSlot::Chest)).unwrap().base,
            ArmorConfig::vanilla_values().elytra.base
        );
        assert_eq!(
            equipment_layer_load_config(uuid(12), true, false, true, Some(EquipmentSlot::Legs)).unwrap().base,
            ArmorConfig::vanilla_values().legs.base
        );
        assert_eq!(
            equipment_layer_load_config(uuid(12), true, false, true, Some(EquipmentSlot::Chest)).unwrap().base,
            ArmorConfig::vanilla_values().chest.base
        );
        assert_eq!(
            equipment_layer_load_config(uuid(12), true, false, true, Some(EquipmentSlot::Head)).unwrap().base,
            ArmorConfig::vanilla_values().head.base
        );
        assert!(equipment_layer_load_config(uuid(12), true, false, true, Some(EquipmentSlot::OffHand)).is_none());
        assert_eq!(
            equipment_layer_load_config(uuid(12), true, false, false, Some(EquipmentSlot::Head)).unwrap().base,
            ArmorConfig::vanilla_values().hat.base
        );
        // no equippable or no config -> nothing
        assert!(equipment_layer_load_config(uuid(12), false, false, false, None).is_none());
        assert!(equipment_layer_load_config(uuid(13), true, false, false, None).is_none());

        // the wrap conditions fall open with no piece picked
        let none: Option<PieceView> = None;
        assert!(equipment_layer_hide_base(&none) && equipment_layer_hide_glint(&none) && equipment_layer_hide_trim(&none));
        let _ = equipment_layer_load_config(uuid(12), true, true, false, None).unwrap();
        let shown = Some(PieceView { base: true, glint: true, trim: None });
        assert!(equipment_layer_hide_base(&shown));
    }

    #[test]
    fn wildfire_compat_cancels_chest_layers() {
        let _ = stamp(14);
        mod_config::with_instance_mut(|c| {
            c.global.chest.base = false;
            c.global.chest.trim = false;
            c.global.chest.glint = false;
        });
        let _ = stamp(14);
        assert!(wildfiregender::hide_breast_armor(uuid(14)));
        assert!(wildfiregender::hide_armor_trim(uuid(14)));
        assert!(wildfiregender::hide_glint(uuid(14)));
        // no config -> no cancel
        assert!(!wildfiregender::hide_breast_armor(uuid(15)));
    }
}
