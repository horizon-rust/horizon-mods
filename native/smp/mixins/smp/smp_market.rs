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

use super::smp_economy::{Economy, MAX_AMOUNT, Out, Player, TREASURY, parse_amount};
use super::smp_market_prices::{Book, Dynamic, MINUTE_MS, Point, Quote, Span, arrow, base_mid};
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
    /// Prices that move with supply and demand (smp_market_prices); the
    /// item prices above are then the base ones.
    pub dynamic: Dynamic,
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
            dynamic: Dynamic::default(),
        }
    }
}

/// The default prices of SMP 0.1 to 0.11: a config that still holds
/// exactly these (never edited) gets today's defaults (`migrate`).
const OLD_PRICES: [(&str, i64, i64); 18] = [
    ("wheat", 0, 1),
    ("carrot", 0, 1),
    ("potato", 0, 1),
    ("sugar_cane", 0, 1),
    ("pumpkin", 0, 3),
    ("melon_slice", 0, 1),
    ("cactus", 0, 1),
    ("oak_log", 4, 1),
    ("coal", 8, 2),
    ("iron_ingot", 30, 8),
    ("gold_ingot", 40, 10),
    ("redstone", 4, 1),
    ("lapis_lazuli", 8, 2),
    ("emerald", 80, 20),
    ("diamond", 400, 100),
    ("bread", 5, 0),
    ("cooked_beef", 6, 0),
    ("torch", 1, 0),
];

/// A config whose prices are still the old defaults, untouched: today's
/// defaults instead. Whether it changed (the file is written again).
pub fn migrate(config: &mut Config) -> bool {
    let old: BTreeMap<String, Price> = OLD_PRICES
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
        .collect();
    if config.items != old {
        return false;
    }
    config.items = Config::default().items;
    true
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
    config.dynamic.validate();
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
    // Moving prices: the buy price high enough that a sale at the highest
    // factor never pays more than a purchase at the lowest cost.
    if config.dynamic.enabled {
        let d = config.dynamic.clone();
        for (id, p) in config.items.iter_mut() {
            if p.buy > 0 && p.sell > 0 && p.buy < d.least_buy(p.sell) {
                closed.push(format!(
                    "{id}: buys for {} but sells for {}; with prices moving {}x to {}x it now buys for {}",
                    p.buy,
                    p.sell,
                    d.min_factor,
                    d.max_factor,
                    d.least_buy(p.sell)
                ));
                p.buy = d.least_buy(p.sell);
            }
        }
    }
    // Chains of recipes, at the base prices and, with moving prices, with
    // every input at its cheapest and the result at its dearest.
    let mut worst = config.items.clone();
    if config.dynamic.enabled {
        for p in worst.values_mut() {
            p.buy = (p.buy as f64 * config.dynamic.min_factor - 1e-9).ceil() as i64;
            p.sell = (p.sell as f64 * config.dynamic.max_factor + 1e-9).floor() as i64;
        }
    }
    let mut loops = super::smp_recipes::loops(&config.items, &super::smp_recipes::chains());
    for l in super::smp_recipes::loops(&worst, &super::smp_recipes::chains()) {
        if !loops.iter().any(|o| o.item == l.item) {
            loops.push(l);
        }
    }
    for l in loops {
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
    /// The moving prices and their history (`market_prices.json`, written
    /// by the glue off the tick thread: `book_to_write`).
    pub book: Book,
    book_written: i64,
    book_dirty: bool,
    /// For the server log (the glue takes them).
    pub warnings: Vec<String>,
}

/// How often the prices' file is written.
const BOOK_EVERY_MS: i64 = 5 * MINUTE_MS;

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
        let book = read_json(&dir.join("market_prices.json")).unwrap_or_else(|e| {
            problems.push(format!("{e}: prices start again from their base"));
            Book::default()
        });
        let market = Self {
            config,
            ledger,
            path: Some(path),
            dirty: false,
            book,
            book_written: 0,
            book_dirty: false,
            warnings: Vec::new(),
        };
        (market, problems)
    }

    /// A market that keeps nothing on disk, its config as given (tests).
    #[cfg(test)]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ledger: Ledger::default(),
            path: None,
            dirty: false,
            book: Book::default(),
            book_written: 0,
            book_dirty: false,
            warnings: Vec::new(),
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

    /// Whether `uuid` may be paid `amount` by another player now (`/pay`):
    /// money received counts toward the same daily limit as selling to the
    /// market, so selling on alts and paying it all to a main doesn't get
    /// past it. Err: what they may still receive today.
    pub fn may_receive(&self, uuid: u128, amount: i64, now: i64) -> Result<(), i64> {
        match self.coins_left(uuid, now) {
            Some(left) if amount > left => Err(left),
            _ => Ok(()),
        }
    }

    /// Counts `coins` paid to `uuid` (`/pay`) against today's limit.
    pub fn count_received(&mut self, uuid: u128, now: i64, coins: i64) {
        let day = now.div_euclid(DAY_MS);
        let s = self.ledger.sold.entry(uuid_string(uuid)).or_default();
        if s.day != day {
            *s = Sold {
                day,
                ..Sold::default()
            };
        }
        s.coins = s.coins.saturating_add(coins.max(0));
        self.ledger.sold.retain(|_, s| s.day == day);
        self.dirty = true;
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

    /// The price of `id` now (nothing on either side when it isn't
    /// traded): the base one moved by supply and demand.
    pub fn price(&self, id: &str) -> Price {
        let base = self.base(id);
        if !self.config.dynamic.enabled {
            return base;
        }
        let q = self.book.quote(id, base.buy, base.sell);
        Price {
            buy: q.buy,
            sell: q.sell,
        }
    }

    /// The config's price of `id`, before supply and demand.
    pub fn base(&self, id: &str) -> Price {
        self.config.items.get(id).copied().unwrap_or_default()
    }

    /// What the market asks and pays for one `id` now, and the mid; None
    /// when it isn't traded. For futures and charts.
    pub fn spot(&self, id: &str) -> Option<Quote> {
        let b = self.config.items.get(id)?;
        Some(if self.config.dynamic.enabled {
            self.book.quote(id, b.buy, b.sell)
        } else {
            Quote {
                mid: base_mid(b.buy, b.sell),
                buy: b.buy,
                sell: b.sell,
            }
        })
    }

    #[allow(
        dead_code,
        reason = "read by the futures (smp-econ), not the market itself"
    )]
    /// The time-weighted mean mid of `id` over the last `minutes` minutes.
    pub fn twap(&self, id: &str, minutes: u32, now: i64) -> Option<f64> {
        self.config
            .items
            .contains_key(id)
            .then(|| self.book.twap(id, minutes, now))
            .flatten()
    }

    #[allow(
        dead_code,
        reason = "read by the futures (smp-econ), not the market itself"
    )]
    /// The 10-minute TWAP, rounded, from at least 5 one-minute points.
    pub fn mark_price(&self, id: &str, now: i64) -> Option<i64> {
        self.config
            .items
            .contains_key(id)
            .then(|| self.book.mark_price(id, now))
            .flatten()
    }

    /// Items of `id` traded (both sides) in the last `hours`.
    pub fn volume(&self, id: &str, now: i64, hours: u32) -> u64 {
        self.book.volume(id, now, hours)
    }

    pub fn history(&self, id: &str, span: Span) -> &[Point] {
        self.book.history(id, span)
    }

    /// The change of `id`'s mid over the last `minutes`, in percent.
    pub fn change(&self, id: &str, now: i64, minutes: u32) -> Option<f64> {
        let mid = self.spot(id)?.mid;
        self.book.change(id, now, minutes, mid)
    }

    /// The minute timer, called once a second: whether prices moved.
    pub fn tick(&mut self, now: i64) -> bool {
        if !self.config.enabled || !self.config.dynamic.enabled {
            return false;
        }
        let mids: BTreeMap<String, f64> = self
            .config
            .items
            .iter()
            .map(|(id, p)| (id.clone(), base_mid(p.buy, p.sell)))
            .collect();
        let moved = self.book.tick(&self.config.dynamic, &mids, now);
        self.book_dirty |= moved;
        moved
    }

    /// The prices' history to write, every 5 minutes when it changed (and
    /// always with `force`): its path and a copy, for a thread to write.
    pub fn book_to_write(&mut self, now: i64, force: bool) -> Option<(PathBuf, Book)> {
        if !self.book_dirty || (!force && now - self.book_written < BOOK_EVERY_MS) {
            return None;
        }
        let path = self.path.as_ref()?.with_file_name("market_prices.json");
        self.book_dirty = false;
        self.book_written = now;
        Some((path, self.book.clone()))
    }

    /// The day's movers and news, once per UTC day (at its first minute):
    /// None until then.
    pub fn summary_due(&mut self, now: i64) -> Option<Vec<String>> {
        let day = now.div_euclid(DAY_MS);
        if !self.config.dynamic.enabled || self.book.summary_day == day {
            return None;
        }
        let first = self.book.summary_day == 0;
        self.book.summary_day = day;
        self.book_dirty = true;
        if first {
            return None;
        }
        let items: Vec<(String, String, f64)> = self
            .config
            .items
            .keys()
            .filter_map(|id| Some((id.clone(), title_name(id), self.spot(id)?.mid)))
            .collect();
        let lines = self.book.summary(&self.config.dynamic, &items, now);
        self.book.last_summary.clone_from(&lines);
        Some(lines)
    }

    /// One line on an item's move: `▲ 2.4% 1h, ▼ 0.3% 24h`.
    pub fn changes_line(&self, id: &str, now: i64) -> String {
        let c = |m| {
            self.change(id, now, m)
                .map_or_else(|| "–".to_owned(), arrow)
        };
        format!("{} 1h, {} 24h", c(60), c(24 * 60))
    }

    /// A copy of what `web/market.json` is made of, for a thread to build
    /// and write it (the tick only copies).
    pub fn web_view(&self) -> WebView {
        WebView {
            config: self.config.clone(),
            book: self.book.clone(),
        }
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
        if self.config.dynamic.enabled {
            out.push(Out::Reply(format!(
                "Moves: {}; traded in 24 h: {}. Chart: /market {}",
                self.changes_line(&id, economy.now),
                self.volume(&id, economy.now, 24),
                id.trim_start_matches("minecraft:")
            )));
        }
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
            // What they bought here lately can't come back yet.
            let back = self.recently_bought(me.uuid, &id, now);
            if let Some(left) = self.coins_left(me.uuid, now) {
                cap = cap.min((left / price).min(i64::from(u32::MAX)) as u32);
            }
            let carried = bag.count(&id).min(at_most);
            if carried == 0 {
                out.push(Out::Error(format!("You have no plain {name}.")));
                continue;
            }
            let limited = carried.min(cap);
            if limited == 0 {
                out.push(Out::Error(format!(
                    "You reached today's selling limit for {name}; it starts again at midnight UTC."
                )));
                continue;
            }
            let have = limited.min(carried.saturating_sub(back));
            if have == 0 {
                let hours = self.config.dynamic.round_trip_hours;
                out.push(Out::Error(format!(
                    "You bought {back} {name} from the market in the last {hours} h: those can't be sold back yet."
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
                    let due = self.tax_on(pay);
                    let tax = self.tax(economy, me.uuid, due, &format!("sell {taken} {id}"));
                    self.moved(me.uuid, &id, taken, false);
                    total = total.saturating_add(pay - tax);
                    out.push(Out::Reply(if tax > 0 {
                        format!(
                            "Sold {taken} {name} for {}, tax {} ({}%): +{}.",
                            economy.money(pay),
                            economy.money(tax),
                            self.config.dynamic.tax_percent,
                            economy.money(pay - tax)
                        )
                    } else {
                        format!("Sold {taken} {name} for {}.", economy.money(pay))
                    }));
                    if taken < limited {
                        let hours = self.config.dynamic.round_trip_hours;
                        out.push(Out::Reply(format!(
                            "{} {name} kept: bought from the market in the last {hours} h.",
                            limited - taken
                        )));
                    }
                    if limited < carried {
                        out.push(Out::Reply(format!(
                            "{} {name} kept: today's selling limit.",
                            carried - limited
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
        &mut self,
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
        let tax = self.tax_on(cost);
        if economy.balance(me.uuid) < cost + tax {
            return vec![Out::Error(format!(
                "{count} {name} cost {}{}; you have {}.",
                economy.money(cost),
                if tax > 0 {
                    format!(" + {} tax", economy.money(tax))
                } else {
                    String::new()
                },
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
        let tax = if given > 0 {
            let due = self.tax_on(price.saturating_mul(i64::from(given)));
            self.tax(economy, me.uuid, due, &format!("buy {given} {id}"))
        } else {
            0
        };
        if given > 0 {
            self.moved(me.uuid, &id, given, true);
            let now = economy.now;
            self.book.bought(&uuid_string(me.uuid), &id, given, now);
            self.book_dirty = true;
        }
        let mut out = vec![Out::Reply(if tax > 0 {
            format!(
                "Bought {given} {name} for {} + {} tax ({}%).",
                economy.money(cost),
                economy.money(tax),
                self.config.dynamic.tax_percent
            )
        } else {
            format!("Bought {given} {name} for {}.", economy.money(cost))
        })];
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

impl Market {
    /// The tax on a trade of `amount` coins (0 without moving prices).
    pub fn tax_on(&self, amount: i64) -> i64 {
        if self.config.dynamic.enabled {
            self.config.dynamic.tax(amount)
        } else {
            0
        }
    }

    /// Takes `tax` from `uuid` into the Treasury (logged as `market_tax`):
    /// what it took (0 when the log refused).
    fn tax(&mut self, economy: &mut Economy, uuid: u128, tax: i64, what: &str) -> i64 {
        let tax = tax.min(economy.balance(uuid));
        if tax <= 0 {
            return 0;
        }
        match economy.to_pool("market_tax", uuid, TREASURY, tax, what) {
            Ok(()) => {
                self.book.taxes = self.book.taxes.saturating_add(tax);
                tax
            }
            Err(e) => {
                self.warnings.push(format!("market tax: {e}"));
                0
            }
        }
    }

    /// A trade's push on the price (at the next minute).
    fn moved(&mut self, uuid: u128, id: &str, n: u32, buy: bool) {
        if !self.config.dynamic.enabled {
            return;
        }
        let b = self.base(id);
        self.book.trade(
            &self.config.dynamic,
            uuid,
            id,
            base_mid(b.buy, b.sell),
            n,
            buy,
        );
        self.book_dirty = true;
    }

    /// How many of `id` `uuid` bought within the round-trip window.
    pub fn recently_bought(&self, uuid: u128, id: &str, now: i64) -> u32 {
        if !self.config.dynamic.enabled {
            return 0;
        }
        self.book
            .recently_bought(&self.config.dynamic, &uuid_string(uuid), id, now)
    }
}

/// `minecraft:iron_ingot` as `Iron Ingot`.
pub fn title_name(id: &str) -> String {
    item_name(id)
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn parse_count(s: &str) -> Option<u32> {
    parse_amount(s).and_then(|n| u32::try_from(n).ok())
}

/// The market as `web/market.json` shows it (built off the tick thread).
pub struct WebView {
    config: Config,
    book: Book,
}

/// A price rounded to the cent, for the web.
fn cents(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Points grouped by `step` (ms): each group's first time, last price and
/// summed volume.
fn series(points: &[Point], step: i64) -> serde_json::Value {
    let mut out: Vec<(i64, f64, u64)> = Vec::new();
    for p in points {
        let t = p.t.div_euclid(step) * step;
        match out.last_mut() {
            Some(last) if last.0 == t => {
                last.1 = p.price;
                last.2 += u64::from(p.volume);
            }
            _ => out.push((t, p.price, u64::from(p.volume))),
        }
    }
    serde_json::json!({
        "step": step,
        "t": out.iter().map(|p| p.0).collect::<Vec<_>>(),
        "price": out.iter().map(|p| cents(p.1)).collect::<Vec<_>>(),
        "volume": out.iter().map(|p| p.2).collect::<Vec<_>>(),
    })
}

impl WebView {
    /// `{"schema":1,"generated",...}`: each item's prices now, its moves,
    /// its 24 h volume, its last day in 5-minute points and its last 30
    /// days in hours. `categories`: (name, items) in menu order.
    pub fn doc(
        &self,
        categories: &[(String, Vec<String>)],
        currency: &str,
        now: i64,
    ) -> serde_json::Value {
        let d = &self.config.dynamic;
        let items: Vec<serde_json::Value> = self
            .config
            .items
            .iter()
            .map(|(id, base)| {
                let q = if d.enabled {
                    self.book.quote(id, base.buy, base.sell)
                } else {
                    Quote {
                        mid: base_mid(base.buy, base.sell),
                        buy: base.buy,
                        sell: base.sell,
                    }
                };
                let change = |m| self.book.change(id, now, m, q.mid).map(cents);
                let category = categories
                    .iter()
                    .find(|c| c.1.contains(id))
                    .map(|c| c.0.as_str());
                serde_json::json!({
                    "id": id,
                    "name": title_name(id),
                    "category": category,
                    "buy": (q.buy > 0).then_some(q.buy),
                    "sell": (q.sell > 0).then_some(q.sell),
                    "mid": cents(q.mid),
                    "base": cents(base_mid(base.buy, base.sell)),
                    "change_1h": change(60),
                    "change_24h": change(24 * 60),
                    "volume_24h": self.book.volume(id, now, 24),
                    "day": series(self.book.history(id, Span::Minute), 5 * MINUTE_MS),
                    "month": series(self.book.history(id, Span::Hour), 60 * MINUTE_MS),
                })
            })
            .collect();
        serde_json::json!({
            "schema": 1,
            "generated": now,
            "currency": currency,
            "moving": self.config.enabled && d.enabled,
            "tax_percent": d.tax_percent,
            "taxes_collected": self.book.taxes,
            "daily_summary": {
                "day": self.book.summary_day * DAY_MS,
                "lines": self.book.last_summary,
            },
            "items": items,
        })
    }
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

    /// The fixed prices of SMP 0.15 (no supply and demand, no tax).
    fn fixed() -> Config {
        let mut c = Config::default();
        c.dynamic.enabled = false;
        c
    }

    fn setup(name: &str) -> (Economy, Market, Player, std::path::PathBuf) {
        setup_with(name, fixed())
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
    fn money_received_by_pay_counts_toward_the_daily_selling_limit() {
        let config = Config {
            daily_sell_coins: 250,
            ..Config::default()
        };
        let (_e, mut m, _me, _dir) = setup_with("received", config);
        let now = 1_793_577_600_000;
        // An alt pays the main 200: fine, and it counts.
        assert_eq!(m.may_receive(ALEX, 200, now), Ok(()));
        m.count_received(ALEX, now, 200);
        assert_eq!(m.coins_left(ALEX, now), Some(50));
        // A second alt's 100 is refused: 50 left today, for sales or payments.
        assert_eq!(m.may_receive(ALEX, 100, now), Err(50));
        assert_eq!(m.may_receive(ALEX, 50, now), Ok(()));
        // The next day the limit is whole again.
        assert_eq!(m.may_receive(ALEX, 250, now + DAY_MS), Ok(()));
        // No limit configured: anything goes.
        let (_e, m, _me, _dir) = setup_with(
            "unlimited",
            Config {
                daily_sell_coins: 0,
                ..Config::default()
            },
        );
        assert_eq!(m.may_receive(ALEX, 1_000_000, now), Ok(()));
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
            ..fixed()
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
        let mut config = fixed();
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
        let mut config = fixed();
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

    #[test]
    fn the_old_default_prices_become_the_new_ones_and_edited_ones_stay() {
        let mut config = Config {
            items: OLD_PRICES
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
            ..fixed()
        };
        let mut edited = config.clone();
        assert!(migrate(&mut config));
        assert_eq!(config.items, Config::default().items);
        assert!(!migrate(&mut config), "once");
        edited.items.get_mut("minecraft:diamond").unwrap().sell = 90;
        let before = edited.clone();
        assert!(!migrate(&mut edited));
        assert_eq!(edited, before);
    }

    /// Moving prices, validated, with no daily caps and no round-trip
    /// window: only the prices themselves stand between a trader and money.
    fn moving() -> Config {
        let mut c = Config {
            daily_sell_coins: 0,
            daily_sell_items: 0,
            ..Config::default()
        };
        c.dynamic.round_trip_hours = 0;
        let _ = validate(&mut c);
        c
    }

    #[test]
    fn no_purchase_and_later_sale_on_moving_prices_ever_earns() {
        // Five players pump and dump every item for 2,000 minutes; the
        // dearest sale price seen never reaches the cheapest buy price seen.
        let c = moving();
        let mut m = Market::new(c.clone());
        let t0 = 1_800_000_000_000i64;
        let mut seed = 12345u64;
        let mut rand = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let ids: Vec<String> = c.items.keys().cloned().collect();
        let mut extremes: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        m.tick(t0);
        for minute in 1..2000i64 {
            // Pump for a while, then dump, by phases.
            let pump = (minute / 200) % 2 == 0;
            for p in 0..5u128 {
                let id = &ids[rand(ids.len() as u64) as usize];
                let n = 1 + rand(640) as u32;
                m.moved(p, id, n, pump || rand(4) == 0);
            }
            m.tick(t0 + minute * MINUTE_MS);
            for id in &ids {
                let q = m.price(id);
                let e = extremes.entry(id.clone()).or_insert((i64::MAX, 0));
                if q.buy > 0 {
                    e.0 = e.0.min(q.buy);
                }
                e.1 = e.1.max(q.sell);
            }
        }
        for (id, (min_buy, max_sell)) in extremes {
            let f = m.book.factor(&id);
            assert!(
                f >= c.dynamic.min_factor - 1e-9 && f <= c.dynamic.max_factor + 1e-9,
                "{id} {f}"
            );
            if min_buy < i64::MAX && max_sell > 0 {
                assert!(
                    max_sell <= min_buy,
                    "{id}: sold for {max_sell}, bought for {min_buy}"
                );
            }
        }
    }

    #[test]
    fn a_whale_who_pumps_a_price_and_sells_into_it_loses_money() {
        let c = moving();
        let (mut e, _, me, dir) = setup("whale");
        let mut m = Market::new(c);
        let t0 = 1_800_000_000_000i64;
        e.now = t0;
        e.adjust("test_funds", me.uuid, 50_000_000, "").unwrap();
        let start = e.balance(me.uuid);
        let mut b = bag(&[], 64 * 36);
        m.tick(t0);
        // Two hours of buying diamonds every minute: the price climbs.
        for minute in 1..=120 {
            let out = m.buy(&mut e, &me, &mut b, &["diamond", "64"]);
            assert!(
                matches!(&out[0], Out::Reply(r) if r.starts_with("Bought 64 diamond")),
                "{out:?}"
            );
            let now = t0 + minute * MINUTE_MS;
            e.now = now;
            m.tick(now);
            b.room = 64 * 36;
        }
        assert!(
            m.book.factor("minecraft:diamond") > 1.5,
            "the price was pushed up"
        );
        // Then sell it all into the price they pushed.
        let n = b.items["minecraft:diamond"];
        let out = m.sell(&mut e, &me, &mut b, &["diamond", &n.to_string()]);
        assert!(
            matches!(&out[0], Out::Reply(r) if r.starts_with(&format!("Sold {n} diamond"))),
            "{out:?}"
        );
        assert!(
            e.balance(me.uuid) < start,
            "{} >= {start}",
            e.balance(me.uuid)
        );
        // The Treasury got the tax on every trade.
        assert!(e.pool(TREASURY) > 0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn moving_prices_tax_the_receipt_and_keep_bought_items_from_coming_back() {
        let mut c = Config::default();
        let _ = validate(&mut c);
        let (mut e, _, me, dir) = setup("dyn");
        let mut m = Market::new(c);
        let t0 = 1_800_000_000_000i64;
        e.now = t0;
        e.adjust("test_funds", me.uuid, 10_000, "").unwrap();
        let mut b = bag(&[("minecraft:diamond", 3)], 64 * 36);
        let out = m.sell(&mut e, &me, &mut b, &["diamond"]);
        assert_eq!(
            out[0],
            Out::Reply("Sold 3 diamond for 300 coins, tax 6 coins (2%): +294 coins.".into())
        );
        assert_eq!(e.pool(TREASURY), 6);
        let out = m.buy(&mut e, &me, &mut b, &["iron_ingot", "10"]);
        assert_eq!(
            out[0],
            Out::Reply("Bought 10 iron ingot for 300 coins + 6 coins tax (2%).".into())
        );
        let out = m.sell(&mut e, &me, &mut b, &["iron_ingot"]);
        assert_eq!(
            out[0],
            Out::Error("You bought 10 iron ingot from the market in the last 24 h: those can't be sold back yet.".into())
        );
        // Five more found in a mine: those sell, the bought ones stay.
        b.items.insert("minecraft:iron_ingot".into(), 15);
        let out = m.sell(&mut e, &me, &mut b, &["iron_ingot"]);
        assert!(
            matches!(&out[0], Out::Reply(r) if r.starts_with("Sold 5 iron ingot")),
            "{out:?}"
        );
        assert_eq!(
            out[1],
            Out::Reply("10 iron ingot kept: bought from the market in the last 24 h.".into())
        );
        b.items.insert("minecraft:iron_ingot".into(), 10);
        // A day later they may.
        e.now = t0 + 25 * 3_600_000;
        m.tick(e.now);
        let out = m.sell(&mut e, &me, &mut b, &["iron_ingot"]);
        assert!(
            matches!(&out[0], Out::Reply(r) if r.starts_with("Sold 10 iron ingot")),
            "{out:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn moving_prices_widen_tight_spreads_at_load() {
        let mut c = Config::default();
        let closed = validate(&mut c);
        // Melon slices bought for 2, sold for 1: with prices moving 0.6x to
        // 1.6x a slice bought at 1.2 could sell at 1.6.
        assert_eq!(c.items["minecraft:melon_slice"], Price { buy: 3, sell: 1 });
        assert!(
            closed
                .iter()
                .any(|l| l.starts_with("minecraft:melon_slice: buys for 2")),
            "{closed:?}"
        );
        for (id, p) in &c.items {
            if p.buy > 0 && p.sell > 0 {
                assert!(
                    p.sell as f64 * c.dynamic.max_factor
                        <= p.buy as f64 * c.dynamic.min_factor + 1e-9,
                    "{id}"
                );
            }
        }
    }

    #[test]
    fn the_web_page_gets_prices_moves_and_five_minute_points() {
        let (mut e, _, me, dir) = setup("web");
        let mut m = Market::new(moving());
        let t0 = 1_800_000_000_000i64;
        e.now = t0;
        e.adjust("test_funds", me.uuid, 10_000_000, "").unwrap();
        let mut b = bag(&[], 64 * 36);
        m.tick(t0);
        // An hour of buying a diamond each minute.
        for minute in 1..=60 {
            m.buy(&mut e, &me, &mut b, &["diamond", "1"]);
            let now = t0 + minute * MINUTE_MS;
            e.now = now;
            m.tick(now);
        }
        let now = t0 + 60 * MINUTE_MS;
        let cats = vec![("Mining".to_owned(), vec!["minecraft:diamond".to_owned()])];
        let doc = m.web_view().doc(&cats, "coins", now);
        assert_eq!(doc["schema"], 1);
        let items = doc["items"].as_array().unwrap();
        assert_eq!(items.len(), m.config.items.len());
        let d = items
            .iter()
            .find(|i| i["id"] == "minecraft:diamond")
            .unwrap();
        assert_eq!(d["category"], "Mining");
        assert_eq!(d["volume_24h"], 60);
        assert!(d["change_1h"].as_f64().unwrap() > 0.0, "{d}");
        assert!(d["buy"].as_i64().unwrap() >= d["sell"].as_i64().unwrap());
        // 60 minute points in 12 five-minute ones (13 when the first hour
        // doesn't start on a 5-minute mark), volumes kept.
        let day = d["day"]["t"].as_array().unwrap();
        assert!((12..=13).contains(&day.len()), "{}", day.len());
        let volume: u64 = d["day"]["volume"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .sum();
        assert_eq!(volume, 60);
        assert!(day.windows(2).all(|w| w[1].as_i64() > w[0].as_i64()));
        let _ = fs::remove_dir_all(dir);
    }
}
