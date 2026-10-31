// Nicknames: `/nick <name>` shows another name for you in chat and the Tab
// list (hovering it gives your real one), `/nick off` your name again;
// operators of level 3 `/nick <player> <name|off>`; anyone `/realname
// <nick>`. A nickname is 3 to 16 letters, digits or `_`, never the real name
// of anyone who has played here (or of an operator), nor another player's
// nickname, and must pass the server's chat filter; one change every 10
// minutes. The real name stays everywhere else (the log, the anticheat,
// commands, the Discord bridge). Kept in `nicks.json`. A change is decided
// here and made by the glue at the next second, outside the player's
// command, where the chat filter and the Tab list are in reach. No game
// types. Added to the game as `game::smp_nick`; also a module of this crate,
// for its tests.

use super::smp_economy::{Out, Player};
use super::smp_store::{parse_uuid, read_json, uuid_string, write_json};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const COMMANDS: [&str; 2] = ["nick", "realname"];

/// `config.json`'s `nicks`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Between two changes of one's own nickname.
    pub cooldown_seconds: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_seconds: 600,
        }
    }
}

/// One player's nickname, and when they last changed it (ms).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Nick {
    pub nick: Option<String>,
    pub changed: i64,
}

/// `nicks.json`, by UUID.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct File {
    pub nicks: BTreeMap<String, Nick>,
}

/// A change to make: whose nickname, the new one (None: off), who asked
/// (told the outcome), and whether it starts the cooldown.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub uuid: u128,
    pub nick: Option<String>,
    pub by: u128,
    pub own: bool,
}

/// The names a nickname may not copy: real names of anyone who has played
/// here and of the operators, lowercased.
pub trait Names {
    fn taken(&self, lower: &str) -> bool;
    /// A player who has played here by name (any case): UUID and name.
    fn find(&self, name: &str) -> Option<(u128, String)>;
    /// A player's real name.
    fn name_of(&self, uuid: u128) -> Option<String>;
}

pub struct Nicks {
    pub config: Config,
    file: File,
    path: PathBuf,
    dirty: bool,
    /// Changes waiting for the next second.
    pub pending: Vec<Change>,
}

/// 3 to 16 of `A-Z a-z 0-9 _`.
pub fn valid(nick: &str) -> bool {
    (3..=16).contains(&nick.len()) && nick.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Nicks {
    /// The nicknames kept in `dir` (`world/horizon-smp/`); an error when
    /// `nicks.json` can't be read (nicknames are off then).
    pub fn open(dir: &std::path::Path, config: Config) -> Result<Self, String> {
        let path = dir.join("nicks.json");
        Ok(Self {
            config,
            file: read_json(&path)?,
            path,
            dirty: false,
            pending: Vec::new(),
        })
    }

    pub fn save(&mut self) -> Result<(), String> {
        if self.dirty {
            write_json(&self.path, &self.file)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Every nickname set, by UUID.
    pub fn all(&self) -> Vec<(u128, String)> {
        self.file
            .nicks
            .iter()
            .filter_map(|(k, n)| Some((parse_uuid(k)?, n.nick.clone()?)))
            .collect()
    }

    pub fn nick_of(&self, uuid: u128) -> Option<&str> {
        self.file.nicks.get(&uuid_string(uuid))?.nick.as_deref()
    }

    /// Whose nickname `nick` is (any case).
    fn owner(&self, nick: &str) -> Option<u128> {
        self.all()
            .into_iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(nick))
            .map(|(u, _)| u)
    }

    /// Runs `/nick` or `/realname`; None for another command.
    pub fn command(
        &mut self,
        me: &Player,
        names: &dyn Names,
        now: i64,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (&name, args) = words.split_first()?;
        if !COMMANDS.contains(&name) {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error("Nicknames are off on this server.".into())]);
        }
        Some(match (name, args) {
            ("realname", [nick]) => match self
                .owner(nick)
                .and_then(|u| Some((u, self.nick_of(u)?.to_owned())))
            {
                Some((u, n)) => {
                    let real = names.name_of(u).unwrap_or_else(|| uuid_string(u));
                    vec![Out::Reply(format!("{n} is {real}."))]
                }
                None => vec![Out::Error(format!("Nobody goes by {nick}."))],
            },
            ("realname", _) => vec![Out::Error("Usage: /realname <nickname>".into())],
            ("nick", [new]) => self.ask(me, me.uuid, &me.name, new, names, now, true),
            ("nick", [who, new]) if me.level >= 3 => match names.find(who) {
                Some((uuid, real)) => self.ask(me, uuid, &real, new, names, now, false),
                None => vec![Out::Error(format!(
                    "No player named {who} has played here."
                ))],
            },
            ("nick", [_, _]) => vec![Out::Error(
                "Only operators of level 3 may set another player's nickname.".into(),
            )],
            _ => vec![Out::Error("Usage: /nick <name>, /nick off".into())],
        })
    }

    /// A change for `uuid` (real name `real`), checked; queued when right.
    #[allow(clippy::too_many_arguments)]
    fn ask(
        &mut self,
        me: &Player,
        uuid: u128,
        real: &str,
        new: &str,
        names: &dyn Names,
        now: i64,
        own: bool,
    ) -> Vec<Out> {
        let current = self.nick_of(uuid);
        let off = new.eq_ignore_ascii_case("off") || new == real;
        if own && let Some(n) = self.file.nicks.get(&uuid_string(uuid)) {
            let wait = n.changed + self.config.cooldown_seconds * 1000 - now;
            if wait > 0 && me.level < 3 {
                return vec![Out::Error(format!(
                    "You changed your nickname recently: wait {} more minute(s).",
                    (wait + 59_999) / 60_000
                ))];
            }
        }
        if off {
            if current.is_none() {
                return vec![Out::Error("No nickname to take off.".into())];
            }
        } else {
            if !valid(new) {
                return vec![Out::Error(
                    "A nickname is 3 to 16 letters, digits or _.".into(),
                )];
            }
            let lower = new.to_ascii_lowercase();
            if self.owner(new).is_some_and(|o| o != uuid) {
                return vec![Out::Error(format!("{new} is someone else's nickname."))];
            }
            if names.taken(&lower) && !lower.eq_ignore_ascii_case(real) {
                return vec![Out::Error(format!("{new} is a player's name."))];
            }
            if current.is_some_and(|c| c == new) {
                return vec![Out::Error(format!(
                    "That is already the nickname of {real}."
                ))];
            }
        }
        self.pending.retain(|c| c.uuid != uuid);
        self.pending.push(Change {
            uuid,
            nick: (!off).then(|| new.to_owned()),
            by: me.uuid,
            own,
        });
        Vec::new()
    }

    /// A pending change the chat filter let through: kept. Lines for the
    /// one who asked and the player.
    pub fn apply(&mut self, change: &Change, real: &str, now: i64) -> Vec<Out> {
        let key = uuid_string(change.uuid);
        let entry = self.file.nicks.entry(key.clone()).or_default();
        entry.nick = change.nick.clone();
        if change.own {
            entry.changed = now;
        }
        if entry.nick.is_none() && entry.changed == 0 {
            self.file.nicks.remove(&key);
        }
        self.dirty = true;
        let line = match &change.nick {
            Some(n) => format!("{real} now goes by {n}."),
            None => format!("{real} goes by their name again."),
        };
        let mut out = vec![Out::Tell(change.by, line.clone())];
        if change.by != change.uuid {
            out.push(Out::Tell(change.uuid, line));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;

    struct Known(Vec<(u128, &'static str)>);
    impl Names for Known {
        fn taken(&self, lower: &str) -> bool {
            self.0.iter().any(|(_, n)| n.eq_ignore_ascii_case(lower))
        }
        fn find(&self, name: &str) -> Option<(u128, String)> {
            self.0
                .iter()
                .find(|(_, n)| n.eq_ignore_ascii_case(name))
                .map(|(u, n)| (*u, (*n).to_owned()))
        }
        fn name_of(&self, uuid: u128) -> Option<String> {
            self.0
                .iter()
                .find(|(u, _)| *u == uuid)
                .map(|(_, n)| (*n).to_owned())
        }
    }

    fn setup(name: &str) -> (Nicks, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-nick-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Nicks::open(&dir, Config::default()).unwrap(), dir)
    }

    fn player(uuid: u128, name: &str, level: u8) -> Player {
        Player {
            uuid,
            name: name.into(),
            level,
        }
    }

    #[test]
    fn a_nickname_is_checked_queued_then_kept() {
        let (mut n, dir) = setup("set");
        let known = Known(vec![(ALEX, "Alex"), (STEVE, "Steve"), (3, "luzW")]);
        let alex = player(ALEX, "Alex", 0);
        for (bad, why) in [
            ("ab", "A nickname is 3 to 16 letters, digits or _."),
            (
                "way_too_long_nickname",
                "A nickname is 3 to 16 letters, digits or _.",
            ),
            ("bad-name", "A nickname is 3 to 16 letters, digits or _."),
            ("steve", "steve is a player's name."),
            ("LUZW", "LUZW is a player's name."),
        ] {
            assert_eq!(
                n.command(&alex, &known, 0, &format!("nick {bad}")).unwrap(),
                [Out::Error(why.into())]
            );
        }
        assert_eq!(n.command(&alex, &known, 0, "nick Sunny").unwrap(), []);
        assert_eq!(
            n.pending,
            [Change {
                uuid: ALEX,
                nick: Some("Sunny".into()),
                by: ALEX,
                own: true
            }]
        );
        let c = n.pending.remove(0);
        assert_eq!(
            n.apply(&c, "Alex", 1000),
            [Out::Tell(ALEX, "Alex now goes by Sunny.".into())]
        );
        assert_eq!(n.nick_of(ALEX), Some("Sunny"));
        // Taken by Alex now.
        let steve = player(STEVE, "Steve", 0);
        assert_eq!(
            n.command(&steve, &known, 2000, "nick sunny").unwrap(),
            [Out::Error("sunny is someone else's nickname.".into())]
        );
        // Ten minutes between one's own changes.
        assert_eq!(
            n.command(&alex, &known, 60_000, "nick off").unwrap(),
            [Out::Error(
                "You changed your nickname recently: wait 10 more minute(s).".into()
            )]
        );
        assert_eq!(n.command(&alex, &known, 601_000, "nick off").unwrap(), []);
        n.save().unwrap();
        let mut again = Nicks::open(&dir, Config::default()).unwrap();
        assert_eq!(
            again.nick_of(ALEX),
            Some("Sunny"),
            "kept until the change is applied"
        );
        assert_eq!(
            again.command(&steve, &known, 0, "realname Sunny").unwrap(),
            [Out::Reply("Sunny is Alex.".into())]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn operators_set_others_nicknames_without_a_cooldown() {
        let (mut n, dir) = setup("op");
        let known = Known(vec![(ALEX, "Alex"), (STEVE, "Steve")]);
        let op = player(ALEX, "Alex", 3);
        assert_eq!(
            n.command(&player(STEVE, "Steve", 0), &known, 0, "nick Alex Bob")
                .unwrap(),
            [Out::Error(
                "Only operators of level 3 may set another player's nickname.".into()
            )]
        );
        assert_eq!(n.command(&op, &known, 0, "nick Steve Rocky").unwrap(), []);
        let c = n.pending.remove(0);
        assert_eq!(
            n.apply(&c, "Steve", 5),
            [
                Out::Tell(ALEX, "Steve now goes by Rocky.".into()),
                Out::Tell(STEVE, "Steve now goes by Rocky.".into())
            ]
        );
        assert_eq!(n.command(&op, &known, 10, "nick Steve off").unwrap(), []);
        assert_eq!(
            n.command(&op, &known, 10, "nick Nobody x").unwrap(),
            [Out::Error("No player named Nobody has played here.".into())]
        );
        assert_eq!(n.command(&op, &known, 10, "balance"), None);
        fs::remove_dir_all(dir).ok();
    }
}
