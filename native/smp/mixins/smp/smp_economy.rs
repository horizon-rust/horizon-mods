// The SMP's economy: one currency, a balance per player UUID
// (`economy.json`), and the commands `/balance`, `/pay`, `/baltop` and the
// operators' `/eco`. Every change is written to the audit log first, then
// made; a change the log refuses is not made. No game types: the glue
// (glue.rs) hands in the player and the line, and sends back what this
// answers. Added to the game as `game::smp_economy`; also a module of this
// crate, for its tests.

use super::smp_store::{Audit, read_json, uuid_string, write_json};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// Amounts above this are refused: far beyond any economy, far below overflow.
pub const MAX_AMOUNT: i64 = 1_000_000_000_000;
/// Lines of `/baltop` a page.
pub const PAGE: usize = 10;

/// `config.json`'s economy settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Shown after amounts: `1,250 coins`.
    pub currency: String,
    /// What a new account holds.
    pub starting_balance: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            currency: "coins".into(),
            starting_balance: 0,
        }
    }
}

/// One player's account.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// The name last seen.
    pub name: String,
    pub balance: i64,
}

/// `economy.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Accounts {
    /// By UUID, as `uuid_string` writes it.
    pub accounts: BTreeMap<String, Account>,
    /// Money held for a game being decided (a coinflip's stake), by UUID:
    /// in the same file as the balances, so a crash can't lose it or count
    /// it twice. Given back when the economy opens (a restart voids games).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub held: BTreeMap<String, i64>,
}

/// A player as a command sees them.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub uuid: u128,
    pub name: String,
    /// Operator level, 0 to 4.
    pub level: u8,
}

/// What a command answers: its feedback, red when an error, and lines to
/// other players.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    Reply(String),
    Error(String),
    Tell(u128, String),
    /// A line to every player (and the server's log).
    All(String),
}

pub struct Economy {
    pub config: Config,
    accounts: Accounts,
    path: PathBuf,
    audit: Audit,
    dirty: bool,
    /// Counts the changes to the accounts, so the sidebar knows when to
    /// look again.
    revision: u64,
    /// Milliseconds since 1970, for the log (the glue sets it each call).
    pub now: i64,
}

impl Economy {
    /// The economy kept in `dir` (`world/horizon-smp/`); an error when
    /// `economy.json` exists and can't be read: the economy then stays off
    /// rather than start from nothing.
    pub fn open(dir: &std::path::Path, config: Config) -> Result<Self, String> {
        let path = dir.join("economy.json");
        let accounts = read_json(&path)?;
        let mut economy = Self {
            config,
            accounts,
            path,
            audit: Audit::new(dir.join("audit.log")),
            dirty: false,
            revision: 0,
            now: 0,
        };
        // Games the last run left undecided: their stakes go back.
        let held: Vec<(String, i64)> = economy.accounts.held.clone().into_iter().collect();
        for (key, amount) in held {
            if let Some(uuid) = super::smp_store::parse_uuid(&key) {
                economy.release("held_returned", uuid, amount)?;
            }
        }
        Ok(economy)
    }

    fn touch(&mut self) {
        self.dirty = true;
        self.revision += 1;
    }

    /// Changes to the accounts so far (in this run).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Every account: UUID, name, balance.
    pub fn accounts(&self) -> Vec<(u128, String, i64)> {
        self.accounts
            .accounts
            .iter()
            .filter_map(|(k, a)| {
                Some((super::smp_store::parse_uuid(k)?, a.name.clone(), a.balance))
            })
            .collect()
    }

    /// Money `uuid` has held for a game.
    pub fn held(&self, uuid: u128) -> i64 {
        self.accounts
            .held
            .get(&uuid_string(uuid))
            .copied()
            .unwrap_or(0)
    }

    /// Moves `amount` of `uuid`'s balance to held, logged as `kind`.
    pub fn hold(&mut self, kind: &str, uuid: u128, amount: i64) -> Result<(), String> {
        let key = uuid_string(uuid);
        let balance = self.balance(uuid);
        if amount <= 0 || amount > MAX_AMOUNT {
            return Err("The amount must be a positive whole number.".into());
        }
        if !self.accounts.accounts.contains_key(&key) {
            return Err("No such account.".into());
        }
        if balance < amount {
            return Err(format!("You only have {}.", self.money(balance)));
        }
        let entry = self.entry(kind, Some(uuid), None, amount, "");
        self.audit.append(&entry)?;
        self.accounts
            .accounts
            .get_mut(&key)
            .expect("checked")
            .balance -= amount;
        *self.accounts.held.entry(key).or_default() += amount;
        self.touch();
        Ok(())
    }

    /// Gives back up to `amount` of what `uuid` has held, logged as `kind`.
    pub fn release(&mut self, kind: &str, uuid: u128, amount: i64) -> Result<(), String> {
        let key = uuid_string(uuid);
        let amount = amount.min(self.held(uuid));
        if amount <= 0 {
            return Ok(());
        }
        let entry = self.entry(kind, None, Some(uuid), amount, "");
        self.audit.append(&entry)?;
        self.take_held(&key, amount);
        let account = self.accounts.accounts.entry(key).or_default();
        account.balance = account.balance.saturating_add(amount);
        self.touch();
        Ok(())
    }

    /// A game decided: each stake leaves held, and `winner` gets `prize`
    /// (at most the stakes together), logged as `kind`.
    pub fn award(
        &mut self,
        kind: &str,
        stakes: &[(u128, i64)],
        winner: u128,
        prize: i64,
    ) -> Result<(), String> {
        let total: i64 = stakes.iter().map(|(_, a)| *a).sum();
        if stakes.iter().any(|(u, a)| self.held(*u) < *a) || !(0..=total).contains(&prize) {
            return Err("The stakes are not held.".into());
        }
        let mut entry = self.entry(kind, None, Some(winner), prize, "");
        entry["stakes"] = stakes
            .iter()
            .map(|(u, a)| serde_json::json!({ "uuid": uuid_string(*u), "amount": a }))
            .collect();
        self.audit.append(&entry)?;
        for (u, a) in stakes {
            self.take_held(&uuid_string(*u), *a);
        }
        let account = self
            .accounts
            .accounts
            .entry(uuid_string(winner))
            .or_default();
        account.balance = account.balance.saturating_add(prize);
        self.touch();
        Ok(())
    }

    fn take_held(&mut self, key: &str, amount: i64) {
        if let Some(h) = self.accounts.held.get_mut(key) {
            *h -= amount;
            if *h <= 0 {
                self.accounts.held.remove(key);
            }
        }
    }

    /// Writes `economy.json` when it changed (end of a tick, world closing).
    pub fn save(&mut self) -> Result<(), String> {
        self.audit.sync();
        if !self.dirty {
            return Ok(());
        }
        write_json(&self.path, &self.accounts)?;
        self.dirty = false;
        Ok(())
    }

    pub fn balance(&self, uuid: u128) -> i64 {
        self.accounts
            .accounts
            .get(&uuid_string(uuid))
            .map_or(0, |a| a.balance)
    }

    /// The account of a player who is on: opened with the starting balance
    /// the first time (logged), its name kept up to date.
    pub fn seen(&mut self, uuid: u128, name: &str) {
        let key = uuid_string(uuid);
        match self.accounts.accounts.get_mut(&key) {
            Some(a) if a.name == name => {}
            Some(a) => {
                a.name = name.to_owned();
                self.touch();
            }
            None => {
                let start = self.config.starting_balance.clamp(0, MAX_AMOUNT);
                let entry = self.entry("open", None, Some(uuid), start, "");
                if self.audit.append(&entry).is_ok() {
                    self.accounts.accounts.insert(
                        key,
                        Account {
                            name: name.to_owned(),
                            balance: start,
                        },
                    );
                    self.touch();
                }
            }
        }
    }

    /// An account by the name last seen, any case.
    pub fn find(&self, name: &str) -> Option<(u128, &Account)> {
        self.accounts
            .accounts
            .iter()
            .find(|(_, a)| a.name.eq_ignore_ascii_case(name))
            .and_then(|(k, a)| Some((super::smp_store::parse_uuid(k)?, a)))
    }

    /// The name last seen of a player who has an account.
    pub fn name_of(&self, uuid: u128) -> Option<&str> {
        let a = self
            .accounts
            .accounts
            .get(&super::smp_store::uuid_string(uuid))?;
        Some(&a.name)
    }

    /// `1,250 coins`.
    pub fn money(&self, amount: i64) -> String {
        let digits = amount.unsigned_abs().to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
        }
        let sign = if amount < 0 { "-" } else { "" };
        format!("{sign}{out} {}", self.config.currency)
    }

    fn entry(
        &self,
        kind: &str,
        from: Option<u128>,
        to: Option<u128>,
        amount: i64,
        by: &str,
    ) -> serde_json::Value {
        let after = |u: Option<u128>| u.map(|u| self.balance(u));
        let mut e = serde_json::json!({
            "time": self.now,
            "kind": kind,
            "amount": amount,
        });
        if let Some(f) = from {
            e["from"] = uuid_string(f).into();
            e["from_before"] = after(Some(f)).into();
        }
        if let Some(t) = to {
            e["to"] = uuid_string(t).into();
            e["to_before"] = after(Some(t)).into();
        }
        if !by.is_empty() {
            e["by"] = by.into();
        }
        e
    }

    /// Moves `amount` from one account to another, logged as `kind`. Both
    /// must exist; `from` must hold it.
    pub fn transfer(
        &mut self,
        kind: &str,
        from: u128,
        to: u128,
        amount: i64,
    ) -> Result<(), String> {
        if amount <= 0 || amount > MAX_AMOUNT {
            return Err("The amount must be a positive whole number.".into());
        }
        if from == to {
            return Err("You can't pay yourself.".into());
        }
        let (Some(f), Some(t)) = (
            self.accounts.accounts.get(&uuid_string(from)),
            self.accounts.accounts.get(&uuid_string(to)),
        ) else {
            return Err("No such account.".into());
        };
        if f.balance < amount {
            return Err(format!("You only have {}.", self.money(f.balance)));
        }
        let Some(to_after) = t
            .balance
            .checked_add(amount)
            .filter(|b| *b <= MAX_AMOUNT * 1000)
        else {
            return Err("That account can't hold more.".into());
        };
        let entry = self.entry(kind, Some(from), Some(to), amount, "");
        self.audit.append(&entry)?;
        self.accounts
            .accounts
            .get_mut(&uuid_string(from))
            .expect("checked")
            .balance -= amount;
        self.accounts
            .accounts
            .get_mut(&uuid_string(to))
            .expect("checked")
            .balance = to_after;
        self.touch();
        Ok(())
    }

    /// Adds (or with a negative `delta`, takes) money from the server: the
    /// market, games, operators. The balance never goes below zero.
    pub fn adjust(&mut self, kind: &str, uuid: u128, delta: i64, by: &str) -> Result<i64, String> {
        let key = uuid_string(uuid);
        let Some(a) = self.accounts.accounts.get(&key) else {
            return Err("No such account.".into());
        };
        let after = a.balance.saturating_add(delta);
        if after < 0 {
            return Err(format!("Only {} there.", self.money(a.balance)));
        }
        if after > MAX_AMOUNT * 1000 {
            return Err("That account can't hold more.".into());
        }
        let (from, to) = if delta < 0 {
            (Some(uuid), None)
        } else {
            (None, Some(uuid))
        };
        let entry = self.entry(kind, from, to, delta.abs(), by);
        self.audit.append(&entry)?;
        self.accounts
            .accounts
            .get_mut(&key)
            .expect("checked")
            .balance = after;
        self.touch();
        Ok(after)
    }

    /// Appends a line of another kind to the audit log (a trade's items),
    /// with the time; the change it records waits for it.
    pub fn record(&mut self, mut entry: serde_json::Value) -> Result<(), String> {
        entry["time"] = self.now.into();
        self.audit.append(&entry)
    }

    /// The accounts, richest first (then by name).
    pub fn top(&self) -> Vec<(&str, i64)> {
        let mut all: Vec<_> = self
            .accounts
            .accounts
            .values()
            .map(|a| (a.name.as_str(), a.balance))
            .collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        all
    }

    /// Runs `/balance`, `/bal`, `/pay`, `/baltop` or `/eco`; None for another
    /// command.
    pub fn command(&mut self, me: &Player, line: &str) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (&name, args) = words.split_first()?;
        Some(match name {
            "balance" | "bal" => self.balance_command(me, args),
            "pay" => self.pay(me, args),
            "baltop" => self.baltop(args),
            "eco" => self.eco(me, args),
            _ => return None,
        })
    }

    fn balance_command(&mut self, me: &Player, args: &[&str]) -> Vec<Out> {
        match args {
            [] => vec![Out::Reply(format!(
                "Balance: {}",
                self.money(self.balance(me.uuid))
            ))],
            [who] => match self.find(who) {
                Some((_, a)) => vec![Out::Reply(format!("{}: {}", a.name, self.money(a.balance)))],
                None => vec![Out::Error(format!("No player named {who} has an account."))],
            },
            _ => vec![Out::Error("Usage: /balance [player]".into())],
        }
    }

    fn pay(&mut self, me: &Player, args: &[&str]) -> Vec<Out> {
        let [who, amount] = args else {
            return vec![Out::Error("Usage: /pay <player> <amount>".into())];
        };
        let Some(amount) = parse_amount(amount) else {
            return vec![Out::Error(
                "The amount must be a positive whole number.".into(),
            )];
        };
        let Some((to, account)) = self.find(who) else {
            return vec![Out::Error(format!("No player named {who} has an account."))];
        };
        let to_name = account.name.clone();
        match self.transfer("pay", me.uuid, to, amount) {
            Ok(()) => vec![
                Out::Reply(format!(
                    "You paid {} to {to_name}. Balance: {}",
                    self.money(amount),
                    self.money(self.balance(me.uuid))
                )),
                Out::Tell(to, format!("{} paid you {}.", me.name, self.money(amount))),
            ],
            Err(e) => vec![Out::Error(e)],
        }
    }

    fn baltop(&mut self, args: &[&str]) -> Vec<Out> {
        let page = match args {
            [] => 1,
            [p] => match p.parse::<usize>() {
                Ok(p) if p >= 1 => p,
                _ => return vec![Out::Error("Usage: /baltop [page]".into())],
            },
            _ => return vec![Out::Error("Usage: /baltop [page]".into())],
        };
        let top = self.top();
        let pages = top.len().div_ceil(PAGE).max(1);
        let mut out = vec![Out::Reply(format!("Richest players ({page}/{pages}):"))];
        for (i, (name, balance)) in top.iter().enumerate().skip((page - 1) * PAGE).take(PAGE) {
            out.push(Out::Reply(format!(
                "{}. {name}: {}",
                i + 1,
                self.money(*balance)
            )));
        }
        out
    }

    fn eco(&mut self, me: &Player, args: &[&str]) -> Vec<Out> {
        if me.level < 3 {
            return vec![Out::Error("Only operators of level 3 may use /eco.".into())];
        }
        let [verb, who, amount] = args else {
            return vec![Out::Error(
                "Usage: /eco give|take|set <player> <amount>".into(),
            )];
        };
        let amount = match amount.parse::<i64>() {
            Ok(a) if (0..=MAX_AMOUNT).contains(&a) => a,
            _ => {
                return vec![Out::Error(
                    "The amount must be a whole number from 0.".into(),
                )];
            }
        };
        let Some((uuid, account)) = self.find(who) else {
            return vec![Out::Error(format!("No player named {who} has an account."))];
        };
        let name = account.name.clone();
        let delta = match *verb {
            "give" => amount,
            "take" => -amount,
            "set" => amount - account.balance,
            _ => {
                return vec![Out::Error(
                    "Usage: /eco give|take|set <player> <amount>".into(),
                )];
            }
        };
        let by = uuid_string(me.uuid);
        match self.adjust(&format!("eco_{verb}"), uuid, delta, &by) {
            Ok(after) => vec![Out::Reply(format!("{name} now has {}.", self.money(after)))],
            Err(e) => vec![Out::Error(e)],
        }
    }
}

/// A positive whole amount, `1250` or `1,250`.
pub fn parse_amount(s: &str) -> Option<i64> {
    let digits: String = s.chars().filter(|c| *c != ',').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits
        .parse::<i64>()
        .ok()
        .filter(|a| (1..=MAX_AMOUNT).contains(a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;

    fn player(uuid: u128, name: &str, level: u8) -> Player {
        Player {
            uuid,
            name: name.into(),
            level,
        }
    }

    fn economy(name: &str) -> (Economy, PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-economy-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = Config {
            starting_balance: 100,
            ..Config::default()
        };
        let mut e = Economy::open(&dir, config).unwrap();
        e.seen(ALEX, "Alex");
        e.seen(STEVE, "Steve");
        (e, dir)
    }

    fn log_lines(dir: &std::path::Path) -> Vec<serde_json::Value> {
        fs::read_to_string(dir.join("audit.log"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn pay_moves_money_and_tells_both() {
        let (mut e, dir) = economy("pay");
        let out = e.command(&player(ALEX, "Alex", 0), "pay steve 30").unwrap();
        assert_eq!(
            out,
            [
                Out::Reply("You paid 30 coins to Steve. Balance: 70 coins".into()),
                Out::Tell(STEVE, "Alex paid you 30 coins.".into()),
            ]
        );
        assert_eq!((e.balance(ALEX), e.balance(STEVE)), (70, 130));
        let log = log_lines(&dir);
        assert_eq!(log.len(), 3, "two accounts opened, one payment");
        assert_eq!(log[2]["kind"], "pay");
        assert_eq!(log[2]["from_before"], 100);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pay_refuses_what_it_must() {
        let (mut e, dir) = economy("refuse");
        let alex = player(ALEX, "Alex", 0);
        for (line, error) in [
            ("pay Steve 101", "You only have 100 coins."),
            ("pay Steve 0", "The amount must be a positive whole number."),
            (
                "pay Steve -5",
                "The amount must be a positive whole number.",
            ),
            (
                "pay Steve 1.5",
                "The amount must be a positive whole number.",
            ),
            ("pay Alex 5", "You can't pay yourself."),
            (
                "pay Herobrine 5",
                "No player named Herobrine has an account.",
            ),
            ("pay Steve", "Usage: /pay <player> <amount>"),
        ] {
            assert_eq!(
                e.command(&alex, line).unwrap(),
                [Out::Error(error.into())],
                "{line}"
            );
        }
        assert_eq!((e.balance(ALEX), e.balance(STEVE)), (100, 100));
        assert_eq!(
            log_lines(&dir).len(),
            2,
            "nothing logged but the two accounts"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn balances_survive_a_restart_and_a_damaged_file_stops_the_economy() {
        let (mut e, dir) = economy("restart");
        e.command(&player(ALEX, "Alex", 0), "pay Steve 1,000")
            .unwrap();
        e.command(&player(ALEX, "Alex", 0), "pay Steve 25").unwrap();
        e.save().unwrap();
        let again = Economy::open(&dir, Config::default()).unwrap();
        assert_eq!((again.balance(ALEX), again.balance(STEVE)), (75, 125));
        fs::write(dir.join("economy.json"), "{ half").unwrap();
        assert!(Economy::open(&dir, Config::default()).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn eco_is_for_level_3_and_never_goes_below_zero() {
        let (mut e, dir) = economy("eco");
        let alex = player(ALEX, "Alex", 2);
        assert_eq!(
            e.command(&alex, "eco give Steve 5").unwrap(),
            [Out::Error("Only operators of level 3 may use /eco.".into())]
        );
        let op = player(ALEX, "Alex", 3);
        assert_eq!(
            e.command(&op, "eco give Steve 5").unwrap(),
            [Out::Reply("Steve now has 105 coins.".into())]
        );
        assert_eq!(
            e.command(&op, "eco take Steve 200").unwrap(),
            [Out::Error("Only 105 coins there.".into())]
        );
        assert_eq!(
            e.command(&op, "eco set Steve 1234567").unwrap(),
            [Out::Reply("Steve now has 1,234,567 coins.".into())]
        );
        let log = log_lines(&dir);
        assert_eq!(log.last().unwrap()["kind"], "eco_set");
        assert_eq!(log.last().unwrap()["by"], uuid_string(ALEX));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn baltop_pages_the_richest_first() {
        let (mut e, dir) = economy("baltop");
        for i in 0..12u128 {
            e.seen(100 + i, &format!("P{i:02}"));
        }
        e.command(&player(ALEX, "Alex", 0), "pay Steve 50").unwrap();
        let first = e.command(&player(ALEX, "Alex", 0), "baltop").unwrap();
        assert_eq!(first[0], Out::Reply("Richest players (1/2):".into()));
        assert_eq!(first[1], Out::Reply("1. Steve: 150 coins".into()));
        assert_eq!(first.len(), 11);
        let last = e.command(&player(ALEX, "Alex", 0), "baltop 2").unwrap();
        assert_eq!(
            last.last().unwrap(),
            &Out::Reply("14. Alex: 50 coins".into())
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_name_change_keeps_the_account() {
        let (mut e, dir) = economy("rename");
        e.seen(ALEX, "Alexandra");
        assert_eq!(e.balance(ALEX), 100);
        assert!(e.find("alexandra").is_some() && e.find("Alex").is_none());
        assert_eq!(log_lines(&dir).len(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
