// The bank (`/bank`): savings that earn a small interest, paid once a day
// from the Treasury (taxes and fees), never minted. A player deposits coins
// (they wait in the `bank` pool) and takes them back at any time, free.
// Interest is paid at 00:00 UTC on the lowest amount the account held since
// the last payment, so a deposit made just before midnight earns nothing
// that night; only the first `max_interest_on` coins of each account earn,
// and one day's interest for everyone is at most `max_treasury_percent` of
// the Treasury (shared out pro rata when it's short). No loans: a loan's
// collateral (items) can't be held safely here, and a loan without it is
// free money for an alt that never pays back.
//
// The accounts live in `economy.json` (`ext.bank`), the coins in its pool:
// money and records are written together. No game types. Added to the game
// as `game::smp_bank`; also a module of this crate, for its tests.

use super::smp_economy::{DAY_MS, Economy, MAX_AMOUNT, Out, Player, TREASURY, parse_amount};
use super::smp_store::{parse_uuid, uuid_string};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The pool savings wait in.
pub const POOL: &str = "bank";

/// `config.json`'s `bank`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Interest a day, in hundredths of a percent (10: 0.1% a day).
    pub daily_basis_points: i64,
    /// Coins of an account that earn interest; more may be saved.
    pub max_interest_on: i64,
    /// What a player may save in all.
    pub max_deposit: i64,
    /// One day's interest for everyone is at most this share of the
    /// Treasury.
    pub max_treasury_percent: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            daily_basis_points: 10,
            max_interest_on: 10_000,
            max_deposit: 25_000,
            max_treasury_percent: 5,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saving {
    pub amount: i64,
    /// The lowest `amount` since the last payment: what earns.
    pub low: i64,
    pub earned: i64,
}

/// `economy.json`'s `ext.bank`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub accounts: BTreeMap<String, Saving>,
    /// The day (since 1970, UTC) interest was last paid.
    pub day: i64,
    pub paid: i64,
    /// The last payment: what it was, and whether the Treasury's cap cut it.
    pub last: i64,
    pub last_cut: bool,
}

pub struct Bank {
    pub config: Config,
    pub data: Data,
    dirty: bool,
}

impl Bank {
    pub fn open(economy: &Economy, config: Config) -> Self {
        let mut data = economy
            .ext("bank")
            .and_then(|v| serde_json::from_value::<Data>(v.clone()).ok())
            .unwrap_or_default();
        if data.day == 0 {
            data.day = economy.now.div_euclid(DAY_MS);
        }
        Self {
            config,
            data,
            dirty: false,
        }
    }

    pub fn store(&mut self, economy: &mut Economy) {
        if self.dirty {
            economy.set_ext("bank", serde_json::to_value(&self.data).unwrap_or_default());
            self.dirty = false;
        }
    }

    /// What each player has saved (the wealth tax counts it).
    pub fn savings(&self) -> HashMap<u128, i64> {
        self.data
            .accounts
            .iter()
            .filter_map(|(k, s)| Some((parse_uuid(k)?, s.amount)))
            .collect()
    }

    pub fn saved(&self, uuid: u128) -> i64 {
        self.data
            .accounts
            .get(&uuid_string(uuid))
            .map_or(0, |s| s.amount)
    }

    /// `/bank ...`; None for another command.
    pub fn command(&mut self, economy: &mut Economy, me: &Player, line: &str) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() != Some(&"bank") {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error(
                "The bank is closed on this server.".into(),
            )]);
        }
        Some(match words[1..] {
            [] => self.show(economy, me),
            ["deposit", amount] => self.deposit(economy, me, amount),
            ["withdraw", amount] => self.withdraw(economy, me, amount),
            _ => vec![Out::Error(
                "Usage: /bank, /bank deposit <amount|all>, /bank withdraw <amount|all>".into(),
            )],
        })
    }

    fn rate(&self) -> String {
        let bp = self.config.daily_basis_points;
        format!("{}.{:02}% a day", bp / 100, bp % 100)
    }

    fn show(&self, economy: &Economy, me: &Player) -> Vec<Out> {
        let s = self
            .data
            .accounts
            .get(&uuid_string(me.uuid))
            .cloned()
            .unwrap_or_default();
        let next =
            (s.low.min(self.config.max_interest_on) * self.config.daily_basis_points) / 10_000;
        vec![
            Out::Reply(format!(
                "Saved: {} (earned so far {}). Wallet: {}.",
                economy.money(s.amount),
                economy.money(s.earned),
                economy.money(economy.balance(me.uuid))
            )),
            Out::Reply(format!(
                "Interest: {} on up to {}, paid at 00:00 UTC on the lowest amount saved since the last payment, from the Treasury (at most {}% of it a day). Tonight: about {}.",
                self.rate(),
                economy.money(self.config.max_interest_on),
                self.config.max_treasury_percent,
                economy.money(next)
            )),
            Out::Reply(format!(
                "/bank deposit <amount|all> (up to {} saved), /bank withdraw <amount|all>: free, at any time. No loans.",
                economy.money(self.config.max_deposit)
            )),
        ]
    }

    fn amount(word: &str, all: i64) -> Option<i64> {
        if word == "all" {
            (all > 0).then_some(all)
        } else {
            parse_amount(word)
        }
    }

    fn deposit(&mut self, economy: &mut Economy, me: &Player, word: &str) -> Vec<Out> {
        let key = uuid_string(me.uuid);
        let saved = self.saved(me.uuid);
        let room = (self.config.max_deposit - saved).max(0);
        let wallet = economy.balance(me.uuid);
        let Some(amount) = Self::amount(word, wallet.min(room)) else {
            return vec![Out::Error(if room == 0 {
                format!(
                    "You have saved the most there is: {}.",
                    economy.money(self.config.max_deposit)
                )
            } else {
                "The amount must be a positive whole number, or all.".into()
            })];
        };
        if amount > room {
            return vec![Out::Error(format!(
                "You may save {} more (at most {} in all).",
                economy.money(room),
                economy.money(self.config.max_deposit)
            ))];
        }
        if let Err(e) = economy.to_pool("bank_deposit", me.uuid, POOL, amount, "") {
            return vec![Out::Error(e)];
        }
        // `low` stays: what was there all day earns, not the new coins.
        let s = self.data.accounts.entry(key).or_default();
        s.amount += amount;
        self.dirty = true;
        vec![Out::Reply(format!(
            "Saved {}: {} in the bank. It earns from tomorrow's payment on.",
            economy.money(amount),
            economy.money(s.amount)
        ))]
    }

    fn withdraw(&mut self, economy: &mut Economy, me: &Player, word: &str) -> Vec<Out> {
        let key = uuid_string(me.uuid);
        let saved = self.saved(me.uuid);
        if saved == 0 {
            return vec![Out::Error("You have nothing in the bank.".into())];
        }
        let Some(amount) = Self::amount(word, saved) else {
            return vec![Out::Error(if saved == 0 {
                "You have nothing in the bank.".into()
            } else {
                "The amount must be a positive whole number, or all.".into()
            })];
        };
        if amount > saved {
            return vec![Out::Error(format!(
                "You have {} in the bank.",
                economy.money(saved)
            ))];
        }
        if let Err(e) = economy.from_pool("bank_withdraw", POOL, me.uuid, amount, "") {
            return vec![Out::Error(e)];
        }
        let s = self.data.accounts.get_mut(&key).expect("saved > 0");
        s.amount -= amount;
        s.low = s.low.min(s.amount);
        let left = s.amount;
        if left == 0 && s.earned == 0 {
            self.data.accounts.remove(&key);
        }
        self.dirty = true;
        vec![Out::Reply(format!(
            "Took {} out: {} left in the bank.",
            economy.money(amount),
            economy.money(left)
        ))]
    }

    /// Once a second from the glue; the work once a day: interest from the
    /// Treasury into each account (it stays saved, and earns from then on).
    pub fn tick(&mut self, economy: &mut Economy, now: i64) -> Vec<Out> {
        let today = now.div_euclid(DAY_MS);
        if today <= self.data.day {
            return Vec::new();
        }
        self.data.day = today;
        self.dirty = true;
        let bp = self.config.daily_basis_points.max(0);
        let due: Vec<(String, i64)> = self
            .data
            .accounts
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    (s.low.clamp(0, self.config.max_interest_on) as i128 * bp as i128 / 10_000)
                        as i64,
                )
            })
            .collect();
        let total: i64 = due.iter().map(|d| d.1).sum();
        let cap = economy.pool(TREASURY).max(0) as i128
            * self.config.max_treasury_percent.clamp(0, 100) as i128
            / 100;
        let cut = i128::from(total) > cap;
        let mut paid = 0;
        let mut out = Vec::new();
        for (key, d) in due {
            let pay = if cut {
                (i128::from(d) * cap / i128::from(total.max(1))) as i64
            } else {
                d
            };
            let pay = pay.min(MAX_AMOUNT);
            if pay > 0
                && economy
                    .pool_move("bank_interest", TREASURY, POOL, pay, "")
                    .is_ok()
            {
                paid += pay;
                let s = self.data.accounts.get_mut(&key).expect("listed");
                s.amount += pay;
                s.earned += pay;
                if let Some(u) = parse_uuid(&key) {
                    out.push(Out::Tell(
                        u,
                        format!(
                            "Bank interest: {} (saved: {}).",
                            economy.money(pay),
                            economy.money(s.amount)
                        ),
                    ));
                }
            }
        }
        // From now on, what's there all day earns.
        for s in self.data.accounts.values_mut() {
            s.low = s.amount;
        }
        self.data.paid += paid;
        self.data.last = paid;
        self.data.last_cut = cut;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config as EcoConfig;

    const ALEX: u128 = 1;
    const ALT: u128 = 2;
    const T0: i64 = 1_793_577_600_000; // a midnight, UTC

    fn me(uuid: u128) -> Player {
        Player {
            uuid,
            name: format!("P{uuid}"),
            level: 0,
        }
    }

    fn setup(name: &str, treasury: i64) -> (Economy, Bank, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-bank-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        e.now = T0 + 1000;
        for u in [ALEX, ALT] {
            e.seen(u, &format!("P{u}"));
            e.adjust("sell", u, 50_000, "").unwrap();
        }
        if treasury > 0 {
            e.to_pool("seed", ALT, TREASURY, treasury, "").unwrap();
        }
        let b = Bank::open(&e, Config::default());
        (e, b, dir)
    }

    fn total(e: &Economy) -> i64 {
        e.balance(ALEX) + e.balance(ALT) + e.pool(TREASURY) + e.pool(POOL)
    }

    fn say(b: &mut Bank, e: &mut Economy, who: u128, line: &str) -> Vec<Out> {
        b.command(e, &me(who), line).unwrap()
    }

    #[test]
    fn a_deposit_earns_from_the_next_full_day_and_interest_comes_from_the_treasury() {
        let (mut e, mut b, _d) = setup("earn", 10_000);
        let start = total(&e);
        say(&mut b, &mut e, ALEX, "bank deposit 5000");
        assert_eq!((e.balance(ALEX), e.pool(POOL)), (45_000, 5_000));
        // The first midnight: deposited during the day, nothing earned.
        assert!(b.tick(&mut e, T0 + DAY_MS).is_empty());
        // The next: 0.1% of 5,000.
        let out = b.tick(&mut e, T0 + 2 * DAY_MS);
        assert_eq!(
            out,
            [Out::Tell(
                ALEX,
                "Bank interest: 5 coins (saved: 5,005 coins).".into()
            )]
        );
        assert_eq!((e.pool(TREASURY), b.saved(ALEX)), (9_995, 5_005));
        assert!(
            b.tick(&mut e, T0 + 2 * DAY_MS + 5_000).is_empty(),
            "once a day"
        );
        assert_eq!(total(&e), start, "nothing minted");
    }

    #[test]
    fn deposit_before_midnight_and_withdraw_after_earns_nothing() {
        let (mut e, mut b, _d) = setup("game", 10_000);
        b.tick(&mut e, T0 + DAY_MS);
        say(&mut b, &mut e, ALEX, "bank deposit 10000");
        b.tick(&mut e, T0 + 2 * DAY_MS); // deposited today: low was 0
        say(&mut b, &mut e, ALEX, "bank withdraw all");
        say(&mut b, &mut e, ALEX, "bank deposit 10000");
        // Took it out during the day: the low was 0 again.
        assert!(b.tick(&mut e, T0 + 3 * DAY_MS).is_empty());
        assert_eq!(e.balance(ALEX) + b.saved(ALEX), 50_000);
    }

    #[test]
    fn caps_hold_and_a_poor_treasury_pays_pro_rata() {
        let (mut e, mut b, _d) = setup("caps", 100);
        let err = |o: Vec<Out>| match &o[0] {
            Out::Error(s) => s.clone(),
            o => panic!("{o:?}"),
        };
        assert!(
            err(say(&mut b, &mut e, ALEX, "bank deposit 30000"))
                .starts_with("You may save 25,000 coins more")
        );
        assert!(err(say(&mut b, &mut e, ALEX, "bank withdraw 1")).starts_with("You have nothing"));
        say(&mut b, &mut e, ALEX, "bank deposit all");
        assert_eq!(b.saved(ALEX), 25_000, "all = up to the cap");
        say(&mut b, &mut e, ALT, "bank deposit 10000");
        b.tick(&mut e, T0 + DAY_MS);
        // Due: 10 each (only 10,000 earns); the Treasury (100) pays 5% = 5 in all.
        b.tick(&mut e, T0 + 2 * DAY_MS);
        assert_eq!((b.data.last, b.data.last_cut), (4, true));
        assert_eq!(e.pool(TREASURY), 96);
        // An empty Treasury pays nothing; nobody goes below zero.
        let (mut e, mut b, _d) = setup("empty", 0);
        say(&mut b, &mut e, ALEX, "bank deposit 5000");
        b.tick(&mut e, T0 + DAY_MS);
        assert!(b.tick(&mut e, T0 + 2 * DAY_MS).is_empty());
        assert_eq!(b.saved(ALEX), 5_000);
    }

    #[test]
    fn savings_survive_a_restart_with_the_money() {
        let (mut e, mut b, dir) = setup("restart", 1_000);
        say(&mut b, &mut e, ALEX, "bank deposit 700");
        b.store(&mut e);
        e.save().unwrap();
        let e2 = Economy::open(&dir, EcoConfig::default()).unwrap();
        let b2 = Bank::open(&e2, Config::default());
        assert_eq!((b2.saved(ALEX), e2.pool(POOL)), (700, 700));
        assert_eq!(b2.savings()[&ALEX], 700);
    }
}
