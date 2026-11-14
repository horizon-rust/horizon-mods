//! Upstream `CapeConfig.kt` + `CapeType.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::capes_mod::{Capes, Component};
use crate::handler::GameProfile;

/// `class CapeConfig` (fields + `save()`; the file write goes through the
/// [`ConfigIo`] seam upstream performs against `FabricLoader`'s config dir).
#[derive(Debug, Clone, PartialEq)]
pub struct CapeConfig {
    pub client_cape_type: CapeType,
    pub enable_optifine: bool,
    pub enable_labymod: bool,
    pub enable_minecraft_capes_mod: bool,
    pub enable_cosmetica: bool,
    pub enable_cloaks_plus: bool,
    pub enable_elytra_texture: bool,
}

impl Default for CapeConfig {
    fn default() -> Self {
        Self {
            client_cape_type: CapeType::Minecraft,
            enable_optifine: true,
            enable_labymod: false,
            enable_minecraft_capes_mod: false,
            enable_cosmetica: false,
            enable_cloaks_plus: false,
            enable_elytra_texture: true,
        }
    }
}

impl CapeConfig {
    /// `save()`: serialize the config and write it to `capes.json5`.
    pub fn save(&self) {
        Capes::config_io().write_config(&self.to_json());
    }

    /// The JSON shape Gson emits for the config (field order preserved).
    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"clientCapeType\": \"{}\",\n  \"enableOptifine\": {},\n  \"enableLabyMod\": {},\n  \"enableMinecraftCapesMod\": {},\n  \"enableCosmetica\": {},\n  \"enableCloaksPlus\": {},\n  \"enableElytraTexture\": {}\n}}",
            self.client_cape_type.name(),
            self.enable_optifine,
            self.enable_labymod,
            self.enable_minecraft_capes_mod,
            self.enable_cosmetica,
            self.enable_cloaks_plus,
            self.enable_elytra_texture
        )
    }
}

/// `enum class CapeType(val stylized: String)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapeType {
    Minecraft,
    Optifine,
    Labymod,
    MinecraftCapes,
    Cosmetica,
    CloaksPlus,
}

impl CapeType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Minecraft => "MINECRAFT",
            Self::Optifine => "OPTIFINE",
            Self::Labymod => "LABYMOD",
            Self::MinecraftCapes => "MINECRAFTCAPES",
            Self::Cosmetica => "COSMETICA",
            Self::CloaksPlus => "CLOAKSPLUS",
        }
    }

    /// `stylized`.
    pub fn stylized(&self) -> &'static str {
        match self {
            Self::Minecraft => "Minecraft",
            Self::Optifine => "OptiFine",
            Self::Labymod => "LabyMod",
            Self::MinecraftCapes => "MinecraftCapes",
            Self::Cosmetica => "Cosmetica",
            Self::CloaksPlus => "Cloaks+",
        }
    }

    /// `cycle()`.
    pub fn cycle(&self) -> Self {
        match self {
            Self::Minecraft => Self::Optifine,
            Self::Optifine => Self::Labymod,
            Self::Labymod => Self::Cosmetica,
            Self::Cosmetica => Self::MinecraftCapes,
            Self::MinecraftCapes => Self::CloaksPlus,
            Self::CloaksPlus => Self::Minecraft,
        }
    }

    /// `getURL(profile)`: the cape URL for this type when the provider is
    /// enabled in the config, else null.
    pub fn get_url(&self, profile: &GameProfile) -> Option<String> {
        let config = Capes::config();
        match self {
            Self::Optifine => config
                .enable_optifine
                .then(|| format!("http://s.optifine.net/capes/{}.png", profile.name)),
            Self::Labymod => config
                .enable_labymod
                .then(|| format!("https://dl.labymod.net/capes/{}", profile.id)),
            Self::Cosmetica => config
                .enable_cosmetica
                .then(|| format!("https://api.cosmetica.cc/users/{}/cape", profile.id)),
            Self::MinecraftCapes => config.enable_minecraft_capes_mod.then(|| {
                format!(
                    "https://api.minecraftcapes.net/profile/{}",
                    profile.id.to_string().replace('-', "")
                )
            }),
            Self::CloaksPlus => config
                .enable_cloaks_plus
                .then(|| format!("http://161.35.130.99/capes/{}.png", profile.name)),
            Self::Minecraft => None,
        }
    }

    /// CapeType variants in declaration order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Minecraft,
            Self::Optifine,
            Self::Labymod,
            Self::MinecraftCapes,
            Self::Cosmetica,
            Self::CloaksPlus,
        ]
    }

    /// `getToggleText(enabled)`.
    pub fn get_toggle_text(&self, enabled: bool) -> Component {
        Component::option_status(Component::literal(self.stylized()), enabled)
    }

    /// `getText()`.
    pub fn get_text(&self) -> Component {
        Component::translatable("options.capes.capetype", self.stylized())
    }
}
