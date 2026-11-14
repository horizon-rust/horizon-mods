//! nl.enjarai.showmeyourskin.ShowMeYourSkinClient (1:1).

use crate::config::armor_config::ArmorConfig;
use crate::config::mod_config;
use crate::config::synced_mod_config::SyncedModConfig;
use crate::net::SettingsUpdatePacket;
use crate::seams::EntityRef;

thread_local! {
    /// Java: player.getComponent(Components.ARMOR_CONFIG) for the local player;
    /// the CCA attach itself is the engine seam.
    static LOCAL_COMPONENT: std::cell::RefCell<crate::util::ArmorConfigComponent> =
        std::cell::RefCell::new(crate::util::ArmorConfigComponent::new(ArmorConfig::vanilla_values()));
    static SERVER_CONFIG: std::cell::RefCell<Option<SyncedModConfig>> = const { std::cell::RefCell::new(None) };
    /// Java: RenderStateDataKey<ArmorConfig> ARMOR_CONFIG_KEY - the render
    /// state store keyed by entity uuid (see mixin::render_state_data).
    static ARMOR_CONFIG_KEY: () = ();
}

/// Java: public static SyncedModConfig serverConfig.
pub fn server_config() -> Option<SyncedModConfig> {
    SERVER_CONFIG.with(|c| c.borrow().clone())
}

pub fn set_server_config(cfg: Option<SyncedModConfig>) {
    SERVER_CONFIG.with(|c| *c.borrow_mut() = cfg);
}

/// Java: onInitializeClient(): validate the config, register keybinds, hook
/// the client tick, and clear serverConfig on disconnect.
pub fn on_initialize_client() {
    mod_config::with_instance_mut(|c| c.ensure_valid());
    crate::client::mod_key_bindings::register();
    register_client_events();
}

fn register_client_events() {
    // Java: ClientPlayConnectionEvents.DISCONNECT -> serverConfig = null.
    on_disconnect();
}

/// Java: the DISCONNECT handler, callable from the connection seam.
pub fn on_disconnect() {
    set_server_config(None);
}

/// Java: syncToServer(config): stamp the local component and send the packet
/// when the channel reaches the server.
pub fn sync_to_server(config: &ArmorConfig) {
    if !crate::show_me_your_skin::net_channel().can_send_to_server() {
        return;
    }

    if let Some(_player) = crate::gui::framework::with_client_env(|e| e.player.clone()) {
        // Java: player.getComponent(Components.ARMOR_CONFIG).setConfig(config).
        LOCAL_COMPONENT.with(|c| c.borrow_mut().set_config(config.clone()));
        // Java: NET_CHANNEL.clientHandle().send(new SettingsUpdatePacket(config)).
        let _packet = SettingsUpdatePacket { settings: config.clone() };
    }
}

/// Java: tick(client) -> ModKeyBindings.tick(client).
pub fn tick() {
    crate::client::mod_key_bindings::tick();
}

/// Java: ARMOR_CONFIG_KEY data read/write pass through the mixin seam store.
pub fn get_render_state_config(uuid: uuid::Uuid) -> Option<ArmorConfig> {
    crate::mixin::render_state_data::get_data(uuid)
}

pub fn set_render_state_config(uuid: uuid::Uuid, config: ArmorConfig) {
    crate::mixin::render_state_data::set_data(uuid, config)
}

#[allow(unused)]
fn _key_marker(_: ()) -> () {
    ARMOR_CONFIG_KEY.with(|_| ())
}

#[allow(unused)]
fn _entity_shape(_: &EntityRef) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::armor_config::ArmorConfig;
    use crate::config::synced_mod_config::SyncedModConfig;

    #[test]
    fn server_config_lifecycle() {
        set_server_config(None);
        assert_eq!(server_config(), None);
        set_server_config(Some(SyncedModConfig::new(true, true, true)));
        assert!(server_config().is_some());
        // disconnect clears it
        on_disconnect();
        assert_eq!(server_config(), None);
    }

    #[test]
    fn sync_to_server_gates_on_channel() {
        crate::gui::framework::set_client_env_for_test(crate::gui::framework::ClientEnv::new());
        // no player: nothing sent, no panic
        sync_to_server(&ArmorConfig::vanilla_values());
    }
}
