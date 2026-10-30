# SMP (Horizon native server mod)

The official Horizon server's survival layer ([design](https://github.com/horizon-rust/horizon/blob/main/docs/smp.md)).
It is **not** part of Horizon's own server: only a server that builds this mod into its engine has it. It isn't listed
in the registry.

Players install nothing: it speaks only what a stock Horizon client already shows (commands with completions, chat
lines; later vanilla chest menus filled by the server, sounds, titles, the action bar, boss bars, the sidebar).

This version: the economy and the server market.

| Command | Does |
|---|---|
| `/balance [player]`, `/bal` | your balance, or another player's |
| `/pay <player> <amount>` | gives money to a player who has an account (anyone who has played since the mod was on) |
| `/baltop [page]` | the richest players, 10 a page |
| `/eco give\|take\|set <player> <amount>` | operators of level 3: corrections, logged |
| `/worth [item]` | what the market pays and asks for an item (the one in your hand without a name) |
| `/sell hand [count]`, `/sell <item> [count]`, `/sell all` | sells to the market: the stack in your hand, an item, or everything it buys |
| `/buy <item> [count]` | buys from the market, only what fits in your inventory |

The market trades only plain items (not worn, named or enchanted), and only with players in survival or adventure
mode. Items move first, then money, both logged; a sale the log refuses gives the items back, and a purchase checks the
money and the room before anything moves.

The currency is earned in game only: it can't be bought with real money or cashed out (Mojang's usage guidelines).

## Data

Everything is in the world's folder, `world/horizon-smp/`, so a backup of the world carries it. A world without that
folder is untouched until the mod first runs on it; it changes no block and no player.

| File | Holds |
|---|---|
| `config.json` | `currency` (default `coins`), `starting_balance` (default 0), and `market`: `enabled` and `items`, each item's `buy` (what the market asks) and `sell` (what it pays), 0 closing that side; written with the defaults the first time, and again when a newer version adds keys (your values kept) |
| `economy.json` | balances by player UUID, with the name last seen |
| `audit.log` | one JSON line per change, written before the change: `time` (ms), `kind` (`open`, `pay`, `eco_give`...), `amount`, `from`/`to` with their balance before, `by` for an operator |

`economy.json` is written whole to a temporary file, flushed and renamed over the old one, at most once a server tick
and when the world closes: a crash leaves the old file or the new one. If it can't be read, the economy stays off (the
commands say so) rather than start from nothing.

## Made on the hook points

It hooks only Horizon's native hook points: `native_register_commands` and `native_perform_command` (API 8), the
acting player's items (API 9), the server tick and the world lifecycle. `compatibility.json` names `"hooks": 9`.

| File | What it is |
|---|---|
| `mixins/smp/horizon-mixin.toml` | The bundle: four modules and the HEAD hooks |
| `mixins/smp/smp_store.rs`, `smp_economy.rs`, `smp_market.rs` | The rules, no game types: added to the game as `game::smp_store`, `game::smp_economy`, `game::smp_market`; also this crate's modules (`src/lib.rs`), for their tests |
| `mixins/smp/glue.rs` | The state for the open world and the edges to the game's types (`game::smp_glue`) |
| `mixins/smp/{register,command,tick,lifecycle}.rs` | One small helper per hook |
| `proof.py` | A dedicated server built with the mod and two real clients, on fresh temporary data |

## Build and prove

```sh
(cd native/smp && cargo test)                          # the rules
python3 native/tools/native.py check smp               # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py proof smp --resources /path/to/.local/horizon-client/26.3
```

## The official server's engine

The server runs `horizon-server` as usual, with `[engine] path` pointing at a game built with this mod:

```sh
NATIVE_NO_DEFAULT_FEATURES=1 python3 native/tools/native.py build smp --horizon <the release's commit>
```

prints the built `horizon-client` (`--no-default-features`: the server build, without audio and controllers). When
to deploy it on the official server is the owners' decision.
