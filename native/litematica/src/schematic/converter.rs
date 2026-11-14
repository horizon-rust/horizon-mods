use malilib::nbt::NbtTag;
use malilib::util::BlockPos;
use super::format::{LitematicaSchematic, SubRegion};
use super::palette::BlockState;

pub struct SchematicConverter;

impl SchematicConverter {
    pub fn from_vanilla_structure(name: &str, tag: &NbtTag) -> Option<LitematicaSchematic> {
        let size_list = tag.get_list("size")?;
        if size_list.len() < 3 {
            return None;
        }
        let size_x = size_list[0].as_i32()?;
        let size_y = size_list[1].as_i32()?;
        let size_z = size_list[2].as_i32()?;
        let size = BlockPos::new(size_x, size_y, size_z);

        let mut schematic = LitematicaSchematic::new(name, "Vanilla Structure");
        let mut region = SubRegion::new(name, BlockPos::ORIGIN, size);

        // Palette
        let mut palette_states = Vec::new();
        if let Some(pal_list) = tag.get_list("palette") {
            for entry in pal_list {
                if let Some(st) = BlockState::from_nbt(entry) {
                    palette_states.push(st);
                } else {
                    palette_states.push(BlockState::new(BlockState::AIR));
                }
            }
        }

        // Blocks
        if let Some(blocks_list) = tag.get_list("blocks") {
            for block_compound in blocks_list {
                let Some(pos_list) = block_compound.get_list("pos") else { continue; };
                let Some(state_idx) = block_compound.get_i32("state") else { continue; };

                if pos_list.len() >= 3 {
                    let bx = pos_list[0].as_i32().unwrap_or(0);
                    let by = pos_list[1].as_i32().unwrap_or(0);
                    let bz = pos_list[2].as_i32().unwrap_or(0);

                    if let Some(st) = palette_states.get(state_idx as usize) {
                        region.set_block_state(bx, by, bz, st.clone());
                    }
                }
            }
        }

        schematic.regions.insert(name.to_string(), region);
        Some(schematic)
    }
}
