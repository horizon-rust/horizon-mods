// Perpetual futures on the market's item prices (`/futures`). A player
// opens a long or short position on an item with a margin in coins and a
// leverage (1x up to the item's cap): its size is margin × leverage / mark,
// in items. The house (the Treasury's insurance fund) is the other side of
// every position: losses go into the fund, profits come out of it, and no
// position may open when the fund couldn't pay what the house's net exposure
// could lose in a 30% move. Nothing is ever minted.
//
// The mark price is the market's time-weighted average over 10 minutes
// (smp_market's `mark_price`), never the last trade: a whale can't move it
// in one go to liquidate others or pump their own position. Each minute (the
// market's price timer, never per tick), the positions are marked: one whose
// equity falls to the maintenance margin is liquidated, offline or not; what
// is left of its margin goes to the fund, and a loss past the margin is the
// fund's (the player never owes more than they posted). Funding: each hour,
// the side with more open interest pays the fund a small rate on its size,
// which pulls the book back toward balance. Opening and closing fees go to
// the Treasury. Positions live in `economy.json` with the money
// (`ext.futures`), their margins in the `futures` pool: a restart in the
// middle of a liquidation finds either the position or its result.
//
// No game types: prices come through `Prices`. Added to the game as
// `game::smp_futures`; also a module of this crate, for its tests.

use super::smp_economy::{Economy, INSURANCE, MAX_AMOUNT, Out, Player, TREASURY, parse_amount};
use super::smp_market::{item_id, item_name};
use super::smp_store::{parse_uuid, uuid_string};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The pool the margins wait in.
pub const POOL: &str = "futures";
/// The market's prices, as the futures read them.
pub trait Prices {
    /// The mark price of `id`: a time-weighted average over minutes, coins
    /// an item; None when it isn't traded or has too little history.
    fn mark(&self, id: &str, now: i64) -> Option<f64>;
    /// Items of `id` traded with the market in the last 24 hours.
    fn volume(&self, id: &str, now: i64) -> u64;
}

/// `config.json`'s `futures`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The highest leverage of the most traded items.
    pub max_leverage: u32,
    /// Daily volume (items) an item needs for each 1x of leverage past 2x:
    /// a thinly traded item is easier to push, so it gets less.
    pub volume_per_leverage: u64,
    /// The margin under which a position is liquidated, in percent of its
    /// size at the mark.
    pub maintenance_percent: f64,
    /// Fees to open and to close, in percent of the size (to the Treasury).
    pub fee_percent: f64,
    /// Funding an hour at the most, in percent of the size, paid by the
    /// bigger side to the fund in proportion to the imbalance.
    pub funding_percent_hour: f64,
    /// Open interest (size at the mark, coins) a player may hold on one
    /// item, and on all.
    pub max_player_item: i64,
    pub max_player_total: i64,
    /// Open interest on one item, at most this many days of its market
    /// volume (in coins).
    pub max_item_volume_days: f64,
    /// The move the fund must be able to pay on the house's net exposure.
    pub stress_percent: f64,
    pub min_margin: i64,
    /// A liquidation of at least this margin is told to everyone.
    pub announce_margin: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_leverage: 10,
            volume_per_leverage: 500,
            maintenance_percent: 5.0,
            fee_percent: 0.2,
            funding_percent_hour: 0.05,
            max_player_item: 20_000,
            max_player_total: 50_000,
            max_item_volume_days: 3.0,
            stress_percent: 30.0,
            min_margin: 10,
            announce_margin: 1_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: u64,
    pub owner: String,
    pub name: String,
    pub item: String,
    pub long: bool,
    /// Items.
    pub size: f64,
    /// The mark when it opened.
    pub entry: f64,
    /// Coins in the pool for it (less funding paid).
    pub margin: i64,
    pub leverage: u32,
    pub opened: i64,
}

impl Position {
    /// Profit (or loss) at `mark`, in coins.
    pub fn pnl(&self, mark: f64) -> f64 {
        let d = (mark - self.entry) * self.size;
        if self.long { d } else { -d }
    }
    /// The margin plus profit at `mark`.
    pub fn equity(&self, mark: f64) -> f64 {
        self.margin as f64 + self.pnl(mark)
    }
    /// The mark at which equity meets the maintenance margin.
    pub fn liquidation(&self, maintenance: f64) -> f64 {
        let (q, m, e) = (self.size, self.margin as f64, self.entry);
        let mm = maintenance / 100.0;
        if self.long {
            ((q * e - m) / (q * (1.0 - mm))).max(0.0)
        } else {
            (m + q * e) / (q * (1.0 + mm))
        }
    }
}

/// What `economy.json`'s `ext.futures` holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub next_id: u64,
    pub positions: Vec<Position>,
    /// The minute last marked, and the hour funding was last charged.
    pub minute: i64,
    pub hour: i64,
    /// Lines for players who were offline.
    pub notes: BTreeMap<String, Vec<String>>,
    /// Since it opened: fees, funding, liquidations, what the fund paid
    /// past margins (gaps).
    pub fees: i64,
    pub funding: i64,
    pub liquidations: u64,
    pub gaps: i64,
}

pub struct Futures {
    pub config: Config,
    pub data: Data,
    dirty: bool,
    pub online: HashSet<u128>,
    /// Money moves that failed (the audit log): the glue logs them.
    pub problems: Vec<String>,
    /// The open menus: their window, and the position a first click armed
    /// to close.
    pub views: HashMap<u128, (Option<i32>, Option<u64>)>,
}

/// The `/futures` menu: three rows, positions on the first two.
pub const ROWS: u8 = 3;
pub const SIZE: usize = 27;
const SHOWN: usize = 18;
pub const HELP_TEXT: &str = "Click a position, then again, to close it · /futures long|short <item> <margin> [leverage] · /futures info <item>";

/// A cell of the menu.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Empty,
    /// A position; `true`: armed (the next click closes it).
    Position(u64, bool),
    Fund,
    Help,
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    Nothing,
    Close,
    Redraw,
    /// Close position `id` (the second click on it).
    Shut(u64),
}

fn fmt_price(p: f64) -> String {
    if p >= 100.0 {
        format!("{p:.0}")
    } else {
        format!("{p:.2}")
    }
}

impl Futures {
    pub fn open(economy: &Economy, config: Config) -> Self {
        let data = economy
            .ext("futures")
            .and_then(|v| serde_json::from_value::<Data>(v.clone()).ok())
            .unwrap_or_default();
        Self {
            config,
            data,
            dirty: false,
            online: HashSet::new(),
            problems: Vec::new(),
            views: HashMap::new(),
        }
    }

    /// Open positions' entries moved as their item's price history was
    /// (`Market::rebased`: a base price of before 0.29 made one price), so a
    /// position neither gains nor loses by it.
    pub fn rebase(&mut self, ratios: &BTreeMap<String, f64>) {
        for p in &mut self.data.positions {
            if let Some(k) = ratios.get(&p.item) {
                p.entry *= k;
                self.dirty = true;
            }
        }
    }

    pub fn store(&mut self, economy: &mut Economy) {
        if self.dirty {
            economy.set_ext(
                "futures",
                serde_json::to_value(&self.data).unwrap_or_default(),
            );
            self.dirty = false;
        }
    }

    /// The highest leverage on `id`: 2x for a thin market, up to
    /// `max_leverage` with the volume.
    pub fn max_leverage(&self, prices: &dyn Prices, id: &str, now: i64) -> u32 {
        let extra = prices.volume(id, now) / self.config.volume_per_leverage.max(1);
        (2 + extra.min(u64::from(u32::MAX)) as u32).min(self.config.max_leverage.max(1))
    }

    /// Open interest at the mark: (long coins, short coins) on `id`.
    fn interest(&self, id: &str, mark: f64) -> (f64, f64) {
        let mut oi = (0.0, 0.0);
        for p in self.data.positions.iter().filter(|p| p.item == id) {
            if p.long {
                oi.0 += p.size * mark;
            } else {
                oi.1 += p.size * mark;
            }
        }
        oi
    }

    /// The fund needed for the house's net exposure, all items, after a
    /// position of `extra` coins (long > 0) on `id`.
    fn stress(&self, prices: &dyn Prices, now: i64, id: &str, extra: f64) -> f64 {
        let mut items: Vec<&str> = self
            .data
            .positions
            .iter()
            .map(|p| p.item.as_str())
            .collect();
        items.push(id);
        items.sort_unstable();
        items.dedup();
        items
            .into_iter()
            .map(|i| {
                let m = prices.mark(i, now).unwrap_or(0.0);
                let (l, s) = self.interest(i, m);
                let net = l - s + if i == id { extra } else { 0.0 };
                net.abs() * self.config.stress_percent / 100.0
            })
            .sum()
    }

    fn notify(&mut self, out: &mut Vec<Out>, uuid: &str, line: String) {
        match parse_uuid(uuid) {
            Some(u) if self.online.contains(&u) => out.push(Out::Tell(u, line)),
            _ => {
                let notes = self.data.notes.entry(uuid.to_owned()).or_default();
                notes.push(line);
                if notes.len() > 20 {
                    notes.remove(0);
                }
                self.dirty = true;
            }
        }
    }

    /// Runs `/futures ...`; None for another command. Some(vec![]) with
    /// no argument: the glue opens the menu.
    pub fn command(
        &mut self,
        economy: &mut Economy,
        prices: &dyn Prices,
        me: &Player,
        now: i64,
        line: &str,
    ) -> Option<Vec<Out>> {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() != Some(&"futures") {
            return None;
        }
        if !self.config.enabled {
            return Some(vec![Out::Error("Futures are off on this server.".into())]);
        }
        Some(match words[1..] {
            [] => Vec::new(),
            [side @ ("long" | "short"), item, margin] => self.open_position(economy, prices, me, side == "long", item, margin, None, now),
            [side @ ("long" | "short"), item, margin, lev] => match lev.trim_end_matches('x').parse::<u32>() {
                Ok(l) => self.open_position(economy, prices, me, side == "long", item, margin, Some(l), now),
                Err(_) => vec![Out::Error("The leverage is a whole number: 1 to 10.".into())],
            },
            ["close", id] => match id.trim_start_matches('#').parse::<u64>() {
                Ok(id) => self.close(economy, prices, me, id, now),
                Err(_) => vec![Out::Error("Usage: /futures close <position>".into())],
            },
            ["list"] => self.list(prices, me, now),
            ["info", item] => self.info(economy, prices, &item_id(item), now),
            _ => vec![Out::Error(
                "Usage: /futures, /futures long|short <item> <margin> [leverage], /futures close <position>, /futures list, /futures info <item>"
                    .into(),
            )],
        })
    }

    /// `/futures info <item>`: the mark, the leverage cap, the open
    /// interest and the funding.
    fn info(&self, economy: &Economy, prices: &dyn Prices, id: &str, now: i64) -> Vec<Out> {
        let Some(mark) = prices.mark(id, now) else {
            return vec![Out::Error(format!(
                "No futures on {}: the market has no price history for it.",
                item_name(id)
            ))];
        };
        let (l, s) = self.interest(id, mark);
        vec![Out::Reply(format!(
            "{}: mark {} (10-min average), up to {}x, open interest long {} / short {}, funding now {:.3}%/h paid by the {} side. Fund: {}.",
            item_name(id),
            fmt_price(mark),
            self.max_leverage(prices, id, now),
            economy.money(l as i64),
            economy.money(s as i64),
            self.funding_rate(l, s),
            if l >= s { "long" } else { "short" },
            economy.money(economy.pool(INSURANCE))
        ))]
    }

    /// The funding rate an hour (percent) on the bigger side.
    fn funding_rate(&self, long: f64, short: f64) -> f64 {
        let total = long + short;
        if total <= 0.0 {
            return 0.0;
        }
        self.config.funding_percent_hour * (long - short).abs() / total
    }

    #[allow(clippy::too_many_arguments)]
    fn open_position(
        &mut self,
        economy: &mut Economy,
        prices: &dyn Prices,
        me: &Player,
        long: bool,
        item: &str,
        margin: &str,
        leverage: Option<u32>,
        now: i64,
    ) -> Vec<Out> {
        let err = |s: String| vec![Out::Error(s)];
        let id = item_id(item);
        let Some(margin) = parse_amount(margin) else {
            return err("The margin must be a positive whole number of coins.".into());
        };
        if margin < self.config.min_margin {
            return err(format!(
                "The smallest margin is {}.",
                economy.money(self.config.min_margin)
            ));
        }
        let Some(mark) = prices.mark(&id, now).filter(|m| *m > 0.0) else {
            return err(format!(
                "No futures on {}: the market has no price history for it.",
                item_name(&id)
            ));
        };
        let cap = self.max_leverage(prices, &id, now);
        let lev = leverage.unwrap_or(1);
        if lev == 0 || lev > cap {
            return err(format!(
                "{} takes 1x to {cap}x (by how much the market trades it).",
                item_name(&id)
            ));
        }
        let notional = margin as f64 * f64::from(lev);
        let fee = ((notional * self.config.fee_percent / 100.0).ceil() as i64).max(1);
        if economy.balance(me.uuid) < margin + fee {
            return err(format!(
                "You need {} (margin and fee).",
                economy.money(margin + fee)
            ));
        }
        let mine = uuid_string(me.uuid);
        let at_mark = |p: &Position| p.size * prices.mark(&p.item, now).unwrap_or(p.entry);
        let held_item: f64 = self
            .data
            .positions
            .iter()
            .filter(|p| p.owner == mine && p.item == id)
            .map(at_mark)
            .sum();
        let held_all: f64 = self
            .data
            .positions
            .iter()
            .filter(|p| p.owner == mine)
            .map(at_mark)
            .sum();
        if held_item + notional > self.config.max_player_item as f64
            || held_all + notional > self.config.max_player_total as f64
        {
            return err(format!(
                "That's past your limit: {} on one item, {} in all.",
                economy.money(self.config.max_player_item),
                economy.money(self.config.max_player_total)
            ));
        }
        let (l, s) = self.interest(&id, mark);
        let item_cap = prices.volume(&id, now) as f64 * mark * self.config.max_item_volume_days;
        if l + s + notional > item_cap {
            return err(format!(
                "Open interest on {} is full ({} at most, {} days of its market volume).",
                item_name(&id),
                economy.money(item_cap as i64),
                self.config.max_item_volume_days
            ));
        }
        let extra = if long { notional } else { -notional };
        let before = self.stress(prices, now, &id, 0.0);
        let after = self.stress(prices, now, &id, extra);
        if after > before && after > economy.pool(INSURANCE) as f64 {
            return err(format!(
                "The insurance fund ({}) can't cover more {} positions on {} now; the other side is open.",
                economy.money(economy.pool(INSURANCE)),
                if long { "long" } else { "short" },
                item_name(&id)
            ));
        }
        self.data.next_id += 1;
        let pid = self.data.next_id;
        let p = Position {
            id: pid,
            owner: mine,
            name: me.name.clone(),
            item: id.clone(),
            long,
            size: notional / mark,
            entry: mark,
            margin,
            leverage: lev,
            opened: now,
        };
        let entry = serde_json::json!({
            "kind": "futures_open", "id": pid, "owner": &p.owner, "item": &id, "long": long,
            "size": p.size, "entry": mark, "margin": margin, "leverage": lev,
        });
        let moved = economy
            .record(entry)
            .and_then(|()| economy.to_pool("futures_margin", me.uuid, POOL, margin, ""))
            .and_then(|()| economy.to_pool("futures_fee", me.uuid, TREASURY, fee, ""));
        if let Err(e) = moved {
            return err(e);
        }
        let liq = p.liquidation(self.config.maintenance_percent);
        let line = format!(
            "Opened #{pid}: {} {lev}x on {} {} at {} (margin {}, fee {}). Liquidated at {}.",
            if long { "LONG" } else { "SHORT" },
            fmt_price(p.size),
            item_name(&id),
            fmt_price(mark),
            economy.money(margin),
            economy.money(fee),
            fmt_price(liq)
        );
        self.data.fees += fee;
        self.data.positions.push(p);
        self.dirty = true;
        vec![Out::Reply(line)]
    }

    /// Settles position `i` at `mark`: its margin plus profit (or less its
    /// loss, never below 0) to its owner, the rest to the fund, or the fund
    /// pays the profit. `fee`: taken from what the owner gets. A liquidated
    /// position's whole margin goes to the fund (what was left of it is the
    /// liquidation penalty): getting yourself liquidated never pays.
    fn settle(
        &mut self,
        economy: &mut Economy,
        i: usize,
        mark: f64,
        fee: i64,
        kind: &str,
    ) -> Result<(i64, f64), String> {
        let p = self.data.positions[i].clone();
        let owner = parse_uuid(&p.owner).ok_or("a damaged position")?;
        let pnl = p.pnl(mark);
        let entry = serde_json::json!({
            "kind": kind, "id": p.id, "owner": &p.owner, "item": &p.item, "mark": mark,
            "pnl": pnl, "margin": p.margin,
        });
        economy.record(entry)?;
        let equity = (p.equity(mark).floor() as i64).clamp(0, MAX_AMOUNT);
        // The margin leaves the pool: to its owner up to the equity, the rest
        // to the fund; a profit past the margin comes out of the fund.
        let liquidated = kind == "futures_liquidated";
        let equity = if liquidated { 0 } else { equity };
        let to_owner = equity.min(p.margin);
        if to_owner > 0 {
            economy.from_pool(kind, POOL, owner, to_owner, "")?;
        }
        if p.margin > to_owner {
            economy.pool_move("futures_loss", POOL, INSURANCE, p.margin - to_owner, "")?;
        }
        if equity > p.margin {
            let profit = equity - p.margin;
            economy.from_pool("futures_profit", INSURANCE, owner, profit, "")?;
        }
        let gap = (-p.equity(mark)).max(0.0).ceil() as i64;
        self.data.gaps += gap;
        let fee = fee.min(economy.balance(owner)).min(equity);
        if fee > 0 {
            economy.to_pool("futures_fee", owner, TREASURY, fee, "")?;
            self.data.fees += fee;
        }
        self.data.positions.remove(i);
        self.dirty = true;
        Ok((equity - fee, pnl))
    }

    pub fn close(
        &mut self,
        economy: &mut Economy,
        prices: &dyn Prices,
        me: &Player,
        id: u64,
        now: i64,
    ) -> Vec<Out> {
        let mine = uuid_string(me.uuid);
        let Some(i) = self
            .data
            .positions
            .iter()
            .position(|p| p.id == id && p.owner == mine)
        else {
            return vec![Out::Error(format!("You have no position #{id}."))];
        };
        let p = self.data.positions[i].clone();
        let Some(mark) = prices.mark(&p.item, now) else {
            return vec![Out::Error(
                "No mark price now: try again in a minute.".into(),
            )];
        };
        let fee = ((p.size * mark * self.config.fee_percent / 100.0).ceil() as i64).max(1);
        // A position past its maintenance margin is the timer's to liquidate.
        if p.equity(mark) <= p.size * mark * self.config.maintenance_percent / 100.0 {
            return vec![Out::Error(format!("#{id} is being liquidated."))];
        }
        if p.pnl(mark) > 0.0 && (p.pnl(mark).floor() as i64) > economy.pool(INSURANCE) {
            return vec![Out::Error(
                "The insurance fund can't pay that profit now: try again later.".into(),
            )];
        }
        match self.settle(economy, i, mark, fee, "futures_close") {
            Ok((got, pnl)) => vec![Out::Reply(format!(
                "Closed #{id} at {}: {}{} (fee {}). You got {}.",
                fmt_price(mark),
                if pnl >= 0.0 { "+" } else { "-" },
                economy.money(pnl.abs().floor() as i64),
                economy.money(fee),
                economy.money(got)
            ))],
            Err(e) => vec![Out::Error(e)],
        }
    }

    fn list(&self, prices: &dyn Prices, me: &Player, now: i64) -> Vec<Out> {
        let mine = uuid_string(me.uuid);
        let lines: Vec<Out> = self
            .data
            .positions
            .iter()
            .filter(|p| p.owner == mine)
            .map(|p| Out::Reply(self.describe(p, prices.mark(&p.item, now))))
            .collect();
        if lines.is_empty() {
            return vec![Out::Reply(
                "You have no position. /futures long|short <item> <margin> [leverage]".into(),
            )];
        }
        lines
    }

    /// `#3 LONG 5x 120 wheat @1.20 · mark 1.31 · +13 coins · liq 0.96`.
    pub fn describe(&self, p: &Position, mark: Option<f64>) -> String {
        let pnl = mark.map(|m| p.pnl(m)).unwrap_or(0.0);
        format!(
            "#{} {} {}x {} {} @{} · mark {} · {}{} coins · liq {}",
            p.id,
            if p.long { "LONG" } else { "SHORT" },
            p.leverage,
            fmt_price(p.size),
            item_name(&p.item),
            fmt_price(p.entry),
            mark.map_or("?".into(), fmt_price),
            if pnl >= 0.0 { "+" } else { "-" },
            pnl.abs().floor(),
            fmt_price(p.liquidation(self.config.maintenance_percent))
        )
    }

    /// Once a second from the glue; the work once a minute (marking and
    /// liquidations) and once an hour (funding). `online`: who is on.
    pub fn tick(
        &mut self,
        economy: &mut Economy,
        prices: &dyn Prices,
        online: &[u128],
        now: i64,
    ) -> Vec<Out> {
        self.online = online.iter().copied().collect();
        let mut out = Vec::new();
        if !self.data.notes.is_empty() {
            for &u in online {
                if let Some(notes) = self.data.notes.remove(&uuid_string(u)) {
                    out.extend(notes.into_iter().map(|n| Out::Tell(u, n)));
                    self.dirty = true;
                }
            }
        }
        let minute = now.div_euclid(60_000);
        if minute == self.data.minute {
            return out;
        }
        self.data.minute = minute;
        self.dirty = true;
        if self.data.positions.is_empty() {
            self.data.hour = now.div_euclid(3_600_000);
            return out;
        }
        let hour = now.div_euclid(3_600_000);
        if hour != self.data.hour {
            // 0: never ticked with positions (opened before the first tick).
            let first = self.data.hour == 0;
            self.data.hour = hour;
            if !first {
                out.extend(self.fund(economy, prices, now));
            }
        }
        // Marks, once per item.
        let mut marks: HashMap<String, Option<f64>> = HashMap::new();
        let mm = self.config.maintenance_percent / 100.0;
        let mut i = 0;
        while i < self.data.positions.len() {
            let p = self.data.positions[i].clone();
            let mark = *marks
                .entry(p.item.clone())
                .or_insert_with(|| prices.mark(&p.item, now));
            let Some(mark) = mark else {
                i += 1;
                continue;
            };
            if p.equity(mark) > p.size * mark * mm {
                i += 1;
                continue;
            }
            match self.settle(economy, i, mark, 0, "futures_liquidated") {
                Ok(_) => {
                    self.data.liquidations += 1;
                    let line = format!(
                        "Your position #{} ({} {}x on {}) was liquidated at {}: its margin of {} went to the insurance fund.",
                        p.id,
                        if p.long { "LONG" } else { "SHORT" },
                        p.leverage,
                        item_name(&p.item),
                        fmt_price(mark),
                        economy.money(p.margin)
                    );
                    self.notify(&mut out, &p.owner, line);
                    if p.margin >= self.config.announce_margin {
                        out.push(Out::All(format!(
                            "Liquidation! {}'s {}x {} on {} ({} margin) at {}.",
                            p.name,
                            p.leverage,
                            if p.long { "long" } else { "short" },
                            item_name(&p.item),
                            economy.money(p.margin),
                            fmt_price(mark)
                        )));
                    }
                }
                Err(e) => {
                    self.problems
                        .push(format!("futures liquidation #{}: {e}", p.id));
                    i += 1;
                }
            }
        }
        out
    }

    /// The hour's funding: the bigger side of each item pays the fund
    /// `funding_rate` of its size, from its margins.
    fn fund(&mut self, economy: &mut Economy, prices: &dyn Prices, now: i64) -> Vec<Out> {
        let mut items: Vec<String> = self.data.positions.iter().map(|p| p.item.clone()).collect();
        items.sort();
        items.dedup();
        for id in items {
            let Some(mark) = prices.mark(&id, now) else {
                continue;
            };
            let (l, s) = self.interest(&id, mark);
            let rate = self.funding_rate(l, s) / 100.0;
            if rate <= 0.0 {
                continue;
            }
            let long_pays = l > s;
            for p in self
                .data
                .positions
                .iter_mut()
                .filter(|p| p.item == id && p.long == long_pays)
            {
                let due = ((p.size * mark * rate).ceil() as i64).clamp(0, p.margin);
                if due > 0 {
                    match economy.pool_move("futures_funding", POOL, INSURANCE, due, "") {
                        Ok(()) => {
                            p.margin -= due;
                            self.data.funding += due;
                        }
                        Err(e) => self
                            .problems
                            .push(format!("futures funding #{}: {e}", p.id)),
                    }
                }
            }
        }
        self.dirty = true;
        Vec::new()
    }

    pub fn position(&self, id: u64) -> Option<&Position> {
        self.data.positions.iter().find(|p| p.id == id)
    }

    /// `uuid`'s menu, cell by cell.
    pub fn layout(&self, uuid: u128) -> Vec<Cell> {
        let armed = self.views.get(&uuid).and_then(|v| v.1);
        let mut cells = vec![Cell::Empty; SIZE];
        for (cell, p) in cells
            .iter_mut()
            .zip(self.positions_of(uuid).into_iter().take(SHOWN))
        {
            *cell = Cell::Position(p.id, armed == Some(p.id));
        }
        cells[SHOWN + 2] = Cell::Fund;
        cells[SHOWN + 4] = Cell::Help;
        cells[SIZE - 1] = Cell::Close;
        cells
    }

    /// A click on `slot` of `uuid`'s menu: a first click on a position arms
    /// it, a second closes it; anything else disarms.
    pub fn click(&mut self, uuid: u128, slot: usize) -> Click {
        let cell = self.layout(uuid).get(slot).cloned().unwrap_or(Cell::Empty);
        let Some(view) = self.views.get_mut(&uuid) else {
            return Click::Nothing;
        };
        match cell {
            Cell::Close => Click::Close,
            Cell::Position(id, true) => {
                view.1 = None;
                Click::Shut(id)
            }
            Cell::Position(id, false) => {
                view.1 = Some(id);
                Click::Redraw
            }
            _ if view.1.is_some() => {
                view.1 = None;
                Click::Redraw
            }
            _ => Click::Nothing,
        }
    }

    pub fn positions_of(&self, uuid: u128) -> Vec<&Position> {
        let mine = uuid_string(uuid);
        self.data
            .positions
            .iter()
            .filter(|p| p.owner == mine)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config as EcoConfig;
    use std::cell::RefCell;
    use std::fs;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;
    const WHALE: u128 = 3;
    const T0: i64 = 1_793_577_600_000;
    const MIN: i64 = 60_000;

    /// Prices: a mark per item and a volume, set by the test.
    struct Fake {
        marks: RefCell<HashMap<String, f64>>,
        volume: u64,
    }
    impl Prices for Fake {
        fn mark(&self, id: &str, _now: i64) -> Option<f64> {
            self.marks.borrow().get(id).copied()
        }
        fn volume(&self, _id: &str, _now: i64) -> u64 {
            self.volume
        }
    }
    fn fake(diamond: f64) -> Fake {
        Fake {
            marks: RefCell::new(HashMap::from([("minecraft:diamond".to_owned(), diamond)])),
            volume: 10_000,
        }
    }
    fn set(f: &Fake, v: f64) {
        f.marks.borrow_mut().insert("minecraft:diamond".into(), v);
    }

    fn me(uuid: u128) -> Player {
        Player {
            uuid,
            name: format!("P{uuid}"),
            level: 0,
        }
    }

    fn setup(name: &str, fund: i64) -> (Economy, Futures, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("smp-fut-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        e.now = T0;
        for u in [ALEX, STEVE, WHALE] {
            e.seen(u, &format!("P{u}"));
            e.adjust("sell", u, 10_000, "").unwrap();
        }
        if fund > 0 {
            e.to_pool("seed", WHALE, TREASURY, fund, "").unwrap();
            e.pool_move("insure", TREASURY, INSURANCE, fund, "")
                .unwrap();
        }
        let f = Futures::open(&e, Config::default());
        (e, f, dir)
    }

    fn total(e: &Economy) -> i64 {
        [ALEX, STEVE, WHALE]
            .iter()
            .map(|u| e.balance(*u))
            .sum::<i64>()
            + e.pool(TREASURY)
            + e.pool(INSURANCE)
            + e.pool(POOL)
    }

    fn run(
        f: &mut Futures,
        e: &mut Economy,
        p: &Fake,
        who: u128,
        now: i64,
        line: &str,
    ) -> Vec<Out> {
        f.command(e, p, &me(who), now, line).unwrap()
    }

    #[test]
    fn a_position_keeps_its_profit_when_the_price_becomes_one_price() {
        // Long diamond at a mark of 260 (old mid 250); the base becomes 200.
        let (_e, mut f, dir) = setup("rebase", 0);
        f.data.positions.push(Position {
            id: 1,
            owner: "a".into(),
            name: "A".into(),
            item: "minecraft:diamond".into(),
            long: true,
            size: 10.0,
            entry: 260.0,
            margin: 500,
            leverage: 5,
            opened: T0,
        });
        f.rebase(&BTreeMap::from([("minecraft:diamond".to_owned(), 0.8)]));
        // The same mark, 1.04 times the base: no gain, no loss.
        assert!(f.data.positions[0].pnl(208.0).abs() < 1e-9);
        assert!(f.dirty);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_liquidation_price_is_where_equity_meets_maintenance() {
        let p = Position {
            id: 1,
            owner: String::new(),
            name: String::new(),
            item: String::new(),
            long: true,
            size: 10.0,
            entry: 100.0,
            margin: 100,
            leverage: 10,
            opened: 0,
        };
        let liq = p.liquidation(5.0);
        assert!((p.equity(liq) - 0.05 * p.size * liq).abs() < 1e-6);
        assert!(liq > 90.0 && liq < 95.0, "{liq}");
        let s = Position { long: false, ..p };
        let liq = s.liquidation(5.0);
        assert!((s.equity(liq) - 0.05 * s.size * liq).abs() < 1e-6);
        assert!(liq > 104.0 && liq < 106.0, "{liq}");
    }

    #[test]
    fn open_and_close_pays_profit_from_the_fund_and_fees_to_the_treasury() {
        let (mut e, mut f, _dir) = setup("close", 5_000);
        let prices = fake(100.0);
        let start = total(&e);
        let out = run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 1000 5x",
        );
        assert!(
            matches!(&out[0], Out::Reply(s) if s.starts_with("Opened #1: LONG 5x on 50.00 diamond at 100 (margin 1,000 coins, fee 10 coins). Liquidated at 8")),
            "{out:?}"
        );
        assert_eq!(
            (e.balance(ALEX), e.pool(POOL), e.pool(TREASURY)),
            (8_990, 1_000, 10)
        );
        set(&prices, 110.0);
        let out = run(&mut f, &mut e, &prices, ALEX, T0 + MIN, "futures close 1");
        // +500 profit, 11 fee on 5,500.
        assert_eq!(
            out,
            [Out::Reply(
                "Closed #1 at 110: +500 coins (fee 11 coins). You got 1,489 coins.".into()
            )]
        );
        assert_eq!(
            (e.balance(ALEX), e.pool(INSURANCE), e.pool(POOL)),
            (10_479, 4_500, 0)
        );
        assert_eq!(total(&e), start, "nothing made");
    }

    #[test]
    fn caps_refuse_what_the_market_and_the_fund_cant_carry() {
        let (mut e, mut f, _dir) = setup("caps", 300);
        let prices = fake(100.0);
        let err = |o: Vec<Out>| match &o[0] {
            Out::Error(s) => s.clone(),
            o => panic!("{o:?}"),
        };
        assert!(
            err(run(
                &mut f,
                &mut e,
                &prices,
                ALEX,
                T0,
                "futures long diamond 100 11"
            ))
            .contains("1x to 10x")
        );
        assert!(
            err(run(
                &mut f,
                &mut e,
                &prices,
                ALEX,
                T0,
                "futures long dirt 100"
            ))
            .starts_with("No futures on dirt")
        );
        assert!(
            err(run(
                &mut f,
                &mut e,
                &prices,
                ALEX,
                T0,
                "futures long diamond 5"
            ))
            .starts_with("The smallest margin")
        );
        // A thin market: 2x.
        let thin = Fake {
            volume: 100,
            ..fake(100.0)
        };
        assert!(
            err(run(
                &mut f,
                &mut e,
                &thin,
                ALEX,
                T0,
                "futures long diamond 100 3"
            ))
            .contains("1x to 2x")
        );
        // The fund (300) covers 30% of 1,000 net exposure, no more.
        assert!(matches!(
            &run(
                &mut f,
                &mut e,
                &prices,
                ALEX,
                T0,
                "futures long diamond 500 2"
            )[0],
            Out::Reply(_)
        ));
        assert!(
            err(run(
                &mut f,
                &mut e,
                &prices,
                STEVE,
                T0,
                "futures long diamond 100"
            ))
            .starts_with("The insurance fund")
        );
        // The other side lowers the house's exposure: open.
        assert!(matches!(
            &run(
                &mut f,
                &mut e,
                &prices,
                STEVE,
                T0,
                "futures short diamond 900"
            )[0],
            Out::Reply(_)
        ));
        // One player's limit.
        e.adjust("sell", WHALE, 100_000, "").unwrap();
        assert!(
            err(run(
                &mut f,
                &mut e,
                &prices,
                WHALE,
                T0,
                "futures short diamond 4000 6"
            ))
            .starts_with("That's past your limit")
        );
        // Open interest: 3 days of volume = 10,000 × 100 × 3 coins; a thin item fills fast.
        let thin = Fake {
            volume: 10,
            ..fake(100.0)
        };
        assert!(
            err(run(
                &mut f,
                &mut e,
                &thin,
                WHALE,
                T0,
                "futures short diamond 4000"
            ))
            .starts_with("Open interest on diamond is full")
        );
    }

    #[test]
    fn liquidation_runs_on_the_minute_offline_and_never_takes_more_than_the_margin() {
        let (mut e, mut f, dir) = setup("liq", 5_000);
        let prices = fake(100.0);
        let start = total(&e);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 1000 10",
        );
        f.tick(&mut e, &prices, &[], T0); // the minute's mark: nothing to do.
        // A gap: the mark falls 20% between two minutes (past the liquidation
        // at about 94.7), Alex offline.
        set(&prices, 80.0);
        assert!(
            f.tick(&mut e, &prices, &[], T0 + 1000).is_empty(),
            "once a minute, not each second"
        );
        let out = f.tick(&mut e, &prices, &[], T0 + MIN);
        assert!(
            matches!(&out[0], Out::All(s) if s.starts_with("Liquidation! P1's 10x long on diamond")),
            "{out:?}"
        );
        assert!(f.data.positions.is_empty());
        assert_eq!(
            e.balance(ALEX),
            10_000 - 1_000 - 20,
            "the margin and the fee, never more"
        );
        assert_eq!(e.pool(INSURANCE), 5_000 + 1_000);
        assert_eq!(f.data.gaps, 1_000, "the fund ate the 1,000 past the margin");
        assert_eq!(total(&e), start);
        // The note waits through a restart and reaches Alex when back.
        f.store(&mut e);
        e.save().unwrap();
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        let mut f = Futures::open(&e, Config::default());
        let out = f.tick(&mut e, &prices, &[ALEX], T0 + MIN + 1000);
        assert!(
            matches!(&out[0], Out::Tell(ALEX, s) if s.contains("was liquidated at 80.00: its margin of 1,000 coins went to the insurance fund")),
            "{out:?}"
        );
    }

    #[test]
    fn a_restart_between_minutes_liquidates_once() {
        let (mut e, mut f, dir) = setup("restart", 5_000);
        let prices = fake(100.0);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures short diamond 500 10",
        );
        f.tick(&mut e, &prices, &[], T0);
        set(&prices, 112.0);
        f.store(&mut e);
        e.save().unwrap();
        // The server stops; on start the same minute passes once.
        let mut e = Economy::open(&dir, EcoConfig::default()).unwrap();
        let mut f = Futures::open(&e, Config::default());
        f.tick(&mut e, &prices, &[], T0 + MIN);
        f.tick(&mut e, &prices, &[], T0 + MIN + 1);
        assert_eq!((f.data.liquidations, f.data.positions.len()), (1, 0));
        assert_eq!(e.pool(POOL), 0);
        f.store(&mut e);
        e.save().unwrap();
        let again = Economy::open(&dir, EcoConfig::default()).unwrap();
        assert_eq!(
            Futures::open(&again, Config::default()).data.liquidations,
            1
        );
    }

    #[test]
    fn alts_long_and_short_against_each_other_only_lose_fees() {
        let (mut e, mut f, _dir) = setup("alts", 5_000);
        let prices = fake(100.0);
        let start = e.balance(ALEX) + e.balance(STEVE);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 1000 5",
        );
        run(
            &mut f,
            &mut e,
            &prices,
            STEVE,
            T0,
            "futures short diamond 1000 5",
        );
        // The mark walks up 1% a minute (a TWAP can't jump) until the short
        // is liquidated (about 114.3); the long is closed right then. The
        // pair was hedged the whole time: the short's whole margin went to
        // the fund, more than the long made, so the pair lost.
        let mut now = T0;
        let mut step = 0;
        while f.data.positions.len() == 2 {
            step += 1;
            now += MIN;
            set(&prices, 100.0 * (1.0 + 0.01 * f64::from(step)));
            f.tick(&mut e, &prices, &[ALEX, STEVE], now);
            assert!(step < 30);
        }
        assert_eq!(step, 15);
        assert!(f.data.positions[0].long, "the short is gone");
        assert_eq!(
            f.data.gaps, 0,
            "no loss past a margin while the mark moves smoothly"
        );
        let id = f.data.positions[0].id;
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            now,
            &format!("futures close {id}"),
        );
        let end = e.balance(ALEX) + e.balance(STEVE);
        // About +750 on the long, −1,000 on the short, 32 in fees.
        assert!(
            (280..=285).contains(&(start - end)),
            "the pair lost: {start} → {end}"
        );
        // Funding: balanced sides pay none.
        assert_eq!(f.data.funding, 0);
    }

    #[test]
    fn the_crowded_side_pays_funding_to_the_fund() {
        let (mut e, mut f, _dir) = setup("funding", 5_000);
        let prices = fake(100.0);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 1000 5",
        );
        f.tick(&mut e, &prices, &[], T0);
        let fund = e.pool(INSURANCE);
        // An hour later: all long, 0.05% of 5,000 = 2.5 → 3 coins.
        f.tick(&mut e, &prices, &[], T0 + 3_600_000);
        assert_eq!(e.pool(INSURANCE), fund + 3);
        assert_eq!(f.data.positions[0].margin, 997);
        assert_eq!(e.pool(POOL), 997);
    }

    #[test]
    fn the_menu_closes_a_position_on_the_second_click_only() {
        let (mut e, mut f, _dir) = setup("menu", 5_000);
        let prices = fake(100.0);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 100 2",
        );
        run(
            &mut f,
            &mut e,
            &prices,
            STEVE,
            T0,
            "futures short diamond 100 2",
        );
        f.views.insert(ALEX, (Some(1), None));
        assert_eq!(
            f.layout(ALEX)[..2],
            [Cell::Position(1, false), Cell::Empty],
            "only Alex's"
        );
        assert_eq!(f.click(ALEX, 0), Click::Redraw);
        assert_eq!(f.layout(ALEX)[0], Cell::Position(1, true));
        assert_eq!(f.click(ALEX, 5), Click::Redraw, "elsewhere disarms");
        assert_eq!(f.click(ALEX, 0), Click::Redraw);
        assert_eq!(f.click(ALEX, 0), Click::Shut(1));
        assert_eq!(f.click(ALEX, SIZE - 1), Click::Close);
        // Steve can't close Alex's position, not even by its number.
        let out = run(&mut f, &mut e, &prices, STEVE, T0, "futures close 1");
        assert_eq!(out, [Out::Error("You have no position #1.".into())]);
    }

    #[test]
    fn a_whale_moving_spot_moves_the_mark_only_through_the_average() {
        // The futures never read a trade: only `Prices::mark`, the market's
        // 10-minute average. A whale who pushes spot 20% for one minute
        // moves a 10-minute average by 2%: a 10x long's liquidation (about
        // 5.3% away) holds.
        let (mut e, mut f, _dir) = setup("whale", 5_000);
        let prices = fake(100.0);
        run(
            &mut f,
            &mut e,
            &prices,
            ALEX,
            T0,
            "futures long diamond 1000 10",
        );
        let twap = |minutes: &[f64]| minutes.iter().sum::<f64>() / minutes.len() as f64;
        let mut window = vec![100.0; 10];
        window[9] = 80.0; // one minute dumped 20%
        set(&prices, twap(&window));
        f.tick(&mut e, &prices, &[], T0 + MIN);
        assert_eq!(
            f.data.positions.len(),
            1,
            "not liquidated by one minute of manipulation"
        );
    }
}
