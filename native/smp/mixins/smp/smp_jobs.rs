// The contracts board (`/jobs`): each day at 00:00 UTC the state posts a few
// contracts ("deliver 64 wheat for 160 coins"), paid from the Treasury: the
// taxes go back to players who gather what the server asks for, never new
// money. A delivery takes plain items from the player's inventory (they
// leave the game: a sink for items) and pays at once. Never AFK income: only
// a command with the items in hand pays; each contract has a few deliveries
// for the whole server, and a player makes at most `max_per_player` a day.
//
// Prices: each item pays its market sell price plus `premium_percent`, and
// never more than `ask_percent` of what the market asks for it (checked
// again at delivery, as the market's prices move): buying from the market to
// deliver never pays. One day's contracts together cost at most
// `budget_percent` of the Treasury when posted. The board lives in
// `economy.json` (`ext.jobs`), written with the money. No game types; the
// market's prices come through a closure. Added to the game as
// `game::smp_jobs`; also a module of this crate, for its tests.

use super::smp_economy::{DAY_MS, Economy, Out, Player, TREASURY};
use super::smp_market::{Bag, item_name};
use super::smp_store::uuid_string;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `config.json`'s `jobs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub contracts_per_day: usize,
    /// Items in one delivery.
    pub batch: u32,
    /// Deliveries of one contract for the whole server.
    pub deliveries: u32,
    /// Deliveries one player makes in a day, all contracts.
    pub max_per_player: u32,
    /// Over the market's sell price.
    pub premium_percent: i64,
    /// Of the market's ask (its buy price), at most.
    pub ask_percent: i64,
    /// The day's contracts cost at most this share of the Treasury.
    pub budget_percent: i64,
    /// What contracts may ask for (items the market buys).
    pub items: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            contracts_per_day: 4,
            batch: 64,
            deliveries: 8,
            max_per_player: 2,
            premium_percent: 30,
            ask_percent: 90,
            budget_percent: 10,
            items: [
                "wheat",
                "carrot",
                "potato",
                "beetroot",
                "sugar_cane",
                "pumpkin",
                "melon_slice",
                "cactus",
                "oak_log",
                "spruce_log",
                "birch_log",
                "cobblestone",
                "coal",
                "raw_iron",
                "raw_copper",
                "redstone",
                "lapis_lazuli",
                "leather",
                "string",
                "bone",
                "feather",
                "gunpowder",
                "kelp",
                "bamboo",
            ]
            .map(|i| format!("minecraft:{i}"))
            .into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contract {
    pub item: String,
    pub count: u32,
    /// Coins a delivery pays (at most; less if the market's ask fell).
    pub pay: i64,
    pub left: u32,
}

/// `ext.jobs`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Board {
    pub day: i64,
    pub contracts: Vec<Contract>,
    /// Deliveries today, by player.
    pub done: BTreeMap<String, u32>,
    pub paid_today: i64,
    pub paid: i64,
}

pub struct Jobs {
    pub config: Config,
    pub board: Board,
    dirty: bool,
}

/// `(sell, buy)`: what the market pays for one item and what it asks; None
/// when it doesn't trade it.
pub type PriceOf<'a> = &'a dyn Fn(&str) -> Option<(i64, i64)>;

/// A number from `day` and `i`, the same on every run (no world random).
fn pick(day: i64, i: u64) -> u64 {
    let mut x =
        (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ i.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 29)
}

impl Jobs {
    pub fn open(economy: &Economy, config: Config) -> Self {
        let board = economy
            .ext("jobs")
            .and_then(|v| serde_json::from_value::<Board>(v.clone()).ok())
            .unwrap_or_default();
        Self {
            config,
            board,
            dirty: false,
        }
    }

    pub fn store(&mut self, economy: &mut Economy) {
        if self.dirty {
            economy.set_ext(
                "jobs",
                serde_json::to_value(&self.board).unwrap_or_default(),
            );
            self.dirty = false;
        }
    }

    /// One item's pay: the sell price plus the premium, below the ask.
    fn unit(&self, sell: i64, buy: i64) -> i64 {
        let mut unit = sell * (100 + self.config.premium_percent) / 100;
        if buy > 0 {
            unit = unit.min(buy * self.config.ask_percent / 100);
        }
        unit.max(0)
    }

    /// Once a second from the glue; at 00:00 UTC, the day's contracts.
    pub fn tick(&mut self, economy: &Economy, price: PriceOf, now: i64) -> Vec<Out> {
        let day = now.div_euclid(DAY_MS);
        if day == self.board.day || !self.config.enabled {
            return Vec::new();
        }
        self.board.day = day;
        self.board.done.clear();
        self.board.paid_today = 0;
        self.board.contracts.clear();
        self.dirty = true;
        let mut pool: Vec<(String, i64)> = self
            .config
            .items
            .iter()
            .filter_map(|i| {
                let (sell, buy) = price(i)?;
                let unit = self.unit(sell, buy);
                (sell > 0 && unit > 0).then(|| (i.clone(), unit))
            })
            .collect();
        let mut budget =
            economy.pool(TREASURY).max(0) * self.config.budget_percent.clamp(0, 100) / 100;
        for n in 0..self.config.contracts_per_day as u64 {
            if pool.is_empty() {
                break;
            }
            let (item, unit) = pool.remove((pick(day, n) % pool.len() as u64) as usize);
            let pay = unit * i64::from(self.config.batch);
            let left = (budget / pay.max(1)).min(i64::from(self.config.deliveries)) as u32;
            if left == 0 {
                continue;
            }
            budget -= pay * i64::from(left);
            self.board.contracts.push(Contract {
                item,
                count: self.config.batch,
                pay,
                left,
            });
        }
        if self.board.contracts.is_empty() {
            return Vec::new();
        }
        vec![Out::All(format!(
            "Today's contracts are up (/jobs): {}.",
            self.board
                .contracts
                .iter()
                .map(|c| format!(
                    "{} {} for {}",
                    c.count,
                    item_name(&c.item),
                    economy.money(c.pay)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))]
    }

    /// `/jobs ...`; None for another command. `trades`: the player is in
    /// survival or adventure (a creative player could make the items).
    pub fn command(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        trades: bool,
        bag: &mut dyn Bag,
        price: PriceOf,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() != Some(&"jobs") {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error(
                "There are no contracts on this server.".into(),
            )]);
        }
        Some(match words[1..] {
            [] => self.list(economy, me),
            ["deliver", n] => match n.trim_start_matches('#').parse::<usize>() {
                Ok(n) if n >= 1 => self.deliver(economy, me, trades, bag, price, n - 1),
                _ => vec![Out::Error("Usage: /jobs deliver <contract number>".into())],
            },
            _ => vec![Out::Error(
                "Usage: /jobs, /jobs deliver <contract number>".into(),
            )],
        })
    }

    fn list(&self, economy: &Economy, me: &Player) -> Vec<Out> {
        if self.board.contracts.is_empty() {
            return vec![Out::Reply(
                "No contracts today: the Treasury has nothing to spend.".into(),
            )];
        }
        let done = self
            .board
            .done
            .get(&uuid_string(me.uuid))
            .copied()
            .unwrap_or(0);
        let mut out = vec![Out::Reply(format!(
            "Today's contracts, paid by the Treasury (you've done {done} of {} today):",
            self.config.max_per_player
        ))];
        for (i, c) in self.board.contracts.iter().enumerate() {
            out.push(Out::Reply(format!(
                "#{} deliver {} {} for {} ({} left) · /jobs deliver {}",
                i + 1,
                c.count,
                item_name(&c.item),
                economy.money(c.pay),
                c.left,
                i + 1
            )));
        }
        out
    }

    fn deliver(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        trades: bool,
        bag: &mut dyn Bag,
        price: PriceOf,
        i: usize,
    ) -> Vec<Out> {
        let err = |s: String| vec![Out::Error(s)];
        if !trades {
            return err("Contracts are for players in survival or adventure mode.".into());
        }
        let Some(c) = self.board.contracts.get(i).cloned() else {
            return err(format!("There's no contract #{} today (/jobs).", i + 1));
        };
        if c.left == 0 {
            return err(format!("Contract #{} is done for today.", i + 1));
        }
        let key = uuid_string(me.uuid);
        let done = self.board.done.get(&key).copied().unwrap_or(0);
        if done >= self.config.max_per_player {
            return err(format!(
                "You've made your {} deliveries today; new contracts at 00:00 UTC.",
                self.config.max_per_player
            ));
        }
        // The market's prices now: buying there to deliver never pays.
        let pay = match price(&c.item) {
            Some((sell, buy)) => c.pay.min(self.unit(sell, buy) * i64::from(c.count)),
            None => c.pay,
        };
        if pay <= 0 || economy.pool(TREASURY) < pay {
            return err("The Treasury can't pay that contract now.".into());
        }
        if bag.count(&c.item) < c.count {
            return err(format!(
                "You need {} plain {} in your inventory.",
                c.count,
                item_name(&c.item)
            ));
        }
        let took = bag.take(&c.item, c.count);
        if took < c.count {
            bag.give(&c.item, took);
            return err(format!(
                "You need {} plain {} in your inventory.",
                c.count,
                item_name(&c.item)
            ));
        }
        if let Err(e) = economy.from_pool(
            &format!("contract {} {}", c.count, c.item),
            TREASURY,
            me.uuid,
            pay,
            "",
        ) {
            bag.give(&c.item, took);
            return err(e);
        }
        let contract = &mut self.board.contracts[i];
        contract.left -= 1;
        self.board.done.insert(key, done + 1);
        self.board.paid_today += pay;
        self.board.paid += pay;
        self.dirty = true;
        vec![Out::Reply(format!(
            "Delivered {} {}: the Treasury paid you {}.",
            c.count,
            item_name(&c.item),
            economy.money(pay)
        ))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config as EcoConfig;
    use std::collections::HashMap;

    const A: u128 = 1;
    const B: u128 = 2;
    const T0: i64 = 1_793_577_600_000;

    struct Inv(HashMap<String, u32>);
    impl Bag for Inv {
        fn held(&self) -> Option<(String, u32, bool)> {
            None
        }
        fn carried(&self) -> Vec<(String, u32)> {
            self.0.iter().map(|(k, v)| (k.clone(), *v)).collect()
        }
        fn count(&mut self, item: &str) -> u32 {
            self.0.get(item).copied().unwrap_or(0)
        }
        fn take(&mut self, item: &str, count: u32) -> u32 {
            let have = self.0.entry(item.into()).or_default();
            let t = count.min(*have);
            *have -= t;
            t
        }
        fn room(&mut self, _: &str) -> u32 {
            u32::MAX
        }
        fn give(&mut self, item: &str, count: u32) -> u32 {
            *self.0.entry(item.into()).or_default() += count;
            count
        }
    }

    fn me(u: u128) -> Player {
        Player {
            uuid: u,
            name: format!("P{u}"),
            level: 0,
        }
    }

    /// wheat sells 2, asks 3; cobblestone sells 1, asks 4; sand sells 1,
    /// asks 1 (no room for a premium: never posted).
    fn price(id: &str) -> Option<(i64, i64)> {
        match id {
            "minecraft:wheat" => Some((2, 3)),
            "minecraft:cobblestone" => Some((1, 4)),
            "minecraft:sand" => Some((1, 1)),
            _ => None,
        }
    }

    fn setup(name: &str, treasury: i64) -> (Economy, Jobs) {
        let dir = std::env::temp_dir().join(format!("smp-jobs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        e.now = T0;
        e.seen(A, "PA");
        e.seen(B, "PB");
        if treasury > 0 {
            e.adjust("sell", B, treasury, "").unwrap();
            e.to_pool("seed", B, TREASURY, treasury, "").unwrap();
        }
        let config = Config {
            items: ["wheat", "cobblestone", "dirt", "sand"]
                .map(|i| format!("minecraft:{i}"))
                .into(),
            ..Config::default()
        };
        let j = Jobs::open(&e, config);
        (e, j)
    }

    #[test]
    fn contracts_pay_from_the_treasury_below_the_market_ask_within_the_budget() {
        let (mut e, mut j) = setup("post", 10_000);
        let out = j.tick(&e, &price, T0);
        assert_eq!(
            j.board.contracts.len(),
            2,
            "dirt isn't traded, sand leaves no premium"
        );
        let wheat = j
            .board
            .contracts
            .iter()
            .find(|c| c.item == "minecraft:wheat")
            .unwrap();
        // 2 × 1.3 = 2 (whole coins), under 90% of 3: 128 a delivery.
        assert_eq!(wheat.pay, 128);
        assert!(matches!(&out[0], Out::All(s) if s.starts_with("Today's contracts are up")));
        let cost: i64 = j
            .board
            .contracts
            .iter()
            .map(|c| c.pay * i64::from(c.left))
            .sum();
        assert!(cost <= 1_000, "{cost}");
        assert!(j.tick(&e, &price, T0 + 5_000).is_empty(), "once a day");
        let _ = &mut e;
    }

    #[test]
    fn a_delivery_takes_the_items_and_pays_once_within_the_limits() {
        let (mut e, mut j) = setup("deliver", 10_000);
        j.tick(&e, &price, T0);
        let n = j
            .board
            .contracts
            .iter()
            .position(|c| c.item == "minecraft:wheat")
            .unwrap()
            + 1;
        let mut bag = Inv(HashMap::from([("minecraft:wheat".to_owned(), 200)]));
        let line = format!("jobs deliver {n}");
        let out = j
            .command(&mut e, &me(A), false, &mut bag, &price, &line)
            .unwrap();
        assert!(
            matches!(&out[0], Out::Error(s) if s.contains("survival")),
            "creative can't"
        );
        let out = j
            .command(&mut e, &me(A), true, &mut bag, &price, &line)
            .unwrap();
        assert_eq!(
            out,
            [Out::Reply(
                "Delivered 64 wheat: the Treasury paid you 128 coins.".into()
            )]
        );
        assert_eq!(
            (e.balance(A), e.pool(TREASURY), bag.0["minecraft:wheat"]),
            (128, 9_872, 136)
        );
        j.command(&mut e, &me(A), true, &mut bag, &price, &line)
            .unwrap();
        let out = j
            .command(&mut e, &me(A), true, &mut bag, &price, &line)
            .unwrap();
        assert!(
            matches!(&out[0], Out::Error(s) if s.contains("2 deliveries today")),
            "{out:?}"
        );
        assert_eq!(bag.0["minecraft:wheat"], 72, "nothing taken when refused");
        // Too few items: nothing taken, nothing paid.
        let mut few = Inv(HashMap::from([("minecraft:wheat".to_owned(), 63)]));
        let out = j
            .command(&mut e, &me(B), true, &mut few, &price, &line)
            .unwrap();
        assert!(matches!(&out[0], Out::Error(s) if s.starts_with("You need 64")));
        assert_eq!(few.0["minecraft:wheat"], 63);
    }

    #[test]
    fn a_falling_ask_lowers_the_pay_so_buying_to_deliver_never_pays() {
        let (mut e, mut j) = setup("ask", 10_000);
        j.tick(&e, &price, T0);
        let n = j
            .board
            .contracts
            .iter()
            .position(|c| c.item == "minecraft:wheat")
            .unwrap()
            + 1;
        // The market now asks 2 for wheat: a delivery pays 64 × 1 (90% of 2), not 128.
        let cheap = |id: &str| {
            if id == "minecraft:wheat" {
                Some((1, 2))
            } else {
                price(id)
            }
        };
        let mut bag = Inv(HashMap::from([("minecraft:wheat".to_owned(), 64)]));
        j.command(
            &mut e,
            &me(A),
            true,
            &mut bag,
            &cheap,
            &format!("jobs deliver {n}"),
        )
        .unwrap();
        assert!(
            e.balance(A) < 64 * 2,
            "less than buying them cost: {}",
            e.balance(A)
        );
    }

    #[test]
    fn an_empty_treasury_posts_nothing_and_the_board_survives_a_restart() {
        let (e, mut j) = setup("empty", 0);
        assert!(j.tick(&e, &price, T0).is_empty());
        assert!(j.board.contracts.is_empty());
        let (mut e, mut j) = setup("restart", 10_000);
        j.tick(&e, &price, T0);
        j.store(&mut e);
        e.save().unwrap();
        let dir = std::env::temp_dir().join(format!("smp-jobs-restart-{}", std::process::id()));
        let e2 = Economy::open(&dir, EcoConfig::default()).unwrap();
        let mut j2 = Jobs::open(&e2, Config::default());
        assert_eq!(j2.board.contracts, j.board.contracts);
        assert!(
            j2.tick(&e2, &price, T0 + 1000).is_empty(),
            "not posted twice"
        );
    }
}
