// Voice chat's server half: the relay (crate voicechat_server_relay) opens its
// UDP port when the first voice chat client asks for a secret, hears the
// clients' messages on the `voicechat:` play channels, and learns each tick
// where every player is.

mod glue;

use mod_api::{
    self, Hooks, PlayerLoggedOutEvent, Registrar, RegistrationContext, ServerHost,
    ServerStoppedEvent, ServerTickEvent, ServerTickPhase,
};

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        glue::register(r);
    }

    /// The end of each server tick: the relay learns where everyone is, and its
    /// messages leave for the players' clients.
    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            glue::tick(host);
        }
    }

    /// A player left: the relay forgets them and tells the others.
    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        glue::left(event.player);
    }

    /// The server stopped: the relay closes its UDP port.
    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut ServerStoppedEvent) {
        glue::stop();
    }
}
