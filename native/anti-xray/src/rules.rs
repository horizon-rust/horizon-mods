use std::collections::HashSet;

use mod_api::{
    BlockMetadata, BlockState, BlockStateRange, HostBlockStates, ServerHost, TerrainBand,
    TerrainPattern, TerrainView,
};

use crate::config::{DIMENSIONS, Policy};

pub(crate) struct Cover {
    solid: Vec<BlockState>,
    lava: Vec<BlockState>,
}

impl Cover {
    pub fn read(host: &dyn ServerHost) -> Result<Self, String> {
        let facts = host.block_states();
        let mut cover = Self {
            solid: Vec::new(),
            lava: Vec::new(),
        };
        let lava = host
            .block_key("minecraft:lava")
            .ok_or("missing lava block")?;
        let water = host
            .block_key("minecraft:water")
            .ok_or("missing water block")?;
        let keys = host.block_keys();
        if keys.is_empty() {
            return Err("empty block registry".into());
        }
        for key in keys {
            let states = facts.states_of(key).ok_or_else(|| {
                format!(
                    "missing state range for {}",
                    facts.name(key).unwrap_or("<unknown block>")
                )
            })?;
            for state in states.iter() {
                if key == lava {
                    cover.lava.push(state);
                } else if key != water
                    && facts.is_solid_render(state).ok_or_else(|| {
                        format!(
                            "missing solid-render fact for {}",
                            facts.name(key).unwrap_or("<unknown block>")
                        )
                    })?
                {
                    cover.solid.push(state);
                }
            }
        }
        Ok(cover)
    }

    fn states(&self, lava_obscures: bool) -> Vec<BlockState> {
        let mut states = self.solid.clone();
        if lava_obscures {
            states.extend_from_slice(&self.lava);
        }
        states
    }
}

fn canonical_block(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    }
}

fn air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Resolve each entry to its whole eligible state range. Repeating an entry
/// contributes another default state to the decoy list, before targets are deduplicated.
fn resolve(
    host: &dyn ServerHost,
    dimension: &str,
    setting: &str,
    names: &[String],
    mut add: impl FnMut(&str, BlockStateRange) -> Result<(), String>,
) -> Result<(), String> {
    let facts = host.block_states();
    for name in names {
        let id = canonical_block(name);
        if air(&id) {
            continue;
        }
        let result = host
            .block_key(&id)
            .and_then(|key| facts.states_of(key))
            .ok_or_else(|| format!("unknown block {name:?}"))
            .and_then(|states| add(&id, states));
        result.map_err(|error| format!("{dimension}.{setting}: {error}"))?;
    }
    Ok(())
}

pub(crate) fn compile(
    host: &dyn ServerHost,
    dimension: &str,
    policy: &Policy<'_>,
    cover: &Cover,
    seed: u64,
) -> Result<TerrainView, String> {
    let decoys = policy.mode != 1;
    let facts = host.block_states();
    let mut targets = Vec::new();
    let mut choices = Vec::new();
    resolve(
        host,
        dimension,
        "hidden_blocks",
        policy.hidden,
        |id, states| {
            if decoys {
                let state = host
                    .block_state_id(id)
                    .ok_or_else(|| format!("missing default state for {id}"))?;
                match facts.has_block_entity(state) {
                    Some(false) => choices.push(state),
                    Some(true) => {
                        return Err(format!("{id} has a block entity and cannot be a decoy"));
                    }
                    None => return Err(format!("missing block-entity fact for {id}")),
                }
            }
            targets.extend(states.iter());
            Ok(())
        },
    )?;
    if decoys {
        if choices.is_empty() || choices.len() > 256 {
            return Err(format!(
                "{dimension}.hidden_blocks: expected 1–256 decoy entries after ignoring air"
            ));
        }
        resolve(
            host,
            dimension,
            "replacement_blocks",
            policy.replacements,
            |_, states| {
                targets.extend(states.iter());
                Ok(())
            },
        )?;
    }
    let mut seen = HashSet::new();
    targets.retain(|state| seen.insert(*state));

    let mut bands = Vec::new();
    // A bound at the bottom of the coordinate range makes this policy empty.
    if !targets.is_empty() && policy.max_y > i32::MIN {
        if decoys {
            let pattern = if policy.mode == 2 {
                TerrainPattern::Block(seed)
            } else {
                TerrainPattern::Layer(seed)
            };
            bands.push(TerrainBand::new(
                i32::MIN,
                policy.max_y,
                targets,
                choices,
                pattern,
            ));
        } else {
            bands = fixed_bands(host, dimension, policy.max_y, targets)?;
        }
    }

    Ok(TerrainView::new(
        bands,
        cover.states(policy.lava_obscures),
        true,
        policy.radius,
    ))
}

fn fixed_bands(
    host: &dyn ServerHost,
    dimension: &str,
    max_y: i32,
    targets: Vec<BlockState>,
) -> Result<Vec<TerrainBand>, String> {
    let fixed = |name: &str| {
        host.block_state_id(name)
            .map(|state| vec![state])
            .ok_or_else(|| format!("{dimension}.mode: missing replacement {name}"))
    };
    let mut bands = Vec::new();
    match dimension {
        d if d == DIMENSIONS[0] => {
            bands.push(TerrainBand::new(
                i32::MIN,
                max_y.min(0),
                targets.clone(),
                fixed("minecraft:deepslate")?,
                TerrainPattern::Block(0),
            ));
            if max_y > 0 {
                bands.push(TerrainBand::new(
                    0,
                    max_y,
                    targets,
                    fixed("minecraft:stone")?,
                    TerrainPattern::Block(0),
                ));
            }
        }
        d => {
            let replacement = if d == DIMENSIONS[1] {
                "minecraft:netherrack"
            } else {
                "minecraft:end_stone"
            };
            bands.push(TerrainBand::new(
                i32::MIN,
                max_y,
                targets,
                fixed(replacement)?,
                TerrainPattern::Block(0),
            ));
        }
    }
    Ok(bands)
}
