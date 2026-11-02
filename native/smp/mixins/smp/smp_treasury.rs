// The Treasury's commands. Every fee and house edge goes into the Treasury
// pool (smp_economy's `TREASURY`), never away, and operators pay only from
// it (`/treasury pay`): nobody makes money. `/treasury` shows the balance,
// income and spending; `/treasury log` the last movements. No game types.
// Added to the game as `game::smp_treasury`; also a module of this crate,
// for its tests.

use super::smp_economy::{DAY_MS, Economy, INSURANCE, Out, Player, TREASURY, parse_amount};
use super::smp_store::uuid_string;
use std::collections::BTreeMap;

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

/// Runs `/treasury ...`; None for another command.
pub fn command(economy: &mut Economy, me: &Player, line: &str) -> Option<Vec<Out>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    Some(match words.as_slice() {
        ["treasury"] => show(economy),
        ["treasury", "log"] => log(economy),
        ["treasury", "pay", who, amount, reason @ ..] => {
            pay(economy, me, who, amount, &reason.join(" "))
        }
        ["treasury", ..] => vec![Out::Error(
            "Usage: /treasury, /treasury log, /treasury pay <player> <amount> <reason>".into(),
        )],
        _ => return None,
    })
}

fn show(economy: &Economy) -> Vec<Out> {
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
            "Treasury: {} (insurance fund {}). Fees and house edges go in; grants, refunds and prizes come out. Nobody can make money.",
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
    lines.push(Out::Reply("Every payment: /treasury log.".into()));
    lines
}

fn log(economy: &Economy) -> Vec<Out> {
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
fn pay(economy: &mut Economy, me: &Player, who: &str, amount: &str, reason: &str) -> Vec<Out> {
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
            "Say why: /treasury pay <player> <amount> <reason> (an event prize, a bug refund...)"
                .into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp_economy::Config;
    use std::fs;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;

    #[test]
    fn only_operators_pay_from_the_treasury_and_never_more_than_it_holds() {
        let dir = std::env::temp_dir().join(format!("smp-treasury-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut e = Economy::open(&dir, Config::default()).unwrap();
        e.seen(ALEX, "Alex");
        e.seen(STEVE, "Steve");
        e.adjust("sell", ALEX, 1_000, "").unwrap();
        e.adjust("sell", STEVE, 20, "").unwrap();
        e.to_pool("ah_listing_fee", ALEX, TREASURY, 500, "")
            .unwrap();
        let steve = Player {
            uuid: STEVE,
            name: "Steve".into(),
            level: 0,
        };
        assert!(matches!(
            &command(&mut e, &steve, "treasury pay Steve 10 because").unwrap()[0],
            Out::Error(_)
        ));
        let op = Player {
            uuid: ALEX,
            name: "Alex".into(),
            level: 3,
        };
        assert!(
            matches!(&command(&mut e, &op, "treasury pay Steve 10").unwrap()[0], Out::Error(s) if s.starts_with("Say why"))
        );
        assert!(matches!(
            &command(&mut e, &op, "treasury pay Steve 501 too much").unwrap()[0],
            Out::Error(_)
        ));
        assert_eq!(
            command(&mut e, &op, "treasury pay Steve 200 bug refund #12").unwrap(),
            [Out::All("The Treasury paid 200 coins to Steve: bug refund #12 (by Alex). Treasury: 300 coins.".into())]
        );
        assert_eq!(e.balance(STEVE), 220);
        let log = command(&mut e, &steve, "treasury log").unwrap();
        assert!(
            matches!(&log[1], Out::Reply(s) if s.contains("-200 coins grant (Steve): bug refund #12, by Alex")),
            "{log:?}"
        );
        let shown = command(&mut e, &steve, "treasury").unwrap();
        assert!(matches!(&shown[0], Out::Reply(s) if s.starts_with("Treasury: 300 coins")));
        assert!(command(&mut e, &steve, "balance").is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
