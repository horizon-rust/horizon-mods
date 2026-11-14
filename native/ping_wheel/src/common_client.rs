//! Upstream: common/CommonClient.java (1:1). The platform event hooks register
//! through the IPlatformClientEventService seam; upstream's `Game = Minecraft.getInstance()`
//! each tick maps to the host-held seam client in `seams::game()`.
use std::rc::Rc;

use crate::config::client_handler;
use crate::core::{GameContext, PingController, PingManager};
use crate::network::{PingLocationS2CPacket, UpdateChannelC2SPacket};
use crate::render::OverlayRenderer;
use crate::seams::{self, PoseStack};
use crate::util::InputUtils;

pub struct CommonClient;

impl CommonClient {
    pub fn on_init() {
        client_handler().load();

        let events = seams::client_event_service();
        events.register_tick_start_event(std::rc::Rc::new(Self::on_tick_start));
        events.register_join_server_event(std::rc::Rc::new(Self::on_join_server));
        events.register_leave_server_event(std::rc::Rc::new(Self::on_leave_server));
        events.register_render_world_event(std::rc::Rc::new(Self::on_render_world));
        events.register_render_gui_event(std::rc::Rc::new(Self::on_render_gui));

        let ctx = seams::context_service();
        // Upstream registers the static KEY_BINDING_PING / KEY_BINDING_SETTINGS; this
        // port constructs the tokens per call, so the host records fresh ones.
        ctx.register_key_mapping(&InputUtils::key_binding_ping());
        ctx.register_key_mapping(&InputUtils::key_binding_settings());

        crate::compat::migrate_key_mappings();
    }

    pub fn on_join_server() {
        seams::network_service().send_to_server(&UpdateChannelC2SPacket::new(
            Some(client_handler().config().get_channel()),
        ));
    }

    pub fn on_leave_server() {
        PingManager::clear_pings();
    }

    pub fn on_tick_start() {
        // Upstream: Game = Minecraft.getInstance(); the host keeps the seam client current.
        GameContext::update_dimension();

        crate::compat::on_tick();

        if InputUtils::consume_ping_hotkey() {
            PingController::queue_ping_action();
        }

        if seams::consume_click_key(&InputUtils::key_binding_settings()) {
            if let Some(game) = seams::game() {
                game.set_screen(Rc::new(|| crate::screen::SettingsScreen::new()));
            }
        }
    }

    pub fn on_render_world(ctx: &crate::core::WorldRenderContext) {
        PingManager::update_pings(ctx);
        PingController::poll_ping_action(ctx.tick_delta);
    }

    pub fn on_render_gui(pose_stack: PoseStack, _tick_delta: f32) {
        OverlayRenderer::draw(pose_stack, _tick_delta);
    }

    pub fn on_ping_location_packet(packet: &PingLocationS2CPacket) {
        PingManager::accept_ping_packet(packet);
    }
}
