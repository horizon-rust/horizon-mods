use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use malilib::nbt::{NbtParser, NbtTag, NbtWriter};
use malilib::util::{BlockPos, IntBoundingBox};
use super::bit_packing::PackedBitArray;
use super::palette::{BlockState, BlockStatePalette};

#[derive(Debug, Clone, PartialEq)]
pub struct SubRegion {
    pub name: String,
    pub pos: BlockPos,
    pub size: BlockPos,
    pub palette: BlockStatePalette,
    pub block_states: PackedBitArray,
    pub tile_entities: Vec<NbtTag>,
    pub entities: Vec<NbtTag>,
    pub pending_block_ticks: Vec<NbtTag>,
}

impl SubRegion {
    pub fn new(name: impl Into<String>, pos: BlockPos, size: BlockPos) -> Self {
        let volume = (size.x.abs() * size.y.abs() * size.z.abs()) as usize;
        let palette = BlockStatePalette::new();
        let bits = palette.bits_per_block();
        Self {
            name: name.into(),
            pos,
            size,
            palette,
            block_states: PackedBitArray::new(bits, volume),
            tile_entities: Vec::new(),
            entities: Vec::new(),
            pending_block_ticks: Vec::new(),
        }
    }

    pub fn volume(&self) -> usize {
        (self.size.x.abs() * self.size.y.abs() * self.size.z.abs()) as usize
    }

    pub fn block_index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let sx = self.size.x.abs();
        let sy = self.size.y.abs();
        let sz = self.size.z.abs();

        if x < 0 || x >= sx || y < 0 || y >= sy || z < 0 || z >= sz {
            return None;
        }

        let idx = ((y * sz + z) * sx + x) as usize;
        Some(idx)
    }

    pub fn get_block_state(&self, x: i32, y: i32, z: i32) -> Option<&BlockState> {
        let idx = self.block_index(x, y, z)?;
        let palette_idx = self.block_states.get(idx) as usize;
        self.palette.get(palette_idx)
    }

    pub fn set_block_state(&mut self, x: i32, y: i32, z: i32, state: BlockState) -> bool {
        let Some(idx) = self.block_index(x, y, z) else {
            return false;
        };

        let old_bits = self.palette.bits_per_block();
        let palette_idx = self.palette.get_or_add(state) as u32;
        let new_bits = self.palette.bits_per_block();

        if new_bits > old_bits {
            let mut new_arr = PackedBitArray::new(new_bits, self.volume());
            for i in 0..self.volume() {
                new_arr.set(i, self.block_states.get(i));
            }
            self.block_states = new_arr;
        }

        self.block_states.set(idx, palette_idx);
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LitematicaSchematic {
    pub name: String,
    pub author: String,
    pub description: String,
    pub time_created: u64,
    pub time_modified: u64,
    pub total_blocks: u64,
    pub total_volume: u64,
    pub enclosing_size: BlockPos,
    pub regions: HashMap<String, SubRegion>,
    pub preview_image: Option<Vec<u8>>,
}

impl Default for LitematicaSchematic {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            author: "".to_string(),
            description: "".to_string(),
            time_created: 0,
            time_modified: 0,
            total_blocks: 0,
            total_volume: 0,
            enclosing_size: BlockPos::new(0, 0, 0),
            regions: HashMap::new(),
            preview_image: None,
        }
    }
}

impl LitematicaSchematic {
    pub fn new(name: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            author: author.into(),
            ..Default::default()
        }
    }

    pub fn enclosing_box(&self) -> IntBoundingBox {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut min_z = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut max_z = i32::MIN;

        if self.regions.is_empty() {
            return IntBoundingBox::new(BlockPos::ORIGIN, BlockPos::ORIGIN);
        }

        for region in self.regions.values() {
            let p1 = region.pos;
            let p2 = BlockPos::new(
                p1.x + region.size.x - if region.size.x > 0 { 1 } else { -1 },
                p1.y + region.size.y - if region.size.y > 0 { 1 } else { -1 },
                p1.z + region.size.z - if region.size.z > 0 { 1 } else { -1 },
            );
            let b = IntBoundingBox::new(p1, p2);
            min_x = min_x.min(b.min_x);
            min_y = min_y.min(b.min_y);
            min_z = min_z.min(b.min_z);
            max_x = max_x.max(b.max_x);
            max_y = max_y.max(b.max_y);
            max_z = max_z.max(b.max_z);
        }

        IntBoundingBox {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    pub fn from_nbt(tag: &NbtTag) -> Result<Self, String> {
        let mut schematic = Self::default();

        if let Some(meta) = tag.get_compound("Metadata") {
            if let Some(n) = meta.get("Name").and_then(|t| t.as_str()) {
                schematic.name = n.to_string();
            }
            if let Some(a) = meta.get("Author").and_then(|t| t.as_str()) {
                schematic.author = a.to_string();
            }
            if let Some(d) = meta.get("Description").and_then(|t| t.as_str()) {
                schematic.description = d.to_string();
            }
            if let Some(tc) = meta.get("TimeCreated").and_then(|t| t.as_i64()) {
                schematic.time_created = tc as u64;
            }
            if let Some(tm) = meta.get("TimeModified").and_then(|t| t.as_i64()) {
                schematic.time_modified = tm as u64;
            }
            if let Some(tb) = meta.get("TotalBlocks").and_then(|t| t.as_i64()) {
                schematic.total_blocks = tb as u64;
            }
            if let Some(tv) = meta.get("TotalVolume").and_then(|t| t.as_i64()) {
                schematic.total_volume = tv as u64;
            }
            if let Some(enc) = meta.get("EnclosingSize").and_then(|t| t.as_compound()) {
                schematic.enclosing_size = BlockPos::new(
                    enc.get("x").and_then(|t| t.as_i32()).unwrap_or(0),
                    enc.get("y").and_then(|t| t.as_i32()).unwrap_or(0),
                    enc.get("z").and_then(|t| t.as_i32()).unwrap_or(0),
                );
            }
            if let Some(preview) = meta.get("PreviewImageData").and_then(|t| t.as_int_array()) {
                let bytes: Vec<u8> = preview.iter().flat_map(|i| i.to_be_bytes()).collect();
                schematic.preview_image = Some(bytes);
            }
        }

        if let Some(regions_compound) = tag.get_compound("Regions") {
            for (region_name, region_tag) in regions_compound {
                let Some(reg_comp) = region_tag.as_compound() else {
                    continue;
                };

                let pos = if let Some(p) = reg_comp.get("Position").and_then(|t| t.as_compound()) {
                    BlockPos::new(
                        p.get("x").and_then(|t| t.as_i32()).unwrap_or(0),
                        p.get("y").and_then(|t| t.as_i32()).unwrap_or(0),
                        p.get("z").and_then(|t| t.as_i32()).unwrap_or(0),
                    )
                } else {
                    BlockPos::ORIGIN
                };

                let size = if let Some(s) = reg_comp.get("Size").and_then(|t| t.as_compound()) {
                    BlockPos::new(
                        s.get("x").and_then(|t| t.as_i32()).unwrap_or(0),
                        s.get("y").and_then(|t| t.as_i32()).unwrap_or(0),
                        s.get("z").and_then(|t| t.as_i32()).unwrap_or(0),
                    )
                } else {
                    BlockPos::ORIGIN
                };

                let mut palette = BlockStatePalette::new();
                if let Some(pal_list) = reg_comp.get("BlockStatePalette").and_then(|t| t.as_list()) {
                    for entry in pal_list {
                        if let Some(state) = BlockState::from_nbt(entry) {
                            palette.get_or_add(state);
                        }
                    }
                }

                let bits = palette.bits_per_block();
                let volume = (size.x.abs() * size.y.abs() * size.z.abs()) as usize;

                let block_states = if let Some(raw_longs) = reg_comp.get("BlockStates").and_then(|t| t.as_long_array()) {
                    let u64s: Vec<u64> = raw_longs.iter().map(|&l| l as u64).collect();
                    PackedBitArray::from_raw(bits, volume, u64s)
                } else {
                    PackedBitArray::new(bits, volume)
                };

                let tile_entities = reg_comp
                    .get("TileEntities")
                    .and_then(|t| t.as_list())
                    .map(|l| l.to_vec())
                    .unwrap_or_default();

                let entities = reg_comp
                    .get("Entities")
                    .and_then(|t| t.as_list())
                    .map(|l| l.to_vec())
                    .unwrap_or_default();

                let pending_block_ticks = reg_comp
                    .get("PendingBlockTicks")
                    .and_then(|t| t.as_list())
                    .map(|l| l.to_vec())
                    .unwrap_or_default();

                schematic.regions.insert(
                    region_name.clone(),
                    SubRegion {
                        name: region_name.clone(),
                        pos,
                        size,
                        palette,
                        block_states,
                        tile_entities,
                        entities,
                        pending_block_ticks,
                    },
                );
            }
        }

        Ok(schematic)
    }

    pub fn to_nbt(&self) -> NbtTag {
        let mut root = HashMap::new();

        root.insert("Version".to_string(), NbtTag::Int(6));
        root.insert("SubVersion".to_string(), NbtTag::Int(1));

        // Metadata
        let mut meta = HashMap::new();
        meta.insert("Name".to_string(), NbtTag::String(self.name.clone()));
        meta.insert("Author".to_string(), NbtTag::String(self.author.clone()));
        meta.insert("Description".to_string(), NbtTag::String(self.description.clone()));
        meta.insert("TimeCreated".to_string(), NbtTag::Long(self.time_created as i64));
        meta.insert("TimeModified".to_string(), NbtTag::Long(self.time_modified as i64));
        meta.insert("RegionCount".to_string(), NbtTag::Int(self.regions.len() as i32));

        let enc_box = self.enclosing_box();
        let enc_size = BlockPos::new(enc_box.size_x(), enc_box.size_y(), enc_box.size_z());
        let mut enc_map = HashMap::new();
        enc_map.insert("x".to_string(), NbtTag::Int(enc_size.x));
        enc_map.insert("y".to_string(), NbtTag::Int(enc_size.y));
        enc_map.insert("z".to_string(), NbtTag::Int(enc_size.z));
        meta.insert("EnclosingSize".to_string(), NbtTag::Compound(enc_map));

        let mut total_blocks = 0u64;
        let mut total_vol = 0u64;
        for r in self.regions.values() {
            total_vol += r.volume() as u64;
            for i in 0..r.volume() {
                let p_idx = r.block_states.get(i) as usize;
                if let Some(st) = r.palette.get(p_idx) {
                    if !st.is_air() {
                        total_blocks += 1;
                    }
                }
            }
        }
        meta.insert("TotalBlocks".to_string(), NbtTag::Long(total_blocks as i64));
        meta.insert("TotalVolume".to_string(), NbtTag::Long(total_vol as i64));

        root.insert("Metadata".to_string(), NbtTag::Compound(meta));

        // Regions
        let mut regions_map = HashMap::new();
        for (reg_name, reg) in &self.regions {
            let mut reg_compound = HashMap::new();

            let mut pos_map = HashMap::new();
            pos_map.insert("x".to_string(), NbtTag::Int(reg.pos.x));
            pos_map.insert("y".to_string(), NbtTag::Int(reg.pos.y));
            pos_map.insert("z".to_string(), NbtTag::Int(reg.pos.z));
            reg_compound.insert("Position".to_string(), NbtTag::Compound(pos_map));

            let mut size_map = HashMap::new();
            size_map.insert("x".to_string(), NbtTag::Int(reg.size.x));
            size_map.insert("y".to_string(), NbtTag::Int(reg.size.y));
            size_map.insert("z".to_string(), NbtTag::Int(reg.size.z));
            reg_compound.insert("Size".to_string(), NbtTag::Compound(size_map));

            // BlockStatePalette
            let pal_list: Vec<NbtTag> = reg.palette.states().iter().map(|s| s.to_nbt()).collect();
            reg_compound.insert("BlockStatePalette".to_string(), NbtTag::List(pal_list));

            // BlockStates
            let raw_i64s: Vec<i64> = reg.block_states.raw_data().iter().map(|&u| u as i64).collect();
            reg_compound.insert("BlockStates".to_string(), NbtTag::LongArray(raw_i64s));

            // Entities, TileEntities
            reg_compound.insert("TileEntities".to_string(), NbtTag::List(reg.tile_entities.clone()));
            reg_compound.insert("Entities".to_string(), NbtTag::List(reg.entities.clone()));
            reg_compound.insert("PendingBlockTicks".to_string(), NbtTag::List(reg.pending_block_ticks.clone()));

            regions_map.insert(reg_name.clone(), NbtTag::Compound(reg_compound));
        }
        root.insert("Regions".to_string(), NbtTag::Compound(regions_map));

        NbtTag::Compound(root)
    }

    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = fs::File::open(path)?;
        let (_, tag) = NbtParser::read_compressed(file)?;
        Self::from_nbt(&tag).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let tag = self.to_nbt();
        let file = fs::File::create(path)?;
        NbtWriter::write_compressed("Litematica", &tag, file)
    }
}
