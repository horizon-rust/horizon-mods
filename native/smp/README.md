# SMP (Horizon native server mod)

The official Horizon server's survival layer ([design](https://github.com/horizon-rust/horizon/blob/main/docs/smp.md)).
It is **not** part of Horizon's own server: only a server that builds this mod into its engine has it. It isn't listed
in the registry.

Players install nothing: it speaks only what a stock Horizon client already shows (commands with completions, chat
lines, vanilla chest menus the server fills with ordinary items (the trade menu); later sounds, titles, the action
bar, boss bars and the sidebar).

This version: the economy, the server market, player trades, the coinflip, the lottery, places (spawn, homes, warps,
teleport requests, the rules) and land claims.

| Command | Does |
|---|---|
| `/balance [player]`, `/bal` | your balance, or another player's |
| `/pay <player> <amount>` | gives money to a player who has an account (anyone who has played since the mod was on) |
| `/baltop [page]` | the richest players, 10 a page |
| `/eco give\|take\|set <player> <amount>` | operators of level 3: corrections, logged |
| `/worth [item]` | what the market pays and asks for an item (the one in your hand without a name) |
| `/sell hand [count]`, `/sell <item> [count]`, `/sell all` | sells to the market: the stack in your hand, an item, or everything it buys |
| `/buy <item> [count]` | buys from the market, only what fits in your inventory |
| `/trade <player>` | asks a player to trade (60 s to answer) |
| `/trade accept [player]`, `/trade deny [player]` | answers a request |
| `/trade money <amount>` | the money you add to your offer (0: none) |
| `/trade cancel` | ends your trade, or withdraws your request |
| `/coinflip <amount> [player]` | stakes an amount for anyone (or one player) to match; off unless `gambling.enabled` |
| `/lottery`, `/lottery buy <tickets>` | the pot, your tickets and the time to the draw; buys tickets (off unless `gambling.enabled`) |
| `/coinflip accept <player>`, `/coinflip cancel`, `/coinflip list` | takes an offer (a fair coin decides, the winner gets both stakes), withdraws yours, lists those waiting |
| `/spawn` | to the server's spawn; `/setspawn` (operators of level 3) sets it where you stand |
| `/home [name]`, `/sethome [name]`, `/delhome [name]`, `/homes` | your homes (`home` when unnamed): go, set or move one where you stand, delete, list |
| `/warp <name>`, `/warps` | to a warp, the list; `/setwarp <name>`, `/delwarp <name>` for operators of level 3 |
| `/tpa <player>`, `/tpaccept [player]`, `/tpdeny [player]` | asks to go to a player (60 s to answer); answers a request |
| `/rules` | the server's rules |
| `/claim`, `/claim info`, `/unclaim` | claims the chunk you stand in (all its height), says whose it is, frees it (operators of level 3: anyone's) |
| `/claims` | your chunks, how many you may hold, and who you trust |
| `/trust <player>`, `/untrust <player>` | lets a player (or no longer) build and use things on all your land |

The market trades only plain items (not worn, named or enchanted), and only with players in survival or adventure
mode. Items move first, then money, both logged; a sale the log refuses gives the items back, and a purchase checks the
money and the room before anything moves.

**Trades.** Once accepted, both players see a 6-row chest menu: their own offer on the left (16 stacks), the other's
on the right, each side's money, their confirm and cancel buttons, and whether the other has confirmed. A click on a
stack in your inventory, below the menu, adds it to your offer; a click on it in your offer takes it back. The menu
moves nothing: offered stacks stay in their owners' inventories, the menu shows copies, and a stack that changes
(used, dropped, grown by a pickup) leaves the offer. Any change clears both confirmations. When both have confirmed,
the server checks that each still holds exactly what they offer, has room for what they get and has the money, logs
the trade, and swaps everything in one step, or nothing (and both must confirm again). Closing the menu,
`/trade cancel`, leaving, dying or the other player leaving ends the trade with nothing moved. Survival and
adventure players only.

**Coinflips.** Off by default: the owner turns them on in `config.json` (`gambling`: `enabled`, `min_bet` 10,
`max_bet` 1000, `cooldown_seconds` 30, `offer_seconds` 60, `fee_percent` 0). A stake is held from the offer, in
`economy.json` with the balances, until the coin falls; an offer cancelled, not taken in time, or whose player
left gives it back, and so does the next start for one the server stopped with. The coin is a fresh random key from
the system, never the world's random. Only the in-game currency is staked and won.

**The lottery.** Part of `gambling` (`ticket_price` 10, `max_tickets` 100 a player a draw, `lottery_seconds` 3600
from a round's first ticket to its draw). Tickets are held like coinflip stakes; at the draw one ticket is picked at
random among all sold (more tickets, more chances), its owner gets the pot (less `fee_percent`), and everyone hears
it. A round with a single player, or one the server stopped with, gives every ticket back.

**Places.** A teleport waits `warmup_seconds` (3) in `config.json`'s `places`; a player who moves or is hurt meanwhile
stays where they are, and `cooldown_seconds` (10) passes between two of one player's teleports. `/tpa` takes the
asker to where the other player is when the warmup ends. Homes grow with the time played on the server since the mod
was on (`homes`: `[hours, homes]`, by default 1 home, 2 from 5 hours, 3 from 20, 5 from 100). `rules` holds `/rules`'
lines. Teleports are vanilla's `tp`, run as the server (`execute in <level> run tp`), so they cross levels and land
as any teleport does.

**Claims.** In a claimed chunk, only its owner, the players they trust and operators of level 3 break blocks or right
click blocks: doors, chests, levers, buttons, placing a block, a bucket or flint on a block. Anyone else's click does
nothing, and the action bar says whose land it is. The chunks a player may hold grow with the time played here
(`claims.chunks`: `[hours, chunks]`, by default 4, 16 from 5 hours, 36 from 20, 64 from 100); `claims.enabled`
turns them off. Not guarded yet: explosions, fire, pistons, liquids flowing in, entities (item frames, armour stands,
animals) and buckets used in the air. The server's `spawn-protection` (vanilla's) guards the spawn.

The currency is earned in game only: it can't be bought with real money or cashed out (Mojang's usage guidelines).

## Data

Everything is in the world's folder, `world/horizon-smp/`, so a backup of the world carries it. A world without that
folder is untouched until the mod first runs on it; it changes no block and no player.

| File | Holds |
|---|---|
| `config.json` | `currency` (default `coins`), `starting_balance` (default 0), and `market`: `enabled` and `items`, each item's `buy` (what the market asks) and `sell` (what it pays), 0 closing that side; written with the defaults the first time, and again when a newer version adds keys (your values kept) |
| `economy.json` | balances by player UUID, with the name last seen, and `held`: coinflip stakes and lottery tickets being decided |
| `places.json` | the spawn, the warps, each player's homes, and the seconds each player has played (written once a minute) |
| `claims.json` | each claimed chunk (`<dimension> <chunk x> <chunk z>`) with its owner, and whom each owner trusts |
| `audit.log` | one JSON line per change, written before the change: `time` (ms), `kind` (`open`, `pay`, `eco_give`...), `amount`, `from`/`to` with their balance before, `by` for an operator; a trade's line (`trade`: both players, their items and money) comes before its swap, `trade_failed` after one that didn't go through |

`economy.json` is written whole to a temporary file, flushed and renamed over the old one, at most once a server tick
and when the world closes: a crash leaves the old file or the new one. If it can't be read, the economy stays off (the
commands say so) rather than start from nothing.

## Made on the hook points

It hooks only Horizon's native hook points: `native_register_commands` and `native_perform_command` (API 8), the
acting player's items (API 9), the server menus and `native_exchange_items` (API 10), the players' health and
`native_run_command` (API 21), `native_destroy_block` (API 14), `native_right_click_block`, the acting player and the
action bar (API 21), the server tick and the world lifecycle. `compatibility.json` names `"hooks": 21`.

| File | What it is |
|---|---|
| `mixins/smp/horizon-mixin.toml` | The bundle: nine modules and the HEAD hooks |
| `mixins/smp/smp_store.rs`, `smp_economy.rs`, `smp_market.rs`, `smp_trade.rs`, `smp_games.rs`, `smp_lottery.rs`, `smp_places.rs`, `smp_claims.rs` | The rules, no game types (a trade's stacks are a type parameter): added to the game as `game::smp_store`, `game::smp_economy`, `game::smp_market`, `game::smp_trade`, `game::smp_games`, `game::smp_lottery`, `game::smp_places`, `game::smp_claims`; also this crate's modules (`src/lib.rs`), for their tests |
| `mixins/smp/glue.rs` | The state for the open world and the edges to the game's types (`game::smp_glue`) |
| `mixins/smp/{register,command,tick,lifecycle,menu_clicked,menu_closed,destroy_block,right_click_block}.rs` | One small helper per hook |
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
