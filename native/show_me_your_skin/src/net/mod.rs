//! nl.enjarai.showmeyourskin.net (1:1): the two owo-net packets.

use crate::config::armor_config::ArmorConfig;
use crate::config::synced_mod_config::SyncedModConfig;
use crate::seams::EntityRef;
use crate::util::ArmorConfigComponent;

/// nl.enjarai.showmeyourskin.net.ConfigSyncPacket (clientbound record).
pub struct ConfigSyncPacket {
    pub config: SyncedModConfig,
}

impl ConfigSyncPacket {
    /// Java: handleClient(access) -> ShowMeYourSkinClient.serverConfig = config.
    pub fn handle_client(&self) {
        crate::show_me_your_skin_client::set_server_config(Some(self.config.clone()));
    }
}

/// nl.enjarai.showmeyourskin.net.SettingsUpdatePacket (serverbound record).
pub struct SettingsUpdatePacket {
    pub settings: ArmorConfig,
}

impl SettingsUpdatePacket {
    /// Java: handleServer(access) -> component.setConfig(settings()),
    /// ensureValid(), Components.ARMOR_CONFIG.sync(player). The component and
    /// the sync send are the engine seam.
    pub fn handle_server(&self, component: &mut ArmorConfigComponent, sync: impl FnOnce()) {
        component.set_config(self.settings.clone());
        component.ensure_valid();
        sync();
    }
}

/// Java: ShowMeYourSkin.initNetworking() registration shape: clientbound
/// ConfigSyncPacket + serverbound SettingsUpdatePacket.
pub fn register_channel_handlers(channel: &mut crate::show_me_your_skin::NetChannelSeam) {
    let _ = channel; // Seamed: NET_CHANNEL.registerClientbound/registerServerbound.
}

/// Java: ServerPlayConnectionEvents.JOIN: sync the server config to players
/// who can receive it and validate their component.
pub fn on_join(
    channel: &crate::show_me_your_skin::NetChannelSeam,
    player: &EntityRef,
    component: &mut ArmorConfigComponent,
    synced: &SyncedModConfig,
) {
    if channel.can_send_to_player() {
        channel.server_send(ConfigSyncPacket { config: synced.clone() });
    }
    component.ensure_valid();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::armor_config::ArmorConfig;

    #[test]
    fn sync_packet_sets_client_config() {
        crate::show_me_your_skin_client::set_server_config(None);
        ConfigSyncPacket { config: SyncedModConfig::new(true, true, true) }.handle_client();
        assert!(crate::show_me_your_skin_client::server_config().is_some());
        crate::show_me_your_skin_client::set_server_config(None);
    }

    #[test]
    fn settings_packet_updates_and_syncs() {
        let mut component = ArmorConfigComponent::new(ArmorConfig::vanilla_values());
        let mut synced = false;
        SettingsUpdatePacket { settings: ArmorConfig::vanilla_values() }.handle_server(&mut component, || synced = true);
        assert!(synced);
    }
}
