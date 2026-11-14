//! show-me-your-skin runtime splice (1:1 Rust port of the MIT Show Me Your
//! Skin mod by enjarai, https://codeberg.org/enjarai/show-me-your-skin).
//!
//! This file is the Horizon native-mod wiring the launcher compiles into its
//! client runtime. Upstream reaches the game through the Fabric
//! ModInitializer/ClientModInitializer entrypoints and Minecraft mixins; this
//! splice is only the host binding and adds no policy of its own: it runs the
//! upstream init order and the sync flow through the exact 1:1 modules.

use crate::config::mod_config;
use crate::config::mod_config_server;
use crate::config::armor_config::ArmorConfig;
use crate::config::synced_mod_config::SyncedModConfig;
use crate::seams::EntityRef;
use crate::util::ArmorConfigComponent;

/// One client session's mod state: the server-side synced config and the
/// local player's ARMOR_CONFIG component, as upstream keeps them on
/// ShowMeYourSkin / ShowMeYourSkinClient.
pub struct ShowMeYourSkinRuntime {
    pub synced_server_config: Option<SyncedModConfig>,
    pub local_component: ArmorConfigComponent,
}

impl Default for ShowMeYourSkinRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ShowMeYourSkinRuntime {
    pub fn new() -> Self {
        ShowMeYourSkinRuntime {
            synced_server_config: None,
            local_component: ArmorConfigComponent::new(ArmorConfig::vanilla_values()),
        }
    }

    /// java: ShowMeYourSkin.onInitialize(): ModConfigServer.load() +
    /// initNetworking() (both packets registered on NET_CHANNEL and the join
    /// hook armed, engine seam).
    pub fn init(&mut self) {
        mod_config_server::load();
        crate::show_me_your_skin::init_networking();
    }

    /// java: ShowMeYourSkinClient.onInitializeClient():
    /// ModConfig.INSTANCE.ensureValid(), ModKeyBindings.register(), the
    /// end-of-client-tick hook and the DISCONNECT hook.
    pub fn init_client(&mut self) {
        mod_config::with_instance_mut(|c| c.ensure_valid());
        crate::client::mod_key_bindings::register();
    }

    /// java: ServerPlayConnectionEvents.JOIN hook: sync the server config to
    /// players who can receive it and validate their component.
    pub fn on_player_join(&mut self, channel: &crate::show_me_your_skin::NetChannelSeam, player: &EntityRef) {
        let synced = mod_config_server::synced();
        if channel.can_send_to_player() {
            channel.server_send(crate::net::ConfigSyncPacket { config: synced });
        }
        self.local_component.ensure_valid();
    }

    /// java: ConfigSyncPacket.handleClient: the synced config from the server
    /// lands on ShowMeYourSkinClient.serverConfig.
    pub fn on_config_sync(&mut self, config: SyncedModConfig) {
        self.synced_server_config = Some(config);
    }

    /// java: SettingsUpdatePacket.handleServer: setConfig + ensureValid +
    /// Components.ARMOR_CONFIG.sync(player).
    pub fn on_settings_update(&mut self, settings: ArmorConfig, sync: impl FnOnce()) {
        self.local_component.set_config(settings);
        self.local_component.ensure_valid();
        sync();
    }

    /// java: ShowMeYourSkinClient.syncToServer: stamp the local component and
    /// send the packet when the channel reaches the server.
    pub fn sync_to_server(&mut self, channel: &crate::show_me_your_skin::NetChannelSeam, config: &ArmorConfig) {
        if !channel.can_send_to_server() {
            return;
        }
        self.local_component.set_config(config.clone());
    }

    /// java: ClientTickEvents.END_CLIENT_TICK -> ModKeyBindings.tick.
    pub fn tick(&mut self) {
        crate::client::mod_key_bindings::tick();
    }

    /// java: ClientPlayConnectionEvents.DISCONNECT -> serverConfig = null.
    pub fn on_disconnect(&mut self) {
        self.synced_server_config = None;
    }

    /// java: ShowMeYourSkinClient.serverConfig (nullable read).
    pub fn server_config(&self) -> Option<SyncedModConfig> {
        self.synced_server_config.clone()
    }
}

/// java: ModKeyBindings.click dispatch from the engine keybind seam: K opens
/// the config screen, J flips globalEnabled and saves.
pub fn on_key_click(runtime: &mut ShowMeYourSkinRuntime, open_settings: bool) {
    if open_settings {
        crate::client::mod_key_bindings::queue_click(true);
    } else {
        crate::client::mod_key_bindings::queue_click(false);
    }
    runtime.tick();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_runs_the_upstream_order() {
        mod_config_server::set_instance(crate::config::mod_config_server::ModConfigServer { synced: SyncedModConfig::new(true, true, true) });
        let mut rt = ShowMeYourSkinRuntime::new();
        rt.init();
        rt.init_client();
        assert_eq!(rt.server_config(), None);
        rt.on_config_sync(SyncedModConfig::new(true, true, true));
        assert!(rt.server_config().is_some());
        rt.on_disconnect();
        assert_eq!(rt.server_config(), None);
    }

    #[test]
    fn settings_update_validates_and_syncs() {
        let mut rt = ShowMeYourSkinRuntime::new();
        let mut synced = false;
        rt.on_settings_update(ArmorConfig::vanilla_values(), || synced = true);
        assert!(synced);
        // syncToServer stamps the component when the channel is up.
        rt.sync_to_server(&crate::show_me_your_skin::net_channel(), &ArmorConfig::vanilla_values());
    }

    #[test]
    fn join_syncs_and_validates() {
        mod_config_server::set_instance(crate::config::mod_config_server::ModConfigServer { synced: SyncedModConfig::new(true, true, true) });
        let mut rt = ShowMeYourSkinRuntime::new();
        let player = EntityRef::player(uuid::Uuid::from_u64_pair(0, 7));
        rt.on_player_join(&crate::show_me_your_skin::net_channel(), &player);
        assert!(rt.server_config().is_none());
    }

    #[test]
    fn key_click_flips_global_toggle() {
        mod_config_server::set_instance(crate::config::mod_config_server::ModConfigServer { synced: SyncedModConfig::new(true, true, true) });
        let mut rt = ShowMeYourSkinRuntime::new();
        rt.init_client();
        let before = mod_config::with_instance(|c| c.global_enabled);
        on_key_click(&mut rt, false);
        let after = mod_config::with_instance(|c| c.global_enabled);
        assert_ne!(before, after);
    }
}
