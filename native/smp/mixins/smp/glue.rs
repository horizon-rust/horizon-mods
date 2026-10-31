// The SMP in the server's game: its state for the open world, and the edges
// between the game's types and the rules (smp_economy, smp_market,
// smp_trade, smp_games, smp_lottery, smp_places). Added to
// the game as `game::smp_glue`; the helpers (register.rs, command.rs,
// tick.rs, lifecycle.rs, menu_clicked.rs, menu_closed.rs) call it from the
// hook points.

use crate::game::{
    Game,
    native_server::{NativeCommand, NativeServerPlayer},
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
pub const VERSION: &str = "0.6.0";

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
}

static STATE: Mutex<Option<Smp>> = Mutex::new(None);
static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
    if !game.native_on(ID) || !(COMMANDS.contains(&word) || smp_places::COMMANDS.contains(&word)) {
        return;
    }
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
    let out = if smp_places::COMMANDS.contains(&word) {
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
    let Some(dir) = game.native_world_dir() else {
        return;
    };
    let n = TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    save(smp);
    drop(guard);
    show(game, out);
}

/// The world closed: its files are written and its state let go.
pub fn closed() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(smp) = state.as_mut() {
        save(smp);
    }
    *state = None;
}
