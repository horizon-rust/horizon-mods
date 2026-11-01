// The tax code and the Treasury's commands. Every tax, fee and house edge
// goes into the Treasury pool (smp_economy's `TREASURY`), never away, and
// operators pay only from it (`/treasury pay`): nobody makes money. Weekly
// taxes (land per claimed chunk, a light progressive wealth tax) are
// collected on Monday 00:00 UTC, looked at once a second for an O(1) "due?".
// `/eco taxes` explains the rates and a player's own bill; `/treasury` shows
// the balance, income by tax and spending; `/treasury log` the last
// movements. The tax state (the week collected, unpaid land tax) is kept in
// `economy.json` with the balances (`Economy::ext("tax")`). No game types.
// Added to the game as `game::smp_tax`; also a module of this crate, for its
// tests.

use super::smp_economy::{DAY_MS, Economy, INSURANCE, Out, Player, TREASURY, parse_amount};
use super::smp_store::uuid_string;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// `config.json`'s `taxes`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Weekly taxes on (land, wealth); the per-sale ones are their parts'.
    pub enabled: bool,
    /// VAT on sales between players (the auction house), paid by the seller
    /// from the price, in percent.
    pub vat_percent: i64,
    /// Claimed chunks nobody pays for.
    pub land_free_chunks: usize,
    /// Land tax a week per claimed chunk past the free ones.
    pub land_tax_per_chunk: i64,
    /// `[from, percent]`: each week, `percent` of the part of a balance
    /// above `from` (up to the next bracket's `from`).
    pub wealth_brackets: Vec<[i64; 2]>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            vat_percent: 5,
            land_free_chunks: 4,
            land_tax_per_chunk: 5,
            wealth_brackets: vec![[50_000, 1], [250_000, 2], [1_000_000, 3]],
        }
    }
}

/// One weekly collection, for `/treasury` and the report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub week: i64,
    pub land: i64,
    pub wealth: i64,
    /// Land tax players couldn't pay (added to their arrears).
    pub unpaid: i64,
    pub payers: usize,
}

/// The tax state, in `economy.json` (`ext.tax`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// The last week collected (Monday-based week number).
    pub week: i64,
    /// Land tax owed, by UUID: no new claim until it's paid.
    pub arrears: BTreeMap<String, i64>,
    pub last: Option<Collection>,
}

/// The week number of `now` (ms), weeks starting Monday 00:00 UTC (1970-01-01
/// was a Thursday).
pub fn week(now: i64) -> i64 {
    (now.div_euclid(DAY_MS) + 3).div_euclid(7)
}

/// When the week after `week` starts (ms).
pub fn week_start(week: i64) -> i64 {
    (week * 7 - 3) * DAY_MS
}

pub struct Taxes {
    pub config: Config,
    pub state: State,
    dirty: bool,
    /// What each player has in the bank (the glue sets it): the wealth tax
    /// counts it with the balance, so saving doesn't hide wealth.
    pub savings: HashMap<u128, i64>,
}

/// `12d 3h`, `3h 05m`, `4m`.
pub fn span(ms: i64) -> String {
    let m = ms.max(0) / 60_000;
    let (d, h, m) = (m / 1440, m / 60 % 24, m % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

impl Taxes {
    /// The taxes, their state read from the economy (none yet: the first
    /// collection is next Monday, never weeks back).
    pub fn open(economy: &Economy, config: Config) -> Self {
        let state = economy
            .ext("tax")
            .and_then(|v| serde_json::from_value::<State>(v.clone()).ok());
        let dirty = state.is_none();
        let state = state.unwrap_or_else(|| State {
            week: week(economy.now),
            ..State::default()
        });
        Self {
            config,
            state,
            dirty,
            savings: HashMap::new(),
        }
    }

    /// Writes the state into the economy (saved with the balances).
    pub fn store(&mut self, economy: &mut Economy) {
        if self.dirty {
            economy.set_ext("tax", serde_json::to_value(&self.state).unwrap_or_default());
            self.dirty = false;
        }
    }

    /// The land tax a week for `chunks` claimed chunks.
    pub fn land_bill(&self, chunks: usize) -> i64 {
        let taxed = chunks.saturating_sub(self.config.land_free_chunks) as i64;
        taxed.saturating_mul(self.config.land_tax_per_chunk.max(0))
    }

    /// The wealth tax a week on `balance`.
    pub fn wealth_bill(&self, balance: i64) -> i64 {
        let mut brackets = self.config.wealth_brackets.clone();
        brackets.sort();
        let mut bill: i128 = 0;
        for (i, [from, percent]) in brackets.iter().enumerate() {
            let to = brackets.get(i + 1).map_or(i64::MAX, |b| b[0]);
            let part = balance.min(to) - from;
            if part > 0 {
                bill += i128::from(part) * i128::from((*percent).clamp(0, 100)) / 100;
            }
        }
        bill as i64
    }

    /// What `uuid` owes in land tax.
    pub fn arrears(&self, uuid: u128) -> i64 {
        self.state
            .arrears
            .get(&uuid_string(uuid))
            .copied()
            .unwrap_or(0)
    }

    /// Whether `uuid` may claim more land: not while they owe land tax.
    pub fn may_claim(&self, economy: &Economy, uuid: u128) -> Result<(), String> {
        match self.arrears(uuid) {
            0 => Ok(()),
            owed => Err(format!(
                "You owe {} of land tax: no new claim until it's paid (/eco taxes pay).",
                economy.money(owed)
            )),
        }
    }

    /// Whether a weekly collection is due (cheap: once a second).
    pub fn due(&self, now: i64) -> bool {
        self.config.enabled && week(now) > self.state.week
    }

    /// Collects the week's land and wealth taxes into the Treasury. `chunks`:
    /// claimed chunks by owner. Unpaid land tax becomes arrears; the wealth
    /// tax takes only what is there (it never exceeds the balance).
    pub fn collect(
        &mut self,
        economy: &mut Economy,
        chunks: &HashMap<u128, usize>,
        now: i64,
    ) -> Vec<Out> {
        let this = week(now);
        if this <= self.state.week {
            return Vec::new();
        }
        self.state.week = this;
        self.dirty = true;
        let mut c = Collection {
            week: this,
            ..Collection::default()
        };
        let mut out = Vec::new();
        let mut owners: Vec<(u128, usize)> = chunks.iter().map(|(u, n)| (*u, *n)).collect();
        owners.sort();
        for (uuid, n) in owners {
            let key = uuid_string(uuid);
            let owed = self.land_bill(n) + self.arrears(uuid);
            if owed <= 0 {
                continue;
            }
            let paid = owed.min(economy.balance(uuid));
            if paid > 0
                && economy
                    .to_pool("land_tax", uuid, TREASURY, paid, "")
                    .is_err()
            {
                continue;
            }
            c.land += paid;
            c.payers += 1;
            let left = owed - paid;
            if left > 0 {
                c.unpaid += left;
                self.state.arrears.insert(key, left);
                out.push(Out::Tell(
                    uuid,
                    format!(
                        "Land tax: you paid {} and still owe {}. No new claim until it's paid (/eco taxes pay).",
                        economy.money(paid),
                        economy.money(left)
                    ),
                ));
            } else {
                self.state.arrears.remove(&key);
                out.push(Out::Tell(
                    uuid,
                    format!("Land tax for {n} chunks: {}.", economy.money(paid)),
                ));
            }
        }
        for (uuid, _, balance) in economy.accounts() {
            let saved = self.savings.get(&uuid).copied().unwrap_or(0);
            let bill = self.wealth_bill(balance.saturating_add(saved)).min(balance);
            if bill > 0
                && economy
                    .to_pool("wealth_tax", uuid, TREASURY, bill, "")
                    .is_ok()
            {
                c.wealth += bill;
                out.push(Out::Tell(
                    uuid,
                    format!("Wealth tax this week: {}.", economy.money(bill)),
                ));
            }
        }
        out.push(Out::All(format!(
            "Weekly taxes collected: land {}, wealth {}. Treasury: {}.",
            economy.money(c.land),
            economy.money(c.wealth),
            economy.money(economy.pool(TREASURY))
        )));
        self.state.last = Some(c);
        out
    }

    /// Runs `/treasury ...` and `/eco taxes ...`; None for another command.
    /// `my_chunks`: the chunks `me` holds.
    pub fn command(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        my_chunks: usize,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        Some(match words.as_slice() {
            ["treasury"] => self.treasury(economy),
            ["treasury", "log"] => self.log(economy),
            ["treasury", "pay", who, amount, reason @ ..] => {
                self.pay(economy, me, who, amount, &reason.join(" "))
            }
            ["treasury", ..] => vec![Out::Error(
                "Usage: /treasury, /treasury log, /treasury pay <player> <amount> <reason>".into(),
            )],
            ["eco" | "taxes", "taxes"] | ["taxes"] => self.explain(economy, me, my_chunks),
            ["eco" | "taxes", "taxes", "pay"] | ["taxes", "pay"] => self.pay_arrears(economy, me),
            _ => return None,
        })
    }

    fn treasury(&self, economy: &Economy) -> Vec<Out> {
        let t = economy.treasury();
        let list = |m: &BTreeMap<String, i64>| -> String {
            let mut v: Vec<(&String, &i64)> = m.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let parts: Vec<String> = v
                .iter()
                .map(|(k, a)| format!("{} {}", k.replace('_', " "), economy.money(**a)))
                .collect();
            if parts.is_empty() {
                "nothing".into()
            } else {
                parts.join(", ")
            }
        };
        let today = t.day == economy.now.div_euclid(DAY_MS);
        let empty = BTreeMap::new();
        let (inc, out) = if today {
            (&t.income_today, &t.spent_today)
        } else {
            (&empty, &empty)
        };
        let mut lines = vec![
            Out::Reply(format!(
                "Treasury: {} (insurance fund {}). Taxes, fees and house edges go in; grants, refunds and prizes come out. Nobody can make money.",
                economy.money(economy.pool(TREASURY)),
                economy.money(economy.pool(INSURANCE))
            )),
            Out::Reply(format!("Today in: {}. Out: {}.", list(inc), list(out))),
            Out::Reply(format!(
                "Since it opened, in: {}. Out: {}.",
                list(&t.income),
                list(&t.spent)
            )),
        ];
        if let Some(c) = &self.state.last {
            lines.push(Out::Reply(format!(
                "Last weekly taxes: land {}, wealth {}, {} unpaid.",
                economy.money(c.land),
                economy.money(c.wealth),
                economy.money(c.unpaid)
            )));
        }
        lines.push(Out::Reply(
            "Every payment: /treasury log. The rates: /eco taxes.".into(),
        ));
        lines
    }

    fn log(&self, economy: &Economy) -> Vec<Out> {
        let t = economy.treasury();
        if t.log.is_empty() {
            return vec![Out::Reply("The Treasury hasn't moved yet.".into())];
        }
        let mut out = vec![Out::Reply(
            "The Treasury's last movements (newest first):".into(),
        )];
        for m in t.log.iter().rev().take(10) {
            let sign = if m.amount >= 0 { "+" } else { "-" };
            let mut line = format!(
                "{} ago: {sign}{} {}",
                span(economy.now - m.time),
                economy.money(m.amount.abs()),
                m.kind.replace('_', " ")
            );
            if !m.player.is_empty() {
                line.push_str(&format!(" ({})", m.player));
            }
            if !m.reason.is_empty() {
                line.push_str(&format!(": {}", m.reason));
            }
            if !m.by.is_empty() {
                line.push_str(&format!(", by {}", m.by));
            }
            out.push(Out::Reply(line));
        }
        out
    }

    /// `/treasury pay`: an operator pays a player from the Treasury, with a
    /// reason everyone sees.
    fn pay(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        who: &str,
        amount: &str,
        reason: &str,
    ) -> Vec<Out> {
        if me.level < 3 {
            return vec![Out::Error(
                "Only operators of level 3 may pay from the Treasury.".into(),
            )];
        }
        let Some(amount) = parse_amount(amount) else {
            return vec![Out::Error(
                "The amount must be a positive whole number.".into(),
            )];
        };
        let reason = reason.trim();
        if reason.chars().count() < 3 {
            return vec![Out::Error(
                "Say why: /treasury pay <player> <amount> <reason> (an event prize, a bug refund...)".into(),
            )];
        }
        let Some((uuid, account)) = economy.find(who) else {
            return vec![Out::Error(format!("No player named {who} has an account."))];
        };
        let name = account.name.clone();
        match economy.out_of_pool(
            "grant",
            TREASURY,
            uuid,
            amount,
            reason,
            &uuid_string(me.uuid),
        ) {
            Ok(()) => vec![Out::All(format!(
                "The Treasury paid {} to {name}: {reason} (by {}). Treasury: {}.",
                economy.money(amount),
                me.name,
                economy.money(economy.pool(TREASURY))
            ))],
            Err(e) => vec![Out::Error(e)],
        }
    }

    /// `/eco taxes`: the tax code, and the player's own bill.
    fn explain(&self, economy: &Economy, me: &Player, chunks: usize) -> Vec<Out> {
        let c = &self.config;
        let mut brackets = c.wealth_brackets.clone();
        brackets.sort();
        let wealth: Vec<String> = brackets
            .iter()
            .map(|[from, p]| format!("{p}% above {}", economy.money(*from)))
            .collect();
        let next = week_start(self.state.week + 1) - economy.now;
        let land = self.land_bill(chunks);
        let saved = self.savings.get(&me.uuid).copied().unwrap_or(0);
        let wealth_bill = self.wealth_bill(economy.balance(me.uuid).saturating_add(saved));
        let mut out = vec![
            Out::Reply("Taxes: all of them go to the Treasury (/treasury), which pays grants, prizes and refunds.".into()),
            Out::Reply(format!(
                "VAT: {}% of each auction house sale, paid by the seller.",
                c.vat_percent
            )),
            Out::Reply(format!(
                "Land tax, weekly: {} a claimed chunk past your first {}. Unpaid tax stops new claims.",
                economy.money(c.land_tax_per_chunk),
                c.land_free_chunks
            )),
            Out::Reply(format!("Wealth tax, weekly, on your balance and bank savings: {}.", wealth.join(", "))),
            Out::Reply("Games of chance: the house's fee goes to the Treasury.".into()),
        ];
        if !c.enabled {
            out.push(Out::Reply(
                "The weekly taxes are off on this server.".into(),
            ));
        } else {
            out.push(Out::Reply(format!(
                "Your bill on Monday (in {}): land {} ({chunks} chunks), wealth {}{}.",
                span(next),
                economy.money(land),
                economy.money(wealth_bill),
                match self.arrears(me.uuid) {
                    0 => String::new(),
                    a => format!(", owed {}", economy.money(a)),
                }
            )));
        }
        out
    }

    /// `/eco taxes pay`: pays the land tax owed, as much as the balance
    /// allows.
    fn pay_arrears(&mut self, economy: &mut Economy, me: &Player) -> Vec<Out> {
        let owed = self.arrears(me.uuid);
        if owed == 0 {
            return vec![Out::Reply("You owe no tax.".into())];
        }
        let paid = owed.min(economy.balance(me.uuid));
        if paid <= 0 {
            return vec![Out::Error(format!(
                "You owe {} and have nothing to pay it with.",
                economy.money(owed)
            ))];
        }
        if let Err(e) = economy.to_pool("land_tax", me.uuid, TREASURY, paid, "arrears") {
            return vec![Out::Error(e)];
        }
        self.dirty = true;
        let key = uuid_string(me.uuid);
        if paid == owed {
            self.state.arrears.remove(&key);
            vec![Out::Reply(format!(
                "Paid {} of land tax: you owe nothing.",
                economy.money(paid)
            ))]
        } else {
            self.state.arrears.insert(key, owed - paid);
            vec![Out::Reply(format!(
                "Paid {}; you still owe {}.",
                economy.money(paid),
                economy.money(owed - paid)
            ))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config as EcoConfig;
    use std::fs;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;
    const RICH: u128 = 3;
    /// Monday 2026-11-02 00:00 UTC.
    const MONDAY: i64 = 1_793_577_600_000;

    fn setup(name: &str) -> (Economy, Taxes, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-tax-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        e.now = MONDAY - 3_600_000;
        for (u, n) in [(ALEX, "Alex"), (STEVE, "Steve"), (RICH, "Rich")] {
            e.seen(u, n);
        }
        e.adjust("sell", ALEX, 1_000, "").unwrap();
        e.adjust("sell", STEVE, 20, "").unwrap();
        e.adjust("sell", RICH, 300_000, "").unwrap();
        let t = Taxes::open(&e, Config::default());
        (e, t, dir)
    }

    fn me(uuid: u128, level: u8) -> Player {
        Player {
            uuid,
            name: "x".into(),
            level,
        }
    }

    #[test]
    fn weeks_start_on_monday() {
        assert_eq!(week(MONDAY) - week(MONDAY - 1), 1);
        assert_eq!(week_start(week(MONDAY)), MONDAY);
        assert_eq!(week(MONDAY + 6 * DAY_MS), week(MONDAY));
    }

    #[test]
    fn bills_are_progressive() {
        let t = Taxes {
            config: Config::default(),
            state: State::default(),
            dirty: false,
            savings: HashMap::new(),
        };
        assert_eq!(t.land_bill(4), 0);
        assert_eq!(t.land_bill(16), 60);
        assert_eq!(t.wealth_bill(50_000), 0);
        assert_eq!(t.wealth_bill(60_000), 100);
        // 1% of 200k + 2% of 50k.
        assert_eq!(t.wealth_bill(300_000), 3_000);
        // 1% of 200k + 2% of 750k + 3% of 1M.
        assert_eq!(t.wealth_bill(2_000_000), 2_000 + 15_000 + 30_000);
    }

    #[test]
    fn bank_savings_count_for_the_wealth_tax() {
        let (mut e, mut t, _dir) = setup("savings");
        // Alex: 1,000 in the wallet, 59,000 saved: taxed on 60,000.
        t.savings.insert(ALEX, 59_000);
        let now = MONDAY;
        t.collect(&mut e, &HashMap::new(), now);
        assert_eq!(e.balance(ALEX), 900);
    }

    #[test]
    fn monday_collects_once_into_the_treasury_and_unpaid_land_tax_blocks_claims() {
        let (mut e, mut t, dir) = setup("collect");
        let chunks = HashMap::from([(ALEX, 10usize), (STEVE, 14), (RICH, 2)]);
        assert!(!t.due(e.now), "not before Monday");
        assert!(
            {
                let now = e.now;
                t.collect(&mut e, &chunks, now)
            }
            .is_empty()
        );
        let total =
            |e: &Economy| e.balance(ALEX) + e.balance(STEVE) + e.balance(RICH) + e.pool(TREASURY);
        let before = total(&e);
        e.now = MONDAY + 1000;
        assert!(t.due(e.now));
        let out = {
            let now = e.now;
            t.collect(&mut e, &chunks, now)
        };
        assert!(matches!(out.last(), Some(Out::All(_))));
        // Alex 6 chunks taxed (30), Steve 10 (50, has 20), Rich wealth 3,000.
        assert_eq!(
            (e.balance(ALEX), e.balance(STEVE), e.balance(RICH)),
            (970, 0, 297_000)
        );
        assert_eq!(e.pool(TREASURY), 30 + 20 + 3_000);
        assert_eq!(total(&e), before, "taxes move money, never make or burn it");
        assert_eq!(t.arrears(STEVE), 30);
        assert!(t.may_claim(&e, STEVE).is_err() && t.may_claim(&e, ALEX).is_ok());
        // Twice the same week: nothing.
        assert!(!t.due(e.now));
        assert!(
            {
                let now = e.now + 1000;
                t.collect(&mut e, &chunks, now)
            }
            .is_empty()
        );
        // Steve earns and pays what he owes.
        e.adjust("sell", STEVE, 100, "").unwrap();
        let out = t
            .command(&mut e, &me(STEVE, 0), 14, "eco taxes pay")
            .unwrap();
        assert_eq!(
            out,
            [Out::Reply(
                "Paid 30 coins of land tax: you owe nothing.".into()
            )]
        );
        assert!(t.may_claim(&e, STEVE).is_ok());
        let tr = e.treasury();
        assert_eq!(
            (tr.income["land_tax"], tr.income["wealth_tax"]),
            (80, 3_000)
        );
        // The state survives a restart with the balances.
        t.store(&mut e);
        e.save().unwrap();
        let mut again = Economy::open(&dir, EcoConfig::default()).unwrap();
        again.now = e.now;
        let t2 = Taxes::open(&again, Config::default());
        assert_eq!(t2.state.week, week(MONDAY));
        assert!(!t2.due(again.now), "a restart doesn't collect again");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn only_operators_pay_from_the_treasury_and_never_more_than_it_holds() {
        let (mut e, mut t, dir) = setup("pay");
        e.to_pool("vat", RICH, TREASURY, 500, "").unwrap();
        let steve = me(STEVE, 0);
        assert!(matches!(
            &t.command(&mut e, &steve, 0, "treasury pay Steve 10 because")
                .unwrap()[0],
            Out::Error(_)
        ));
        let op = Player {
            uuid: ALEX,
            name: "Alex".into(),
            level: 3,
        };
        assert!(
            matches!(&t.command(&mut e, &op, 0, "treasury pay Steve 10").unwrap()[0], Out::Error(s) if s.starts_with("Say why"))
        );
        assert!(matches!(
            &t.command(&mut e, &op, 0, "treasury pay Steve 501 too much")
                .unwrap()[0],
            Out::Error(_)
        ));
        assert_eq!(
            t.command(&mut e, &op, 0, "treasury pay Steve 200 bug refund #12").unwrap(),
            [Out::All("The Treasury paid 200 coins to Steve: bug refund #12 (by Alex). Treasury: 300 coins.".into())]
        );
        assert_eq!(e.balance(STEVE), 220);
        let log = t.command(&mut e, &steve, 0, "treasury log").unwrap();
        assert!(
            matches!(&log[1], Out::Reply(s) if s.contains("-200 coins grant (Steve): bug refund #12, by Alex")),
            "{log:?}"
        );
        let shown = t.command(&mut e, &steve, 0, "treasury").unwrap();
        assert!(matches!(&shown[0], Out::Reply(s) if s.starts_with("Treasury: 300 coins")));
        let taxes = t.command(&mut e, &steve, 16, "eco taxes").unwrap();
        assert!(
            taxes
                .iter()
                .any(|o| matches!(o, Out::Reply(s) if s.contains("land 60 coins (16 chunks)"))),
            "{taxes:?}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
