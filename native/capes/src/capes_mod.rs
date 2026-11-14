//! Upstream `Capes.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::config::CapeConfig;
use crate::handler::GameProfile;
use crate::handler::PlayerHandler;
use std::cell::RefCell;
use std::rc::Rc;

/// `net.minecraft.network.chat.Component` seam: enough surface for the
/// debug info and menu labels this mod builds.
#[derive(Debug, Clone, PartialEq)]
pub enum Component {
    Empty,
    Literal(String),
    Translatable(String, String),
    /// `Component.empty().append(..)` chains, in order.
    Appended(Vec<String>),
}

impl Component {
    pub fn literal(text: impl Into<String>) -> Self {
        Self::Literal(text.into())
    }

    pub fn translatable(key: impl Into<String>, arg: impl Into<String>) -> Self {
        Self::Translatable(key.into(), arg.into())
    }

    /// `CommonComponents.optionStatus(name, enabled)`.
    pub fn option_status(name: Component, enabled: bool) -> Self {
        Self::Literal(format!(
            "{}: {}",
            match name {
                Component::Literal(t) => t,
                other => format!("{:?}", other),
            },
            if enabled { "ON" } else { "OFF" }
        ))
    }

    /// `.append(text)` for the empty base component.
    pub fn append(mut self, text: impl Into<String>) -> Self {
        match &mut self {
            Self::Appended(parts) => parts.push(text.into()),
            _ => self = Self::Appended(vec![text.into()]),
        }
        self
    }
}

/// The config-file surface upstream uses (`configDir/capes.json5` via Gson):
/// reads and writes the serialized config, tracked for tests.
pub trait ConfigIo {
    /// Create-or-read: returns `None` when no file exists (upstream creates
    /// one with the defaults), else the file's contents.
    fn read_config(&self) -> Option<String>;
    fn write_config(&self, json: &str);
}

/// The config backing store upstream creates when the file is missing.
#[derive(Default)]
pub struct MissingConfig;

impl ConfigIo for MissingConfig {
    fn read_config(&self) -> Option<String> {
        None
    }
    fn write_config(&self, _json: &str) {}
}

/// `object Capes : ClientModInitializer`.
pub struct Capes;

thread_local! {
    static CONFIG: RefCell<Option<CapeConfig>> = const { RefCell::new(None) };
    static CONFIG_IO: RefCell<Rc<dyn ConfigIo>> = RefCell::new(Rc::new(MissingConfig));
}

impl Capes {
    /// `const val MOD_ID`.
    pub const MOD_ID: &'static str = "capes";

    /// `val CONFIG by lazy { ... }`: create-or-load the config, defaulting on
    /// any error, and patch a missing `clientCapeType` to `MINECRAFT`.
    pub fn config() -> CapeConfig {
        CONFIG.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(config) = &*slot {
                return config.clone();
            }
            let io = CONFIG_IO.with(|io| io.borrow().clone());
            let config = match io.read_config() {
                None => CapeConfig::default(),
                Some(json) => parse_config(&json).unwrap_or_default(),
            };
            *slot = Some(config.clone());
            config
        })
    }

    /// The current config backing store.
    pub fn config_io() -> Rc<dyn ConfigIo> {
        CONFIG_IO.with(|io| io.borrow().clone())
    }

    /// Test/production seam swap before the first `config()` call.
    pub fn set_config_io(io: Rc<dyn ConfigIo>) {
        CONFIG_IO.with(|slot| *slot.borrow_mut() = io);
    }

    /// A fresh config state for tests (upstream the lazy val lives once per
    /// process).
    pub fn reset_config() {
        CONFIG.with(|slot| *slot.borrow_mut() = None);
    }

    /// The saved config after `save()` calls (test mirror of the file).
    pub fn saved_config_json() -> Option<String> {
        CONFIG_IO.with(|io| io.borrow().read_config())
    }

    /// `fun identifier(id) = Identifier.fromNamespaceAndPath(MOD_ID, id)`.
    pub fn identifier(id: impl Into<String>) -> crate::handler::Identifier {
        crate::handler::Identifier::from_namespace_and_path(Self::MOD_ID, id)
    }

    /// `getDebugInfoForPlayer(profile)`: the multi-line debug body the
    /// `/capes debug` command attaches (the click/hover styling is engine
    /// surface and stays behind the message the command sends).
    pub fn get_debug_info_for_player(profile: &GameProfile) -> String {
        let handler = PlayerHandler::from_profile(profile);
        let handler = handler.borrow();
        format!(
            "Name: {}\nUUID: {}\nType: {}\nIsAnimated: {}\nHasElytraTexture: {}\nURL: {}",
            profile.name,
            profile.id,
            handler
                .cape_type
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "null".into()),
            handler.has_animated_cape,
            handler.has_elytra_texture,
            handler
                .cape_type
                .and_then(|t| t.get_url(profile))
                .unwrap_or_else(|| "null".into())
        )
    }
}

/// Gson field decoding for the config keys the upstream data class carries.
pub fn parse_config(json: &str) -> Option<CapeConfig> {
    let get = |key: &str| -> Option<String> {
        let marker = format!("\"{}\":", key);
        let start = json.find(&marker)? + marker.len();
        let rest = &json[start..];
        let value = rest.trim_start();
        let end = value
            .find(|c| c == ',' || c == '\n' || c == '}')
            .unwrap_or(value.len());
        Some(value[..end].trim().trim_matches('"').to_string())
    };
    Some(CapeConfig {
        client_cape_type: match get("clientCapeType")?.as_str() {
            "MINECRAFT" => crate::config::CapeType::Minecraft,
            "OPTIFINE" => crate::config::CapeType::Optifine,
            "LABYMOD" => crate::config::CapeType::Labymod,
            "MINECRAFTCAPES" => crate::config::CapeType::MinecraftCapes,
            "COSMETICA" => crate::config::CapeType::Cosmetica,
            "CLOAKSPLUS" => crate::config::CapeType::CloaksPlus,
            _ => return None,
        },
        enable_optifine: get("enableOptifine")? == "true",
        enable_labymod: get("enableLabyMod")? == "true",
        enable_minecraft_capes_mod: get("enableMinecraftCapesMod")? == "true",
        enable_cosmetica: get("enableCosmetica")? == "true",
        enable_cloaks_plus: get("enableCloaksPlus")? == "true",
        enable_elytra_texture: get("enableElytraTexture")? == "true",
    })
}
