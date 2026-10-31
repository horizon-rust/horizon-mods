// The server market: the server buys and sells vanilla items at the prices
// of `config.json`'s `market`, the economy's way to earn without changing
// survival. `/worth`, `/sell hand|all|<item> [count]`, `/buy <item> [count]`.
// Only plain items (no damage, no name or enchantment) are traded, and only
// by players in survival or adventure mode. Items move first, then money,
// both logged; a sale the log refuses gives the items back. No game types:
// the glue lends the acting player's items as a `Bag`. Added to the game as
// `game::smp_market`; also a module of this crate, for its tests.

use super::smp_economy::{Economy, MAX_AMOUNT, Out, Player, parse_amount};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// By item id, `minecraft:diamond`.
    pub items: BTreeMap<String, Price>,
}

impl Default for Config {
    /// A small starting list the owner edits: farm and mine produce the
    /// server buys, a few things it sells, buying always dearer than selling.
    fn default() -> Self {
        let list: [(&str, i64, i64); 18] = [
            ("minecraft:wheat", 0, 1),
            ("minecraft:carrot", 0, 1),
            ("minecraft:potato", 0, 1),
            ("minecraft:sugar_cane", 0, 1),
            ("minecraft:pumpkin", 0, 3),
            ("minecraft:melon_slice", 0, 1),
            ("minecraft:cactus", 0, 1),
            ("minecraft:oak_log", 4, 1),
            ("minecraft:coal", 8, 2),
            ("minecraft:iron_ingot", 30, 8),
            ("minecraft:gold_ingot", 40, 10),
            ("minecraft:redstone", 4, 1),
            ("minecraft:lapis_lazuli", 8, 2),
            ("minecraft:emerald", 80, 20),
            ("minecraft:diamond", 400, 100),
            ("minecraft:bread", 5, 0),
            ("minecraft:cooked_beef", 6, 0),
            ("minecraft:torch", 1, 0),
        ];
        Self {
            enabled: true,
            items: list
                .iter()
                .map(|(id, buy, sell)| {
                    (
                        (*id).to_owned(),
                        Price {
                            buy: *buy,
                            sell: *sell,
                        },
                    )
                })
                .collect(),
        }
    }
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
}

impl Market {
    /// Runs `/worth`, `/sell` or `/buy`; None for another command. `trades`
    /// is false for a player in creative or spectator mode.
    pub fn command(
        &self,
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
            return Some(self.worth(economy, bag, args));
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

    fn price(&self, id: &str) -> Price {
        self.config.items.get(id).copied().unwrap_or_default()
    }

    fn worth(&self, economy: &Economy, bag: &mut dyn Bag, args: &[&str]) -> Vec<Out> {
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
        vec![Out::Reply(format!("{name}: {}.", parts.join(", ")))]
    }

    fn sell(
        &self,
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
            // Never more than the account can be paid for at once.
            let cap = (MAX_AMOUNT / price).min(u32::MAX as i64) as u32;
            let have = bag.count(&id).min(at_most).min(cap);
            if have == 0 {
                out.push(Out::Error(format!("You have no plain {name}.")));
                continue;
            }
            let taken = bag.take(&id, have);
            let pay = price * i64::from(taken);
            let kind = format!("sell {taken} {id}");
            match economy.adjust(&kind, me.uuid, pay, "") {
                Ok(_) => {
                    total += pay;
                    out.push(Out::Reply(format!(
                        "Sold {taken} {name} for {}.",
                        economy.money(pay)
                    )));
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

    fn buy(
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
        let cost = price * i64::from(count);
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
            let refund = price * i64::from(count - given);
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
        let dir = std::env::temp_dir().join(format!("smp-market-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, EconomyConfig::default()).unwrap();
        e.seen(ALEX, "Alex");
        let me = Player {
            uuid: ALEX,
            name: "Alex".into(),
            level: 0,
        };
        (
            e,
            Market {
                config: Config::default(),
            },
            me,
            dir,
        )
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
        let (mut e, m, me, dir) = setup("sell");
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
        let (mut e, m, me, dir) = setup("hand");
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
        let (mut e, m, me, dir) = setup("buy");
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
        let out = m.command(&mut e, &me, true, &mut b, "buy wheat").unwrap();
        assert_eq!(out, [Out::Error("The market doesn't sell wheat.".into())]);
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
            worth,
            [Out::Reply(
                "diamond: the market pays 100 coins each, sells it for 400 coins each.".into()
            )]
        );
        m.config.enabled = false;
        let out = m.command(&mut e, &me, true, &mut b, "sell all").unwrap();
        assert_eq!(out, [Out::Error("The market is closed.".into())]);
        assert_eq!((e.balance(ALEX), b.items["minecraft:diamond"]), (0, 64));
        assert!(m.command(&mut e, &me, true, &mut b, "balance").is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
