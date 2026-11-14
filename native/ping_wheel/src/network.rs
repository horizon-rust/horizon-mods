//! Upstream: common/network/ (1:1). FriendlyByteBuf is a host seam with the exact
//! read/write operations upstream calls; PacketHandler.readSafe keeps the
//! fall-back-to-default-constructor and skip-remaining-bytes behavior.
use crate::config::MAX_CHANNEL_LENGTH;
pub use crate::seams::FriendlyByteBuf;
use crate::seams::Vec3;

/// net.minecraft.resources.ResourceLocation identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceLocation {
    pub namespace: &'static str,
    pub path: &'static str,
}

pub trait IPacket {
    fn write(&self, buf: &mut FriendlyByteBuf);
    fn is_corrupt(&self) -> bool;
    fn get_id(&self) -> ResourceLocation;
}

// ---------------------------------------------------------------------------
// PingLocationC2SPacket. The no-arg constructor produces channel == null (corrupt).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PingLocationC2SPacket {
    pub channel: Option<String>,
    pub pos: Option<Vec3>,
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
}

pub const PING_LOCATION_C2S_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-c2s", path: "ping-location" };

impl PingLocationC2SPacket {
    pub fn new(channel: Option<String>, pos: Option<Vec3>, entity: Option<u128>, sequence: i32, dimension: i32) -> Self {
        Self { channel, pos, entity, sequence, dimension }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None, None, None, 0, 0)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(
            Some(buf.read_utf(MAX_CHANNEL_LENGTH)),
            Some(Vec3::new(buf.read_double(), buf.read_double(), buf.read_double())),
            if buf.read_boolean() { Some(buf.read_uuid()) } else { None },
            buf.read_int(),
            buf.read_int(),
        )
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }
}

impl IPacket for PingLocationC2SPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
        let p = self.pos.unwrap_or(Vec3::ZERO);
        buf.write_double(p.x);
        buf.write_double(p.y);
        buf.write_double(p.z);
        buf.write_boolean(self.entity.is_some());

        if let Some(e) = self.entity {
            buf.write_uuid(e);
        }

        buf.write_int(self.sequence);
        buf.write_int(self.dimension);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        PING_LOCATION_C2S_ID
    }
}

// ---------------------------------------------------------------------------
// PingLocationS2CPacket
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PingLocationS2CPacket {
    pub channel: Option<String>,
    pub pos: Option<Vec3>,
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
    pub author: u128,
}

pub const PING_LOCATION_S2C_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-s2c", path: "ping-location" };

impl PingLocationS2CPacket {
    pub fn new(channel: Option<String>, pos: Option<Vec3>, entity: Option<u128>, sequence: i32, dimension: i32, author: u128) -> Self {
        Self { channel, pos, entity, sequence, dimension, author }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None, None, None, 0, 0, 0)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(
            Some(buf.read_utf(MAX_CHANNEL_LENGTH)),
            Some(Vec3::new(buf.read_double(), buf.read_double(), buf.read_double())),
            if buf.read_boolean() { Some(buf.read_uuid()) } else { None },
            buf.read_int(),
            buf.read_int(),
            buf.read_uuid(),
        )
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }

    pub fn from_client_packet(client_packet: &PingLocationC2SPacket, author: u128) -> Self {
        Self::new(
            client_packet.channel.clone(),
            client_packet.pos,
            client_packet.entity,
            client_packet.sequence,
            client_packet.dimension,
            author,
        )
    }
}

impl IPacket for PingLocationS2CPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
        let p = self.pos.unwrap_or(Vec3::ZERO);
        buf.write_double(p.x);
        buf.write_double(p.y);
        buf.write_double(p.z);
        buf.write_boolean(self.entity.is_some());

        if let Some(e) = self.entity {
            buf.write_uuid(e);
        }

        buf.write_int(self.sequence);
        buf.write_int(self.dimension);
        buf.write_uuid(self.author);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        PING_LOCATION_S2C_ID
    }
}

// ---------------------------------------------------------------------------
// UpdateChannelC2SPacket
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct UpdateChannelC2SPacket {
    pub channel: Option<String>,
}

pub const UPDATE_CHANNEL_C2S_ID: ResourceLocation = ResourceLocation { namespace: "ping-wheel-c2s", path: "update-channel" };

impl UpdateChannelC2SPacket {
    pub fn new(channel: Option<String>) -> Self {
        Self { channel }
    }

    pub fn default_corrupt() -> Self {
        Self::new(None)
    }

    pub fn from_buf(buf: &mut FriendlyByteBuf) -> Self {
        Self::new(Some(buf.read_utf(MAX_CHANNEL_LENGTH)))
    }

    pub fn read_safe(buf: &mut FriendlyByteBuf) -> Self {
        PacketHandler::read_safe(buf, Self::from_buf, Self::default_corrupt)
    }
}

impl IPacket for UpdateChannelC2SPacket {
    fn write(&self, buf: &mut FriendlyByteBuf) {
        buf.write_utf(self.channel.as_deref(), MAX_CHANNEL_LENGTH);
    }

    fn is_corrupt(&self) -> bool {
        self.channel.is_none()
    }

    fn get_id(&self) -> ResourceLocation {
        UPDATE_CHANNEL_C2S_ID
    }
}

// ---------------------------------------------------------------------------
// PacketHandler
// ---------------------------------------------------------------------------

pub struct PacketHandler;

impl PacketHandler {
    /// readSafe: try the buf constructor; on any failure build the default (corrupt) packet,
    /// then always skip whatever bytes are left in the buffer.
    pub fn read_safe<T>(buf: &mut FriendlyByteBuf, from_buf: impl FnOnce(&mut FriendlyByteBuf) -> T, default: impl FnOnce() -> T) -> T {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| from_buf(buf)));

        let out = match result {
            Ok(v) => v,
            Err(_) => default(),
        };

        // finally: readerIndex += readableBytes()
        buf.skip_remaining();
        out
    }
}
