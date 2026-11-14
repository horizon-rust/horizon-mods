//! nl.enjarai.showmeyourskin.ShowMeYourSkin (1:1). Networking is an engine
//! seam until the net/ stage lands; the init order matches upstream.

use crate::config::mod_config_server;

/// Java: public static Identifier id(String path) -> Identifier.fromNamespaceAndPath(MODID, path).
pub fn id(path: &str) -> String {
    format!("{}/{}", crate::MODID, path)
}

/// Java: ShowMeYourSkin.NET_CHANNEL (OwoNetChannel). The real channel arrives
/// with the net stage; the seam carries the id and the canSendToPlayer hook.
pub struct NetChannelSeam {
    pub id: String,
}

impl NetChannelSeam {
    /// Java: NET_CHANNEL.canSendToServer().
    pub fn can_send_to_server(&self) -> bool {
        // Seamed: true once the channel handshake lands with the net stage.
        true
    }

    /// Java: NET_CHANNEL.canSendToPlayer(player).
    pub fn can_send_to_player(&self) -> bool {
        // Seamed: true once the channel handshake lands with the net stage.
        true
    }

    /// Java: NET_CHANNEL.serverHandle(player).send(packet).
    pub fn server_send(&self, _packet: crate::net::ConfigSyncPacket) {
        // Seamed: the actual channel send lands with the net stage.
    }
}

/// Java: public static final OwoNetChannel<...> NET_CHANNEL
///     = OwoNetChannel.create(ShowMeYourSkin.id("networking")).
pub fn net_channel() -> NetChannelSeam {
    NetChannelSeam {
        id: id("networking"),
    }
}

/// Java: onInitialize() (ModInitializer).
pub fn on_initialize() {
    mod_config_server::load();

    init_networking();
}

/// Java: initNetworking() registers ConfigSyncPacket clientbound and
/// SettingsUpdatePacket serverbound and the join event syncing the server
/// config + ensureValid on the joining player's component. The owo-net
/// channel is a Horizon networking seam.
pub fn init_networking() {
    // Seamed: ConfigSyncPacket::handle_client, SettingsUpdatePacket::handle_server,
    // ServerPlayConnectionEvents.JOIN. Landed with the net/ stage.
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_namespaced() {
        assert_eq!(super::id("networking"), "showmeyourskin/networking");
        assert_eq!(crate::MODID, "showmeyourskin");
    }
}
