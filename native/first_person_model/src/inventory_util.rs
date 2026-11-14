//! Upstream InventoryUtil.java, 1:1. The //? preprocessor branches collapse to
//! the >= 1.21.5 path (Horizon's 26.3 tree).

use crate::seams::{Entity, Inventory, ItemStack};

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
