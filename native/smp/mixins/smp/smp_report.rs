// The economy in the open: `/eco stats` (the money supply and where it
// sits, what the server made and destroyed today, the Treasury's day, how
// evenly money is spread), a report to everyone at 00:00 UTC on the day
// before, and `web/economy.json` for the site's /hub (written once a minute,
// off the tick thread, by the glue). One snapshot a day is kept (60 days) in
// `economy.json` (`ext.report`) for the curves. Reading only: this part
// moves no money. No game types. Added to the game as `game::smp_report`;
// also a module of this crate, for its tests.

use super::smp_economy::{DAY_MS, Economy, INSURANCE, Out, TREASURY};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

/// `config.json`'s `report`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The 00:00 UTC report in everyone's chat.
    pub daily_report: bool,
    /// `web/economy.json` once a minute.
    pub web_json: bool,
    pub history_days: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daily_report: true,
            web_json: true,
            history_days: 60,
        }
    }
}

/// One day, as the report keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Day {
    /// Days since 1970 (UTC).
    pub day: i64,
    /// All the money there is, at the end of the day.
    pub supply: i64,
    /// Players' money: wallets, savings, money held (bids, stakes, margins).
    pub players: i64,
    pub treasury: i64,
    pub insurance: i64,
    /// Made and destroyed by the server that day.
    pub minted: i64,
    pub burned: i64,
    /// The Treasury's income and spending that day.
    pub taxes: i64,
    pub spent: i64,
    /// Over players with money: Gini (0 to 1, in thousandths) and the
    /// richest ten's share (percent).
    pub gini_milli: i64,
    pub top10_percent: i64,
}

/// `ext.report`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub days: Vec<Day>,
    /// The last day reported.
    pub day: i64,
}

pub struct Report {
    pub config: Config,
    pub state: State,
    dirty: bool,
    minute: i64,
}

/// Pools that are the state's, not players' money.
const STATE_POOLS: [&str; 3] = [TREASURY, INSURANCE, "casino"];

/// Gini (thousandths) and the top ten's share (percent) of `wealth`.
pub fn spread(mut wealth: Vec<i64>) -> (i64, i64) {
    wealth.retain(|w| *w > 0);
    wealth.sort_unstable();
    let n = wealth.len() as i128;
    let total: i128 = wealth.iter().map(|w| i128::from(*w)).sum();
    if total == 0 {
        return (0, 0);
    }
    let weighted: i128 = wealth
        .iter()
        .enumerate()
        .map(|(i, w)| (2 * i as i128 - n + 1) * i128::from(*w))
        .sum();
    let top: i128 = wealth.iter().rev().take(10).map(|w| i128::from(*w)).sum();
    (
        (weighted * 1000 / (n * total)) as i64,
        (top * 100 / total) as i64,
    )
}

fn sum(m: &BTreeMap<String, i64>) -> i64 {
    m.values().sum()
}

/// `market sale 60,000 coins, daily reward 20,000 coins` (largest first).
fn list(e: &Economy, m: &BTreeMap<String, i64>, n: usize) -> String {
    let mut v: Vec<(&String, &i64)> = m.iter().filter(|(_, a)| **a > 0).collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let parts: Vec<String> = v
        .iter()
        .take(n)
        .map(|(k, a)| format!("{} {}", label(k), e.money(**a)))
        .collect();
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join(", ")
    }
}

fn label(kind: &str) -> String {
    match kind {
        "sell" => "market sales".into(),
        "buy" => "market purchases".into(),
        k => k.replace('_', " "),
    }
}

impl Report {
    pub fn open(economy: &Economy, config: Config) -> Self {
        let mut state = economy
            .ext("report")
            .and_then(|v| serde_json::from_value::<State>(v.clone()).ok())
            .unwrap_or_default();
        if state.day == 0 {
            state.day = economy.now.div_euclid(DAY_MS);
        }
        Self {
            config,
            state,
            dirty: false,
            minute: 0,
        }
    }

    pub fn store(&mut self, economy: &mut Economy) {
        if self.dirty {
            economy.set_ext(
                "report",
                serde_json::to_value(&self.state).unwrap_or_default(),
            );
            self.dirty = false;
        }
    }

    /// The day's numbers now. `savings`: what each player has in the bank.
    pub fn snapshot(economy: &mut Economy, savings: &HashMap<u128, i64>, day: i64) -> Day {
        let accounts = economy.accounts();
        let wallets: i64 = accounts.iter().map(|a| a.2).sum();
        let pools: i64 = economy.pools().values().sum();
        let state: i64 = STATE_POOLS.iter().map(|p| economy.pool(p)).sum();
        let supply = wallets + economy.held_total() + pools;
        let mut wealth: HashMap<u128, i64> = accounts.iter().map(|a| (a.0, a.2)).collect();
        for (u, s) in savings {
            *wealth.entry(*u).or_default() += s;
        }
        let (gini_milli, top10_percent) = spread(wealth.into_values().collect());
        let today = economy.now.div_euclid(DAY_MS);
        let (minted, burned, taxes, spent) = {
            let t = economy.treasury();
            let (inc, out) = if t.day == day {
                (sum(&t.income_today), sum(&t.spent_today))
            } else if t.day == day + 1 {
                (sum(&t.income_yesterday), sum(&t.spent_yesterday))
            } else {
                (0, 0)
            };
            let f = economy.flows();
            let (m, b) = if day == today {
                (sum(&f.minted), sum(&f.burned))
            } else if day + 1 == today {
                (sum(&f.minted_yesterday), sum(&f.burned_yesterday))
            } else {
                (0, 0)
            };
            (m, b, inc, out)
        };
        Day {
            day,
            supply,
            players: supply - state,
            treasury: economy.pool(TREASURY),
            insurance: economy.pool(INSURANCE),
            minted,
            burned,
            taxes,
            spent,
            gini_milli,
            top10_percent,
        }
    }

    /// Runs `/eco stats` (or `/money stats`); None for another command.
    pub fn command(
        &self,
        economy: &mut Economy,
        savings: &HashMap<u128, i64>,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        if !matches!(words.as_slice(), ["eco" | "money", "stats"]) {
            return None;
        }
        let today = economy.now.div_euclid(DAY_MS);
        let d = Self::snapshot(economy, savings, today);
        let since = self.state.days.last().map(|y| d.supply - y.supply);
        let pools: Vec<String> = economy
            .pools()
            .iter()
            .filter(|(k, v)| **v > 0 && !STATE_POOLS.contains(&k.as_str()))
            .map(|(k, v)| format!("{} {}", k.replace('_', " "), economy.money(*v)))
            .collect();
        let flows = economy.flows().clone();
        let t = economy.treasury().clone();
        let (inc, out) = if t.day == today {
            (t.income_today, t.spent_today)
        } else {
            Default::default()
        };
        let mut lines = vec![
            Out::Reply(format!(
                "Money supply: {}{}. Players hold {} (of it in pools: {}), the Treasury {}, the insurance fund {}.",
                economy.money(d.supply),
                since.map_or(String::new(), |s| format!(
                    " ({}{} since midnight)",
                    if s >= 0 { "+" } else { "-" },
                    economy.money(s.abs())
                )),
                economy.money(d.players),
                if pools.is_empty() {
                    "none".into()
                } else {
                    pools.join(", ")
                },
                economy.money(d.treasury),
                economy.money(d.insurance)
            )),
            Out::Reply(format!(
                "Today the server made {} ({}) and destroyed {} ({}).",
                economy.money(sum(&flows.minted)),
                list(economy, &flows.minted, 3),
                economy.money(sum(&flows.burned)),
                list(economy, &flows.burned, 3)
            )),
            Out::Reply(format!(
                "Taxes and fees today: {} ({}). The Treasury paid out {}.",
                economy.money(sum(&inc)),
                list(economy, &inc, 4),
                economy.money(sum(&out))
            )),
            Out::Reply(format!(
                "Spread: Gini {}.{:03} (0 equal, 1 one player has all), the richest ten hold {}%.",
                d.gini_milli / 1000,
                d.gini_milli % 1000,
                d.top10_percent
            )),
        ];
        lines.push(Out::Reply(
            "The Treasury: /treasury. The taxes: /eco taxes.".into(),
        ));
        Some(lines)
    }

    /// Whether `tick` has work at `now` (a new minute or day): the glue
    /// gathers the savings only then.
    pub fn due(&self, now: i64) -> bool {
        now.div_euclid(60_000) != self.minute || now.div_euclid(DAY_MS) > self.state.day
    }

    /// Once a second from the glue. At the day's turn: the day before is
    /// kept and, if on, told to everyone. Returns the lines, and whether
    /// `web/economy.json` is due (once a minute).
    pub fn tick(
        &mut self,
        economy: &mut Economy,
        savings: &HashMap<u128, i64>,
        now: i64,
    ) -> (Vec<Out>, bool) {
        let minute = now.div_euclid(60_000);
        let web = self.config.web_json && minute != self.minute;
        self.minute = minute;
        let today = now.div_euclid(DAY_MS);
        if today <= self.state.day {
            return (Vec::new(), web);
        }
        let yesterday = today - 1;
        self.state.day = today;
        self.dirty = true;
        // The supply now is yesterday's end, give or take a second.
        let d = Self::snapshot(economy, savings, yesterday);
        let before = self
            .state
            .days
            .last()
            .filter(|p| p.day + 1 == yesterday)
            .map(|p| p.supply);
        self.state.days.push(d.clone());
        let keep = self.config.history_days.max(1);
        if self.state.days.len() > keep {
            let extra = self.state.days.len() - keep;
            self.state.days.drain(..extra);
        }
        let mut out = Vec::new();
        if self.config.daily_report {
            out.push(Out::All(self.daily_line(economy, &d, before)));
        }
        (out, web)
    }

    fn daily_line(&self, economy: &Economy, d: &Day, before: Option<i64>) -> String {
        format!(
            "Economy yesterday: money supply {}{}; the server made {} and destroyed {}; taxes and fees {} to the Treasury (now {}), which paid out {}. /eco stats",
            economy.money(d.supply),
            before.map_or(String::new(), |b| {
                let c = d.supply - b;
                format!(
                    " ({}{})",
                    if c >= 0 { "+" } else { "-" },
                    economy.money(c.abs())
                )
            }),
            economy.money(d.minted),
            economy.money(d.burned),
            economy.money(d.taxes),
            economy.money(d.treasury),
            economy.money(d.spent)
        )
    }

    /// `web/economy.json`: aggregates, the Treasury's every movement (public
    /// in game too), the tax code, the days kept.
    pub fn web(
        &self,
        economy: &mut Economy,
        savings: &HashMap<u128, i64>,
        taxes: Value,
        now: i64,
    ) -> Value {
        let today = now.div_euclid(DAY_MS);
        let d = Self::snapshot(economy, savings, today);
        let flows = economy.flows().clone();
        let t = economy.treasury().clone();
        let fresh = t.day == today;
        let pools: BTreeMap<String, i64> = economy.pools().clone();
        let daily = self.state.days.last().map(|y| {
            self.daily_line(
                economy,
                y,
                self.state
                    .days
                    .iter()
                    .rev()
                    .nth(1)
                    .filter(|p| p.day + 1 == y.day)
                    .map(|p| p.supply),
            )
        });
        json!({
            "schema": 1,
            "generated": now,
            "currency": economy.config.currency,
            "today": d,
            "pools": pools,
            "minted_today": flows.minted,
            "burned_today": flows.burned,
            "treasury": {
                "balance": economy.pool(TREASURY),
                "insurance": economy.pool(INSURANCE),
                "income": t.income,
                "spent": t.spent,
                "income_today": if fresh { t.income_today } else { BTreeMap::new() },
                "spent_today": if fresh { t.spent_today } else { BTreeMap::new() },
                "log": t.log,
            },
            "taxes": taxes,
            "days": self.state.days,
            "daily_report": daily,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config as EcoConfig;

    const A: u128 = 1;
    const B: u128 = 2;
    const T0: i64 = 1_793_577_600_000; // a midnight, UTC

    fn setup(name: &str) -> (Economy, Report) {
        let dir = std::env::temp_dir().join(format!("smp-report-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        e.now = T0 + 1000;
        e.seen(A, "Alex");
        e.seen(B, "Bo");
        let r = Report::open(&e, Config::default());
        (e, r)
    }

    #[test]
    fn spread_measures_inequality() {
        assert_eq!(spread(vec![10, 10, 10, 10]), (0, 100));
        let (g, _) = spread(vec![0, 0, 0, 100]);
        assert_eq!(g, 0, "only players with money count");
        let (g, top) = spread((1..=20).map(|i| if i == 20 { 1_000 } else { 1 }).collect());
        assert!(g > 900, "{g}");
        assert_eq!(top, 99);
    }

    #[test]
    fn stats_show_what_was_made_destroyed_and_taxed_today() {
        let (mut e, r) = setup("stats");
        e.adjust("sell 20 minecraft:wheat", A, 600, "").unwrap();
        e.adjust("daily_reward", B, 50, "").unwrap();
        e.adjust("buy 5 minecraft:torch", A, -100, "").unwrap();
        e.to_pool("vat", A, TREASURY, 25, "").unwrap();
        let out = r
            .command(&mut e, &HashMap::from([(B, 0)]), "eco stats")
            .unwrap();
        let text: Vec<String> = out
            .iter()
            .map(|o| match o {
                Out::Reply(s) => s.clone(),
                o => panic!("{o:?}"),
            })
            .collect();
        assert!(
            text[0].starts_with("Money supply: 550 coins. Players hold 525 coins"),
            "{}",
            text[0]
        );
        assert_eq!(
            text[1],
            "Today the server made 650 coins (market sales 600 coins, daily reward 50 coins) and destroyed 100 coins (market purchases 100 coins)."
        );
        assert!(
            text[2].starts_with("Taxes and fees today: 25 coins (vat 25 coins)"),
            "{}",
            text[2]
        );
        assert!(r.command(&mut e, &HashMap::new(), "eco give A 5").is_none());
    }

    #[test]
    fn midnight_reports_the_day_before_once_and_keeps_it_through_a_restart() {
        let (mut e, mut r) = setup("daily");
        e.adjust("sell 1 minecraft:diamond", A, 100, "").unwrap();
        e.to_pool("vat", A, TREASURY, 5, "").unwrap();
        let (out, web) = r.tick(&mut e, &HashMap::new(), T0 + 2000);
        assert!(out.is_empty() && web, "same day; the JSON's first minute");
        assert!(
            !r.tick(&mut e, &HashMap::new(), T0 + 3000).1,
            "once a minute"
        );
        e.now = T0 + DAY_MS + 500;
        let now = e.now;
        let (out, _) = r.tick(&mut e, &HashMap::new(), now);
        assert_eq!(
            out,
            [Out::All(
                "Economy yesterday: money supply 100 coins; the server made 100 coins and destroyed 0 coins; taxes and fees 5 coins to the Treasury (now 5 coins), which paid out 0 coins. /eco stats"
                    .into()
            )]
        );
        assert!(
            r.tick(&mut e, &HashMap::new(), now + 1000).0.is_empty(),
            "once"
        );
        let json = r.web(&mut e, &HashMap::new(), json!({}), now);
        assert_eq!(json["schema"], 1);
        assert_eq!(json["days"][0]["minted"], 100);
        assert_eq!(json["treasury"]["balance"], 5);
        assert!(
            json["daily_report"]
                .as_str()
                .unwrap()
                .starts_with("Economy yesterday")
        );
        r.store(&mut e);
        e.save().unwrap();
        let dir = std::env::temp_dir().join(format!("smp-report-daily-{}", std::process::id()));
        let e2 = Economy::open(&dir, EcoConfig::default()).unwrap();
        let r2 = Report::open(&e2, Config::default());
        assert_eq!(r2.state.days.len(), 1);
        assert_eq!(r2.state.day, (T0 + DAY_MS).div_euclid(DAY_MS));
    }
}
