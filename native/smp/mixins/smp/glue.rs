// The SMP in the server's game: its state for the open world, and the edges
// between the game's types and the rules (smp_economy, smp_market,
// smp_trade, smp_games, smp_lottery, smp_places, smp_claims). Added to
// the game as `game::smp_glue`; the helpers (register.rs, command.rs,
// tick.rs, lifecycle.rs, menu_clicked.rs, menu_closed.rs, destroy_block.rs,
// right_click_block.rs, block_spread.rs, piston_move.rs,
// explosion_detonate.rs, fill_bucket.rs, attack_entity.rs) call it from the
// hook points.

use crate::game::{
    Game,
    native_server::{NativeCommand, NativeServerPlayer, NativeSidebar},
    smp_board::{self, Board, Line},
    smp_claims::{self, Claims},
    smp_economy::{self, Economy, Out, Player},
    smp_games::{self, Games},
    smp_lottery::Lottery,
    smp_places::{self, Body, Place, Places},
    native_menus::native_menu_inventory_slot,
    smp_market::{self, Bag, Market},
    smp_store,
    smp_trade::{self, Act, Cell, Trades},
};
use horizon_client::{
    items::ItemStack,
    lan::data::component_nbt,
    text::{self, Text},
};
use std::{path::PathBuf, sync::Mutex};

/// The mod's id, name and version.
pub const ID: &str = "smp";
pub const NAME: &str = "SMP";
pub const VERSION: &str = "0.10.0";

/// `config.json` in `world/horizon-smp/`, written with its defaults the
/// first time, and again with the keys a newer version adds (values kept).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub economy: smp_economy::Config,
    pub market: smp_market::Config,
    pub gambling: smp_games::Config,
    pub places: smp_places::Config,
    pub claims: smp_claims::Config,
    pub board: smp_board::Config,
}

/// The SMP of the open world.
pub struct Smp {
    dir: PathBuf,
    /// An error when `economy.json` can't be read: the commands say so.
    economy: Result<Economy, String>,
    market: Market,
    trades: Trades<ItemStack>,
    games: Games,
    lottery: Lottery,
    /// An error when `places.json` can't be read: the commands say so.
    places: Result<Places, String>,
    /// An error when `claims.json` can't be read: nothing is guarded then,
    /// and the commands say so.
    claims: Result<Claims, String>,
    /// An error when `board.json` can't be read: no sidebar then.
    board: Result<Board, String>,
    /// `/claim show`: whose chunk border is drawn (level, chunk x, chunk z)
    /// until when.
    shows: std::collections::HashMap<u128, (String, i32, i32, i64)>,
}

static STATE: Mutex<Option<Smp>> = Mutex::new(None);
static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Whether the next tick has work (a teleport warming up, a trade open, a
/// command's changes to write): otherwise only one tick a second does any.
static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn open(dir: PathBuf) -> Smp {
    let config_path = dir.join("config.json");
    let (config, write) = match smp_store::read_json::<Option<serde_json::Value>>(&config_path) {
        // No file yet: the defaults, written for the owner to edit.
        Ok(None) => (Config::default(), true),
        Ok(Some(v)) => match serde_json::from_value::<Config>(v.clone()) {
            // Written again only when a newer version adds keys; values kept.
            Ok(c) => {
                let keys = |v: &serde_json::Value| v.as_object().map(|o| o.len());
                let full = serde_json::to_value(&c).unwrap_or_default();
                let added = keys(&full) != keys(&v);
                (c, added)
            }
            Err(e) => {
                log::error!(
                    "[smp] {}: {e}: the defaults are used",
                    config_path.display()
                );
                (Config::default(), false)
            }
        },
        // A damaged file stays as it is for the owner to mend.
        Err(e) => {
            log::error!("[smp] {e}: the defaults are used");
            (Config::default(), false)
        }
    };
    if write && let Err(e) = smp_store::write_json(&config_path, &config) {
        log::error!("[smp] {e}");
    }
    log::info!("[smp] {NAME} {VERSION} on {}", dir.display());
    let economy = Economy::open(&dir, config.economy);
    if let Err(e) = &economy {
        log::error!("[smp] the economy is off: {e}");
    }
    let places = Places::open(&dir, config.places);
    if let Err(e) = &places {
        log::error!("[smp] homes and warps are off: {e}");
    }
    let claims = Claims::open(&dir, config.claims);
    if let Err(e) = &claims {
        log::error!("[smp] claims are off: {e}");
    }
    let board = Board::open(&dir, config.board);
    if let Err(e) = &board {
        log::error!("[smp] the sidebar is off: {e}");
    }
    Smp {
        dir,
        economy,
        market: Market {
            config: config.market,
        },
        trades: Trades::default(),
        games: Games::new(config.gambling),
        lottery: Lottery::default(),
        places,
        claims,
        board,
        shows: Default::default(),
    }
}

/// The SMP's state, opened for the world in `dir` on first use (the files
/// of another world it held are written first).
fn state(dir: PathBuf) -> std::sync::MutexGuard<'static, Option<Smp>> {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.as_ref().is_none_or(|s| s.dir != dir) {
        if let Some(old) = state.as_mut() {
            save(old);
        }
        *state = Some(open(dir));
    }
    if let Some(Ok(e)) = state.as_mut().map(|s| &mut s.economy) {
        e.now = chrono::Utc::now().timestamp_millis();
    }
    state
}

fn save(smp: &mut Smp) {
    if let Ok(e) = &mut smp.economy
        && let Err(err) = e.save()
    {
        log::error!("[smp] {err}");
    }
    if let Ok(p) = &mut smp.places
        && let Err(err) = p.save(chrono::Utc::now().timestamp_millis())
    {
        log::error!("[smp] {err}");
    }
    if let Ok(c) = &mut smp.claims
        && let Err(err) = c.save()
    {
        log::error!("[smp] {err}");
    }
    if let Ok(b) = &mut smp.board
        && let Err(err) = b.save()
    {
        log::error!("[smp] {err}");
    }
}

/// The commands, for every player's command tree.
pub fn commands(commands: &mut Vec<NativeCommand>) {
    for usage in [
        "balance [player:word]",
        "bal [player:word]",
        "pay <player:word> <amount:integer>",
        "baltop [page:integer]",
        "worth [item:item]",
        "sell hand [count:integer]",
        "sell all",
        "sell <item:item> [count:integer]",
        "buy <item:item> [count:integer]",
        "trade <player:word>",
        "trade accept [player:word]",
        "trade deny [player:word]",
        "trade cancel",
        "trade money <amount:integer>",
        "coinflip <amount:integer> [player:word]",
        "coinflip accept <player:word>",
        "coinflip cancel",
        "coinflip list",
        "lottery",
        "lottery buy <tickets:integer>",
        "spawn",
        "home [name:word]",
        "sethome [name:word]",
        "delhome [name:word]",
        "homes",
        "warp <name:word>",
        "warps",
        "tpa <player:word>",
        "tpaccept [player:word]",
        "tpdeny [player:word]",
        "rules",
        "back",
        "playtime [player:word]",
        "claim",
        "claim info",
        "claim show",
        "unclaim",
        "claims",
        "trust <player:word>",
        "untrust <player:word>",
        "sidebar",
        "sidebar on",
        "sidebar off",
    ] {
        commands.push(NativeCommand::new(usage));
    }
    for usage in ["setspawn", "setwarp <name:word>", "delwarp <name:word>"] {
        commands.push(NativeCommand::op(usage, 3));
    }
    for verb in ["give", "take", "set"] {
        commands.push(NativeCommand::op(
            &format!("eco {verb} <player:word> <amount:integer>"),
            3,
        ));
    }
}

/// The acting player's items (inside `native_perform_command`).
struct GameBag<'a, 'g>(&'a mut Game<'g>);

impl Bag for GameBag<'_, '_> {
    fn held(&self) -> Option<(String, u32, bool)> {
        self.0.native_held()
    }
    fn carried(&self) -> Vec<(String, u32)> {
        self.0.native_carried_items()
    }
    fn count(&mut self, item: &str) -> u32 {
        self.0.native_count_items(item)
    }
    fn take(&mut self, item: &str, count: u32) -> u32 {
        self.0.native_take_items(item, count)
    }
    fn room(&mut self, item: &str) -> u32 {
        self.0.native_room_for(item)
    }
    fn give(&mut self, item: &str, count: u32) -> u32 {
        self.0.native_give_items(item, count)
    }
}

const COMMANDS: [&str; 11] = [
    "balance", "bal", "pay", "baltop", "eco", "worth", "sell", "buy", "trade", "coinflip", "lottery",
];

/// The players on, as the places see them.
fn bodies(players: &[NativeServerPlayer]) -> Vec<Body> {
    players
        .iter()
        .map(|p| Body {
            uuid: p.uuid,
            name: p.name.clone(),
            at: Place {
                dimension: p.dimension.clone(),
                x: p.position[0],
                y: p.position[1],
                z: p.position[2],
            },
            health: p.health,
        })
        .collect()
}

/// A random number below `n` (n > 0), never the world's random: a fresh
/// SipHash key (seeded from the system's randomness) over the time.
fn below(n: u64) -> u64 {
    use std::hash::BuildHasher;
    let time = std::time::SystemTime::now();
    std::collections::hash_map::RandomState::new().hash_one(time) % n.max(1)
}

/// A fair coin, never the world's random: a fresh SipHash key (seeded from
/// the system's randomness) over the time.
fn heads() -> bool {
    use std::hash::BuildHasher;
    let time = std::time::SystemTime::now();
    std::collections::hash_map::RandomState::new().hash_one(time) & 1 == 1
}

/// A player's command.
pub(in crate::game) fn perform(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    let word = line.split(' ').next().unwrap_or_default();
    let ours = [
        &COMMANDS[..],
        &smp_places::COMMANDS,
        &smp_claims::COMMANDS,
        &smp_board::COMMANDS,
    ];
    if !game.native_on(ID) || !ours.iter().any(|c| c.contains(&word)) {
        return;
    }
    BUSY.store(true, std::sync::atomic::Ordering::Relaxed);
    let Some(dir) = game.native_world_dir() else {
        return;
    };
    let me = Player {
        uuid: player.uuid,
        name: player.name.clone(),
        level: player.permission,
    };
    // Survival and adventure: a creative player could sell what they made.
    let trades = matches!(player.game_mode, 0 | 2);
    let players = game.native_server_players();
    let online = |name: &str| {
        players
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map(|p| (p.uuid, p.name.clone(), matches!(p.game_mode, 0 | 2)))
    };
    let now = chrono::Utc::now().timestamp_millis();
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let mut acts = Vec::new();
    let out = if smp_board::COMMANDS.contains(&word) {
        match &mut smp.board {
            Ok(b) => {
                let (out, show) = b.command(&me, line).unwrap_or_default();
                // Shown again at the next second's update; taken away now.
                if show == Some(false) {
                    game.native_sidebar(me.uuid, None);
                }
                out
            }
            Err(_) => vec![Out::Error(
                "The sidebar is off: its file can't be read (see the server log).".into(),
            )],
        }
    } else if line.split_whitespace().eq(["claim", "show"]) {
        match bodies(&players).into_iter().find(|b| b.uuid == me.uuid) {
            Some(b) => {
                let (cx, cz) = ((b.at.x.floor() as i32) >> 4, (b.at.z.floor() as i32) >> 4);
                smp.shows.insert(me.uuid, (b.at.dimension, cx, cz, now + 10_000));
                vec![Out::Reply(
                    "This chunk's border, for 10 s: green yours, red someone else's, white free.".into(),
                )]
            }
            None => vec![Out::Error("You are not in the world.".into())],
        }
    } else if smp_claims::COMMANDS.contains(&word) {
        let at = bodies(&players).into_iter().find(|b| b.uuid == me.uuid).map(|b| b.at);
        let played = smp.places.as_ref().map_or(0, |p| p.played_seconds(me.uuid));
        let economy = smp.economy.as_ref().ok();
        let find = |n: &str| {
            let on = players.iter().find(|p| p.name.eq_ignore_ascii_case(n));
            on.map(|p| (p.uuid, p.name.clone()))
                .or_else(|| economy?.find(n).map(|(u, a)| (u, a.name.clone())))
        };
        let name_of = |u: u128| {
            let on = players.iter().find(|p| p.uuid == u).map(|p| p.name.clone());
            on.or_else(|| economy?.name_of(u).map(str::to_owned))
                .unwrap_or_else(|| smp_store::uuid_string(u))
        };
        match (&mut smp.claims, at) {
            (Ok(c), Some(at)) => c.command(&me, &at, played, &find, &name_of, line).unwrap_or_default(),
            (Ok(_), None) => vec![Out::Error("You are not in the world.".into())],
            (Err(_), _) => vec![Out::Error(
                "Claims are off: their file can't be read (see the server log).".into(),
            )],
        }
    } else if smp_places::COMMANDS.contains(&word) {
        match &mut smp.places {
            Ok(p) => p.command(&me, &bodies(&players), now, line).unwrap_or_default(),
            Err(_) => vec![Out::Error(
                "Homes and warps are off: their file can't be read (see the server log).".into(),
            )],
        }
    } else {
        match &mut smp.economy {
        Ok(e) => {
            e.seen(me.uuid, &me.name);
            let mut bag = GameBag(game);
            e.command(&me, line)
                .or_else(|| smp.market.command(e, &me, trades, &mut bag, line))
                .or_else(|| {
                    let (out, a) = smp.trades.command(e, &me, trades, &online, now, line)?;
                    acts = a;
                    Some(out)
                })
                .or_else(|| {
                    let named = |n: &str| online(n).map(|(u, name, _)| (u, name));
                    smp.games.command(e, &me, &named, now, &mut heads, line)
                })
                .or_else(|| smp.lottery.command(&smp.games.config, e, &me, now, line))
                .unwrap_or_default()
        }
        Err(_) => vec![Out::Error(
            "The economy is off: its file can't be read (see the server log).".into(),
        )],
        }
    };
    act(game, smp, acts);
    drop(guard);
    *ran = true;
    show(game, out);
}

/// A player breaks or right clicks the block at `pos`: canceled in a chunk
/// claimed by someone who doesn't trust them (`native_destroy_block`,
/// `native_right_click_block`).
pub(in crate::game) fn guard(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || !game.native_on(ID) {
        return;
    }
    let (Some(who), Some(dir)) = (game.native_acting_player(), game.native_world_dir()) else {
        return;
    };
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let Ok(claims) = &mut smp.claims else {
        return;
    };
    let me = Player {
        uuid: who.uuid,
        name: who.name.clone(),
        level: who.permission,
    };
    let economy = smp.economy.as_ref().ok();
    let name_of = |u: u128| {
        economy
            .and_then(|e| e.name_of(u))
            .map_or_else(|| smp_store::uuid_string(u), str::to_owned)
    };
    let now = chrono::Utc::now().timestamp_millis();
    if let Err(line) = claims.check(&me, &who.dimension, pos, &name_of, now) {
        *canceled = true;
        drop(guard);
        if let Some(line) = line {
            game.native_action_bar(who.uuid, Text::literal(line).with_color(text::RED));
        }
    }
}
/// A player hits the entity of type `kind` at `pos`: canceled when claims
/// protect that type and the chunk is claimed by someone who doesn't trust
/// them (`native_attack_entity`).
pub(in crate::game) fn attack(game: &mut Game<'_>, kind: &str, pos: [f64; 3], canceled: &mut bool) {
    if *canceled || !game.native_on(ID) {
        return;
    }
    let protected = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|s| s.claims.as_ref().is_ok_and(|c| c.protects(kind)));
    if protected {
        guard(game, pos.map(|v| v.floor() as i32), canceled);
    }
}

/// The world's dimension id, as players' places name it.
pub(in crate::game) fn dimension_id(dimension: crate::game::LevelDimension) -> &'static str {
    match dimension {
        crate::game::LevelDimension::Overworld => "minecraft:overworld",
        crate::game::LevelDimension::Nether => "minecraft:the_nether",
        crate::game::LevelDimension::End => "minecraft:the_end",
    }
}

/// Whether the world may change the blocks `to` from `from` (a fluid, fire,
/// a piston's blocks): not into a claim from land of another owner. Asked
/// on every flow: it only reads the claims already open, and allows while
/// the SMP's state is busy (a hook inside the SMP's own call).
pub(in crate::game) fn world_allows(dimension: &str, from: [i32; 3], to: &[[i32; 3]]) -> bool {
    let Ok(state) = STATE.try_lock() else {
        return true;
    };
    match state.as_ref().map(|s| &s.claims) {
        Some(Ok(c)) => to.iter().all(|&t| c.spread_allowed(dimension, from, t)),
        _ => true,
    }
}

/// An explosion is about to break `blocks`: claimed land is spared
/// (`native_explosion_detonate`).
pub(in crate::game) fn explosion(game: &Game<'_>, dimension: &str, blocks: &mut Vec<[i32; 3]>) {
    if !game.native_on(ID) {
        return;
    }
    let Ok(state) = STATE.try_lock() else {
        return;
    };
    if let Some(Ok(c)) = state.as_ref().map(|s| &s.claims) {
        c.spare(dimension, blocks);
    }
}

/// An item named `name` (not in italics, as a name the game gives).
fn button(item: &str, name: &str, color: &str) -> ItemStack {
    let mut stack = ItemStack {
        item: item.into(),
        count: 1,
        damage: 0,
        extra: None,
    };
    let title = serde_json::json!({ "text": name, "italic": false, "color": color });
    stack.data_mut().custom_name = Some(component_nbt(&title));
    stack
}

/// The trade menu's cells as items.
fn menu_items(smp: &Smp, uuid: u128) -> Vec<Option<ItemStack>> {
    let money = |n| match &smp.economy {
        Ok(e) => e.money(n),
        Err(_) => n.to_string(),
    };
    smp.trades
        .layout(uuid)
        .into_iter()
        .map(|cell| match cell {
            Cell::Empty => None,
            Cell::Stack(s) => Some(s),
            Cell::Divider => Some(button("minecraft:gray_stained_glass_pane", " ", "gray")),
            Cell::Money(true, n) => Some(button(
                "minecraft:gold_ingot",
                &format!("Your money: {} (/trade money <amount>)", money(n)),
                "gold",
            )),
            Cell::Money(false, n) => Some(button(
                "minecraft:gold_ingot",
                &format!("Their money: {}", money(n)),
                "gold",
            )),
            Cell::Confirm(false) => Some(button(
                "minecraft:lime_stained_glass_pane",
                "Confirm the trade",
                "green",
            )),
            Cell::Confirm(true) => Some(button(
                "minecraft:lime_concrete",
                "Confirmed (click to take it back)",
                "green",
            )),
            Cell::Cancel => Some(button(
                "minecraft:red_stained_glass_pane",
                "Cancel the trade",
                "red",
            )),
            Cell::Other(name, true) => Some(button(
                "minecraft:lime_concrete",
                &format!("{name} confirmed"),
                "green",
            )),
            Cell::Other(name, false) => Some(button(
                "minecraft:gray_concrete",
                &format!("{name} has not confirmed"),
                "gray",
            )),
        })
        .collect()
}

/// Carries out what the trade rules ask.
fn act(game: &mut Game<'_>, smp: &mut Smp, acts: Vec<Act>) {
    for a in acts {
        match a {
            Act::Show(uuid) => {
                let items = menu_items(smp, uuid);
                let Some(t) = smp.trades.open.iter_mut().find(|t| t.side_of(uuid).is_some()) else {
                    continue;
                };
                let me = t.side_of(uuid).expect("found");
                if t.sides[me].window.is_some() {
                    game.native_set_menu_items(uuid, items);
                } else {
                    let title = Text::literal(format!("Trade with {}", t.sides[1 - me].name));
                    t.sides[me].window = game.native_open_menu(uuid, smp_trade::ROWS, title, items);
                }
            }
            Act::Close(uuid) => game.native_close_menu(uuid),
            Act::Settle(uuid) => settle(game, smp, uuid),
        }
    }
}

/// Both confirmed: the trade is logged, then both offers swap in one step
/// with the money, or nothing moves and both are told why.
fn settle(game: &mut Game<'_>, smp: &mut Smp, uuid: u128) {
    let Some(t) = smp.trades.trade_of(uuid).cloned() else {
        return;
    };
    let [a, b] = &t.sides;
    let players = game.native_server_players();
    let may = |u: u128| players.iter().any(|p| p.uuid == u && matches!(p.game_mode, 0 | 2));
    let Ok(economy) = &mut smp.economy else {
        return;
    };
    let fail = |why: String| -> Vec<Out> {
        t.sides
            .iter()
            .map(|s| Out::Tell(s.uuid, format!("The trade didn't go through: {why}.")))
            .collect()
    };
    let stacks = |side: &smp_trade::Side<ItemStack>| -> serde_json::Value {
        side.offer
            .iter()
            .map(|(_, s)| serde_json::json!({ "item": &*s.item, "count": s.count, "plain": s.damage == 0 && s.extra.is_none() }))
            .collect()
    };
    let out = if !may(a.uuid) || !may(b.uuid) {
        fail("both must be here, in survival or adventure mode".into())
    } else if economy.balance(a.uuid) < a.money || economy.balance(b.uuid) < b.money {
        fail("one of you no longer has the money offered".into())
    } else if let Err(e) = economy.record(serde_json::json!({
        "kind": "trade",
        "a": smp_store::uuid_string(a.uuid), "a_items": stacks(a), "a_money": a.money,
        "b": smp_store::uuid_string(b.uuid), "b_items": stacks(b), "b_money": b.money,
    })) {
        fail(e)
    } else if let Err(e) = game.native_exchange_items(a.uuid, &a.offer, b.uuid, &b.offer) {
        let _ = economy.record(serde_json::json!({ "kind": "trade_failed", "why": &e }));
        fail(e)
    } else {
        // The balances were checked above: only the log could refuse now.
        let net = a.money - b.money;
        let paid = match net.signum() {
            1 => economy.transfer("trade_money", a.uuid, b.uuid, net),
            -1 => economy.transfer("trade_money", b.uuid, a.uuid, -net),
            _ => Ok(()),
        };
        if let Err(e) = paid {
            log::error!("[smp] a trade's money: {e}");
        }
        let (_, acts) = smp.trades.end(uuid, "");
        // Ended as a success: the menus close, with their own line.
        for act in acts {
            if let Act::Close(u) = act {
                game.native_close_menu(u);
            }
        }
        let line = |me: &smp_trade::Side<ItemStack>, them: &smp_trade::Side<ItemStack>| {
            Out::Tell(
                me.uuid,
                format!("Trade with {} done: you gave {} stack(s), got {}.", them.name, me.offer.len(), them.offer.len()),
            )
        };
        show(game, vec![line(a, b), line(b, a)]);
        return;
    };
    // Not settled: both must confirm again.
    if let Some(t) = smp.trades.open.iter_mut().find(|t| t.side_of(uuid).is_some()) {
        for s in &mut t.sides {
            s.confirmed = false;
        }
    }
    act(game, smp, vec![Act::Show(a.uuid), Act::Show(b.uuid)]);
    show(game, out);
}

/// A click in a native menu: a trade's, when it is one.
pub(in crate::game) fn menu_clicked(game: &mut Game<'_>, uuid: u128, window: i32, slot: i32, mode: i32) {
    // A plain click or a shift-click; drags and double clicks are ignored.
    if !game.native_on(ID) || !matches!(mode, 0 | 1) {
        return;
    }
    let Some(dir) = game.native_world_dir() else {
        return;
    };
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let Some(t) = smp.trades.trade_of(uuid) else {
        return;
    };
    let me = t.side_of(uuid).expect("found");
    if t.sides[me].window != Some(window) {
        return;
    }
    let (out, acts) = match native_menu_inventory_slot(smp_trade::SIZE, slot) {
        Some(i) => match game.native_player_stacks(uuid).and_then(|s| s.get(i).cloned().flatten()) {
            Some(stack) => smp.trades.click(uuid, None, Some((i, stack))),
            None => (Vec::new(), Vec::new()),
        },
        None => match usize::try_from(slot).ok().filter(|&s| s < smp_trade::SIZE) {
            Some(s) => smp.trades.click(uuid, Some(s), None),
            None => (Vec::new(), Vec::new()),
        },
    };
    act(game, smp, acts);
    drop(guard);
    show(game, out);
}

/// A native menu closed: a trade's ends, nothing moved.
pub(in crate::game) fn menu_closed(game: &mut Game<'_>, uuid: u128, window: i32) {
    if !game.native_on(ID) {
        return;
    }
    let Some(dir) = game.native_world_dir() else {
        return;
    };
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let Some(t) = smp.trades.trade_of(uuid) else {
        return;
    };
    let me = t.side_of(uuid).expect("found");
    if t.sides[me].window != Some(window) {
        return;
    }
    let why = format!("{} closed the trade", t.sides[me].name);
    let (out, acts) = smp.trades.end(uuid, &why);
    act(game, smp, acts);
    drop(guard);
    show(game, out);
}

fn show(game: &mut Game<'_>, out: Vec<Out>) {
    for o in out {
        match o {
            Out::Reply(s) => game.native_feedback(Text::literal(s)),
            Out::Error(s) => game.native_feedback(Text::literal(s).with_color(text::RED)),
            Out::Tell(uuid, s) => {
                game.native_tell(uuid, Text::literal(s).with_color(text::GOLD));
            }
            Out::All(s) => game.native_broadcast(Text::literal(s).with_color(text::GOLD)),
        }
    }
}

/// The end of each server tick: accounts for the players on (once a second)
/// and the files that changed.
pub(in crate::game) fn tick(game: &mut Game<'_>) {
    use std::sync::atomic::Ordering::Relaxed;
    let n = TICKS.fetch_add(1, Relaxed);
    // An idle tick costs a counter: the rest runs once a second, or each
    // tick while something waits on it.
    if n % 20 != 0 && !BUSY.load(Relaxed) {
        return;
    }
    let Some(dir) = game.native_world_dir() else {
        return;
    };
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let warming = smp.places.as_ref().is_ok_and(|p| !p.pending.is_empty());
    let players = if n % 20 == 0 || warming {
        game.native_server_players()
    } else {
        Vec::new()
    };
    let now = chrono::Utc::now().timestamp_millis();
    let mut out = Vec::new();
    // Teleports: each tick while one warms up; requests expire once a second.
    if let Ok(p) = &mut smp.places
        && (n % 20 == 0 || warming)
    {
        if n % 20 == 0 {
            let on: Vec<u128> = players.iter().map(|p| p.uuid).collect();
            p.played(&on, now);
        }
        let (o, go) = p.tick(&bodies(&players), now);
        out.extend(o);
        for t in go {
            // `.3`: a whole number would be a block's centre to `tp`.
            let line = format!(
                "execute in {} run tp {} {:.3} {:.3} {:.3}",
                t.to.dimension, t.name, t.to.x, t.to.y, t.to.z
            );
            out.push(if game.native_run_command(&line) {
                Out::Tell(t.uuid, t.done)
            } else {
                log::warn!("[smp] a teleport failed: {line}");
                Out::Tell(t.uuid, "The teleport failed.".into())
            });
        }
    }
    if let Ok(e) = &mut smp.economy
        && n % 20 == 0
    {
        for p in &players {
            e.seen(p.uuid, &p.name);
        }
        // Once a second: coinflips nobody took, and those of players gone.
        out.extend(smp.games.expire(e, chrono::Utc::now().timestamp_millis()));
        let gone: Vec<u128> = smp
            .games
            .offers
            .iter()
            .map(|o| o.from)
            .filter(|u| !players.iter().any(|p| p.uuid == *u))
            .collect();
        for uuid in gone {
            smp.games.left(e, uuid);
        }
        out.extend(smp.lottery.tick(&smp.games.config, e, now, &mut below));
    }
    // Each trade's offers against the inventories now: a player gone or
    // dead ends it; a stack that changed leaves its offer.
    let sides: Vec<(u128, String)> = smp
        .trades
        .open
        .iter()
        .flat_map(|t| t.sides.iter().map(|s| (s.uuid, s.name.clone())))
        .collect();
    for (uuid, name) in sides {
        match game.native_player_stacks(uuid) {
            None => {
                let (o, acts) = smp.trades.end(uuid, &format!("{name} is gone"));
                out.extend(o);
                act(game, smp, acts);
            }
            Some(stacks) if smp.trades.refresh(uuid, &stacks) => {
                let other = smp.trades.trade_of(uuid).and_then(|t| {
                    t.sides.iter().find(|s| s.uuid != uuid).map(|s| s.uuid)
                });
                act(game, smp, vec![Act::Show(uuid)]);
                if let Some(o) = other {
                    act(game, smp, vec![Act::Show(o)]);
                    out.push(Out::Tell(o, format!("{name}'s offer changed.")));
                }
                out.push(Out::Tell(uuid, "A stack you offered changed: it left your offer.".into()));
            }
            Some(_) => {}
        }
    }
    // `/claim show`: the border once a second, to that player only.
    let mut particles = Vec::new();
    if n % 20 == 0 {
        smp.shows.retain(|_, s| s.3 > now);
        for (uuid, (dimension, cx, cz, _)) in &smp.shows {
            let Some(p) = players.iter().find(|p| p.uuid == *uuid && p.dimension == *dimension) else {
                continue;
            };
            let relation = smp.claims.as_ref().ok().and_then(|c| c.relation(dimension, cx * 16, cz * 16, *uuid));
            let color = match relation {
                Some(true) => "0,1,0",
                Some(false) => "1,0,0",
                None => "1,1,1",
            };
            for [x, y, z] in smp_claims::border(*cx, *cz, p.position[1] + 1.0, 2) {
                particles.push(format!(
                    "execute in {dimension} run particle minecraft:dust{{color:[{color}],scale:1.5}} {x:.1} {y:.1} {z:.1} 0 0 0 0 1 force {}",
                    p.name
                ));
            }
        }
    }
    // The sidebars and the Tab list, once a second, sent only where they
    // changed.
    let mut sidebars = Vec::new();
    let mut tab = None;
    if n % 20 == 0
        && let Ok(b) = &mut smp.board
    {
        let on: Vec<u128> = players.iter().map(|p| p.uuid).collect();
        if let Ok(e) = &smp.economy
            && b.due(now, e.revision())
        {
            b.rebuild(e.accounts(), now, e.revision());
        }
        sidebars = b.updates(&on);
        let to = b.tab_due(now, &on);
        if !to.is_empty() {
            let (rate, mspt) = game.native_tick_stats();
            let tps = f64::from(rate).min(1000.0 / mspt.max(0.001));
            tab = Some((to, b.tab_lines(tps, mspt, on.len())));
        }
    }
    let title = smp.board.as_ref().map(|b| b.config.title.clone()).unwrap_or_default();
    let busy = smp.places.as_ref().is_ok_and(|p| !p.pending.is_empty()) || !smp.trades.open.is_empty();
    BUSY.store(busy, Relaxed);
    save(smp);
    drop(guard);
    for line in particles {
        game.native_run_command(&line);
    }
    for (uuid, lines) in sidebars {
        game.native_sidebar(uuid, Some(&sidebar(&title, &lines)));
    }
    if let Some((to, (header, footer))) = tab {
        let (header, footer) = (Text::literal(header).with_color(text::GOLD), Text::literal(footer).with_color(text::GRAY));
        for uuid in to {
            game.native_tab_list(uuid, &header, &footer);
        }
    }
    show(game, out);
}

/// A player's sidebar lines as the game shows them.
fn sidebar(title: &str, lines: &[Line]) -> NativeSidebar {
    let lines = lines
        .iter()
        .map(|l| match l {
            Line::Top { rank, name, amount, me } => {
                let color = if *me { text::GREEN } else { text::WHITE };
                (
                    Text::literal(format!("{rank}. ")).with_color(text::GRAY).append(Text::literal(name.clone()).with_color(color)),
                    Some(Text::literal(amount.clone()).with_color(text::GOLD)),
                )
            }
            Line::Gap => (Text::literal(""), None),
            Line::Me { rank, amount } => (
                Text::literal(if *rank == 0 { "You".to_owned() } else { format!("You: #{rank}") }).with_color(text::GREEN),
                Some(Text::literal(amount.clone()).with_color(text::GOLD)),
            ),
        })
        .collect();
    NativeSidebar {
        title: Text::literal(title).with_color(text::GOLD).bold(),
        lines,
    }
}

/// The world closed: its files are written and its state let go.
pub fn closed() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(smp) = state.as_mut() {
        save(smp);
    }
    *state = None;
}
