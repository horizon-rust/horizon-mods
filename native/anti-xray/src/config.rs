use std::collections::BTreeMap;

use mod_api::{ConfigFileError, ConfigPath, ServerHost};
use serde::Deserialize;

pub(crate) const DIMENSIONS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    defaults: Overrides,
    dimensions: BTreeMap<String, Overrides>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Overrides {
    enabled: Option<bool>,
    mode: Option<u8>,
    hidden_blocks: Option<Vec<String>>,
    replacement_blocks: Option<Vec<String>>,
    max_block_height: Option<i32>,
    update_radius: Option<i32>,
    lava_obscures: Option<bool>,
    permission_bypass: Option<bool>,
}

pub(crate) struct Policy<'a> {
    pub enabled: bool,
    pub mode: u8,
    pub hidden: &'a [String],
    pub replacements: &'a [String],
    pub max_y: i32,
    pub radius: u8,
    pub lava_obscures: bool,
    pub bypass: bool,
}

impl Config {
    pub fn load(host: &dyn ServerHost) -> Result<Self, String> {
        let path = ConfigPath::new("anti_xray.toml").expect("static config name");
        let bytes = match host.config_files().read(&path) {
            Ok(bytes) => bytes,
            Err(ConfigFileError::NotFound) => return Ok(Self::default()),
            Err(error) => return Err(error.to_string()),
        };
        Self::parse(std::str::from_utf8(&bytes).map_err(|error| error.to_string())?)
    }

    fn parse(source: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(source).map_err(|error| error.to_string())?;
        for dimension in config.dimensions.keys() {
            if !DIMENSIONS.contains(&dimension.as_str()) {
                return Err(format!("dimensions.{dimension}: unknown dimension"));
            }
        }
        Ok(config)
    }

    pub fn policy(&self, dimension: &str) -> Result<Policy<'_>, String> {
        let base = &self.defaults;
        let local = self.dimensions.get(dimension).unwrap_or(base);
        let mode = local.mode.or(base.mode).unwrap_or(1);
        if !(1..=3).contains(&mode) {
            return Err(format!("{dimension}.mode: expected 1, 2, or 3, got {mode}"));
        }
        Ok(Policy {
            enabled: local.enabled.or(base.enabled).unwrap_or(false),
            mode,
            hidden: local
                .hidden_blocks
                .as_deref()
                .or(base.hidden_blocks.as_deref())
                .unwrap_or(&[]),
            replacements: local
                .replacement_blocks
                .as_deref()
                .or(base.replacement_blocks.as_deref())
                .unwrap_or(&[]),
            max_y: local
                .max_block_height
                .or(base.max_block_height)
                .unwrap_or(64)
                .div_euclid(16)
                * 16,
            radius: local
                .update_radius
                .or(base.update_radius)
                .unwrap_or(2)
                .clamp(0, 2) as u8,
            lava_obscures: local.lava_obscures.or(base.lava_obscures).unwrap_or(false),
            bypass: local
                .permission_bypass
                .or(base.permission_bypass)
                .unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_and_radius_are_normalized_after_merging_dimension_overrides() {
        let config = Config::parse(
            r#"
            [defaults]
            enabled = true
            update_radius = 12
            max_block_height = 63
            ["dimensions"."minecraft:the_nether"]
            enabled = false
            update_radius = -1
            max_block_height = -1
        "#,
        )
        .unwrap();
        let overworld = config.policy(DIMENSIONS[0]).unwrap();
        assert!(overworld.enabled);
        assert_eq!((overworld.max_y, overworld.radius), (48, 2));
        let nether = config.policy(DIMENSIONS[1]).unwrap();
        assert!(!nether.enabled);
        assert_eq!((nether.max_y, nether.radius), (-16, 0));
    }
}
