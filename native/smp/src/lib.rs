//! The SMP, a native server mod: its rules (`rules/`, plain Rust with their own tests), the glue that reads and
//! changes the game through the mod API's `ServerHost` (`host.rs`, `glue.rs` and the `*_glue.rs` files), and the
//! hooks below, which hand the game's moments to that glue.

pub mod host;
mod util;
pub(crate) use mod_api::profiling as spans;
pub(crate) use util::now_ms;
mod casino_glue;
mod claims_glue;
mod command_line;
mod glue;
mod invsee_glue;
pub mod nbt_codec;
mod ping_glue;
mod smp_chat;
pub mod text;

use host::Game;
use mod_api::{
    Dispensed, Flow, Hooks, HostSide, InteractionResult, Registrar, RegistrationContext,
    ServerHost, ServerTickPhase, WorldPhase,
};

#[derive(Default)]
struct Mod;

fn cancel(canceled: bool) -> Flow {
    if canceled {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// `/rtp` holds the chunks it looks at under a ticket of its own.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let _ = host::TICKETS.set(r.register_ticket_controller("smp:rtp", false));
    }

    fn dispenser_acts(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::DispenserActsEvent<'_>,
    ) -> Option<Dispensed> {
        let mut answer = None;
        claims_glue::dispense(
            event.dimension,
            event.pos,
            event.facing.normal(),
            event.stack,
            &mut answer,
        );
        answer
    }

    /// The SMP's commands in every player's command tree.
    fn register_commands(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::RegisterCommandsEvent<'_>,
    ) {
        command_line::registered(event.commands);
    }

    /// A command a player typed, the SMP's own when it is one.
    fn registered_player_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::RegisteredPlayerCommandEvent<'_>,
    ) -> Flow {
        let line = event.raw;
        if !command_line::player_can_dispatch(line, event.permission) {
            return Flow::Continue;
        }
        let mut game = Game::new(host);
        game.acting = Some(event.player.uuid());
        let Some(mut me) = game.player_info(event.player.uuid()) else {
            return Flow::Continue;
        };
        me.permission = event.permission;
        let mut ran = false;
        ping_glue::perform(&mut game, &me, line, &mut ran);
        if !ran {
            invsee_glue::perform(&mut game, &me, line, &mut ran);
        }
        if !ran {
            glue::perform(&mut game, &me, line, &mut ran);
        }
        cancel(ran)
    }

    /// A command a player typed: a /msg pings its recipient.
    fn player_command(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerCommandEvent<'_>,
    ) -> Flow {
        ping_glue::command_heard(event.player, event.command);
        Flow::Continue
    }

    /// A player's chat line: queued for the SMP's pings, read at the tick's end.
    fn server_chat(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::ServerChatEvent<'_>,
    ) -> Flow {
        smp_chat::heard(event.player, event.raw_text);
        Flow::Continue
    }

    /// The end of each server tick: the SMP's accounts and files.
    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut mod_api::ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            return;
        }
        let mut game = Game::new(host);
        {
            let _span = spans::guard("smp_glue");
            glue::tick(&mut game);
        }
        {
            let _span = spans::guard("smp_invsee");
            invsee_glue::tick(&mut game);
        }
        {
            let _span = spans::guard("smp_ping");
            ping_glue::tick(&mut game);
        }
    }

    /// After a level's tick: in the Overworld, the `/rtp`s waiting for their land read it.
    fn level_tick_post(&self, host: HostSide, event: &mut mod_api::LevelTickPostEvent) {
        let HostSide::Server(host) = host else { return };
        let overworld = host.level(smp_places::OVERWORLD) == Some(event.level);
        glue::level_ticked(&mut Game::new(host), overworld);
    }

    /// The world closing: the SMP writes its files and lets its state go.
    fn world_lifecycle(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::WorldLifecycleEvent<'_>,
    ) {
        if matches!(
            event.phase,
            WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed
        ) {
            glue::closed();
        }
    }

    /// A click in a menu a mod opened: the SMP's, when it is one.
    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut mod_api::MenuClickedEvent) {
        let mut game = Game::new(host);
        let (uuid, window, slot, mode) = (event.uuid, event.window, event.slot, event.mode);
        if !invsee_glue::menu_clicked(&mut game, uuid, window, slot, mode) {
            glue::menu_clicked(&mut game, uuid, window, slot, mode);
        }
    }

    /// A menu a mod opened closed: a trade's ends, nothing moved.
    fn menu_closed(&self, host: &mut dyn ServerHost, event: &mut mod_api::MenuClosedEvent) {
        let mut game = Game::new(host);
        if !invsee_glue::menu_closed(&mut game, event.uuid, event.window) {
            glue::menu_closed(&mut game, event.uuid, event.window);
        }
    }

    /// A player breaks a block: kept out of the land others claimed.
    fn break_block(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::BreakBlockEvent<'_>,
    ) -> Flow {
        let mut canceled = event.cancelled;
        glue::guard(&mut Game::new(host), event.pos, &mut canceled);
        cancel(canceled && !event.cancelled)
    }

    /// A player right clicks a block: kept out of the land others claimed.
    fn right_click_block(
        &self,
        host: HostSide,
        event: &mut mod_api::RightClickBlockEvent<'_>,
    ) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let mut canceled = event.cancelled;
        glue::guard(&mut Game::new(host), event.click.pos, &mut canceled);
        cancel(canceled && !event.cancelled)
    }

    /// A player fills or empties a bucket: kept out of the land others claimed.
    fn fill_bucket(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::FillBucketEvent<'_>,
    ) -> Flow {
        let mut canceled = event.cancelled;
        glue::guard(&mut Game::new(host), event.pos, &mut canceled);
        cancel(canceled && !event.cancelled)
    }

    /// A player hits an entity: a claim's frames, armor stands and animals are kept from players its owner
    /// doesn't trust.
    fn attack_entity(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::AttackEntityEvent<'_>,
    ) -> Flow {
        let mut canceled = event.cancelled;
        glue::attack(&mut Game::new(host), event.kind, event.pos, &mut canceled);
        cancel(canceled && !event.cancelled)
    }

    /// A player uses an item on a block: nothing placed against another's claim from outside it.
    fn use_item_on_block(
        &self,
        host: HostSide,
        event: &mut mod_api::UseItemOnBlockEvent<'_>,
    ) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let Some(stack) = event.click.stack else {
            return Flow::Continue;
        };
        let mut claimed = event.cancelled;
        claims_glue::use_item_on(
            &mut Game::new(host),
            stack,
            event.click.pos,
            event.click.hit.face,
            &mut claimed,
        );
        cancel(claimed && !event.cancelled)
    }

    /// A player uses an entity: a claim's animals, frames and stands keep to its members.
    fn entity_interact(
        &self,
        host: HostSide,
        event: &mut mod_api::EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let mut claimed = false;
        claims_glue::interact_entity(&mut Game::new(host), event.entity, event.kind, &mut claimed);
        claimed.then_some(InteractionResult::Fail)
    }

    /// Fire is about to be set: lava outside a claim lights nothing in it.
    fn block_ignite(
        &self,
        _level: &dyn mod_api::LevelHost,
        event: &mut mod_api::BlockIgniteEvent<'_>,
    ) -> Flow {
        let dimension = glue::dimension_id(event.dimension);
        let mut cancel_it = event.cancelled;
        claims_glue::ignite(
            &dimension,
            [event.pos.x, event.pos.y, event.pos.z],
            event.cause,
            &mut cancel_it,
        );
        cancel(cancel_it && !event.cancelled)
    }

    /// A fluid flows or fire spreads into a block: kept out of the land others claimed.
    fn block_spread(
        &self,
        _level: &dyn mod_api::LevelHost,
        event: &mut mod_api::BlockSpreadEvent<'_>,
    ) -> Flow {
        let dimension = glue::dimension_id(event.dimension);
        let (from, to) = (
            [event.from.x, event.from.y, event.from.z],
            [event.to.x, event.to.y, event.to.z],
        );
        cancel(!glue::world_allows(&dimension, from, &[to]))
    }

    /// A piston pushes, pulls or destroys blocks: kept out of the land others claimed.
    fn piston_pre(
        &self,
        _level: &dyn mod_api::LevelHost,
        event: &mut mod_api::PistonPreEvent<'_>,
    ) -> Flow {
        let dimension = glue::dimension_id(event.dimension);
        let d = event.direction;
        let step = |p: &mod_api::BlockPos| [p.x + d[0], p.y + d[1], p.z + d[2]];
        let touched: Vec<[i32; 3]> = event
            .push
            .iter()
            .flat_map(|p| [[p.x, p.y, p.z], step(p)])
            .chain(event.destroy.iter().map(|p| [p.x, p.y, p.z]))
            .collect();
        let piston = [event.piston.x, event.piston.y, event.piston.z];
        cancel(!glue::world_allows(&dimension, piston, &touched))
    }

    /// An explosion's blocks: claimed land is spared, and so are the animals and frames there it would reach.
    fn explosion_detonate(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::ExplosionDetonateEvent<'_>,
    ) {
        let game = Game::new(host);
        glue::explosion(event.dimension, event.blocks);
        claims_glue::blast(event.dimension, event.center);
        claims_glue::spare(&game, event.dimension, event.entities);
    }

    /// An entity joins the level: where a projectile left from.
    fn entity_join_level(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::EntityJoinLevelEvent<'_>,
    ) -> Flow {
        claims_glue::joined(event.entity, event.kind, event.pos, event.loaded_from_disk);
        Flow::Continue
    }

    /// The SMP hears projectiles' impacts (claims).
    fn hears_projectile_impacts(
        &self,
        _host: &dyn mod_api::Host,
        event: &mut mod_api::HearsProjectileImpactsEvent,
    ) {
        event.hears |= glue::claims_live();
    }

    /// A projectile hits something: in another's claim, only a member's shot acts.
    fn projectile_impact(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::ProjectileImpactEvent<'_>,
    ) -> Flow {
        let mut canceled = event.cancelled;
        claims_glue::impact(
            &mut Game::new(host),
            event.dimension,
            event.projectile,
            event.owner,
            event.entity,
            event.entity_kind,
            event.block,
            event.at,
            &mut canceled,
        );
        cancel(canceled && !event.cancelled)
    }

    /// A painting or frame is about to break: not in another's claim by a player or a blast.
    fn hanging_break(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::HangingBreakEvent<'_>,
    ) -> Flow {
        let mut canceled = event.cancelled;
        claims_glue::hanging_break(&mut Game::new(host), event.player, event.pos, &mut canceled);
        cancel(canceled && !event.cancelled)
    }

    /// A player joined: a newcomer or a player with no spawn point starts at a random spot.
    fn player_logged_in(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerLoggedInEvent<'_>,
    ) {
        glue::spawn_news(event.player, "join");
    }

    /// A player left: where they stood is kept for the operators' `/smpadmin lastpos`.
    fn player_logged_out(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerLoggedOutEvent<'_>,
    ) {
        glue::player_left(&mut Game::new(host), event.player, event.name);
    }

    /// A player respawned: a newcomer or a player with no spawn point starts at a random spot.
    fn player_respawned(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerRespawnedEvent<'_>,
    ) {
        glue::spawn_news(event.player, "respawn");
    }

    /// A spawn point is set (a bed, an anchor, a command) or reset: a player with one never starts at a random
    /// spot.
    fn player_set_spawn(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerSetSpawnEvent<'_>,
    ) -> Flow {
        let kind = if event.pos.is_none() {
            "death_reset"
        } else {
            "set"
        };
        glue::spawn_news(event.player, kind);
        Flow::Continue
    }
}

#[path = "rules/smp_admin.rs"]
pub mod smp_admin;
#[path = "rules/smp_auction.rs"]
pub mod smp_auction;
#[path = "rules/smp_board.rs"]
pub mod smp_board;
#[path = "rules/smp_casino.rs"]
pub mod smp_casino;
#[path = "rules/smp_casino_blackjack.rs"]
pub mod smp_casino_blackjack;
#[path = "rules/smp_casino_crash.rs"]
pub mod smp_casino_crash;
#[path = "rules/smp_casino_roulette.rs"]
pub mod smp_casino_roulette;
#[path = "rules/smp_casino_slots.rs"]
pub mod smp_casino_slots;
#[path = "rules/smp_claims.rs"]
pub mod smp_claims;
#[path = "rules/smp_economy.rs"]
pub mod smp_economy;
#[path = "rules/smp_games.rs"]
pub mod smp_games;
#[path = "rules/smp_invsee.rs"]
pub mod smp_invsee;
#[path = "rules/smp_ledger.rs"]
pub mod smp_ledger;
#[path = "rules/smp_lottery.rs"]
pub mod smp_lottery;
#[path = "rules/smp_market.rs"]
pub mod smp_market;
#[path = "rules/smp_market_chart.rs"]
pub mod smp_market_chart;
#[path = "rules/smp_market_menu.rs"]
pub mod smp_market_menu;
#[path = "rules/smp_market_prices.rs"]
pub mod smp_market_prices;
#[path = "rules/smp_nick.rs"]
pub mod smp_nick;
#[path = "rules/smp_ping.rs"]
pub mod smp_ping;
#[path = "rules/smp_places.rs"]
pub mod smp_places;
#[path = "rules/smp_presence.rs"]
pub mod smp_presence;
#[path = "rules/smp_recipes.rs"]
pub mod smp_recipes;
#[path = "rules/smp_report.rs"]
pub mod smp_report;
#[path = "rules/smp_reports.rs"]
pub mod smp_reports;
#[path = "rules/smp_shop.rs"]
pub mod smp_shop;
#[path = "rules/smp_store.rs"]
pub mod smp_store;
#[path = "rules/smp_switch.rs"]
pub mod smp_switch;
#[path = "rules/smp_trade.rs"]
pub mod smp_trade;
#[path = "rules/smp_vanish.rs"]
pub mod smp_vanish;
