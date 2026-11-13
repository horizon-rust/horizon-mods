use std::collections::HashMap;
use malilib::util::{BlockPos, LayerRange};
use crate::schematic::LitematicaSchematic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialEntry {
    pub item_name: String,
    pub total_required: usize,
    pub available_count: usize,
    pub missing_count: usize,
}

impl MaterialEntry {
    pub fn new(item_name: impl Into<String>, total_required: usize) -> Self {
        Self {
            item_name: item_name.into(),
            total_required,
            available_count: 0,
            missing_count: total_required,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MaterialList {
    pub entries: HashMap<String, MaterialEntry>,
    pub total_items_required: usize,
    pub total_items_missing: usize,
}

impl MaterialList {
    pub fn from_schematic(schematic: &LitematicaSchematic, layer_range: Option<&LayerRange>) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();

        for region in schematic.regions.values() {
            let sx = region.size.x.abs();
            let sy = region.size.y.abs();
            let sz = region.size.z.abs();

            for y in 0..sy {
                let world_y = region.pos.y + y;
                if let Some(lr) = layer_range {
                    if !lr.is_position_within(BlockPos::new(0, world_y, 0)) {
                        continue;
                    }
                }

                for z in 0..sz {
                    for x in 0..sx {
                        if let Some(st) = region.get_block_state(x, y, z) {
                            if !st.is_air() {
                                let item_id = Self::block_to_item(&st.name);
                                *counts.entry(item_id).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut entries = HashMap::new();
        let mut total_req = 0;

        for (item, count) in counts {
            total_req += count;
            entries.insert(item.clone(), MaterialEntry::new(item, count));
        }

        Self {
            entries,
            total_items_required: total_req,
            total_items_missing: total_req,
        }
    }

    pub fn update_inventory(&mut self, inventory: &HashMap<String, usize>) {
        let mut missing_total = 0;

        for (item, entry) in &mut self.entries {
            let available = inventory.get(item).copied().unwrap_or(0);
            entry.available_count = available;
            entry.missing_count = entry.total_required.saturating_sub(available);
            missing_total += entry.missing_count;
        }

        self.total_items_missing = missing_total;
    }

    pub fn block_to_item(block_name: &str) -> String {
        // Handle common vanilla block-to-item special mappings:
        match block_name {
            "minecraft:wall_torch" => "minecraft:torch".to_string(),
            "minecraft:soul_wall_torch" => "minecraft:soul_torch".to_string(),
            "minecraft:redstone_wall_torch" => "minecraft:redstone_torch".to_string(),
            "minecraft:water" | "minecraft:flowing_water" => "minecraft:water_bucket".to_string(),
            "minecraft:lava" | "minecraft:flowing_lava" => "minecraft:lava_bucket".to_string(),
            other => other.to_string(),
        }
    }

    pub fn sorted_entries(&self) -> Vec<&MaterialEntry> {
        let mut list: Vec<&MaterialEntry> = self.entries.values().collect();
        list.sort_by(|a, b| b.total_required.cmp(&a.total_required).then_with(|| a.item_name.cmp(&b.item_name)));
        list
    }
}
