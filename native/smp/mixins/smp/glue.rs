// The SMP in the server's game: its state for the open world, and the edges
// between the game's types and the rules (smp_economy). Added to the game as
// `game::smp_glue`; the helpers (register.rs, command.rs, tick.rs,
// lifecycle.rs) call it from the hook points.

use crate::game::{
    Game,
    native_server::{NativeCommand, NativeServerPlayer},
    smp_economy::{self, Economy, Out, Player},
    smp_store,
};
use horizon_client::text::{self, Text};
use std::{path::PathBuf, sync::Mutex};

/// The mod's id, name and version.
pub const ID: &str = "smp";
pub const NAME: &str = "SMP";
pub const VERSION: &str = "0.1.0";

/// `config.json` in `world/horizon-smp/`, written with its defaults the
/// first time.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub economy: smp_economy::Config,
}

/// The SMP of the open world.
pub struct Smp {
    dir: PathBuf,
    /// An error when `economy.json` can't be read: the commands say so.
    economy: Result<Economy, String>,
}

static STATE: Mutex<Option<Smp>> = Mutex::new(None);
static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn open(dir: PathBuf) -> Smp {
    let config_path = dir.join("config.json");
    let config: Config = match smp_store::read_json(&config_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[smp] {e}: the defaults are used");
            Config::default()
        }
    };
    if !config_path.exists()
        && let Err(e) = smp_store::write_json(&config_path, &config)
    {
        log::error!("[smp] {e}");
    }
    log::info!("[smp] {NAME} {VERSION} on {}", dir.display());
    let economy = Economy::open(&dir, config.economy);
    if let Err(e) = &economy {
        log::error!("[smp] the economy is off: {e}");
    }
    Smp { dir, economy }
}

/// Runs `f` on the SMP of the game's open world (opened on first use).
fn with<R>(game: &Game<'_>, f: impl FnOnce(&mut Smp) -> R) -> Option<R> {
    let dir = game.native_world_dir()?.join(smp_store::DIR);
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.as_ref().is_none_or(|s| s.dir != dir) {
        if let Some(old) = state.as_mut() {
            save(old);
        }
        *state = Some(open(dir));
    }
    let smp = state.as_mut().expect("opened above");
    if let Ok(e) = &mut smp.economy {
        e.now = chrono::Utc::now().timestamp_millis();
    }
    Some(f(smp))
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

/// A player's command.
pub(in crate::game) fn perform(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    if !game.native_on(ID) {
        return;
    }
    let me = Player {
        uuid: player.uuid,
        name: player.name.clone(),
        level: player.permission,
    };
    let Some(out) = with(game, |smp| match &mut smp.economy {
        Ok(e) => {
            e.seen(me.uuid, &me.name);
            e.command(&me, line)
        }
        Err(_) => matches!(
            line.split(' ').next(),
            Some("balance" | "bal" | "pay" | "baltop" | "eco")
        )
        .then(|| {
            vec![Out::Error(
                "The economy is off: its file can't be read (see the server log).".into(),
            )]
        }),
    })
    .flatten() else {
        return;
    };
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
    let n = TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let players = if n % 20 == 0 {
        game.native_server_players()
    } else {
        Vec::new()
    };
    with(game, |smp| {
        if let Ok(e) = &mut smp.economy {
            for p in &players {
                e.seen(p.uuid, &p.name);
            }
        }
        save(smp);
    });
}

/// The world closed: its files are written and its state let go.
pub fn closed() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(smp) = state.as_mut() {
        save(smp);
    }
    *state = None;
}
