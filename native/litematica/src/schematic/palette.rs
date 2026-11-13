use std::collections::BTreeMap;
use std::fmt;
use malilib::nbt::NbtTag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockState {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

impl Default for BlockState {
    fn default() -> Self {
        Self {
            name: "minecraft:air".to_string(),
            properties: BTreeMap::new(),
        }
    }
}

impl BlockState {
    pub const AIR: &'static str = "minecraft:air";

    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: BTreeMap::new(),
        }
    }

    pub fn with_property(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.properties.insert(key.into(), val.into());
        self
    }

    pub fn is_air(&self) -> bool {
        self.name == "minecraft:air" || self.name == "minecraft:cave_air" || self.name == "minecraft:void_air"
    }

    pub fn from_string(s: &str) -> Self {
        if let Some(bracket_pos) = s.find('[') {
            let name = &s[..bracket_pos];
            let props_str = &s[bracket_pos + 1..s.len().saturating_sub(1)];
            let mut properties = BTreeMap::new();
            for part in props_str.split(',') {
                if let Some((k, v)) = part.split_once('=') {
                    properties.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            Self {
                name: name.trim().to_string(),
                properties,
            }
        } else {
            Self {
                name: s.trim().to_string(),
                properties: BTreeMap::new(),
            }
        }
    }

    pub fn from_nbt(tag: &NbtTag) -> Option<Self> {
        let name = tag.get_str("Name")?.to_string();
        let mut properties = BTreeMap::new();

        if let Some(props_tag) = tag.get_compound("Properties") {
            for (k, v) in props_tag {
                if let Some(val_str) = v.as_str() {
                    properties.insert(k.clone(), val_str.to_string());
                }
            }
        }

        Some(Self { name, properties })
    }

    pub fn to_nbt(&self) -> NbtTag {
        let mut map = std::collections::HashMap::new();
        map.insert("Name".to_string(), NbtTag::String(self.name.clone()));

        if !self.properties.is_empty() {
            let mut props_map = std::collections::HashMap::new();
            for (k, v) in &self.properties {
                props_map.insert(k.clone(), NbtTag::String(v.clone()));
            }
            map.insert("Properties".to_string(), NbtTag::Compound(props_map));
        }

        NbtTag::Compound(map)
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.properties.is_empty() {
            write!(f, "{}", self.name)
        } else {
            let props_formatted: Vec<_> = self.properties.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            write!(f, "{}[{}]", self.name, props_formatted.join(","))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BlockStatePalette {
    states: Vec<BlockState>,
    index_map: std::collections::HashMap<BlockState, usize>,
}

impl BlockStatePalette {
    pub fn new() -> Self {
        let air = BlockState::new(BlockState::AIR);
        let mut pal = Self {
            states: Vec::new(),
            index_map: std::collections::HashMap::new(),
        };
        pal.get_or_add(air);
        pal
    }

    pub fn get_or_add(&mut self, state: BlockState) -> usize {
        if let Some(&idx) = self.index_map.get(&state) {
            idx
        } else {
            let idx = self.states.len();
            self.index_map.insert(state.clone(), idx);
            self.states.push(state);
            idx
        }
    }

    pub fn get(&self, index: usize) -> Option<&BlockState> {
        self.states.get(index)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn states(&self) -> &[BlockState] {
        &self.states
    }

    pub fn bits_per_block(&self) -> usize {
        let count = self.states.len().max(2);
        (usize::BITS - (count - 1).leading_zeros()) as usize
    }
}
