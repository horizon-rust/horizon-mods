// The SMP in the server's game: its state for the open world, and the edges
// between the game's types and the rules (smp_economy, smp_market). Added to
// the game as `game::smp_glue`; the helpers (register.rs, command.rs,
// tick.rs, lifecycle.rs) call it from the hook points.

use crate::game::{
    Game,
    native_server::{NativeCommand, NativeServerPlayer},
    smp_economy::{self, Economy, Out, Player},
    smp_market::{self, Bag, Market},
    smp_store,
};
use horizon_client::text::{self, Text};
use std::{path::PathBuf, sync::Mutex};

/// The mod's id, name and version.
pub const ID: &str = "smp";
pub const NAME: &str = "SMP";
pub const VERSION: &str = "0.2.0";

/// `config.json` in `world/horizon-smp/`, written with its defaults the
/// first time, and again with the keys a newer version adds (values kept).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub economy: smp_economy::Config,
    pub market: smp_market::Config,
}

/// The SMP of the open world.
pub struct Smp {
    dir: PathBuf,
    /// An error when `economy.json` can't be read: the commands say so.
    economy: Result<Economy, String>,
    market: Market,
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
    Smp {
        dir,
        economy,
        market: Market {
            config: config.market,
        },
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
    ] {
        commands.push(NativeCommand::new(usage));
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

const COMMANDS: [&str; 8] = [
    "balance", "bal", "pay", "baltop", "eco", "worth", "sell", "buy",
];

/// A player's command.
pub(in crate::game) fn perform(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    if !game.native_on(ID) || !COMMANDS.contains(&line.split(' ').next().unwrap_or_default()) {
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
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    let out = match &mut smp.economy {
        Ok(e) => {
            e.seen(me.uuid, &me.name);
            let mut bag = GameBag(game);
            e.command(&me, line)
                .or_else(|| smp.market.command(e, &me, trades, &mut bag, line))
                .unwrap_or_default()
        }
        Err(_) => vec![Out::Error(
            "The economy is off: its file can't be read (see the server log).".into(),
        )],
    };
    drop(guard);
    *ran = true;
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
    let players = if n % 20 == 0 {
        game.native_server_players()
    } else {
        Vec::new()
    };
    let mut guard = state(dir.join(smp_store::DIR));
    let smp = guard.as_mut().expect("opened");
    if let Ok(e) = &mut smp.economy {
        for p in &players {
            e.seen(p.uuid, &p.name);
        }
    }
    save(smp);
}

/// The world closed: its files are written and its state let go.
pub fn closed() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(smp) = state.as_mut() {
        save(smp);
    }
    *state = None;
}
