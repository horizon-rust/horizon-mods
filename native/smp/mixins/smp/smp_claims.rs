// Land claims: a player claims the chunk they stand in (`/claim`), all its
// height, and only they and the players they trust (`/trust`) break blocks
// or right click blocks there (doors, chests, levers, placing, buckets on a
// block). The chunks a player may hold grow with the time they played here.
// Operators of level 3 pass everywhere and may `/unclaim` any chunk. Kept in
// `claims.json`, written atomically. No game types. Added to the game as
// `game::smp_claims`; also a module of this crate, for its tests.

use super::smp_economy::{Out, Player};
use super::smp_places::Place;
use super::smp_store;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// `config.json`'s `claims` section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Chunks a player may claim, by hours played here: `[hours, chunks]`,
    /// the last line reached counts.
    pub chunks: Vec<[i64; 2]>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            chunks: vec![[0, 4], [5, 16], [20, 36], [100, 64]],
        }
    }
}

/// `claims.json`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    /// Claimed chunks, `<dimension> <chunk x> <chunk z>`, to their owner's UUID.
    chunks: BTreeMap<String, String>,
    /// The players each owner trusts, by UUID.
    trust: BTreeMap<String, BTreeSet<String>>,
}

pub struct Claims {
    pub config: Config,
    path: PathBuf,
    data: Data,
    changed: bool,
    /// `data.chunks` by (dimension, chunk x, chunk z), for each block event.
    owners: HashMap<(String, i32, i32), u128>,
    /// When each player was last told a chunk isn't theirs.
    told: HashMap<u128, i64>,
}

fn key(dimension: &str, cx: i32, cz: i32) -> String {
    format!("{dimension} {cx} {cz}")
}

fn chunk_of(at: &Place) -> (i32, i32) {
    ((at.x.floor() as i32) >> 4, (at.z.floor() as i32) >> 4)
}

impl Claims {
    /// `claims.json` in `dir`; an error when it exists but can't be read
    /// (never replaced by an empty one).
    pub fn open(dir: &Path, config: Config) -> Result<Self, String> {
        let path = dir.join("claims.json");
        let data: Data = smp_store::read_json(&path)?;
        let mut owners = HashMap::new();
        for (k, owner) in &data.chunks {
            let mut w = k.split(' ');
            let (Some(d), Some(x), Some(z), Some(u)) = (
                w.next(),
                w.next().and_then(|x| x.parse().ok()),
                w.next().and_then(|z| z.parse().ok()),
                smp_store::parse_uuid(owner),
            ) else {
                return Err(format!("{}: a bad claim `{k}`", path.display()));
            };
            owners.insert((d.to_owned(), x, z), u);
        }
        Ok(Self {
            config,
            path,
            data,
            changed: false,
            owners,
            told: HashMap::new(),
        })
    }

    pub fn save(&mut self) -> Result<(), String> {
        if self.changed {
            smp_store::write_json(&self.path, &self.data)?;
            self.changed = false;
        }
        Ok(())
    }

    /// Who holds the chunk of this block, if anyone.
    pub fn owner(&self, dimension: &str, x: i32, z: i32) -> Option<u128> {
        self.owners
            .get(&(dimension.to_owned(), x >> 4, z >> 4))
            .copied()
    }

    fn trusts(&self, owner: u128, who: u128) -> bool {
        self.data
            .trust
            .get(&smp_store::uuid_string(owner))
            .is_some_and(|t| t.contains(&smp_store::uuid_string(who)))
    }

    /// Whether `who` may break or use the block at `pos`. When not, the line
    /// to tell them: at most one each 2 s, so holding a click doesn't flood
    /// their chat.
    pub fn check(
        &mut self,
        who: &Player,
        dimension: &str,
        pos: [i32; 3],
        name_of: &dyn Fn(u128) -> String,
        now: i64,
    ) -> Result<(), Option<String>> {
        if !self.config.enabled || who.level >= 3 {
            return Ok(());
        }
        let Some(owner) = self.owner(dimension, pos[0], pos[2]) else {
            return Ok(());
        };
        if owner == who.uuid || self.trusts(owner, who.uuid) {
            return Ok(());
        }
        let last = self.told.insert(who.uuid, now);
        if last.is_some_and(|t| now - t < 2_000) {
            return Err(None);
        }
        Err(Some(format!("This land is claimed by {}.", name_of(owner))))
    }

    /// How many chunks `uuid` holds.
    fn held(&self, uuid: u128) -> usize {
        self.owners.values().filter(|&&u| u == uuid).count()
    }

    /// How many chunks a player may hold after `played` seconds here.
    pub fn limit(&self, played: i64) -> usize {
        self.config
            .chunks
            .iter()
            .filter(|[h, _]| played / 3600 >= *h)
            .map(|[_, n]| (*n).max(0) as usize)
            .next_back()
            .unwrap_or(0)
    }

    /// The claims' commands; None for another command. `at` is where the
    /// player stands; `find` a player's UUID and name from a name (on or
    /// known); `name_of` a name from a UUID.
    pub fn command(
        &mut self,
        me: &Player,
        at: &Place,
        played: i64,
        find: &dyn Fn(&str) -> Option<(u128, String)>,
        name_of: &dyn Fn(u128) -> String,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let command = *words.first()?;
        if !COMMANDS.contains(&command) {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error("Claims are off on this server.".into())]);
        }
        let (cx, cz) = chunk_of(at);
        let here = (at.dimension.clone(), cx, cz);
        let blocks = format!(
            "x {} to {}, z {} to {}",
            cx * 16,
            cx * 16 + 15,
            cz * 16,
            cz * 16 + 15
        );
        let me_key = smp_store::uuid_string(me.uuid);
        Some(match (command, &words[1..]) {
            ("claim", []) => match self.owners.get(&here).copied() {
                Some(u) if u == me.uuid => vec![Out::Error("This chunk is already yours.".into())],
                Some(u) => vec![Out::Error(format!("This chunk is {}'s.", name_of(u)))],
                None => {
                    let (held, limit) = (self.held(me.uuid), self.limit(played));
                    if held >= limit {
                        vec![Out::Error(format!(
                            "You hold {held} of {limit} chunks: play longer for more, or /unclaim one."
                        ))]
                    } else {
                        self.data.chunks.insert(key(&here.0, cx, cz), me_key);
                        self.owners.insert(here, me.uuid);
                        self.changed = true;
                        vec![Out::Reply(format!(
                            "Claimed this chunk ({blocks}), {} of {limit}. /trust <player> lets a friend build.",
                            held + 1
                        ))]
                    }
                }
            },
            ("claim", ["info"]) => vec![Out::Reply(match self.owners.get(&here) {
                Some(&u) => format!("This chunk ({blocks}) is {}'s.", name_of(u)),
                None => format!("This chunk ({blocks}) is free: /claim it."),
            })],
            ("unclaim", []) => match self.owners.get(&here).copied() {
                Some(u) if u == me.uuid || me.level >= 3 => {
                    self.owners.remove(&here);
                    self.data.chunks.remove(&key(&here.0, cx, cz));
                    self.changed = true;
                    vec![Out::Reply(format!("Unclaimed this chunk ({blocks})."))]
                }
                Some(u) => vec![Out::Error(format!("This chunk is {}'s.", name_of(u)))],
                None => vec![Out::Error("This chunk isn't claimed.".into())],
            },
            ("claims", []) => {
                let mine: Vec<String> = self
                    .data
                    .chunks
                    .iter()
                    .filter(|(_, o)| **o == me_key)
                    .map(|(k, _)| {
                        let w: Vec<&str> = k.split(' ').collect();
                        let n = |s: &str| s.parse::<i32>().unwrap_or(0) * 16;
                        format!(
                            "{} at x {} z {}",
                            w[0].trim_start_matches("minecraft:"),
                            n(w[1]),
                            n(w[2])
                        )
                    })
                    .collect();
                let trusted: Vec<String> = self
                    .data
                    .trust
                    .get(&me_key)
                    .into_iter()
                    .flatten()
                    .filter_map(|u| smp_store::parse_uuid(u))
                    .map(name_of)
                    .collect();
                vec![
                    Out::Reply(format!(
                        "Your chunks ({} of {}): {}",
                        mine.len(),
                        self.limit(played),
                        if mine.is_empty() {
                            "none, /claim where you stand".into()
                        } else {
                            mine.join("; ")
                        }
                    )),
                    Out::Reply(format!(
                        "Trusted: {}",
                        if trusted.is_empty() {
                            "nobody".into()
                        } else {
                            trusted.join(", ")
                        }
                    )),
                ]
            }
            ("trust", [name]) | ("untrust", [name]) => match find(name) {
                None => vec![Out::Error(format!(
                    "No player named {name} has played here."
                ))],
                Some((u, _)) if u == me.uuid => vec![Out::Error("That's you.".into())],
                Some((u, name)) => {
                    let set = self.data.trust.entry(me_key.clone()).or_default();
                    let changed = if command == "trust" {
                        set.insert(smp_store::uuid_string(u))
                    } else {
                        set.remove(&smp_store::uuid_string(u))
                    };
                    if set.is_empty() {
                        self.data.trust.remove(&me_key);
                    }
                    self.changed |= changed;
                    vec![Out::Reply(match (command, changed) {
                        ("trust", true) => {
                            format!("{name} may now build and open things on your land.")
                        }
                        ("trust", false) => format!("You already trust {name}."),
                        (_, true) => format!("{name} no longer may build on your land."),
                        _ => format!("You didn't trust {name}."),
                    })]
                }
            },
            _ => vec![Out::Error(
                "Usage: /claim, /claim info, /unclaim, /claims, /trust <player>, /untrust <player>"
                    .into(),
            )],
        })
    }
}

/// The claims' commands.
pub const COMMANDS: [&str; 5] = ["claim", "unclaim", "claims", "trust", "untrust"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const A: u128 = 1;
    const B: u128 = 2;

    fn dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("smp-claims-{}-{n}", std::process::id()))
    }
    fn at(x: f64, z: f64) -> Place {
        Place {
            dimension: "minecraft:overworld".into(),
            x,
            y: 64.0,
            z,
        }
    }
    fn player(uuid: u128, level: u8) -> Player {
        Player {
            uuid,
            name: if uuid == A { "Alex" } else { "Steve" }.into(),
            level,
        }
    }
    fn find(n: &str) -> Option<(u128, String)> {
        match n.to_ascii_lowercase().as_str() {
            "alex" => Some((A, "Alex".into())),
            "steve" => Some((B, "Steve".into())),
            _ => None,
        }
    }
    fn name_of(u: u128) -> String {
        if u == A { "Alex" } else { "Steve" }.into()
    }

    #[test]
    fn a_claim_keeps_others_out_until_trusted() {
        let d = dir();
        let mut c = Claims::open(&d, Config::default()).unwrap();
        let out = c
            .command(&player(A, 0), &at(-3.5, 20.0), 0, &find, &name_of, "claim")
            .unwrap();
        assert!(
            matches!(&out[0], Out::Reply(s) if s.contains("x -16 to -1, z 16 to 31")),
            "{out:?}"
        );
        let ow = "minecraft:overworld";
        assert_eq!(
            c.check(&player(A, 0), ow, [-16, 0, 31], &name_of, 0),
            Ok(())
        );
        assert_eq!(
            c.check(&player(B, 0), ow, [-1, 100, 16], &name_of, 0),
            Err(Some("This land is claimed by Alex.".into()))
        );
        // Told once each 2 s; the next chunk and the Nether are free.
        assert_eq!(
            c.check(&player(B, 0), ow, [-1, 100, 16], &name_of, 1_000),
            Err(None)
        );
        assert_eq!(c.check(&player(B, 0), ow, [0, 64, 16], &name_of, 0), Ok(()));
        assert_eq!(
            c.check(
                &player(B, 0),
                "minecraft:the_nether",
                [-1, 64, 16],
                &name_of,
                0
            ),
            Ok(())
        );
        // Operators pass.
        assert_eq!(
            c.check(&player(B, 3), ow, [-1, 64, 16], &name_of, 0),
            Ok(())
        );
        c.command(
            &player(A, 0),
            &at(0.0, 0.0),
            0,
            &find,
            &name_of,
            "trust steve",
        )
        .unwrap();
        assert_eq!(
            c.check(&player(B, 0), ow, [-1, 64, 16], &name_of, 9_000),
            Ok(())
        );
        let out = c
            .command(
                &player(B, 0),
                &at(-3.5, 20.0),
                0,
                &find,
                &name_of,
                "unclaim",
            )
            .unwrap();
        assert!(
            matches!(&out[0], Out::Error(s) if s.contains("Alex's")),
            "trust isn't ownership"
        );
        c.save().unwrap();
        let mut again = Claims::open(&d, Config::default()).unwrap();
        assert_eq!(again.owner(ow, -1, 16), Some(A));
        again
            .command(
                &player(A, 0),
                &at(0.0, 0.0),
                0,
                &find,
                &name_of,
                "untrust Steve",
            )
            .unwrap();
        assert!(
            again
                .check(&player(B, 0), ow, [-1, 64, 16], &name_of, 20_000)
                .is_err()
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn chunks_grow_with_time_played() {
        let d = dir();
        let mut c = Claims::open(&d, Config::default()).unwrap();
        for x in 0..4 {
            let out = c
                .command(
                    &player(A, 0),
                    &at(x as f64 * 16.0, 0.0),
                    0,
                    &find,
                    &name_of,
                    "claim",
                )
                .unwrap();
            assert!(matches!(&out[0], Out::Reply(_)), "{out:?}");
        }
        let out = c
            .command(&player(A, 0), &at(64.0, 0.0), 0, &find, &name_of, "claim")
            .unwrap();
        assert!(
            matches!(&out[0], Out::Error(s) if s.contains("4 of 4")),
            "{out:?}"
        );
        let out = c
            .command(
                &player(A, 0),
                &at(64.0, 0.0),
                5 * 3600,
                &find,
                &name_of,
                "claim",
            )
            .unwrap();
        assert!(
            matches!(&out[0], Out::Reply(s) if s.contains("5 of 16")),
            "{out:?}"
        );
        let out = c
            .command(&player(B, 0), &at(0.0, 0.0), 0, &find, &name_of, "claim")
            .unwrap();
        assert_eq!(out, vec![Out::Error("This chunk is Alex's.".into())]);
        // An operator may free any chunk.
        let out = c
            .command(&player(B, 3), &at(0.0, 0.0), 0, &find, &name_of, "unclaim")
            .unwrap();
        assert!(matches!(&out[0], Out::Reply(_)));
        let out = c
            .command(&player(A, 0), &at(0.0, 0.0), 0, &find, &name_of, "claims")
            .unwrap();
        assert!(
            matches!(&out[0], Out::Reply(s) if s.starts_with("Your chunks (4 of 4)")),
            "{out:?}"
        );
        let _ = std::fs::remove_dir_all(d);
    }
}
