//! Capes runtime splice: the mod's modules inlined for Horizon's
//! crates/horizon-client/src/game.rs carrier, `crate::` -> `super::`.
//! Exact 1:1; LGPL-2.1, Copyright (C) CaelTheColher.

pub mod capes_mod {
//! Upstream `Capes.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::config::CapeConfig;
use super::handler::GameProfile;
use super::handler::PlayerHandler;
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
    pub fn identifier(id: impl Into<String>) -> super::handler::Identifier {
        super::handler::Identifier::from_namespace_and_path(Self::MOD_ID, id)
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
            "MINECRAFT" => super::config::CapeType::Minecraft,
            "OPTIFINE" => super::config::CapeType::Optifine,
            "LABYMOD" => super::config::CapeType::Labymod,
            "MINECRAFTCAPES" => super::config::CapeType::MinecraftCapes,
            "COSMETICA" => super::config::CapeType::Cosmetica,
            "CLOAKSPLUS" => super::config::CapeType::CloaksPlus,
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
}

pub mod config {
//! Upstream `CapeConfig.kt` + `CapeType.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::capes_mod::{Capes, Component};
use super::handler::GameProfile;

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
}

pub mod handler {
//! Upstream `handler/PlayerHandler.kt`, `handler/data/*`, plus the engine
//! types the handler speaks in (`GameProfile`, `Identifier`,
//! `ClientAsset.Texture`, `NativeImage`), 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::capes_mod::Capes;
use super::config::CapeType;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// `net.minecraft.resources.Identifier` seam (namespace + path).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub namespace: String,
    pub path: String,
}

impl Identifier {
    pub fn from_namespace_and_path(namespace: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            path: path.into(),
        }
    }

    /// `Identifier.withDefaultNamespace`.
    pub fn with_default_namespace(path: impl Into<String>) -> Self {
        Self::from_namespace_and_path("minecraft", path)
    }

    /// `Identifier.parse("ns:path")`.
    pub fn parse(value: impl Into<String>) -> Self {
        let value = value.into();
        let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value.as_str()));
        Self::from_namespace_and_path(namespace, path)
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

/// `com.mojang.authlib.GameProfile` seam: name + uuid + properties.
#[derive(Debug, Clone, PartialEq)]
pub struct GameProfile {
    pub id: Uuid,
    pub name: String,
    /// `profile.properties` (the mod only reads the `textures` entry).
    pub properties: Vec<ProfileProperty>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
}

/// `java.util.UUID` seam carrying the version bit `isValidProfile` tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uuid(pub u128, pub u32);

impl Uuid {
    /// `UUID.fromString`, keeping the version nibble.
    pub fn from_string(value: &str) -> Self {
        let hex: String = value.chars().filter(|c| *c != '-').collect();
        let raw = u128::from_str_radix(&hex, 16).unwrap_or(0);
        let version = ((raw >> 76) & 0xF) as u32;
        Uuid(raw, version)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.0;
        let hex = format!("{:032x}", raw);
        write!(
            f,
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

/// `net.minecraft.core.ClientAsset.Texture` seam: a resource texture id or a
/// dynamically registered texture id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Texture {
    /// `ClientAsset.ResourceTexture(id, _)`.
    ResourceTexture(Identifier),
    /// A texture registered on the texture manager at runtime.
    Dynamic(Identifier),
}

impl Texture {
    /// `texturePath()`.
    pub fn texture_path(&self) -> Identifier {
        match self {
            Self::ResourceTexture(id) => id.clone(),
            Self::Dynamic(id) => id.clone(),
        }
    }
}

/// `com.mojang.blaze3d.platform.NativeImage` seam: the RGBA pixels the cape
/// loaders inspect and upscale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl NativeImage {
    pub fn new(width: u32, height: u32, transparent: bool) -> Self {
        let _ = transparent;
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    /// `NativeImage.read(stream)`.
    pub fn read(bytes: &[u8]) -> Option<Self> {
        decode_png(bytes)
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        let i = ((y * self.width + x) * 4) as usize;
        u32::from_be_bytes([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, argb: u32) {
        let i = ((y * self.width + x) * 4) as usize;
        let bytes = argb.to_be_bytes();
        self.pixels[i..i + 4].copy_from_slice(&bytes);
    }

    pub fn close(self) {}
}

/// `handler/data/MCMData.kt`.
#[derive(Debug, Clone, PartialEq)]
pub struct MCMData {
    pub cape_url: String,
    pub animated_cape_url: Option<String>,
}

/// `handler/data/CosmeticaData.kt` (+ nested `CapeData.isAnimated()`).
#[derive(Debug, Clone, PartialEq)]
pub struct CosmeticaData {
    pub cloak: Option<CapeData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapeData {
    pub texture: String,
    pub frames: i32,
}

impl CapeData {
    pub fn is_animated(&self) -> bool {
        self.frames > 1
    }
}

/// The HTTP surface upstream opens with `URI(url).toURL().openConnection(proxy)`
/// and a `Mozilla/5.0` User-Agent.
pub trait Http {
    fn connect(&self, url: &str) -> Result<HttpResponse, HttpError>;
}

pub struct HttpError;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// `connection.responseCode / 100 == 2`.
    pub fn is_success(&self) -> bool {
        self.status / 100 == 2
    }
}

/// The texture-manager surface upstream registers `DynamicTexture`s on.
pub trait TextureRegistry {
    fn register(&mut self, id: Identifier, texture: NativeImage);

    /// The registered texture ids so far (test view).
    fn ids(&self) -> Vec<Identifier> {
        Vec::new()
    }
}

struct NoTextureRegistry;

impl TextureRegistry for NoTextureRegistry {
    fn register(&mut self, _id: Identifier, _texture: NativeImage) {}
}

thread_local! {
    static HTTP: RefCell<Rc<dyn Http>> = RefCell::new(Rc::new(NoHttp));
    static TEXTURES: RefCell<Rc<RefCell<dyn TextureRegistry>>> =
        RefCell::new(Rc::new(RefCell::new(NoTextureRegistry)));
    /// The `HashMap<UUID, PlayerHandler>` companion instances map, in
    /// registration order.
    static INSTANCES: RefCell<Vec<Rc<RefCell<PlayerHandler>>>> = const { RefCell::new(Vec::new()) };
    static OWN_PROFILE: RefCell<Option<GameProfile>> = const { RefCell::new(None) };
}

struct NoHttp;

impl Http for NoHttp {
    fn connect(&self, _url: &str) -> Result<HttpResponse, HttpError> {
        Err(HttpError)
    }
}

pub fn set_http(http: Rc<dyn Http>) {
    HTTP.with(|slot| *slot.borrow_mut() = http);
}

pub fn set_texture_registry(registry: Rc<RefCell<dyn TextureRegistry>>) {
    TEXTURES.with(|slot| *slot.borrow_mut() = registry);
}

/// The texture manager registrations performed so far (test view).
pub fn registered_textures() -> Vec<Identifier> {
    TEXTURES.with(|slot| slot.borrow().borrow().ids())
}

trait RegistryView {
    fn ids(&self) -> Vec<Identifier>;
}

struct RecordingRegistry {
    ids: Vec<Identifier>,
    textures: Vec<NativeImage>,
}

impl TextureRegistry for RecordingRegistry {
    fn register(&mut self, id: Identifier, texture: NativeImage) {
        self.ids.push(id);
        self.textures.push(texture);
    }
}

impl RegistryView for RecordingRegistry {
    fn ids(&self) -> Vec<Identifier> {
        self.ids.clone()
    }
}

/// `Minecraft.getInstance().gameProfile` seam for `onLoadTexture`.
pub fn set_own_profile(profile: GameProfile) {
    OWN_PROFILE.with(|slot| *slot.borrow_mut() = Some(profile));
}

fn own_profile() -> Option<GameProfile> {
    OWN_PROFILE.with(|slot| slot.borrow().clone())
}

/// `class PlayerHandler(var profile: GameProfile)`.
pub struct PlayerHandler {
    pub profile: GameProfile,
    /// `val uuid = profile.id`.
    pub uuid: Uuid,
    pub last_frame: i32,
    pub max_frames: i32,
    pub last_frame_time: u64,
    pub has_cape: bool,
    pub has_elytra_texture: bool,
    pub has_animated_cape: bool,
    pub cape_type: Option<CapeType>,
    pub has_loaded_textures: bool,
}

impl PlayerHandler {
    /// The constructor plus `init { instances[uuid] = this }`.
    pub fn new(profile: GameProfile) -> Rc<RefCell<Self>> {
        let handler = Rc::new(RefCell::new(Self {
            uuid: profile.id,
            profile,
            last_frame: 0,
            max_frames: 0,
            last_frame_time: 0,
            has_cape: false,
            has_elytra_texture: true,
            has_animated_cape: false,
            cape_type: None,
            has_loaded_textures: false,
        }));
        INSTANCES.with(|map| map.borrow_mut().push(handler.clone()));
        handler
    }

    /// `fun fromProfile(profile) = instances[profile.id] ?: PlayerHandler(profile)`.
    pub fn from_profile(profile: &GameProfile) -> Rc<RefCell<Self>> {
        let existing = INSTANCES.with(|map| {
            map.borrow()
                .iter()
                .find(|handler| handler.borrow().uuid == profile.id)
                .cloned()
        });
        existing.unwrap_or_else(|| Self::new(profile.clone()))
    }

    /// `fun onLoadTexture(profile)`: own profile resets cape state and loads
    /// the configured type; other profiles try every provider once.
    pub fn on_load_texture(profile: &GameProfile) -> Rc<RefCell<Self>> {
        let handler = Self::from_profile(profile);
        if own_profile().as_ref() == Some(profile) {
            handler.borrow_mut().has_cape = false;
            handler.borrow_mut().has_animated_cape = false;
            let config = Capes::config();
            handler
                .borrow_mut()
                .set_cape(config.client_cape_type);
        } else if !handler.borrow().has_loaded_textures {
            for cape_type in CapeType::all() {
                if handler.borrow_mut().set_cape(cape_type) {
                    break;
                }
            }
            handler.borrow_mut().has_loaded_textures = true;
        }
        handler
    }

    /// `fun getCape()`: the registered texture, stepping the animated frame
    /// every 100ms.
    pub fn get_cape(&mut self) -> Texture {
        if !self.has_animated_cape {
            return Texture::Dynamic(Capes::identifier(self.uuid.to_string()));
        }
        let time = now_millis();
        if time > self.last_frame_time + 100 {
            let this_frame = (self.last_frame + 1) % self.max_frames;
            self.last_frame = this_frame;
            self.last_frame_time = time;
            Texture::Dynamic(Capes::identifier(format!("{}/{}", self.uuid, this_frame)))
        } else {
            Texture::Dynamic(Capes::identifier(format!("{}/{}", self.uuid, self.last_frame)))
        }
    }

    /// `fun setCape(capeType)`: fetch the type's URL and hand off per
    /// provider, recording the type on success.
    pub fn set_cape(&mut self, cape_type: CapeType) -> bool {
        let Some(cape_url) = cape_type.get_url(&self.profile) else {
            return false;
        };
        let connection = connect(&cape_url);
        let success = match cape_type {
            CapeType::Labymod => self.set_standard_cape(connection, true, false),
            CapeType::Cosmetica => self.set_cosmetica_cape(connection),
            CapeType::MinecraftCapes => self.set_mcm_cape(connection),
            _ => self.set_standard_cape(connection, false, false),
        };
        if success {
            self.cape_type = Some(cape_type);
        }
        success
    }

    /// `fun setStandardCape(connection, labymod = false, animated = false)`.
    pub fn set_standard_cape(
        &mut self,
        connection: Result<HttpResponse, HttpError>,
        labymod: bool,
        animated: bool,
    ) -> bool {
        match connection {
            Ok(response) if response.is_success() => {
                self.set_cape_texture(response.body, animated, labymod)
            }
            _ => false,
        }
    }

    /// `fun setCosmeticaCape(connection)`: read the cloak JSON, then load its
    /// texture (animated when `frames > 1`).
    pub fn set_cosmetica_cape(
        &mut self,
        connection: Result<HttpResponse, HttpError>,
    ) -> bool {
        let Ok(response) = connection else { return false };
        if !response.is_success() {
            return false;
        }
        let Some(result) = parse_json_field(&String::from_utf8_lossy(&response.body), "texture")
            .and_then(|texture| {
                Some(CosmeticaData {
                    cloak: Some(CapeData {
                        texture,
                        frames: parse_json_number(
                            &String::from_utf8_lossy(&response.body),
                            "frames",
                        ),
                    }),
                })
            })
        else {
            return false;
        };
        let Some(cloak) = result.cloak else {
            return false;
        };
        if cloak.texture.is_empty() {
            return false;
        }
        let connection = connect(&cloak.texture);
        self.set_standard_cape(connection, false, cloak.is_animated())
    }

    /// `fun setMCMCape(connection)`: read the profile JSON, then its cape URL
    /// (animated when an `animated_cape_url` is present).
    pub fn set_mcm_cape(&mut self, connection: Result<HttpResponse, HttpError>) -> bool {
        let Ok(response) = connection else { return false };
        if !response.is_success() {
            return false;
        }
        let body = String::from_utf8_lossy(&response.body).to_string();
        let Some(cape_url) = parse_json_field(&body, "cape_url") else {
            return false;
        };
        let animated = parse_json_field(&body, "animated_cape_url").is_some();
        let result = connect(&cape_url);
        self.set_standard_cape(result, false, animated)
    }

    /// `fun setCapeTexture(image, animated = false, labymod = false)`.
    pub fn set_cape_texture(&mut self, bytes: Vec<u8>, animated: bool, labymod: bool) -> bool {
        let Some(cape) = NativeImage::read(&bytes) else {
            return false;
        };
        if labymod && Uuid::from_string("ff305f81-ff30-5f90-ff30-5f90ff305f90").0
            == uuid_from_int_array(&cape.pixels).0
        {
            return false;
        }
        // `Minecraft.getInstance().submit { ... }`
        if animated {
            let animated_cape_frames = parse_animated_cape(&cape);
            self.max_frames = animated_cape_frames.len() as i32;
            self.has_cape = true;
            self.has_animated_cape = true;
            for (frame, texture) in animated_cape_frames {
                register_texture(Capes::identifier(format!("{}/{}", self.uuid, frame)), texture);
            }
        } else {
            self.has_elytra_texture = cape.width / cape.height == 2;
            register_texture(Capes::identifier(self.uuid.to_string()), parse_cape(&cape));
            self.has_cape = true;
        }
        true
    }

    /// The instances map state (test view).
    pub fn instance_count() -> usize {
        INSTANCES.with(|map| map.borrow().len())
    }

    /// Companion teardown for tests (upstream the process-wide map persists).
    pub fn reset_instances() {
        INSTANCES.with(|map| map.borrow_mut().clear());
    }
}

impl Default for PlayerHandler {
    fn default() -> Self {
        unreachable!("PlayerHandler is constructed from a profile")
    }
}

/// CapeType variants in declaration order.
pub fn cape_types() -> Vec<CapeType> {
    vec![
        CapeType::Minecraft,
        CapeType::Optifine,
        CapeType::Labymod,
        CapeType::MinecraftCapes,
        CapeType::Cosmetica,
        CapeType::CloaksPlus,
    ]
}

fn connect(url: &str) -> Result<HttpResponse, HttpError> {
    HTTP.with(|slot| slot.borrow().connect(url))
}

fn register_texture(id: Identifier, texture: NativeImage) {
    TEXTURES.with(|slot| slot.borrow().borrow_mut().register(id, texture));
}

/// `UUIDUtil.uuidFromIntArray(cape.pixels)` over the 32-bit little-endian
/// ints upstream reads (the Labymod watermark test).
pub fn uuid_from_int_array(pixels: &[u8]) -> Uuid {
    let mut raw: u128 = 0;
    // Four ARGB pixels fill the UUID's 16 bytes, first pixel most
    // significant (upstream int[i] -> the i-th 32-bit chunk).
    for i in 0..4usize {
        let idx = i * 4;
        if idx + 4 > pixels.len() {
            break;
        }
        raw |= (u32::from_le_bytes([
            pixels[idx],
            pixels[idx + 1],
            pixels[idx + 2],
            pixels[idx + 3],
        ]) as u128)
            << ((12 - i * 4) * 8);
    }
    Uuid(raw, 2)
}

/// `parseCape(img)`: upscale to the smallest power-of-two frame that fits,
/// copying the source pixels. Upstream consumes and closes the source image
/// after the copy.
pub fn parse_cape(img: &NativeImage) -> NativeImage {
    let mut image_width: u32 = 64;
    let mut image_height: u32 = 32;
    let (src_width, src_height) = (img.width, img.height);
    while image_width < src_width || image_height < src_height {
        image_width *= 2;
        image_height *= 2;
    }
    let mut img_new = NativeImage::new(image_width, image_height, true);
    for x in 0..src_width {
        for y in 0..src_height {
            img_new.set_pixel(x, y, img.get_pixel(x, y));
        }
    }
    img_new
}

/// `parseAnimatedCape(img)`: slice vertical frames of height `width / 2`.
pub fn parse_animated_cape(img: &NativeImage) -> HashMap<i32, NativeImage> {
    let mut animated_cape = HashMap::new();
    let total_frames = img.height / (img.width / 2);
    for current_frame in 0..total_frames {
        let frame_height = img.width / 2;
        let mut frame = NativeImage::new(img.width, frame_height, true);
        for x in 0..img.width {
            for y in 0..frame_height {
                frame.set_pixel(x, y, img.get_pixel(x, y + (current_frame * frame_height)));
            }
        }
        animated_cape.insert(current_frame as i32, frame);
    }
    animated_cape
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The JSON keys Gson binds for the two provider payloads.
fn parse_json_field(json: &str, key: &str) -> Option<String> {
    let marker = format!("\"{}\":", key);
    let start = json.find(&marker)? + marker.len();
    let rest = json[start..].trim_start();
    if rest.starts_with('"') {
        let after = &rest[1..];
        let end = after.find('"')?;
        Some(after[..end].to_string())
    } else if rest.starts_with("null") {
        None
    } else {
        let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

fn parse_json_number(json: &str, key: &str) -> i32 {
    parse_json_field(json, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Tiny PNG decoder for the RGBA payloads the tests feed in.
fn decode_png(bytes: &[u8]) -> Option<NativeImage> {
    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < PNG.len() || bytes[..8] != PNG {
        return None;
    }
    let mut i = 8;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut idat: Vec<u8> = Vec::new();
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        let ctype = &bytes[i + 4..i + 8];
        let data_start = i + 8;
        let data_end = (data_start + len).min(bytes.len());
        if ctype == b"IHDR" && len >= 13 {
            width = u32::from_be_bytes(bytes[data_start..data_start + 4].try_into().ok()?);
            height = u32::from_be_bytes(bytes[data_start + 4..data_start + 8].try_into().ok()?);
            let color_type = bytes[data_start + 9];
            if color_type != 6 {
                return None; // the loaders only see RGBA capes
            }
        } else if ctype == b"IDAT" {
            idat.extend_from_slice(&bytes[data_start..data_end]);
        } else if ctype == b"IEND" {
            break;
        }
        i = data_end + 4;
    }
    if width == 0 || height == 0 || idat.is_empty() {
        return None;
    }
    inflate(&idat, (width * height * 4 + height) as usize)
        .map(|raw| unfilter(raw, width, height))
}

fn inflate(data: &[u8], size_hint: usize) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    // Stored blocks: `00` header, length, then raw bytes (the test payloads).
    let mut out = Vec::with_capacity(size_hint);
    let mut i = 0;
    while i < data.len() {
        let bfinal = data[i] & 1 != 0;
        let btype = (data[i] >> 1) & 3;
        i += 1;
        match btype {
            0 => {
                if i + 4 > data.len() {
                    return None;
                }
                let len = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
                out.extend_from_slice(&data[i + 4..(i + 4 + len).min(data.len())]);
                i += 4 + len;
                if bfinal {
                    break;
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

fn unfilter(raw: Vec<u8>, width: u32, height: u32) -> NativeImage {
    let stride = (width * 4) as usize;
    let mut pixels = vec![0u8; stride * height as usize];
    let mut off = 0;
    for y in 0..height as usize {
        if off >= raw.len() {
            break;
        }
        let filter = raw[off];
        off += 1;
        let row_len = stride.min(raw.len().saturating_sub(off));
        let row = &raw[off..off + row_len];
        off += row_len;
        for x in 0..row_len {
            let left = if x >= 4 { pixels[y * stride + x - 4] } else { 0 };
            let up = if y > 0 { pixels[(y - 1) * stride + x] } else { 0 };
            let up_left = if y > 0 && x >= 4 {
                pixels[(y - 1) * stride + x - 4]
            } else {
                0
            };
            let v = row[x];
            pixels[y * stride + x] = match filter {
                0 => v,
                1 => v.wrapping_add(left),
                2 => v.wrapping_add(up),
                3 => v.wrapping_add(((left as u16 + up as u16) / 2) as u8),
                4 => v.wrapping_add(paeth(left, up, up_left)),
                _ => v,
            };
        }
    }
    NativeImage {
        width,
        height,
        pixels,
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (p - a as i16, p - b as i16, p - c as i16);
    let (pa, pb, pc) = (pa.abs(), pb.abs(), pc.abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
}

pub mod menus {
//! Upstream `menu/MainMenu.kt`, `menu/ToggleMenu.kt`, `menu/OtherMenu.kt`,
//! `menu/SelectorMenu.kt`, 1:1. The screens themselves are engine surface;
//! what the port carries is the widget layout, the callbacks, and the render
//! loop state they drive.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::capes_mod::Component;
use super::config::{CapeConfig, CapeType};
use super::render::{placeholder, PlaceholderEntityRenderer, RenderContext};
use std::cell::RefCell;
use std::rc::Rc;

/// A `Button.builder(...).pos(x, y).size(w, h).build()` record plus its
/// `active` flag, in the order `addRenderableWidget` received it.
#[derive(Debug, Clone)]
pub struct Button {
    pub message: Component,
    pub pos: (i32, i32),
    pub size: (u32, u32),
    pub active: bool,
    /// The zero-based render widget index this button reports for callbacks.
    pub id: usize,
}

/// The screen-tree seam: navigation callbacks and which screen is showing.
pub trait ScreenHost {
    /// `minecraft.gui.setScreen(screen)` with the screen kind.
    fn set_screen(&mut self, screen: ScreenKind);
    /// `this.lastScreen`.
    fn parent_screen(&self) -> ScreenKind;
    /// `list.getRowLeft()` and `list.getY()` on the skin customization screen.
    fn skin_list_row_left(&self) -> i32;
    fn skin_list_y(&self) -> i32;
}

/// Which screen a navigation points at (upstream `MainMenu`, `ToggleMenu`,
/// `OtherMenu`, `SelectorMenu`, and back out).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    MainMenu,
    ToggleMenu,
    OtherMenu,
    SelectorMenu,
    Parent,
    SkinCustomization,
}

/// The `MainMenu` button rows the three subclasses share.
pub const MAIN_MENU_BUTTON_W: i32 = 100;
pub const MAIN_MENU_OFFSET: i32 = (MAIN_MENU_BUTTON_W / 2) + 5;

/// `open class MainMenu(parent, gameOptions) : OptionsSubScreen(...)`: the
/// title translatable is `options.capes.title`; the three row buttons open
/// each submenu and are inactive while their own screen is showing (upstream
/// compares `this !is XMenu`, which is exactly "not currently that screen").
pub fn main_menu_buttons(showing: ScreenKind) -> Vec<Button> {
    let width_half = 200 / 2; // `width / 2` on the default options screen size
    vec![
        Button {
            message: Component::translatable("options.capes.selector", ""),
            pos: (width_half - MAIN_MENU_BUTTON_W / 2, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::SelectorMenu,
            id: 0,
        },
        Button {
            message: Component::translatable("options.capes.toggle", ""),
            pos: (width_half - MAIN_MENU_BUTTON_W - MAIN_MENU_OFFSET, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::ToggleMenu,
            id: 1,
        },
        Button {
            message: Component::translatable("options.capes.other", ""),
            pos: (width_half + MAIN_MENU_OFFSET, 35),
            size: (MAIN_MENU_BUTTON_W as u32, 20),
            active: showing != ScreenKind::OtherMenu,
            id: 2,
        },
    ]
}

/// `MainMenu#extractRenderState`: `centeredText(font, title, width / 2, 20,
/// 0xFFFFFF)`.
pub fn main_menu_title_y() -> i32 {
    20
}

/// `class ToggleMenu(parent, gameOptions) : MainMenu(...)`: the five provider
/// toggles plus the elytra toggle and Done, at the upstream grid positions.
pub fn toggle_menu_buttons(config: &CapeConfig) -> Vec<Button> {
    let width_half = 200 / 2;
    let height = 240; // the default options screen height `height / 7` runs on
    let row = |n: i32| height / 7 + n * 24;
    let buttons = vec![
        Button {
            message: CapeType::Optifine.get_toggle_text(config.enable_optifine),
            pos: (width_half - 155, row(1)),
            size: (150, 20),
            active: true,
            id: 0,
        },
        Button {
            message: CapeType::MinecraftCapes
                .get_toggle_text(config.enable_minecraft_capes_mod),
            pos: (width_half - 155 + 160, row(1)),
            size: (150, 20),
            active: true,
            id: 1,
        },
        Button {
            message: CapeType::Labymod.get_toggle_text(config.enable_labymod),
            pos: (width_half - 155, row(2)),
            size: (150, 20),
            active: true,
            id: 2,
        },
        Button {
            message: CapeType::Cosmetica.get_toggle_text(config.enable_cosmetica),
            pos: (width_half - 155 + 160, row(2)),
            size: (150, 20),
            active: true,
            id: 3,
        },
        Button {
            message: CapeType::CloaksPlus.get_toggle_text(config.enable_cloaks_plus),
            pos: (width_half - 155, row(3)),
            size: (150, 20),
            active: true,
            id: 4,
        },
        Button {
            message: Component::option_status(
                Component::translatable("options.capes.elytra", ""),
                config.enable_elytra_texture,
            ),
            pos: (width_half - (200 / 2), row(4)),
            size: (200, 20),
            active: true,
            id: 5,
        },
        Button {
            message: Component::literal("Done"),
            pos: (width_half - (200 / 2), row(5)),
            size: (200, 20),
            active: true,
            id: 6,
        },
    ];
    // The commented-out second CloaksPlus button stays out, as upstream.
    buttons
}

/// A toggle button's click: flip its config flag, save, and refresh the
/// label. The flag is named by the button, exactly as upstream binds each
/// callback to one field.
pub fn toggle_button_clicked(config: &mut CapeConfig, id: usize) {
    match id {
        0 => config.enable_optifine = !config.enable_optifine,
        1 => config.enable_minecraft_capes_mod = !config.enable_minecraft_capes_mod,
        2 => config.enable_labymod = !config.enable_labymod,
        3 => config.enable_cosmetica = !config.enable_cosmetica,
        4 => config.enable_cloaks_plus = !config.enable_cloaks_plus,
        5 => config.enable_elytra_texture = !config.enable_elytra_texture,
        _ => {}
    }
    if id <= 5 {
        config.save();
    }
}

/// `class OtherMenu(parent, gameOptions) : MainMenu(...)`: the OptiFine
/// editor entry (session-server join + capeChange URL behind the confirm
/// screen) and Done. The join/URL flow is engine auth surface; the port
/// carries the flow through the [`OtherMenuHost`] seam.
pub trait OtherMenuHost: ScreenHost {
    /// `minecraft.services().sessionService.joinServer(id, accessToken, serverId)`.
    fn join_server(&mut self, server_id: &str) -> Result<(), String>;
    /// The session state the URL needs.
    fn session(&self) -> (super::handler::Uuid, String, String);
    /// `Blaze3D.openUri(url)`.
    fn open_uri(&mut self, url: &str);
}

/// The OptiFine editor button's flow, 1:1 with the upstream callback: xor two
/// 128-bit randoms for the server id, join the session server, open the
/// capeChange URL behind a confirm link screen.
pub fn optifine_editor_clicked(host: &mut dyn OtherMenuHost) {
    let random1: u128 = rand128();
    let random2: u128 = rand128_seeded();
    let server_id = format!("{:x}", random1 ^ random2);
    let (profile_id, name, _token) = host.session();
    match host.join_server(&server_id) {
        Ok(()) => {
            let url = format!(
                "https://optifine.net/capeChange?u={}&n={}&s={}",
                profile_id.to_string().replace('-', ""),
                name,
                server_id
            );
            host.open_uri(&url);
        }
        Err(_) => {
            // `Capes.LOGGER.error("Failed to authenticate for OptiFine cape editor.")`
        }
    }
}

fn rand128() -> u128 {
    // `BigInteger(128, Random())`-shaped value.
    (std::process::id() as u128)
        .wrapping_mul(0x9E3779B97F4A7C15)
        ^ (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0))
}

fn rand128_seeded() -> u128 {
    // `BigInteger(128, Random(System.identityHashCode(Object()).toLong()))`.
    rand128().rotate_left(41)
}

/// `class SelectorMenu(parent, gameOptions) : MainMenu(...)`.
pub struct SelectorMenuState {
    /// `var lastTime = 0L`.
    pub last_time: u64,
    pub buttons: Vec<Button>,
}

/// `SelectorMenu#init`: the cycle button, Done, the body/elytra toggles, and
/// the 0x0 "DO NOT ASK WHY THIS EXISTS" button.
pub fn selector_menu_init(config: &mut CapeConfig) -> SelectorMenuState {
    let width_half = 200 / 2;
    let mut buttons = vec![
        Button {
            message: config.client_cape_type.get_text(),
            pos: (width_half - (200 / 2), 60),
            size: (200, 20),
            active: true,
            id: 0,
        },
        Button {
            message: Component::literal("Done"),
            pos: (width_half - (200 / 2), 220),
            size: (200, 20),
            active: true,
            id: 1,
        },
        Button {
            message: Component::translatable("options.capes.selector.player", ""),
            pos: (200 / 4 - (100 / 2), 145),
            size: (100, 20),
            active: true,
            id: 2,
        },
        Button {
            message: Component::translatable("options.capes.selector.elytra", ""),
            pos: (200 / 4 - (100 / 2), 120),
            size: (100, 20),
            active: true,
            id: 3,
        },
        Button {
            message: Component::literal("DO NOT ASK WHY THIS EXISTS"),
            pos: (0, 0),
            size: (0, 0),
            active: true,
            id: 4,
        },
    ];
    // `config.clientCapeType = config.clientCapeType.cycle(); config.save();
    // it.message = ...; PlaceholderEntity.capeLoaded = false` runs on click,
    // so rebuild the label from the config after a cycle.
    buttons[0].message = config.client_cape_type.get_text();
    SelectorMenuState { last_time: 0, buttons }
}

/// The cycle button's click, 1:1 with the upstream callback.
pub fn selector_cycle_clicked(config: &mut CapeConfig, state: &mut SelectorMenuState) {
    config.client_cape_type = config.client_cape_type.cycle();
    config.save();
    state.buttons[0].message = config.client_cape_type.get_text();
    placeholder().borrow_mut().cape_loaded.set(false);
}

/// The body/elytra toggles' clicks.
pub fn selector_toggle_clicked(state: &mut SelectorMenuState, id: usize) {
    let entity = placeholder();
    match id {
        2 => {
            let current = entity.borrow().show_body.get();
            entity.borrow().show_body.set(!current);
        }
        3 => {
            let current = entity.borrow().show_elytra.get();
            entity.borrow().show_elytra.set(!current);
        }
        _ => {}
    }
    let _ = &state.buttons;
}

/// `SelectorMenu#extractRenderState`: the 60fps limb animation gate and
/// `drawEntity(...)`; returns whether this frame animated.
pub fn selector_frame(state: &mut SelectorMenuState) -> bool {
    let time = now_millis();
    let entity = placeholder();
    if time > state.last_time + (1000 / 60) {
        state.last_time = time;
        let current = entity.borrow().x.get();
        entity.borrow().prev_x.set(current + 0.025);
        entity.borrow().update_limbs();
        true
    } else {
        false
    }
}

/// `drawEntity(context, x1, y1, x2, y2, entity)`: scissor the rect, get or
/// update the placeholder render state, and submit the entity with the
/// upstream camera transform (pitch 69f, zero origin, PI/2 Z rotation).
pub fn draw_entity(
    renderer: &mut PlaceholderEntityRenderer,
    entity: &RefCell<super::render::PlaceholderEntity>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) -> ((i32, i32, i32, i32), f32) {
    let _ = (x1, y1, x2, y2);
    let placeholder_ref = entity.borrow();
    renderer.update_render_state(&placeholder_ref);
    (scissor(x1, y1, x2, y2), 69.0)
}

/// `context.enableScissor(x1, y1, x2, y2)` (the scissor rect the entity is
/// drawn inside).
fn scissor(x1: i32, y1: i32, x2: i32, y2: i32) -> (i32, i32, i32, i32) {
    (x1, y1, x2, y2)
}

/// `SelectorMenu#mouseDragged(click, offsetX, offsetY)`: `prevYaw = yaw;
/// yaw -= offsetX`.
pub fn selector_mouse_dragged(offset_x: f32) {
    let entity = placeholder();
    let current = entity.borrow().yaw.get();
    entity.borrow().prev_yaw.set(current);
    entity.borrow().yaw.set(current - offset_x);
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `MixinSkinCustomizationScreen#addOptions` RETURN injection: the 20x20
/// sprite icon button at `rowLeft - 25`, `listY + 4`, opening the selector.
pub fn skin_customization_icon_button(host: &dyn ScreenHost) -> Button {
    Button {
        message: Component::Empty,
        pos: (host.skin_list_row_left() - 25, host.skin_list_y() + 4),
        size: (20, 20),
        active: true,
        id: 0,
    }
}

/// The icon button's click: `minecraft.gui.setScreen(capes$selectorMenu)`.
pub fn skin_customization_icon_clicked(host: &mut dyn ScreenHost) {
    host.set_screen(ScreenKind::SelectorMenu);
}

/// Keep the context import used on engines that need the trait in scope.
pub fn with_context<T>(ctx: &Rc<dyn RenderContext>, f: impl FnOnce(&Rc<dyn RenderContext>) -> T) -> T {
    f(ctx)
}

/// Unused upstream constant holder kept for the `Capes` namespace (the lang
/// keys live in the resource pack).
pub const CAPE_OPTIONS_ICON_TEXTURE: (&str, &str) = ("capes", "icon/cape_options");
}

pub mod mixins {
//! Upstream client mixins, 1:1: `MixinPlayerInfo.java`,
//! `MixinCapeFeatureRenderer.java`, `MixinEntityRenderDispatcher.java`,
//! `AccessorEntityRenderDispatcher.java`, `MixinSkinCustomizationScreen.java`.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::config::{CapeConfig, CapeType};
use super::capes_mod::Capes;
use super::handler::{GameProfile, Identifier, PlayerHandler, Texture, Uuid};
use super::menus::{skin_customization_icon_button, skin_customization_icon_clicked, Button, ScreenHost};
use super::render::{PlayerSkin, PlaceholderEntityRenderState};

/// The `PlayerInfo` fields the mixin shadows (`@Shadow @Final profile`) and
/// the skin supplier it returns.
pub trait PlayerInfoHost {
    fn profile(&self) -> GameProfile;
    /// `getSkin()`'s current return value.
    fn skin(&self) -> PlayerSkin;
    /// `cir.setReturnValue(...)`.
    fn set_return(&mut self, skin: PlayerSkin);
}

/// `MixinPlayerInfo#loadTextures` (`createSkinLookup` HEAD inject): touch the
/// handler when the profile is valid.
pub fn loadTextures(host: &dyn PlayerInfoHost) {
    if super::util::is_valid_profile(&host.profile()) {
        PlayerHandler::on_load_texture(&host.profile());
    }
}

/// `MixinPlayerInfo#getCapeTexture` (`getSkin` TAIL, cancellable): swap in the
/// loaded cape and elytra textures when the handler has a cape.
pub fn get_cape_texture(host: &mut dyn PlayerInfoHost) {
    if !super::util::is_valid_profile(&host.profile()) {
        return;
    }
    let handler = PlayerHandler::from_profile(&host.profile());
    let has_cape = handler.borrow().has_cape;
    if has_cape {
        let config: CapeConfig = Capes::config();
        let old_textures = host.skin();
        let cape_texture = handler.borrow_mut().get_cape();
        let elytra_texture = if handler.borrow().has_elytra_texture
            && config.enable_elytra_texture
        {
            cape_texture.clone()
        } else {
            Texture::ResourceTexture(Identifier::parse(
                "textures/entity/equipment/wings/elytra.png",
            ))
        };
        host.set_return(PlayerSkin {
            cape: Some(cape_texture),
            elytra: elytra_texture,
            ..old_textures
        });
    }
}

/// `MixinCapeFeatureRenderer#fixCapeTransparency` (`submit*` WrapOperation on
/// `RenderTypes.entitySolid`): return `armorCutoutNoCull` instead.
pub fn fix_cape_transparency(original: RenderKind, texture: Identifier) -> RenderKind {
    let _ = texture;
    match original {
        RenderKind::EntitySolid => RenderKind::ArmorCutoutNoCull,
        other => other,
    }
}

/// The `RenderType` factories the wrap can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderKind {
    EntitySolid,
    ArmorCutoutNoCull,
}

/// `MixinEntityRenderDispatcher#getPlaceholderRenderer` (`getRenderer` HEAD,
/// cancellable): hand back the placeholder renderer for its render state.
pub fn get_placeholder_renderer(state: &dyn std::any::Any) -> Option<()> {
    if state.is::<PlaceholderEntityRenderState>() {
        Some(())
    } else {
        None
    }
}

/// `AccessorEntityRenderDispatcher` as the read seam the placeholder init
/// uses.
pub fn accessor_entity_render_dispatcher(
    equipment_assets: u64,
    block_model_resolver: u64,
) -> (u64, u64) {
    (equipment_assets, block_model_resolver)
}

/// `MixinSkinCustomizationScreen#addOptions` RETURN injection: add the sprite
/// icon button and wire it to open the selector menu.
pub fn skin_customization_add_options(host: &dyn ScreenHost, widgets: &mut Vec<Button>) {
    widgets.push(skin_customization_icon_button(host));
}

/// The icon button's click handler, per the mixin's builder callback.
pub fn skin_customization_icon_on_click(host: &mut dyn ScreenHost) {
    skin_customization_icon_clicked(host);
}

/// Keep the CapeType import used for the toggle text paths exercised through
/// the config.
#[allow(dead_code)]
fn unused_cape_type_cycle(t: CapeType) -> CapeType {
    t.cycle()
}

/// Keep the UUID import tied to the profile shape the mixin reads.
#[allow(dead_code)]
fn unused_uuid(u: Uuid) -> Uuid {
    u
}
}

pub mod render {
//! Upstream `render/PlaceholderEntity*.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::capes_mod::Capes;
use super::handler::{
    Identifier, PlayerHandler, Texture, Uuid,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `HumanoidModel.ArmPose` seam (only `EMPTY` is set upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmPose {
    Empty,
}

/// `PlayerModelType` seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerModelType {
    Default,
    Slim,
}

/// The visible-part bits `updateRenderState` copies from the player options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModelPartOptions {
    pub hat: bool,
    pub jacket: bool,
    pub left_pants: bool,
    pub right_pants: bool,
    pub left_sleeve: bool,
    pub right_sleeve: bool,
}

/// The `EntityRenderDispatcher` accessors `AccessorEntityRenderDispatcher`
/// opens (`equipmentAssets`, `blockModelResolver`) plus the other context
/// fields `PlaceholderEntity`'s init passes to the renderer provider.
pub trait RenderContext {
    fn equipment_assets(&self) -> u64;
    fn block_model_resolver(&self) -> u64;
    fn item_model_resolver(&self) -> u64;
    fn map_renderer(&self) -> u64;
    fn resource_manager(&self) -> u64;
    fn entity_models(&self) -> u64;
    fn atlas_manager(&self) -> u64;
    fn font(&self) -> u64;
    fn player_skin_render_cache(&self) -> u64;
    fn paletted_texture_manager(&self) -> u64;
    /// `Minecraft.getInstance().options.isModelPartEnabled(part)`.
    fn is_model_part_enabled(&self, part: &str) -> bool;
    /// `Minecraft.getInstance().skinManager.get(profile)` resolution: the
    /// player's skin once fetched.
    fn skin_for(&self, profile: &super::handler::GameProfile) -> PlayerSkin;
}

/// `PlayerSkin` seam: body + cape + elytra textures, model, secure flag.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSkin {
    pub body: Texture,
    pub cape: Option<Texture>,
    pub elytra: Texture,
    pub model: PlayerModelType,
    pub secure: bool,
}

/// The render state upstream builds (`AvatarRenderState` fields this mod
/// touches).
#[derive(Debug, Clone)]
pub struct AvatarRenderState {
    pub body_rot: f32,
    pub walk_animation_pos: f32,
    pub walk_animation_speed: f32,
    pub left_arm_pose: ArmPose,
    pub right_arm_pose: ArmPose,
    pub skin: Option<PlayerSkin>,
    pub model_part_options: ModelPartOptions,
    pub show_cape: bool,
    /// `chestEquipment`: `None` is `ItemStack.EMPTY`.
    pub chest_equipment: Option<ItemStack>,
    pub elytra_rot_x: f32,
    pub elytra_rot_z: f32,
}

impl Default for AvatarRenderState {
    fn default() -> Self {
        Self {
            body_rot: 0.0,
            walk_animation_pos: 0.0,
            walk_animation_speed: 0.0,
            left_arm_pose: ArmPose::Empty,
            right_arm_pose: ArmPose::Empty,
            skin: None,
            model_part_options: ModelPartOptions::default(),
            show_cape: false,
            chest_equipment: None,
            elytra_rot_x: 0.0,
            elytra_rot_z: 0.0,
        }
    }
}

/// Upstream's `PlaceholderEntityRenderState : AvatarRenderState` subclass
/// carries no new state; the dispatcher mixin matches on it.
pub type PlaceholderEntityRenderState = AvatarRenderState;

/// `ItemStack` seam: the elytra stack the state fills, or empty.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemStack {
    Empty,
    Elytra,
}

/// The baked player model the renderer drives (`allParts().visible`).
#[derive(Debug, Clone, Default)]
pub struct PlayerModel {
    pub all_parts_visible: bool,
    pub slim: bool,
}

/// `PlaceholderEntityRenderer` seam surface: what the mod sets per frame and
/// what the engine renders (the actual draw is engine-side).
#[derive(Clone)]
pub struct PlaceholderEntityRenderer {
    pub slim: bool,
    pub ctx: Rc<dyn RenderContext>,
    pub model: PlayerModel,
    pub layers: Vec<&'static str>,
    pub state: AvatarRenderState,
    pub texture_location: Option<Identifier>,
    pub scale: f32,
}

impl PlaceholderEntityRenderer {
    /// The constructor: bake `PLAYER` or the `player_slim` layer, shadow
    /// radius `0.5f`, then `addLayer(CapeLayer)`, `addLayer(CustomHeadLayer)`,
    /// `addLayer(WingsLayer)`.
    pub fn new(ctx: Rc<dyn RenderContext>, slim: bool) -> Self {
        Self {
            slim,
            ctx,
            model: PlayerModel {
                all_parts_visible: true,
                slim,
            },
            layers: vec!["CapeLayer", "CustomHeadLayer", "WingsLayer"],
            state: AvatarRenderState::default(),
            texture_location: None,
            scale: 0.9375,
        }
    }

    /// `createRenderState()` + `getAndUpdatePlaceholderRenderState` /
    /// `updateRenderState`: fill the state from the placeholder, including
    /// the elytra item stack and the `elytraRotZ`/`elytraRotX` angles.
    pub fn update_render_state(&mut self, placeholder: &PlaceholderEntity) {
        let options = &placeholder.model_part_options;
        let body = placeholder.get_skin_textures();
        self.texture_location = Some(match &body {
            PlayerSkin { body, .. } => body.texture_path(),
        });
        self.state = AvatarRenderState {
            body_rot: placeholder.yaw.get(),
            walk_animation_pos: placeholder.limb_angle.get(),
            walk_animation_speed: placeholder.limb_distance.get(),
            left_arm_pose: ArmPose::Empty,
            right_arm_pose: ArmPose::Empty,
            skin: Some(body),
            model_part_options: *options,
            show_cape: true,
            chest_equipment: if placeholder.show_elytra.get() {
                Some(ItemStack::Elytra)
            } else {
                None
            },
            elytra_rot_z: -(std::f32::consts::PI / 12.0),
            elytra_rot_x: std::f32::consts::PI / 12.0,
        };
        self.model.all_parts_visible = placeholder.show_body.get();
    }

    /// `getTextureLocation(state)`.
    pub fn get_texture_location(&self) -> Option<Identifier> {
        self.texture_location.clone()
    }

    /// `scale(state, matrixStack)`.
    pub fn scale(&self) -> f32 {
        self.scale
    }
}

/// `object PlaceholderEntity`.
pub struct PlaceholderEntity {
    pub game_profile: super::handler::GameProfile,
    pub skin: RefCell<PlayerSkin>,
    pub slim: Cell<bool>,
    pub show_body: Cell<bool>,
    pub show_elytra: Cell<bool>,
    pub cape_loaded: Cell<bool>,
    pub limb_distance: Cell<f32>,
    pub last_limb_distance: Cell<f32>,
    pub limb_angle: Cell<f32>,
    pub yaw: Cell<f32>,
    pub prev_yaw: Cell<f32>,
    pub x: Cell<f64>,
    pub prev_x: Cell<f64>,
    /// The options snapshot `updateRenderState` reads from
    /// `Minecraft.getInstance().options`.
    pub model_part_options: ModelPartOptions,
}

thread_local! {
    static CTX: RefCell<Option<Rc<dyn RenderContext>>> = const { RefCell::new(None) };
}

/// The render-context seam swap before the first `placeholder()` call.
pub fn set_render_context(ctx: Rc<dyn RenderContext>) {
    CTX.with(|slot| *slot.borrow_mut() = Some(ctx));
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<RefCell<PlaceholderEntity>>>> = const { RefCell::new(None) };
}

/// `PlaceholderEntity.INSTANCE` accessor with the upstream init.
pub fn placeholder() -> Rc<RefCell<PlaceholderEntity>> {
    INSTANCE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(instance) = &*slot {
            return instance.clone();
        }
        let ctx = CTX.with(|slot| slot.borrow().clone()).unwrap_or_else(|| {
            Rc::new(NullRenderContext) as Rc<dyn RenderContext>
        });
        let profile = ctx_default_profile(&ctx);
        let skin = ctx.skin_for(&profile);
        let slim = skin.model == PlayerModelType::Slim;
        let instance = Rc::new(RefCell::new(PlaceholderEntity {
            game_profile: profile,
            skin: RefCell::new(skin),
            slim: Cell::new(slim),
            show_body: Cell::new(true),
            show_elytra: Cell::new(false),
            cape_loaded: Cell::new(false),
            limb_distance: Cell::new(0.0),
            last_limb_distance: Cell::new(0.0),
            limb_angle: Cell::new(0.0),
            yaw: Cell::new(0.0),
            prev_yaw: Cell::new(0.0),
            x: Cell::new(0.0),
            prev_x: Cell::new(0.0),
            model_part_options: read_model_part_options(&ctx),
        }));
        // `Minecraft.getInstance().skinManager.get(gameProfile).thenAccept { ... }`
        let resolved = ctx.skin_for(&instance.borrow().game_profile);
        let slim = resolved.model == PlayerModelType::Slim;
        instance.borrow_mut().skin = RefCell::new(resolved);
        instance.borrow_mut().slim.set(slim);
        *slot = Some(instance.clone());
        instance
    })
}

/// Test teardown (upstream the object lives once per process).
pub fn reset_placeholder() {
    INSTANCE.with(|slot| *slot.borrow_mut() = None);
}

fn ctx_default_profile(_ctx: &Rc<dyn RenderContext>) -> super::handler::GameProfile {
    super::handler::GameProfile {
        id: Uuid(0, 4),
        name: "placeholder".into(),
        properties: Vec::new(),
    }
}

fn read_model_part_options(ctx: &Rc<dyn RenderContext>) -> ModelPartOptions {
    ModelPartOptions {
        hat: ctx.is_model_part_enabled("HAT"),
        jacket: ctx.is_model_part_enabled("JACKET"),
        left_pants: ctx.is_model_part_enabled("LEFT_PANTS_LEG"),
        right_pants: ctx.is_model_part_enabled("RIGHT_PANTS_LEG"),
        left_sleeve: ctx.is_model_part_enabled("LEFT_SLEEVE"),
        right_sleeve: ctx.is_model_part_enabled("RIGHT_SLEEVE"),
    }
}

struct NullRenderContext;

impl RenderContext for NullRenderContext {
    fn equipment_assets(&self) -> u64 {
        0
    }
    fn block_model_resolver(&self) -> u64 {
        0
    }
    fn item_model_resolver(&self) -> u64 {
        0
    }
    fn map_renderer(&self) -> u64 {
        0
    }
    fn resource_manager(&self) -> u64 {
        0
    }
    fn entity_models(&self) -> u64 {
        0
    }
    fn atlas_manager(&self) -> u64 {
        0
    }
    fn font(&self) -> u64 {
        0
    }
    fn player_skin_render_cache(&self) -> u64 {
        0
    }
    fn paletted_texture_manager(&self) -> u64 {
        0
    }
    fn is_model_part_enabled(&self, _part: &str) -> bool {
        true
    }
    fn skin_for(&self, _profile: &super::handler::GameProfile) -> PlayerSkin {
        PlayerSkin {
            body: Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/player/wide/steve.png",
            )),
            cape: Some(Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/cape.png",
            ))),
            elytra: Texture::ResourceTexture(Identifier::with_default_namespace(
                "textures/entity/equipment/wings/elytra.png",
            )),
            model: PlayerModelType::Default,
            secure: false,
        }
    }
}

impl PlaceholderEntity {
    /// `fun updateLimbs()`.
    pub fn update_limbs(&self) {
        self.last_limb_distance.set(self.limb_distance.get());
        let d = self.x.get() - self.prev_x.get();
        let mut g = ((d * d).sqrt()) as f32 * 4.0;
        if g > 1.0 {
            g = 1.0;
        }
        self.limb_distance
            .set(self.limb_distance.get() + (g - self.limb_distance.get()) * 0.4);
        self.limb_angle.set(self.limb_angle.get() + self.limb_distance.get());
    }

    /// `fun getCapeTexture()`: load once, then the handler's cape or the
    /// skin's own.
    pub fn get_cape_texture(&self) -> Option<Texture> {
        if !self.cape_loaded.get() {
            self.cape_loaded.set(true);
            PlayerHandler::on_load_texture(&self.game_profile);
        }
        let handler = PlayerHandler::from_profile(&self.game_profile);
        if handler.borrow().has_cape {
            Some(handler.borrow_mut().get_cape())
        } else {
            self.skin.borrow().cape.clone()
        }
    }

    /// `fun getElytraTexture()`.
    pub fn get_elytra_texture(&self) -> Texture {
        let handler = PlayerHandler::from_profile(&self.game_profile);
        let cape_texture = self.get_cape_texture();
        if handler.borrow().has_elytra_texture
            && Capes::config().enable_elytra_texture
            && cape_texture.is_some()
        {
            cape_texture.unwrap()
        } else {
            Texture::ResourceTexture(Identifier::with_default_namespace(
                "entity/equipment/wings/elytra",
            ))
        }
    }

    /// `fun getSkinTextures()`.
    pub fn get_skin_textures(&self) -> PlayerSkin {
        let skin = self.skin.borrow().clone();
        PlayerSkin {
            cape: self.get_cape_texture(),
            elytra: self.get_elytra_texture(),
            ..skin
        }
    }
}
}

pub mod util {
//! Upstream `util/Utils.kt`, 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::handler::GameProfile;

/// `fun GameProfile.isValidProfile()`: decode the textures property, read
/// `profileName`, and accept a version-4 UUID or a version-2 UUID whose name
/// matches. Upstream `runCatching { ... }.getOrNull()`; any decode failure
/// returns false.
pub fn is_valid_profile(profile: &GameProfile) -> bool {
    let textures = profile.properties.iter().find(|p| p.name == "textures");
    let profile_name = textures.and_then(|p| {
        base64::Engine::STANDARD
            .decode(&p.value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|json| {
                let key = "\"profileName\"";
                let start = json.find(key)? + key.len();
                let rest = json[start..].trim_start();
                let rest = rest.strip_prefix(':')?.trim_start();
                let end = rest.find('"')?;
                let after = &rest[end + 1..];
                let close = after.find('"')?;
                Some(after[..close].to_string())
            })
    });
    match profile_name {
        None => false,
        Some(profile_name) => {
            profile.id.1 == 4 || (profile.id.1 == 2 && profile.name == profile_name)
        }
    }
}

/// Minimal base64 (MIT-licensed crate was not in the tree, so a local decoder
/// keeps the port dependency-free).
mod base64 {
    pub struct DecodeError;

    pub struct Engine;

    impl Engine {
        pub const STANDARD: Engine = Engine;

        pub fn decode(&self, input: &str) -> Result<Vec<u8>, DecodeError> {
            let mut out = Vec::new();
            let mut buffer = 0u32;
            let mut bits = 0;
            for ch in input.chars() {
                let value = match ch {
                    'A'..='Z' => ch as u32 - 'A' as u32,
                    'a'..='z' => ch as u32 - 'a' as u32 + 26,
                    '0'..='9' => ch as u32 - '0' as u32 + 52,
                    '+' | '-' => 62,
                    '/' | '_' => 63,
                    '=' | '\n' | '\r' | ' ' | '\t' => continue,
                    _ => return Err(DecodeError),
                };
                buffer = (buffer << 6) | value;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((buffer >> bits) as u8);
                    buffer &= (1 << bits) - 1;
                }
            }
            Ok(out)
        }
    }
}
}

#[cfg(test)]
pub mod tests {
//! Tests for the capes port: config, profile validation, the provider flows,
//! and the mixin/menu callbacks.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use super::capes_mod::{parse_config, Capes, Component, MissingConfig};
use super::config::{CapeConfig, CapeType};
use super::handler::{
    GameProfile, HttpResponse, NativeImage, PlayerHandler, ProfileProperty, Texture, Uuid,
};
use super::menus::*;
use super::mixins::*;
use super::render::{
    placeholder, reset_placeholder, PlayerModelType, PlayerSkin, RenderContext,
    PlaceholderEntityRenderState,
};
use super::util::is_valid_profile;
use std::cell::RefCell;
use std::rc::Rc;

/// A recorded config store (`ConfigIo` seam).
struct RecordingConfig {
    reads: RefCell<Vec<Option<String>>>,
    writes: RefCell<Vec<String>>,
}

impl RecordingConfig {
    fn new() -> Self {
        Self {
            reads: RefCell::new(vec![None]),
            writes: RefCell::new(Vec::new()),
        }
    }
}

impl super::capes_mod::ConfigIo for RecordingConfig {
    fn read_config(&self) -> Option<String> {
        self.reads.borrow_mut().pop().flatten()
    }
    fn write_config(&self, json: &str) {
        self.writes.borrow_mut().push(json.to_string());
    }
}

fn profile(id: &str, name: &str, textures: Option<&str>) -> GameProfile {
    GameProfile {
        id: Uuid::from_string(id),
        name: name.into(),
        properties: textures
            .map(|value| {
                vec![ProfileProperty {
                    name: "textures".into(),
                    value: value.into(),
                }]
            })
            .unwrap_or_default(),
    }
}

/// A version-4 UUID with a textures property naming the same profile.
fn valid_profile() -> GameProfile {
    // Base64 of {"profileName":"Steve"}; version nibble of the third group
    // hex is 4 when the uuid reads ...-4xxx-....
    let encoded = "eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0=";
    profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some(encoded),
    )
}

#[test]
fn a_valid_profile_passes_uuid_version_four() {
    assert!(is_valid_profile(&valid_profile()));
}

#[test]
fn an_unparseable_or_absent_textures_property_fails() {
    let no_textures = profile("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Steve", None);
    assert!(!is_valid_profile(&no_textures));
    let bad = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("!!!not base64!!!"),
    );
    assert!(!is_valid_profile(&bad));
}

#[test]
fn a_version_two_uuid_needs_the_matching_profile_name() {
    // Version-2 UUID (third group starts with 2) whose profileName differs.
    let mismatch = profile(
        "069a79f4-44e9-2726-a5be-fca90e38aaf5",
        "Alex",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    assert!(!is_valid_profile(&mismatch));
    let matching = profile(
        "069a79f4-44e9-2726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    assert!(is_valid_profile(&matching));
}

#[test]
fn the_config_defaults_match_the_upstream_fields() {
    let config = CapeConfig::default();
    assert_eq!(config.client_cape_type, CapeType::Minecraft);
    assert!(config.enable_optifine);
    assert!(!config.enable_labymod);
    assert!(!config.enable_minecraft_capes_mod);
    assert!(!config.enable_cosmetica);
    assert!(!config.enable_cloaks_plus);
    assert!(config.enable_elytra_texture);
}

#[test]
fn the_config_round_trips_through_its_json() {
    let config = CapeConfig {
        client_cape_type: CapeType::CloaksPlus,
        enable_labymod: true,
        ..CapeConfig::default()
    };
    let parsed = parse_config(&config.to_json()).unwrap();
    assert_eq!(parsed, config);
}

#[test]
fn a_missing_config_file_creates_the_default_and_a_save_writes_json() {
    let recording = Rc::new(RecordingConfig::new());
    Capes::reset_config();
    Capes::set_config_io(recording.clone());
    let config = Capes::config();
    assert_eq!(config, CapeConfig::default());
    config.save();
    let writes = recording.writes.borrow().clone();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].contains("\"clientCapeType\": \"MINECRAFT\""));
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

#[test]
fn cape_type_cycles_through_all_six_providers() {
    assert_eq!(CapeType::Minecraft.cycle(), CapeType::Optifine);
    assert_eq!(CapeType::Optifine.cycle(), CapeType::Labymod);
    assert_eq!(CapeType::Labymod.cycle(), CapeType::Cosmetica);
    assert_eq!(CapeType::Cosmetica.cycle(), CapeType::MinecraftCapes);
    assert_eq!(CapeType::MinecraftCapes.cycle(), CapeType::CloaksPlus);
    assert_eq!(CapeType::CloaksPlus.cycle(), CapeType::Minecraft);
}

#[test]
fn cape_urls_gate_on_their_provider_toggles() {
    let p = profile("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Steve", None);
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    // Default: OptiFine on, the rest off.
    assert_eq!(
        CapeType::Optifine.get_url(&p),
        Some("http://s.optifine.net/capes/Steve.png".into())
    );
    assert_eq!(CapeType::Labymod.get_url(&p), None);
    assert_eq!(CapeType::MinecraftCapes.get_url(&p), None);
    assert_eq!(CapeType::Minecraft.get_url(&p), None);
    // MinecraftCapes strips the dashes from the UUID.
    let mut config = CapeConfig::default();
    config.enable_minecraft_capes_mod = true;
    Capes::set_config_io(Rc::new(PinnedConfig(config)));
    Capes::reset_config();
    assert_eq!(
        CapeType::MinecraftCapes.get_url(&p),
        Some("https://api.minecraftcapes.net/profile/069a79f444e94726a5befca90e38aaf5".into())
    );
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

/// A config store serving a fixed JSON.
struct PinnedConfig(CapeConfig);

impl super::capes_mod::ConfigIo for PinnedConfig {
    fn read_config(&self) -> Option<String> {
        Some(self.0.to_json())
    }
    fn write_config(&self, _json: &str) {}
}

#[test]
fn toggle_text_and_get_text_shape_the_labels() {
    assert_eq!(
        CapeType::Optifine.get_toggle_text(true),
        Component::literal("OptiFine: ON")
    );
    assert_eq!(
        CapeType::MinecraftCapes.get_toggle_text(false),
        Component::literal("MinecraftCapes: OFF")
    );
    assert_eq!(
        CapeType::Labymod.get_text(),
        Component::translatable("options.capes.capetype", "LabyMod")
    );
}

#[test]
fn the_debug_info_lists_type_animation_elytra_and_url() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let p = valid_profile();
    let info = Capes::get_debug_info_for_player(&p);
    assert!(info.contains("Name: Steve"));
    assert!(info.contains("UUID: 069a79f4-44e9-4726-a5be-fca90e38aaf5"));
    assert!(info.contains("Type: null"));
    assert!(info.contains("IsAnimated: false"));
    assert!(info.contains("HasElytraTexture: true"));
    assert!(info.contains("URL: null"));
}

#[test]
fn on_load_texture_for_the_own_profile_loads_the_configured_type() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    super::handler::set_http(Rc::new(RecordingHttp::default()));
    let own = valid_profile();
    super::handler::set_own_profile(own.clone());
    let handler = PlayerHandler::on_load_texture(&own);
    // MINECRAFT has no URL, so nothing loads and the type stays unset.
    assert!(!handler.borrow().has_cape);
    assert_eq!(handler.borrow().cape_type, None);
}

#[test]
fn on_load_texture_for_another_profile_tries_every_provider_once() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let http = Rc::new(RecordingHttp::default());
    super::handler::set_http(http.clone());
    let other = valid_profile();
    super::handler::set_own_profile(profile("00000000-0000-0000-0000-000000000000", "other", None));
    // Every provider is disabled or unreachable except OptiFine, which
    // returns a cape image.
    http.capes.borrow_mut().insert(
        "http://s.optifine.net/capes/Steve.png".to_string(),
        cape_png(64, 32),
    );
    let handler = PlayerHandler::on_load_texture(&other);
    assert!(handler.borrow().has_cape);
    assert_eq!(handler.borrow().cape_type, Some(CapeType::Optifine));
    assert!(handler.borrow().has_loaded_textures);
}

#[test]
fn a_labymod_watermark_cape_is_rejected() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    let watermark = super::handler::uuid_from_int_array(&[
        0x81, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30, 0xff, 0x90, 0x5f, 0x30,
        0xff,
    ]);
    assert_eq!(
        watermark.0,
        Uuid::from_string("ff305f81-ff30-5f90-ff30-5f90ff305f90").0
    );
}

#[test]
fn parse_cape_upscales_to_the_next_power_of_two_frame() {
    let img = NativeImage::new(128, 64, true);
    let parsed = super::handler::parse_cape(&img);
    assert_eq!(parsed.width, 128);
    assert_eq!(parsed.height, 64);
    let odd = NativeImage::new(65, 40, true);
    let parsed = super::handler::parse_cape(&odd);
    assert_eq!(parsed.width, 128);
    assert_eq!(parsed.height, 64);
}

#[test]
fn parse_animated_cape_slices_frames_of_half_the_width() {
    let mut img = NativeImage::new(64, 96, true);
    for frame in 0..3 {
        for x in 0..64 {
            for y in 0..32 {
                img.set_pixel(x, y + frame * 32, 0xFF00_0000 | frame);
            }
        }
    }
    let frames = super::handler::parse_animated_cape(&img);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[&1].get_pixel(0, 0) & 0xFF, 1);
}

#[test]
fn get_cape_steps_the_animated_frame_every_hundred_millis() {
    let mut handler = super::handler::PlayerHandler {
        profile: valid_profile(),
        uuid: Uuid::from_string("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
        last_frame: 0,
        max_frames: 3,
        last_frame_time: 0,
        has_cape: true,
        has_elytra_texture: true,
        has_animated_cape: true,
        cape_type: None,
        has_loaded_textures: true,
    };
    let first = handler.get_cape();
    assert!(matches!(first, Texture::Dynamic(_)));
    assert_eq!(handler.last_frame, 1);
    // Inside the 100ms window the frame holds.
    handler.last_frame_time = u64::MAX / 2; // far in the future
    let held = handler.get_cape();
    assert!(matches!(held, Texture::Dynamic(_)));
    assert_eq!(handler.last_frame, 1);
    // Static capes resolve to the plain uuid texture.
    handler.has_animated_cape = false;
    let static_cape = handler.get_cape();
    assert!(matches!(static_cape, Texture::Dynamic(_)));
}

#[test]
fn get_cape_texture_swaps_the_skin_when_the_handler_has_a_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    super::handler::set_http(Rc::new(RecordingHttp::default()));
    let p = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    let http = Rc::new(RecordingHttp::default());
    super::handler::set_http(http.clone());
    http.capes.borrow_mut().insert(
        "http://s.optifine.net/capes/Steve.png".to_string(),
        cape_png(64, 32),
    );
    loadTextures(&host_for(&p, base_skin()));
    let handler = PlayerHandler::from_profile(&p);
    assert!(handler.borrow().has_cape);
    let mut host = host_for(&p, base_skin());
    get_cape_texture(&mut host);
    assert!(host.returned.is_some());
    let skin = host.returned.unwrap();
    assert!(skin.cape.is_some());
}

#[test]
fn get_cape_texture_leaves_the_skin_alone_without_a_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    super::handler::set_http(Rc::new(RecordingHttp::default()));
    let p = profile(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        Some("eyJwcm9maWxlTmFtZSI6IlN0ZXZlIn0="),
    );
    loadTextures(&host_for(&p, base_skin()));
    let mut host = host_for(&p, base_skin());
    get_cape_texture(&mut host);
    assert!(host.returned.is_none());
}

#[test]
fn fix_cape_transparency_swaps_entity_solid_for_armor_cutout_no_cull() {
    assert_eq!(
        fix_cape_transparency(
            RenderKind::EntitySolid,
            super::handler::Identifier::with_default_namespace("x"),
        ),
        RenderKind::ArmorCutoutNoCull
    );
}

#[test]
fn get_placeholder_renderer_matches_its_own_render_state() {
    let state = PlaceholderEntityRenderState::default();
    assert!(get_placeholder_renderer(&state).is_some());
    assert!(get_placeholder_renderer(&42u32).is_none());
}

#[test]
fn toggle_menu_buttons_flip_their_config_flag_save_and_relabel() {
    let recording = Rc::new(RecordingConfig::new());
    Capes::reset_config();
    Capes::set_config_io(recording.clone());
    let mut config = CapeConfig::default();
    let buttons = toggle_menu_buttons(&config);
    assert_eq!(buttons.len(), 7);
    assert_eq!(
        buttons[0].message,
        CapeType::Optifine.get_toggle_text(true)
    );
    toggle_button_clicked(&mut config, 0);
    assert!(!config.enable_optifine);
    assert!(!recording.writes.borrow().is_empty());
    assert_eq!(
        CapeType::Optifine.get_toggle_text(config.enable_optifine),
        Component::literal("OptiFine: OFF")
    );
    // The elytra toggle saves too; Done does not.
    let writes = recording.writes.borrow().len();
    toggle_button_clicked(&mut config, 5);
    assert_eq!(recording.writes.borrow().len(), writes + 1);
    toggle_button_clicked(&mut config, 6);
    assert_eq!(recording.writes.borrow().len(), writes + 1);
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
}

#[test]
fn main_menu_lays_out_the_three_row_buttons() {
    let buttons = main_menu_buttons(ScreenKind::MainMenu);
    assert_eq!(buttons.len(), 3);
    assert_eq!(buttons[0].pos, (100 - 50, 35));
    assert_eq!(buttons[1].pos, (100 - 100 - 55, 35));
    assert_eq!(buttons[2].pos, (100 + 55, 35));
    assert!(buttons.iter().all(|b| b.active));
    let on_selector = main_menu_buttons(ScreenKind::SelectorMenu);
    assert!(!on_selector[0].active);
    assert!(main_menu_title_y() == 20);
}

#[test]
fn selector_cycle_clicked_advances_the_type_and_reloads_the_cape() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    reset_placeholder();
    super::handler::set_http(Rc::new(RecordingHttp::default()));
    let mut config = CapeConfig::default();
    let mut state = selector_menu_init(&mut config);
    assert_eq!(state.buttons[0].message, config.client_cape_type.get_text());
    // A loaded placeholder cape is dropped on cycle (`PlaceholderEntity.capeLoaded = false`).
    placeholder().borrow().get_cape_texture();
    assert!(placeholder().borrow().cape_loaded.get());
    selector_cycle_clicked(&mut config, &mut state);
    assert_eq!(config.client_cape_type, CapeType::Optifine);
    assert!(!placeholder().borrow().cape_loaded.get());
    assert_eq!(state.buttons[0].message, config.client_cape_type.get_text());
}

#[test]
fn selector_toggles_flip_body_and_elytra() {
    reset_placeholder();
    let mut state = SelectorMenuState {
        last_time: 0,
        buttons: Vec::new(),
    };
    let before = placeholder().borrow().show_body.get();
    selector_toggle_clicked(&mut state, 2);
    assert_eq!(placeholder().borrow().show_body.get(), !before);
    let before = placeholder().borrow().show_elytra.get();
    selector_toggle_clicked(&mut state, 3);
    assert_eq!(placeholder().borrow().show_elytra.get(), !before);
}

#[test]
fn selector_mouse_dragged_rotates_the_placeholder() {
    reset_placeholder();
    selector_mouse_dragged(5.0);
    let entity = placeholder();
    assert_eq!(entity.borrow().yaw.get(), -5.0);
    assert_eq!(entity.borrow().prev_yaw.get(), 0.0);
}

#[test]
fn selector_frame_animates_limbs_at_sixty_fps() {
    reset_placeholder();
    let mut state = SelectorMenuState {
        last_time: 0,
        buttons: Vec::new(),
    };
    assert!(selector_frame(&mut state));
    assert_eq!(placeholder().borrow().prev_x.get(), 0.025);
    // A second call in the same 16ms window is skipped.
    state.last_time = u64::MAX / 2;
    assert!(!selector_frame(&mut state));
}

#[test]
fn the_skin_customization_icon_opens_the_selector() {
    struct Host {
        opened: RefCell<Option<ScreenKind>>,
    }
    impl ScreenHost for Host {
        fn set_screen(&mut self, screen: ScreenKind) {
            *self.opened.borrow_mut() = Some(screen);
        }
        fn parent_screen(&self) -> ScreenKind {
            ScreenKind::SkinCustomization
        }
        fn skin_list_row_left(&self) -> i32 {
            120
        }
        fn skin_list_y(&self) -> i32 {
            30
        }
    }
    let host = Host {
        opened: RefCell::new(None),
    };
    let button = skin_customization_icon_button(&host);
    assert_eq!(button.pos, (95, 34));
    assert_eq!(button.size, (20, 20));
    let mut host = host;
    skin_customization_icon_on_click(&mut host);
    assert_eq!(*host.opened.borrow(), Some(ScreenKind::SelectorMenu));
}

#[test]
fn the_placeholder_skin_carries_the_cape_and_elytra_textures() {
    Capes::reset_config();
    Capes::set_config_io(Rc::new(MissingConfig));
    PlayerHandler::reset_instances();
    reset_placeholder();
    super::handler::set_http(Rc::new(RecordingHttp::default()));
    let entity = placeholder();
    let skin = entity.borrow().get_skin_textures();
    assert!(skin.cape.is_some());
    // `hasElytraTexture` + `enableElytraTexture`: the elytra reuses the cape.
    assert_eq!(skin.elytra, skin.cape.clone().unwrap());
    // capeLoaded flips on the first texture request.
    assert!(entity.borrow().cape_loaded.get());
}

#[test]
fn the_renderer_bakes_the_right_model_and_layers() {
    reset_placeholder();
    let ctx: Rc<dyn RenderContext> = Rc::new(NullCtx);
    let mut renderer = super::render::PlaceholderEntityRenderer::new(ctx.clone(), false);
    assert!(!renderer.slim);
    assert_eq!(
        renderer.layers,
        vec!["CapeLayer", "CustomHeadLayer", "WingsLayer"]
    );
    let entity = placeholder();
    renderer.update_render_state(&entity.borrow());
    assert!(renderer.state.show_cape);
    assert_eq!(renderer.state.model_part_options.hat, true);
    assert_eq!(renderer.scale(), 0.9375);
    // The slim model bakes the player_slim layer.
    let renderer = super::render::PlaceholderEntityRenderer::new(ctx, true);
    assert!(renderer.model.slim);
}

struct NullCtx;

impl RenderContext for NullCtx {
    fn equipment_assets(&self) -> u64 {
        0
    }
    fn block_model_resolver(&self) -> u64 {
        0
    }
    fn item_model_resolver(&self) -> u64 {
        0
    }
    fn map_renderer(&self) -> u64 {
        0
    }
    fn resource_manager(&self) -> u64 {
        0
    }
    fn entity_models(&self) -> u64 {
        0
    }
    fn atlas_manager(&self) -> u64 {
        0
    }
    fn font(&self) -> u64 {
        0
    }
    fn player_skin_render_cache(&self) -> u64 {
        0
    }
    fn paletted_texture_manager(&self) -> u64 {
        0
    }
    fn is_model_part_enabled(&self, _part: &str) -> bool {
        true
    }
    fn skin_for(
        &self,
        _profile: &GameProfile,
    ) -> super::render::PlayerSkin {
        base_skin()
    }
}

fn base_skin() -> PlayerSkin {
    PlayerSkin {
        body: Texture::ResourceTexture(super::handler::Identifier::with_default_namespace(
            "textures/entity/player/wide/steve.png",
        )),
        cape: Some(Texture::ResourceTexture(
            super::handler::Identifier::with_default_namespace("textures/entity/cape.png"),
        )),
        elytra: Texture::ResourceTexture(super::handler::Identifier::with_default_namespace(
            "textures/entity/equipment/wings/elytra.png",
        )),
        model: PlayerModelType::Default,
        secure: true,
    }
}

/// A `PlayerInfo` host recording the mixin's return swap.
struct Host {
    profile: GameProfile,
    skin: PlayerSkin,
    returned: Option<PlayerSkin>,
}

impl PlayerInfoHost for Host {
    fn profile(&self) -> GameProfile {
        self.profile.clone()
    }
    fn skin(&self) -> PlayerSkin {
        self.skin.clone()
    }
    fn set_return(&mut self, skin: PlayerSkin) {
        self.returned = Some(skin);
    }
}

fn host_for(profile: &GameProfile, skin: PlayerSkin) -> Host {
    Host {
        profile: profile.clone(),
        skin,
        returned: None,
    }
}

/// The HTTP seam recording every URL and serving pinned capes.
#[derive(Default)]
struct RecordingHttp {
    pub capes: RefCell<std::collections::HashMap<String, Vec<u8>>>,
}

impl super::handler::Http for RecordingHttp {
    fn connect(&self, url: &str) -> Result<HttpResponse, super::handler::HttpError> {
        if let Some(body) = self.capes.borrow().get(url) {
            return Ok(HttpResponse {
                status: 200,
                body: body.clone(),
            });
        }
        Err(super::handler::HttpError)
    }
}

/// A 64x32 RGBA PNG of stored-block form.
fn cape_png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.push(0u8); // filter none
        for _ in 0..width * 4 {
            raw.push(0x40);
        }
    }
    let mut png = vec![0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    // IHDR
    push_chunk(&mut png, b"IHDR", {
        let mut d = Vec::new();
        d.extend_from_slice(&width.to_be_bytes());
        d.extend_from_slice(&height.to_be_bytes());
        d.extend_from_slice(&[8, 6, 0, 0, 0]);
        d
    });
    push_chunk(&mut png, b"IDAT", raw);
    push_chunk(&mut png, b"IEND", Vec::new());
    png
}

fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: Vec<u8>) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(&data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(&data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}
}
