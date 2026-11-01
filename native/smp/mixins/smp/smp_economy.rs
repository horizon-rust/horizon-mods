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

/// What the Treasury paid or got, for `/treasury log`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Movement {
    pub time: i64,
    /// `vat`, `land_tax`, `grant`...
    pub kind: String,
    /// Positive: income; negative: spending.
    pub amount: i64,
    /// The player's name last seen.
    pub player: String,
    /// Why, as the operator wrote it (payments); empty for taxes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// The operator who paid it (name).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
}

/// The Treasury: the state's account. Every tax, fee and house edge goes in;
/// operators' grants, refunds and prizes (and later bank interest and
/// contracts) come out, never more than it holds. Nobody can make money.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Treasury {
    /// Since the Treasury opened, by kind.
    pub income: BTreeMap<String, i64>,
    pub spent: BTreeMap<String, i64>,
    /// Today's (UTC day number), by kind.
    pub day: i64,
    pub income_today: BTreeMap<String, i64>,
    pub spent_today: BTreeMap<String, i64>,
    /// The day before `day`'s, kept for the daily report.
    pub income_yesterday: BTreeMap<String, i64>,
    pub spent_yesterday: BTreeMap<String, i64>,
    /// The latest movements, newest last (at most `LOG`).
    pub log: Vec<Movement>,
}

/// Movements `/treasury log` keeps.
pub const LOG: usize = 50;
/// Milliseconds in a day.
pub const DAY_MS: i64 = 86_400_000;

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
    /// Other parts' state that holds money or items (the auction house's
    /// listings, bids and mail), by part: written in the same file as the
    /// balances, so one write moves both and a crash can't split them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, serde_json::Value>,
    /// System pools, by name: the Treasury (`TREASURY`), its insurance
    /// fund (`INSURANCE`), the casino's pot. Money in a pool belongs to no
    /// player: never in `accounts()`, `/baltop` or the sidebar.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pools: BTreeMap<String, i64>,
    /// What went into and out of the Treasury pool, for `/treasury`.
    #[serde(default)]
    pub treasury: Treasury,
    /// Money made and destroyed by the server (`adjust`: market sales and
    /// purchases, the daily reward), by kind, for `/eco stats`.
    #[serde(default)]
    pub flows: Flows,
}

/// Money the server made (`minted`) or took out of the game (`burned`),
/// today and the day before, by kind (the audit kind's first word).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flows {
    pub day: i64,
    pub minted: BTreeMap<String, i64>,
    pub burned: BTreeMap<String, i64>,
    pub minted_yesterday: BTreeMap<String, i64>,
    pub burned_yesterday: BTreeMap<String, i64>,
}

impl Flows {
    /// Starts `day` (moving the last one to yesterday's when it was the day
    /// before).
    pub fn roll(&mut self, day: i64) {
        if self.day == day {
            return;
        }
        let next = self.day + 1 == day;
        self.minted_yesterday = std::mem::take(&mut self.minted);
        self.burned_yesterday = std::mem::take(&mut self.burned);
        if !next {
            self.minted_yesterday.clear();
            self.burned_yesterday.clear();
        }
        self.day = day;
    }
}

/// The state's pool: taxes, fees and house edges in; grants out.
pub const TREASURY: &str = "treasury";
/// Part of the state: covers what a liquidated position can't.
pub const INSURANCE: &str = "insurance";

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

    /// Money the server made and destroyed, by kind (rolled to `now`'s day).
    pub fn flows(&mut self) -> &Flows {
        self.accounts.flows.roll(self.now.div_euclid(DAY_MS));
        &self.accounts.flows
    }

    /// All the money held for games.
    pub fn held_total(&self) -> i64 {
        self.accounts.held.values().sum()
    }

    /// Every pool, by name.
    pub fn pools(&self) -> &BTreeMap<String, i64> {
        &self.accounts.pools
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
        let fee = total - prize;
        let mut entry = self.entry(kind, None, Some(winner), prize, "");
        entry["stakes"] = stakes
            .iter()
            .map(|(u, a)| serde_json::json!({ "uuid": uuid_string(*u), "amount": a }))
            .collect();
        if fee > 0 {
            // The house's fee goes to the Treasury, not away.
            entry["fee"] = fee.into();
            entry["to_pool"] = TREASURY.into();
        }
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
        if fee > 0 {
            self.pool_add(TREASURY, fee);
            self.count_treasury("gambling_fee", fee, Some(winner), "", "");
        }
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

    /// Money in the pool `name` (0 for one never used).
    pub fn pool(&self, name: &str) -> i64 {
        self.accounts.pools.get(name).copied().unwrap_or(0)
    }

    /// What went into and out of the Treasury.
    pub fn treasury(&self) -> &Treasury {
        &self.accounts.treasury
    }

    fn pool_add(&mut self, name: &str, delta: i64) {
        let p = self.accounts.pools.entry(name.to_owned()).or_default();
        *p = p.saturating_add(delta);
        if *p == 0 {
            self.accounts.pools.remove(name);
        }
    }

    /// Counts a Treasury movement (`amount` > 0 income, < 0 spending).
    fn count_treasury(
        &mut self,
        kind: &str,
        amount: i64,
        player: Option<u128>,
        reason: &str,
        by: &str,
    ) {
        if amount == 0 {
            return;
        }
        let day = self.now.div_euclid(DAY_MS);
        let player = player
            .and_then(|u| self.name_of(u).map(str::to_owned))
            .unwrap_or_default();
        let by = super::smp_store::parse_uuid(by)
            .and_then(|u| self.name_of(u).map(str::to_owned))
            .unwrap_or_else(|| by.to_owned());
        let t = &mut self.accounts.treasury;
        if t.day != day {
            let next = t.day + 1 == day;
            t.income_yesterday = std::mem::take(&mut t.income_today);
            t.spent_yesterday = std::mem::take(&mut t.spent_today);
            if !next {
                t.income_yesterday.clear();
                t.spent_yesterday.clear();
            }
            t.day = day;
        }
        let (all, today) = if amount > 0 {
            (&mut t.income, &mut t.income_today)
        } else {
            (&mut t.spent, &mut t.spent_today)
        };
        for map in [all, today] {
            let v = map.entry(kind.to_owned()).or_default();
            *v = v.saturating_add(amount.abs());
        }
        t.log.push(Movement {
            time: self.now,
            kind: kind.to_owned(),
            amount,
            player,
            reason: reason.to_owned(),
            by,
        });
        if t.log.len() > LOG {
            let extra = t.log.len() - LOG;
            t.log.drain(..extra);
        }
    }

    /// A tax, a fee, a stake lost: `amount` of `uuid`'s balance goes into
    /// `pool`, logged as `kind`. An Err when they hold less.
    pub fn to_pool(
        &mut self,
        kind: &str,
        uuid: u128,
        pool: &str,
        amount: i64,
        reason: &str,
    ) -> Result<(), String> {
        self.into_pool(kind, uuid, pool, amount, reason, "")
    }

    /// [`Self::to_pool`] by an operator (`by`: their UUID), logged so.
    pub fn into_pool(
        &mut self,
        kind: &str,
        uuid: u128,
        pool: &str,
        amount: i64,
        reason: &str,
        by: &str,
    ) -> Result<(), String> {
        if amount <= 0 || amount > MAX_AMOUNT {
            return Err("The amount must be a positive whole number.".into());
        }
        let key = uuid_string(uuid);
        let Some(a) = self.accounts.accounts.get(&key) else {
            return Err("No such account.".into());
        };
        if a.balance < amount {
            return Err(format!("Only {} there.", self.money(a.balance)));
        }
        let mut entry = self.entry(kind, Some(uuid), None, amount, by);
        entry["to_pool"] = pool.into();
        entry["pool_before"] = self.pool(pool).into();
        if !reason.is_empty() {
            entry["reason"] = reason.into();
        }
        self.audit.append(&entry)?;
        self.accounts
            .accounts
            .get_mut(&key)
            .expect("checked")
            .balance -= amount;
        self.pool_add(pool, amount);
        if pool == TREASURY {
            self.count_treasury(kind, amount, Some(uuid), reason, by);
        }
        self.touch();
        Ok(())
    }

    /// A payment from `pool` to `uuid` (a grant, a refund, a payout),
    /// logged as `kind`: never more than the pool holds.
    #[allow(dead_code)] // the casino, the auction house and the futures
    pub fn from_pool(
        &mut self,
        kind: &str,
        pool: &str,
        uuid: u128,
        amount: i64,
        reason: &str,
    ) -> Result<(), String> {
        self.out_of_pool(kind, pool, uuid, amount, reason, "")
    }

    /// [`Self::from_pool`] by an operator (`by`: their UUID), logged so.
    pub fn out_of_pool(
        &mut self,
        kind: &str,
        pool: &str,
        uuid: u128,
        amount: i64,
        reason: &str,
        by: &str,
    ) -> Result<(), String> {
        if amount <= 0 || amount > MAX_AMOUNT {
            return Err("The amount must be a positive whole number.".into());
        }
        let key = uuid_string(uuid);
        let Some(a) = self.accounts.accounts.get(&key) else {
            return Err("No such account.".into());
        };
        if a.balance.saturating_add(amount) > MAX_AMOUNT * 1000 {
            return Err("That account can't hold more.".into());
        }
        let held = self.pool(pool);
        if held < amount {
            return Err(format!("The {pool} only holds {}.", self.money(held)));
        }
        let mut entry = self.entry(kind, None, Some(uuid), amount, by);
        entry["from_pool"] = pool.into();
        entry["pool_before"] = held.into();
        if !reason.is_empty() {
            entry["reason"] = reason.into();
        }
        self.audit.append(&entry)?;
        self.accounts
            .accounts
            .get_mut(&key)
            .expect("checked")
            .balance += amount;
        self.pool_add(pool, -amount);
        if pool == TREASURY {
            self.count_treasury(kind, -amount, Some(uuid), reason, by);
        }
        self.touch();
        Ok(())
    }

    /// Moves `amount` from one pool to another (the casino's profit to the
    /// Treasury), logged as `kind`; an Err when `from` holds less.
    #[allow(dead_code)] // the casino, the auction house and the futures
    pub fn pool_move(
        &mut self,
        kind: &str,
        from: &str,
        to: &str,
        amount: i64,
        reason: &str,
    ) -> Result<(), String> {
        if amount <= 0 || amount > MAX_AMOUNT * 1000 || from == to {
            return Err("The amount must be a positive whole number.".into());
        }
        let held = self.pool(from);
        if held < amount {
            return Err(format!("The {from} only holds {}.", self.money(held)));
        }
        let mut entry = serde_json::json!({
            "time": self.now, "kind": kind, "amount": amount,
            "from_pool": from, "from_pool_before": held,
            "to_pool": to, "to_pool_before": self.pool(to),
        });
        if !reason.is_empty() {
            entry["reason"] = reason.into();
        }
        self.audit.append(&entry)?;
        self.pool_add(from, -amount);
        self.pool_add(to, amount);
        if to == TREASURY {
            self.count_treasury(kind, amount, None, reason, "");
        }
        if from == TREASURY {
            self.count_treasury(kind, -amount, None, reason, "");
        }
        self.touch();
        Ok(())
    }

    /// A bet against a pool decided: `uuid`'s held `stake` goes into
    /// `pool`, then `payout` (at most what the pool then holds) comes out
    /// to them; one log line. What was paid. Never makes money.
    #[allow(dead_code)] // the casino, the auction house and the futures
    pub fn settle_pool(
        &mut self,
        kind: &str,
        uuid: u128,
        stake: i64,
        payout: i64,
        pool: &str,
    ) -> Result<i64, String> {
        if stake < 0 || payout < 0 || self.held(uuid) < stake {
            return Err("The stake is not held.".into());
        }
        let before = self.pool(pool);
        let paid = payout.min(before.saturating_add(stake));
        let mut entry = self.entry(kind, None, Some(uuid), paid, "");
        entry["stake"] = stake.into();
        entry["pool"] = pool.into();
        entry["pool_before"] = before.into();
        entry["asked"] = payout.into();
        self.audit.append(&entry)?;
        let key = uuid_string(uuid);
        self.take_held(&key, stake);
        self.pool_add(pool, stake - paid);
        let account = self.accounts.accounts.entry(key).or_default();
        account.balance = account.balance.saturating_add(paid);
        if pool == TREASURY {
            self.count_treasury(kind, stake - paid, Some(uuid), "", "");
        }
        self.touch();
        Ok(paid)
    }

    /// A part's state kept with the balances (`Accounts::ext`).
    pub fn ext(&self, part: &str) -> Option<&serde_json::Value> {
        self.accounts.ext.get(part)
    }

    /// Replaces a part's state, written with the balances at the next save.
    pub fn set_ext(&mut self, part: &str, value: serde_json::Value) {
        self.accounts.ext.insert(part.to_owned(), value);
        self.dirty = true;
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
        let flows = &mut self.accounts.flows;
        flows.roll(self.now.div_euclid(DAY_MS));
        let family = kind.split(' ').next().unwrap_or(kind).to_owned();
        let map = if delta >= 0 {
            &mut flows.minted
        } else {
            &mut flows.burned
        };
        *map.entry(family).or_default() += delta.abs();
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

    /// Runs `/balance`, `/bal`, `/pay`, `/baltop` or the operators' `/eco`
    /// (and its other name `/money`); None for another command. `_online`:
    /// the players on (no command uses it now that `/eco give *` is gone).
    pub fn command(
        &mut self,
        me: &Player,
        _online: &[(u128, String)],
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (&name, args) = words.split_first()?;
        Some(match name {
            "balance" | "bal" => self.balance_command(me, args),
            "pay" => self.pay(me, args),
            "baltop" => self.baltop(args),
            "eco" | "money" => self.eco(me, args),
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

    /// `/eco take <player> <amount> [reason]`: an operator takes money from
    /// a player (a cheat's gains) into the Treasury. Nobody can make money:
    /// `give` and `set` say where grants come from now.
    fn eco(&mut self, me: &Player, args: &[&str]) -> Vec<Out> {
        if me.level < 3 {
            return vec![Out::Error("Only operators of level 3 may use /eco.".into())];
        }
        const USAGE: &str = "Usage: /eco take <player> <amount> [reason]";
        match args {
            ["give" | "set" | "add", ..] => vec![Out::Error(
                "Nobody can make money, operators included. Pay from the Treasury: /treasury pay <player> <amount> <reason>"
                    .into(),
            )],
            ["take", who, amount, reason @ ..] => {
                let Some(amount) = parse_amount(amount) else {
                    return vec![Out::Error(
                        "The amount must be a positive whole number.".into(),
                    )];
                };
                let Some((uuid, account)) = self.find(who) else {
                    return vec![Out::Error(format!("No player named {who} has an account."))];
                };
                let name = account.name.clone();
                let reason = reason.join(" ");
                let by = uuid_string(me.uuid);
                match self.into_pool("eco_take", uuid, TREASURY, amount, &reason, &by) {
                    Ok(()) => vec![Out::Reply(format!(
                        "Took {} from {name} into the Treasury. They have {}.",
                        self.money(amount),
                        self.money(self.balance(uuid))
                    ))],
                    Err(e) => vec![Out::Error(e)],
                }
            }
            _ => vec![Out::Error(USAGE.into())],
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
        let out = e
            .command(&player(ALEX, "Alex", 0), &[], "pay steve 30")
            .unwrap();
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
                e.command(&alex, &[], line).unwrap(),
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
        e.command(&player(ALEX, "Alex", 0), &[], "pay Steve 1,000")
            .unwrap();
        e.command(&player(ALEX, "Alex", 0), &[], "pay Steve 25")
            .unwrap();
        e.save().unwrap();
        let again = Economy::open(&dir, Config::default()).unwrap();
        assert_eq!((again.balance(ALEX), again.balance(STEVE)), (75, 125));
        fs::write(dir.join("economy.json"), "{ half").unwrap();
        assert!(Economy::open(&dir, Config::default()).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn nobody_makes_money_and_eco_take_goes_to_the_treasury() {
        let (mut e, dir) = economy("eco");
        let op = player(ALEX, "Alex", 4);
        for line in [
            "eco give Steve 5",
            "money give * 1000",
            "eco set Steve 1234567",
            "eco add Steve 1",
        ] {
            assert!(
                matches!(&e.command(&op, &[], line).unwrap()[0], Out::Error(s) if s.starts_with("Nobody can make money")),
                "{line}"
            );
        }
        assert_eq!(
            e.command(&player(ALEX, "Alex", 2), &[], "eco take Steve 5")
                .unwrap(),
            [Out::Error("Only operators of level 3 may use /eco.".into())]
        );
        assert_eq!(
            e.command(&op, &[], "eco take Steve 200").unwrap(),
            [Out::Error("Only 100 coins there.".into())]
        );
        assert_eq!(
            e.command(&op, &[], "eco take Steve 40 duped diamonds")
                .unwrap(),
            [Out::Reply(
                "Took 40 coins from Steve into the Treasury. They have 60 coins.".into()
            )]
        );
        assert_eq!((e.balance(STEVE), e.pool(TREASURY)), (60, 40));
        assert_eq!(e.treasury().income["eco_take"], 40);
        let m = e.treasury().log.last().unwrap();
        assert_eq!(
            (m.player.as_str(), m.reason.as_str(), m.by.as_str()),
            ("Steve", "duped diamonds", "Alex")
        );
        let log = log_lines(&dir);
        let last = log.last().unwrap();
        assert_eq!(
            (last["kind"].as_str(), last["to_pool"].as_str()),
            (Some("eco_take"), Some("treasury"))
        );
        assert_eq!(last["by"], uuid_string(ALEX));
        // Total money (balances + pools) never changed: 2 accounts of 100.
        assert_eq!(e.balance(ALEX) + e.balance(STEVE) + e.pool(TREASURY), 200);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pools_pay_only_what_they_hold_and_survive_a_restart() {
        let (mut e, dir) = economy("pools");
        e.to_pool("vat", ALEX, TREASURY, 30, "").unwrap();
        assert!(
            e.to_pool("vat", ALEX, TREASURY, 71, "").is_err(),
            "more than Alex has"
        );
        assert!(
            e.from_pool("grant", TREASURY, STEVE, 31, "event").is_err(),
            "more than the Treasury holds"
        );
        e.from_pool("grant", TREASURY, STEVE, 10, "event prize")
            .unwrap();
        e.pool_move("insurance_fund", TREASURY, INSURANCE, 5, "")
            .unwrap();
        assert_eq!((e.pool(TREASURY), e.pool(INSURANCE)), (15, 5));
        assert_eq!((e.balance(ALEX), e.balance(STEVE)), (70, 110));
        let t = e.treasury();
        assert_eq!(
            (t.income["vat"], t.spent["grant"], t.spent["insurance_fund"]),
            (30, 10, 5)
        );
        assert_eq!(t.income_today["vat"], 30);
        // Pools are nobody's: not in the ranking.
        assert_eq!(e.accounts().len(), 2);
        e.save().unwrap();
        let again = Economy::open(&dir, Config::default()).unwrap();
        assert_eq!((again.pool(TREASURY), again.pool(INSURANCE)), (15, 5));
        assert_eq!(again.treasury().log.len(), 3);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settle_pool_never_pays_more_than_the_pool_and_award_fees_go_to_the_treasury() {
        let (mut e, dir) = economy("settle");
        e.hold("bet", ALEX, 50).unwrap();
        // The pool held nothing: a 1,000 payout is capped at the stake.
        assert_eq!(
            e.settle_pool("roulette", ALEX, 50, 1000, "casino").unwrap(),
            50
        );
        assert_eq!((e.balance(ALEX), e.pool("casino")), (100, 0));
        e.hold("bet", ALEX, 40).unwrap();
        assert_eq!(e.settle_pool("roulette", ALEX, 40, 0, "casino").unwrap(), 0);
        assert_eq!((e.balance(ALEX), e.pool("casino")), (60, 40));
        assert!(
            e.settle_pool("roulette", ALEX, 10, 0, "casino").is_err(),
            "nothing held"
        );
        // A coinflip with a 10% fee: the fee is the Treasury's.
        e.hold("coinflip_stake", ALEX, 50).unwrap();
        e.hold("coinflip_stake", STEVE, 50).unwrap();
        e.award("coinflip_win", &[(ALEX, 50), (STEVE, 50)], STEVE, 90)
            .unwrap();
        assert_eq!(
            (e.balance(ALEX), e.balance(STEVE), e.pool(TREASURY)),
            (10, 140, 10)
        );
        assert_eq!(e.treasury().income["gambling_fee"], 10);
        // Nothing made: 200 at the start.
        assert_eq!(
            e.balance(ALEX) + e.balance(STEVE) + e.pool(TREASURY) + e.pool("casino"),
            200
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn baltop_pages_the_richest_first() {
        let (mut e, dir) = economy("baltop");
        for i in 0..12u128 {
            e.seen(100 + i, &format!("P{i:02}"));
        }
        e.command(&player(ALEX, "Alex", 0), &[], "pay Steve 50")
            .unwrap();
        let first = e.command(&player(ALEX, "Alex", 0), &[], "baltop").unwrap();
        assert_eq!(first[0], Out::Reply("Richest players (1/2):".into()));
        assert_eq!(first[1], Out::Reply("1. Steve: 150 coins".into()));
        assert_eq!(first.len(), 11);
        let last = e
            .command(&player(ALEX, "Alex", 0), &[], "baltop 2")
            .unwrap();
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
