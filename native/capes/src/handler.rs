//! Upstream `handler/PlayerHandler.kt`, `handler/data/*`, plus the engine
//! types the handler speaks in (`GameProfile`, `Identifier`,
//! `ClientAsset.Texture`, `NativeImage`), 1:1.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::capes_mod::Capes;
use crate::config::CapeType;
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
