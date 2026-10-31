// The server market: the server buys and sells vanilla items at the prices
// of `config.json`'s `market`, the economy's way to earn without changing
// survival. `/worth`, `/sell hand|all|<item> [count]`, `/buy <item> [count]`,
// and the `/shop` menu (smp_shop) on the same prices. Only plain items (no
// damage, no name or enchantment) are traded, and only by players in survival
// or adventure mode. Items move first, then money, both logged; a sale the
// log refuses gives the items back. Nobody gets rich from it: buying always
// costs more than selling (an item priced otherwise isn't bought), no chain
// of recipes turns bought items into dearer ones (smp_recipes; such an item
// isn't bought either), and what a player sells is capped each UTC day, in
// coins and per item (`market.json` counts it). No game types: the glue
// lends the player's items as a `Bag`. Added to the game as
// `game::smp_market`; also a module of this crate, for its tests.

use super::smp_economy::{Economy, MAX_AMOUNT, Out, Player, parse_amount};
use super::smp_store::{read_json, uuid_string, write_json};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// A UTC day, in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// What the server pays for one item (`sell`) and asks for one (`buy`); 0
/// closes that side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Price {
    pub buy: i64,
    pub sell: i64,
}

/// `config.json`'s `market`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// By item id, `minecraft:diamond`. Items that stack to 64.
    pub items: BTreeMap<String, Price>,
    /// The most a player earns selling to the market in a UTC day (0: no
    /// limit).
    pub daily_sell_coins: i64,
    /// The most of one item a player sells to the market in a UTC day (0:
    /// no limit).
    pub daily_sell_items: u32,
}

/// The default prices: (item, buy, sell). Farm, mine and mob produce the
/// market buys cheaply; blocks, food and tools it only sells.
pub const PRICES: &[(&str, i64, i64)] = &[
    // Farming.
    ("wheat", 3, 1),
    ("wheat_seeds", 2, 0),
    ("carrot", 3, 1),
    ("potato", 3, 1),
    ("beetroot", 3, 1),
    ("sugar_cane", 3, 1),
    ("pumpkin", 8, 3),
    ("melon_slice", 2, 1),
    ("cactus", 3, 1),
    ("bone_meal", 3, 0),
    // Mining.
    ("coal", 8, 2),
    ("copper_ingot", 6, 2),
    ("iron_ingot", 30, 8),
    ("gold_ingot", 40, 10),
    ("redstone", 4, 1),
    ("lapis_lazuli", 8, 2),
    ("quartz", 8, 2),
    ("emerald", 80, 20),
    ("diamond", 400, 100),
    // Wood.
    ("oak_log", 4, 1),
    ("spruce_log", 4, 1),
    ("birch_log", 4, 1),
    ("jungle_log", 4, 1),
    ("acacia_log", 4, 1),
    ("dark_oak_log", 4, 1),
    ("mangrove_log", 4, 1),
    ("cherry_log", 4, 1),
    // Mob drops.
    ("rotten_flesh", 0, 1),
    ("bone", 3, 1),
    ("string", 3, 1),
    ("gunpowder", 8, 2),
    ("spider_eye", 4, 1),
    ("slime_ball", 10, 2),
    ("blaze_rod", 30, 8),
    ("leather", 6, 2),
    ("feather", 2, 1),
    // Building.
    ("cobblestone", 1, 0),
    ("stone", 2, 0),
    ("dirt", 1, 0),
    ("sand", 2, 0),
    ("gravel", 2, 0),
    ("glass", 3, 0),
    ("bricks", 4, 0),
    ("stone_bricks", 3, 0),
    // Food.
    ("bread", 5, 0),
    ("cooked_beef", 6, 0),
    ("cooked_porkchop", 6, 0),
    ("cooked_chicken", 5, 0),
    ("baked_potato", 4, 0),
    ("apple", 4, 0),
    ("golden_carrot", 40, 0),
    // Tools.
    ("torch", 1, 0),
    ("ladder", 2, 0),
    ("crafting_table", 4, 0),
    ("chest", 6, 0),
    ("furnace", 6, 0),
];

impl Default for Config {
    /// `PRICES`, buying always dearer than selling, and the daily limits.
    fn default() -> Self {
        Self {
            enabled: true,
            items: PRICES
                .iter()
                .map(|(id, buy, sell)| {
                    (
                        format!("minecraft:{id}"),
                        Price {
                            buy: *buy,
                            sell: *sell,
                        },
                    )
                })
                .collect(),
            daily_sell_coins: 2000,
            daily_sell_items: 640,
        }
    }
}

/// What one player sold today.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sold {
    /// The UTC day (days since 1970) the counts are for.
    pub day: i64,
    pub coins: i64,
    pub items: BTreeMap<String, u32>,
}

/// `market.json`: what each player sold today, by UUID.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub sold: BTreeMap<String, Sold>,
}

/// A config's prices made safe: an amount out of range, or a sale price not
/// below the buying price, closes that item's sale; so does an item a chain
/// of recipes makes for less than it sells for. What it closed, to log.
pub fn validate(config: &mut Config) -> Vec<String> {
    let mut closed = Vec::new();
    config.daily_sell_coins = config.daily_sell_coins.clamp(0, MAX_AMOUNT);
    for (id, p) in config.items.iter_mut() {
        if !valid_id(id) {
            closed.push(format!("{id}: not an item id; left out"));
            *p = Price::default();
            continue;
        }
        p.buy = p.buy.clamp(0, MAX_AMOUNT);
        p.sell = p.sell.clamp(0, MAX_AMOUNT);
        if p.sell > 0 && p.buy > 0 && p.sell >= p.buy {
            closed.push(format!(
                "{id}: sells for {} but buys for {}; the market won't buy it",
                p.sell, p.buy
            ));
            p.sell = 0;
        }
    }
    for l in super::smp_recipes::loops(&config.items, &super::smp_recipes::chains()) {
        closed.push(format!(
            "{}: crafted from the market's items for {:.2}, sells for {}; the market won't buy it",
            l.item, l.cost, l.sell
        ));
        if let Some(p) = config.items.get_mut(&l.item) {
            p.sell = 0;
        }
    }
    config.items.retain(|_, p| p.buy > 0 || p.sell > 0);
    closed
}

/// `minecraft:iron_ingot`: lowercase letters, digits, `_`, `.`, `-`, `/`
/// and one `:`, as a command line may carry it.
pub fn valid_id(id: &str) -> bool {
    let mut parts = id.split(':');
    let (Some(ns), Some(path), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |s: &str, extra: &str| {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || "_.-".contains(c)
                    || extra.contains(c)
            })
    };
    ok(ns, "") && ok(path, "/")
}

/// The acting player's items, lent by the game for one command.
pub trait Bag {
    /// The main hand: item id, count, plain.
    fn held(&self) -> Option<(String, u32, bool)>;
    /// Every plain item carried, with how many.
    fn carried(&self) -> Vec<(String, u32)>;
    fn count(&mut self, item: &str) -> u32;
    /// Takes up to `count`; what it took.
    fn take(&mut self, item: &str, count: u32) -> u32;
    /// How many more fit.
    fn room(&mut self, item: &str) -> u32;
    /// Gives up to `count` where they fit; what it gave.
    fn give(&mut self, item: &str, count: u32) -> u32;
}

/// `diamond` or `minecraft:diamond` as `minecraft:diamond`.
pub fn item_id(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    if name.contains(':') {
        name
    } else {
        format!("minecraft:{name}")
    }
}

/// `minecraft:iron_ingot` as `iron ingot`.
pub fn item_name(id: &str) -> String {
    id.strip_prefix("minecraft:")
        .unwrap_or(id)
        .replace('_', " ")
}

pub struct Market {
    pub config: Config,
    ledger: Ledger,
    /// `market.json`; None in tests that keep nothing.
    path: Option<PathBuf>,
    dirty: bool,
}

impl Market {
    /// The market of `dir` (`world/horizon-smp/`), its config made safe
    /// (`validate`), with what to log. A `market.json` that can't be read
    /// starts the day's counts from nothing (they only limit, never hold
    /// money).
    pub fn open(dir: &std::path::Path, mut config: Config) -> (Self, Vec<String>) {
        let mut problems: Vec<String> = validate(&mut config)
            .into_iter()
            .map(|l| format!("market: {l}"))
            .collect();
        let path = dir.join("market.json");
        let ledger = read_json(&path).unwrap_or_else(|e| {
            problems.push(format!("{e}: today's sale limits start again"));
            Ledger::default()
        });
        let market = Self {
            config,
            ledger,
            path: Some(path),
            dirty: false,
        };
        (market, problems)
    }

    /// A market that keeps nothing on disk, its config as given.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ledger: Ledger::default(),
            path: None,
            dirty: false,
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(path) = &self.path {
            write_json(path, &self.ledger)?;
        }
        self.dirty = false;
        Ok(())
    }

    /// What `uuid` sold today (`now`: milliseconds since 1970).
    fn sold(&self, uuid: u128, now: i64) -> Sold {
        match self.ledger.sold.get(&uuid_string(uuid)) {
            Some(s) if s.day == now.div_euclid(DAY_MS) => s.clone(),
            _ => Sold::default(),
        }
    }

    /// The coins `uuid` may still earn selling today (None: no limit).
    pub fn coins_left(&self, uuid: u128, now: i64) -> Option<i64> {
        let cap = self.config.daily_sell_coins;
        (cap > 0).then(|| (cap - self.sold(uuid, now).coins).max(0))
    }

    /// How many of `id` `uuid` may still sell today (None: no limit).
    pub fn items_left(&self, uuid: u128, now: i64, id: &str) -> Option<u32> {
        let cap = self.config.daily_sell_items;
        (cap > 0)
            .then(|| cap.saturating_sub(self.sold(uuid, now).items.get(id).copied().unwrap_or(0)))
    }

    /// Counts a sale against today's limits.
    fn count_sale(&mut self, uuid: u128, now: i64, id: &str, n: u32, coins: i64) {
        let day = now.div_euclid(DAY_MS);
        let s = self.ledger.sold.entry(uuid_string(uuid)).or_default();
        if s.day != day {
            *s = Sold {
                day,
                ..Sold::default()
            };
        }
        s.coins = s.coins.saturating_add(coins);
        let c = s.items.entry(id.to_owned()).or_default();
        *c = c.saturating_add(n);
        // Yesterday's counts go.
        self.ledger.sold.retain(|_, s| s.day == day);
        self.dirty = true;
    }

    /// The price of `id` (nothing on either side when it isn't traded).
    pub fn price(&self, id: &str) -> Price {
        self.config.items.get(id).copied().unwrap_or_default()
    }

    /// Today's limits for `uuid`, as a line (None when there are none).
    pub fn limits_line(&self, economy: &Economy, uuid: u128) -> Option<String> {
        let now = economy.now;
        let coins = self.coins_left(uuid, now).map(|left| {
            format!(
                "{} left of your {} a day",
                economy.money(left),
                economy.money(self.config.daily_sell_coins)
            )
        });
        let items = (self.config.daily_sell_items > 0).then(|| {
            format!(
                "at most {} of each item a day",
                self.config.daily_sell_items
            )
        });
        let parts: Vec<String> = coins.into_iter().chain(items).collect();
        (!parts.is_empty()).then(|| format!("Selling today: {}.", parts.join(", ")))
    }

    /// Runs `/worth`, `/sell` or `/buy`; None for another command. `trades`
    /// is false for a player in creative or spectator mode.
    pub fn command(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        trades: bool,
        bag: &mut dyn Bag,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (&name, args) = words.split_first()?;
        if !matches!(name, "worth" | "sell" | "buy") {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error("The market is closed.".into())]);
        }
        if name == "worth" {
            return Some(self.worth(economy, me, bag, args));
        }
        if !trades {
            return Some(vec![Out::Error(
                "The market trades only with players in survival or adventure mode.".into(),
            )]);
        }
        Some(match name {
            "sell" => self.sell(economy, me, bag, args),
            _ => self.buy(economy, me, bag, args),
        })
    }

    fn worth(&self, economy: &Economy, me: &Player, bag: &mut dyn Bag, args: &[&str]) -> Vec<Out> {
        let id = match args {
            [] => match bag.held() {
                Some((id, ..)) => id,
                None => {
                    return vec![Out::Error(
                        "Hold an item, or name one: /worth <item>".into(),
                    )];
                }
            },
            [item] => item_id(item),
            _ => return vec![Out::Error("Usage: /worth [item]".into())],
        };
        let p = self.price(&id);
        let name = item_name(&id);
        let mut parts = Vec::new();
        if p.sell > 0 {
            parts.push(format!("the market pays {} each", economy.money(p.sell)));
        }
        if p.buy > 0 {
            parts.push(format!("sells it for {} each", economy.money(p.buy)));
        }
        if parts.is_empty() {
            return vec![Out::Reply(format!("The market doesn't trade {name}."))];
        }
        let mut out = vec![Out::Reply(format!("{name}: {}.", parts.join(", ")))];
        if p.sell > 0 {
            if let Some(left) = self.items_left(me.uuid, economy.now, &id) {
                out.push(Out::Reply(format!(
                    "You may sell {left} more {name} today."
                )));
            }
            out.extend(self.limits_line(economy, me.uuid).map(Out::Reply));
        }
        out
    }

    /// Sells what `args` names (`hand [n]`, `all`, `<item> [n]`), within
    /// today's limits.
    pub fn sell(
        &mut self,
        economy: &mut Economy,
        me: &Player,
        bag: &mut dyn Bag,
        args: &[&str],
    ) -> Vec<Out> {
        let usage = || vec![Out::Error("Usage: /sell hand|all|<item> [count]".into())];
        // What to sell: (item, at most how many).
        let wanted: Vec<(String, u32)> = match args {
            ["all"] => bag
                .carried()
                .into_iter()
                .filter(|(id, _)| self.price(id).sell > 0)
                .collect(),
            ["hand", rest @ ..] => {
                let Some((id, count, plain)) = bag.held() else {
                    return vec![Out::Error("Your hand is empty.".into())];
                };
                if !plain {
                    return vec![Out::Error(
                        "The market only takes plain items (not worn, named or enchanted).".into(),
                    )];
                }
                match rest {
                    [] => vec![(id, count)],
                    [n] => match parse_count(n) {
                        Some(n) => vec![(id, n.min(count))],
                        None => return usage(),
                    },
                    _ => return usage(),
                }
            }
            [item] => vec![(item_id(item), u32::MAX)],
            [item, n] => match parse_count(n) {
                Some(n) => vec![(item_id(item), n)],
                None => return usage(),
            },
            _ => return usage(),
        };
        if wanted.is_empty() {
            return vec![Out::Error("You carry nothing the market buys.".into())];
        }
        let mut out = Vec::new();
        let mut total = 0i64;
        for (id, at_most) in wanted {
            let price = self.price(&id).sell;
            let name = item_name(&id);
            if price <= 0 {
                out.push(Out::Error(format!("The market doesn't buy {name}.")));
                continue;
            }
            // Never more than the account can be paid for at once, nor
            // than today's limits leave.
            let now = economy.now;
            let mut cap = (MAX_AMOUNT / price).min(i64::from(u32::MAX)) as u32;
            if let Some(left) = self.items_left(me.uuid, now, &id) {
                cap = cap.min(left);
            }
            if let Some(left) = self.coins_left(me.uuid, now) {
                cap = cap.min((left / price).min(i64::from(u32::MAX)) as u32);
            }
            let carried = bag.count(&id).min(at_most);
            if carried == 0 {
                out.push(Out::Error(format!("You have no plain {name}.")));
                continue;
            }
            let have = carried.min(cap);
            if have == 0 {
                out.push(Out::Error(format!(
                    "You reached today's selling limit for {name}; it starts again at midnight UTC."
                )));
                continue;
            }
            let taken = bag.take(&id, have);
            if taken == 0 {
                continue;
            }
            let Some(pay) = price.checked_mul(i64::from(taken)) else {
                bag.give(&id, taken);
                continue;
            };
            let kind = format!("sell {taken} {id}");
            match economy.adjust(&kind, me.uuid, pay, "") {
                Ok(_) => {
                    self.count_sale(me.uuid, now, &id, taken, pay);
                    total = total.saturating_add(pay);
                    out.push(Out::Reply(format!(
                        "Sold {taken} {name} for {}.",
                        economy.money(pay)
                    )));
                    if taken < carried {
                        out.push(Out::Reply(format!(
                            "{} {name} kept: today's selling limit.",
                            carried - taken
                        )));
                    }
                }
                Err(e) => {
                    let back = bag.give(&id, taken);
                    out.push(Out::Error(format!(
                        "The sale failed ({e}); {back} {name} given back."
                    )));
                }
            }
        }
        if total > 0 {
            out.push(Out::Reply(format!(
                "Balance: {}",
                economy.money(economy.balance(me.uuid))
            )));
        }
        out
    }

    /// Buys `args` (`<item> [count]`).
    pub fn buy(
        &self,
        economy: &mut Economy,
        me: &Player,
        bag: &mut dyn Bag,
        args: &[&str],
    ) -> Vec<Out> {
        let (item, count) = match args {
            [item] => (*item, 1),
            [item, n] => match parse_count(n) {
                Some(n) => (*item, n),
                None => return vec![Out::Error("Usage: /buy <item> [count]".into())],
            },
            _ => return vec![Out::Error("Usage: /buy <item> [count]".into())],
        };
        let id = item_id(item);
        let name = item_name(&id);
        let price = self.price(&id).buy;
        if price <= 0 {
            return vec![Out::Error(format!("The market doesn't sell {name}."))];
        }
        if count > 64 * 36 {
            return vec![Out::Error("At most a full inventory at once.".into())];
        }
        if count == 0 {
            return vec![Out::Error("Usage: /buy <item> [count]".into())];
        }
        let Some(cost) = price
            .checked_mul(i64::from(count))
            .filter(|c| *c <= MAX_AMOUNT)
        else {
            return vec![Out::Error("That costs too much at once.".into())];
        };
        if economy.balance(me.uuid) < cost {
            return vec![Out::Error(format!(
                "{count} {name} cost {}; you have {}.",
                economy.money(cost),
                economy.money(economy.balance(me.uuid))
            ))];
        }
        let room = bag.room(&id);
        if room < count {
            return vec![Out::Error(format!(
                "Only {room} {name} fit in your inventory."
            ))];
        }
        if let Err(e) = economy.adjust(&format!("buy {count} {id}"), me.uuid, -cost, "") {
            return vec![Out::Error(e)];
        }
        let given = bag.give(&id, count);
        let mut out = vec![Out::Reply(format!(
            "Bought {given} {name} for {}.",
            economy.money(cost)
        ))];
        if given < count {
            // Checked above; kept so money never goes for nothing.
            let refund = price.saturating_mul(i64::from(count - given));
            if economy
                .adjust(
                    &format!("refund {} {id}", count - given),
                    me.uuid,
                    refund,
                    "",
                )
                .is_ok()
            {
                out.push(Out::Reply(format!(
                    "{} refunded for what didn't fit.",
                    economy.money(refund)
                )));
            }
        }
        out.push(Out::Reply(format!(
            "Balance: {}",
            economy.money(economy.balance(me.uuid))
        )));
        out
    }
}

fn parse_count(s: &str) -> Option<u32> {
    parse_amount(s).and_then(|n| u32::try_from(n).ok())
}

#[cfg(test)]
mod tests {
    use super::super::smp_economy::Config as EconomyConfig;
    use super::*;
    use std::fs;

    /// A pretend inventory: plain stacks by item, one held item, a room cap.
    struct TestBag {
        items: BTreeMap<String, u32>,
        held: Option<(String, u32, bool)>,
        room: u32,
    }

    impl Bag for TestBag {
        fn held(&self) -> Option<(String, u32, bool)> {
            self.held.clone()
        }
        fn carried(&self) -> Vec<(String, u32)> {
            self.items.iter().map(|(k, v)| (k.clone(), *v)).collect()
        }
        fn count(&mut self, item: &str) -> u32 {
            self.items.get(item).copied().unwrap_or(0)
        }
        fn take(&mut self, item: &str, count: u32) -> u32 {
            let have = self.items.entry(item.into()).or_default();
            let n = (*have).min(count);
            *have -= n;
            n
        }
        fn room(&mut self, _item: &str) -> u32 {
            self.room
        }
        fn give(&mut self, item: &str, count: u32) -> u32 {
            let n = count.min(self.room);
            self.room -= n;
            *self.items.entry(item.into()).or_default() += n;
            n
        }
    }

    const ALEX: u128 = 1;

    fn setup(name: &str) -> (Economy, Market, Player, std::path::PathBuf) {
        setup_with(name, Config::default())
    }

    fn setup_with(name: &str, config: Config) -> (Economy, Market, Player, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-market-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EconomyConfig::default()).unwrap();
        e.seen(ALEX, "Alex");
        let me = Player {
            uuid: ALEX,
            name: "Alex".into(),
            level: 0,
        };
        (e, Market::new(config), me, dir)
    }

    fn bag(items: &[(&str, u32)], room: u32) -> TestBag {
        TestBag {
            items: items.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            held: None,
            room,
        }
    }

    #[test]
    fn selling_takes_the_items_then_pays_and_logs_both() {
        let (mut e, mut m, me, dir) = setup("sell");
        let mut b = bag(
            &[
                ("minecraft:diamond", 3),
                ("minecraft:wheat", 20),
                ("minecraft:dirt", 64),
            ],
            0,
        );
        let out = m
            .command(&mut e, &me, true, &mut b, "sell diamond 2")
            .unwrap();
        assert_eq!(out[0], Out::Reply("Sold 2 diamond for 200 coins.".into()));
        assert_eq!(b.items["minecraft:diamond"], 1);
        let out = m.command(&mut e, &me, true, &mut b, "sell all").unwrap();
        assert!(
            out.contains(&Out::Reply("Sold 1 diamond for 100 coins.".into())),
            "{out:?}"
        );
        assert!(
            out.contains(&Out::Reply("Sold 20 wheat for 20 coins.".into())),
            "{out:?}"
        );
        assert_eq!(b.items["minecraft:dirt"], 64, "dirt has no price");
        assert_eq!(e.balance(ALEX), 320);
        let log = fs::read_to_string(dir.join("audit.log")).unwrap();
        assert_eq!(log.matches("\"sell ").count(), 3, "{log}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn selling_the_hand_takes_only_plain_items() {
        let (mut e, mut m, me, dir) = setup("hand");
        let mut b = bag(&[("minecraft:iron_ingot", 10)], 0);
        b.held = Some(("minecraft:iron_ingot".into(), 10, true));
        let out = m.command(&mut e, &me, true, &mut b, "sell hand 4").unwrap();
        assert_eq!(out[0], Out::Reply("Sold 4 iron ingot for 32 coins.".into()));
        b.held = Some(("minecraft:diamond_sword".into(), 1, false));
        let out = m.command(&mut e, &me, true, &mut b, "sell hand").unwrap();
        assert_eq!(
            out,
            [Out::Error(
                "The market only takes plain items (not worn, named or enchanted).".into()
            )]
        );
        b.held = None;
        let out = m.command(&mut e, &me, true, &mut b, "sell hand").unwrap();
        assert_eq!(out, [Out::Error("Your hand is empty.".into())]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn buying_checks_money_and_room_before_anything_moves() {
        let (mut e, mut m, me, dir) = setup("buy");
        e.adjust("test", ALEX, 1000, "").unwrap();
        let mut b = bag(&[], 3);
        let out = m
            .command(&mut e, &me, true, &mut b, "buy diamond 3")
            .unwrap();
        assert_eq!(
            out,
            [Out::Error(
                "3 diamond cost 1,200 coins; you have 1,000 coins.".into()
            )]
        );
        let out = m.command(&mut e, &me, true, &mut b, "buy torch 4").unwrap();
        assert_eq!(
            out,
            [Out::Error("Only 3 torch fit in your inventory.".into())]
        );
        assert_eq!(e.balance(ALEX), 1000);
        let out = m
            .command(&mut e, &me, true, &mut b, "buy minecraft:diamond 2")
            .unwrap();
        assert_eq!(out[0], Out::Reply("Bought 2 diamond for 800 coins.".into()));
        assert_eq!((e.balance(ALEX), b.items["minecraft:diamond"]), (200, 2));
        let out = m
            .command(&mut e, &me, true, &mut b, "buy rotten_flesh")
            .unwrap();
        assert_eq!(
            out,
            [Out::Error("The market doesn't sell rotten flesh.".into())]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creative_players_and_a_closed_market_trade_nothing() {
        let (mut e, mut m, me, dir) = setup("closed");
        let mut b = bag(&[("minecraft:diamond", 64)], 64);
        let out = m.command(&mut e, &me, false, &mut b, "sell all").unwrap();
        assert_eq!(
            out,
            [Out::Error(
                "The market trades only with players in survival or adventure mode.".into()
            )]
        );
        let worth = m
            .command(&mut e, &me, false, &mut b, "worth diamond")
            .unwrap();
        assert_eq!(
            worth[0],
            Out::Reply(
                "diamond: the market pays 100 coins each, sells it for 400 coins each.".into()
            )
        );
        m.config.enabled = false;
        let out = m.command(&mut e, &me, true, &mut b, "sell all").unwrap();
        assert_eq!(out, [Out::Error("The market is closed.".into())]);
        assert_eq!((e.balance(ALEX), b.items["minecraft:diamond"]), (0, 64));
        assert!(m.command(&mut e, &me, true, &mut b, "balance").is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sales_stop_at_the_daily_limits_and_start_again_the_next_day() {
        let config = Config {
            daily_sell_coins: 250,
            daily_sell_items: 30,
            ..Config::default()
        };
        let (mut e, mut m, me, dir) = setup_with("limits", config);
        e.now = 5 * DAY_MS + 1000;
        let mut b = bag(&[("minecraft:diamond", 5), ("minecraft:wheat", 64)], 0);
        // 250 coins a day: two diamonds (200), then 50 wheat would pass the
        // coins but not the 30 items of one kind.
        let out = m
            .command(&mut e, &me, true, &mut b, "sell diamond")
            .unwrap();
        assert_eq!(
            out[0],
            Out::Reply("Sold 2 diamond for 200 coins.".into()),
            "{out:?}"
        );
        assert_eq!(
            out[1],
            Out::Reply("3 diamond kept: today's selling limit.".into())
        );
        let out = m.command(&mut e, &me, true, &mut b, "sell wheat").unwrap();
        assert_eq!(
            out[0],
            Out::Reply("Sold 30 wheat for 30 coins.".into()),
            "{out:?}"
        );
        let out = m.command(&mut e, &me, true, &mut b, "sell wheat").unwrap();
        assert_eq!(
            out,
            [Out::Error(
                "You reached today's selling limit for wheat; it starts again at midnight UTC."
                    .into()
            )]
        );
        let worth = m
            .command(&mut e, &me, true, &mut b, "worth diamond")
            .unwrap();
        assert!(
            worth.contains(&Out::Reply("You may sell 28 more diamond today.".into())),
            "{worth:?}"
        );
        assert!(
            worth.contains(&Out::Reply(
                "Selling today: 20 coins left of your 250 coins a day, at most 30 of each item a day.".into()
            )),
            "{worth:?}"
        );
        assert_eq!((e.balance(ALEX), b.items["minecraft:diamond"]), (230, 3));
        // Kept across a restart, in market.json.
        m.path = Some(dir.join("market.json"));
        m.dirty = true;
        m.save().unwrap();
        let (mut m, _) = Market::open(&dir, m.config.clone());
        let out = m
            .command(&mut e, &me, true, &mut b, "sell diamond")
            .unwrap();
        assert!(
            matches!(&out[0], Out::Error(s) if s.contains("limit")),
            "{out:?}"
        );
        // A new UTC day.
        e.now = 6 * DAY_MS;
        let out = m
            .command(&mut e, &me, true, &mut b, "sell diamond")
            .unwrap();
        assert_eq!(
            out[0],
            Out::Reply("Sold 2 diamond for 200 coins.".into()),
            "{out:?}"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prices_that_would_make_money_are_closed() {
        let mut config = Config::default();
        let set = |c: &mut Config, id: &str, buy, sell| {
            c.items
                .insert(format!("minecraft:{id}"), Price { buy, sell });
        };
        set(&mut config, "emerald", 20, 20);
        set(&mut config, "iron_block", 0, 300);
        set(&mut config, "rotten_flesh", -5, i64::MAX);
        config.items.insert(
            "minecraft:diamond 1\nop me".into(),
            Price { buy: 1, sell: 0 },
        );
        let closed = validate(&mut config);
        assert_eq!(closed.len(), 3, "{closed:?}");
        assert_eq!(
            config.items["minecraft:emerald"],
            Price { buy: 20, sell: 0 }
        );
        assert!(!config.items.contains_key("minecraft:iron_block"));
        assert_eq!(
            config.items["minecraft:rotten_flesh"],
            Price {
                buy: 0,
                sell: MAX_AMOUNT
            }
        );
        assert!(config.items.keys().all(|k| valid_id(k)));
        // At the largest price: one at a time, without overflow.
        let (mut e, mut m, me, dir) = setup_with(
            "validate",
            Config {
                daily_sell_coins: 0,
                ..config
            },
        );
        let mut b = bag(&[("minecraft:rotten_flesh", 64)], 0);
        let out = m
            .command(&mut e, &me, true, &mut b, "sell rotten_flesh")
            .unwrap();
        assert_eq!(
            out[0],
            Out::Reply(format!("Sold 1 rotten flesh for {}.", e.money(MAX_AMOUNT))),
            "{out:?}"
        );
        assert_eq!(b.items["minecraft:rotten_flesh"], 63);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_purchase_too_dear_to_count_is_refused() {
        let mut config = Config::default();
        config.items.insert(
            "minecraft:diamond".into(),
            Price {
                buy: MAX_AMOUNT,
                sell: 0,
            },
        );
        let (mut e, mut m, me, dir) = setup_with("dear", config);
        let mut b = bag(&[], 64 * 36);
        let out = m
            .command(&mut e, &me, true, &mut b, "buy diamond 2")
            .unwrap();
        assert_eq!(out, [Out::Error("That costs too much at once.".into())]);
        let out = m
            .command(&mut e, &me, true, &mut b, "buy diamond 0")
            .unwrap();
        assert!(matches!(&out[0], Out::Error(_)), "{out:?}");
        fs::remove_dir_all(dir).unwrap();
    }
}
