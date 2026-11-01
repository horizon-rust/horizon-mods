# SMP (Horizon native server mod)

The official Horizon server's survival layer ([design](https://github.com/horizon-rust/horizon/blob/main/docs/smp.md)).
It is **not** part of Horizon's own server: only a server that builds this mod into its engine has it. It isn't listed
in the registry.

Players install nothing: it speaks only what a stock Horizon client already shows (commands with completions, chat
lines, vanilla chest menus the server fills with ordinary items (the trade menu); later sounds, titles, the action
bar, boss bars and the sidebar).

This version: the economy with its Treasury and taxes, the auction house, the bank, the server market and its `/shop` menu, player trades, the coinflip, the lottery, places (spawn, homes, warps,
teleport requests, the rules), land claims, and the scoreboard (a sidebar with your balance, playtime and the richest players, the Tab list's
header and footer).

| Command | Does |
|---|---|
| `/balance [player]`, `/bal` | your balance, or another player's |
| `/pay <player> <amount>` | gives money to a player who has an account (anyone who has played since the mod was on) |
| `/baltop [page]` | the richest players, 10 a page |
| `/eco take <player> <amount> [reason]`, `/money take ...` | operators of level 3: a cheat's gains go into the Treasury, logged with the reason. Nobody gives or sets money (`/eco give` and `/eco set` say so) |
| `/treasury`, `/treasury log` | the Treasury's balance, its income by tax and its spending (today and since it opened), its last movements |
| `/treasury pay <player> <amount> <reason>` | operators of level 3: pays a player from the Treasury (an event prize, a bug refund, a grant), never more than it holds; everyone sees it |
| `/jobs`, `/jobs deliver <n>` | the day's contracts: deliver items the state asks for, paid from the Treasury |
| `/eco stats` | the money supply and where it sits, what the server made and destroyed today, taxes and fees, how evenly money is spread |
| `/bank`, `/bank deposit <amount\|all>`, `/bank withdraw <amount\|all>` | your savings, which earn a small interest from the Treasury each night |
| `/ah` | the auction house: a chest menu of listings by category, cheapest a unit first; click one to buy it or bid; your listings (click to take one back) and your mail |
| `/ah sell <price> [count]` | lists the stack in your hand (or part of it) at a fixed price, for `listing_hours` (48); the listing fee (1%, at least 1 coin) goes to the Treasury |
| `/ah auction <start> [hours]` | auctions the stack in your hand from a starting bid, for 1 to 24 hours (12) |
| `/ah bid <listing> <amount>`, `/ah buy <listing>` | bids (the money is held until someone bids more or it ends), buys a fixed-price listing |
| `/ah cancel <listing>`, `/ah claim`, `/ah search <words>`, `/ah mine`, `/ah mail` | takes back a listing nobody bid on; gives you your mail; the menu on a search, your listings, your mail |
| `/eco taxes`, `/taxes` | the tax code and your own bill on Monday; `/eco taxes pay` pays land tax you owe |
| `/shop` | the market as a chest menu: categories, a click buys one, a shift-click 64, a click on a stack of yours sells it, "Sell all" |
| `/worth [item]` | what the market pays and asks for an item (the one in your hand without a name), and what you may still sell today |
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
| `/back` | back to where you died, or where your last teleport took you from (warmup and cooldown as any teleport) |
| `/afk` | marks you away from the keyboard until you move, turn or run a command |
| `/playtime [player]` | how long you (or a player who is on) have played here |
| `/claim`, `/claim info`, `/unclaim` | claims the chunk you stand in (all its height), says whose it is, frees it (operators of level 3: anyone's) |
| `/claim show` | draws the border of the chunk you stand in for 10 s, for you only: green yours or a trusting owner's, red someone else's, white free |
| `/claims` | your chunks, how many you may hold, and who you trust |
| `/trust <player>`, `/untrust <player>` | lets a player (or no longer) build and use things on all your land |
| `/nick <name>`, `/nick off`, `/nick` | operators of level 3 (with `ops_only`, the default): another name for you in chat and the Tab list, shown as `~name` (hovering it gives your real one); `/nick off` (anyone with a nickname) takes it off; `/nick` alone says how |
| `/nick <player> <name\|off>` | operators of level 3: sets or takes off a player's nickname |
| `/realname <nick>` | who goes by that nickname |
| `/sidebar off`, `/sidebar on`, `/sidebar` | hides the scoreboard sidebar for you (remembered), shows it again |

The market trades only plain items (not worn, named or enchanted), and only with players in survival or adventure
mode. Items move first, then money, both logged; a sale the log refuses gives the items back, and a purchase checks the
money and the room before anything moves.

**No way to get rich from the market.** The market makes money only from what players gather, and only so much a day:
- Buying always costs more than selling: an item whose `sell` is not below its `buy` isn't bought (the server log
  says so at the start).
- No chain of recipes turns what the market sells into something it buys for more: the cheapest way to get each item
  from the market (bought, or crafted, smelted, cut from bought items, through any number of recipes) is worked out,
  and an item the market would buy for more than that isn't bought. The tests walk every vanilla recipe
  (`SMP_VANILLA_DATA=<.local/horizon-client/26.3/data/minecraft> cargo test`); the server checks the iron, gold,
  copper, diamond, emerald, coal, redstone, lapis, wheat, wood and food chains at each start.
- Each player sells at most `daily_sell_coins` (2,000 coins) and `daily_sell_items` (640 of each item) a UTC day,
  `/worth` and the shop show what is left, and `market.json` keeps the day's count across restarts.
- Amounts are checked: no price above 1,000,000,000,000, no sum that overflows.
- The coinflip and the lottery give the winner the stakes (less `fee_percent`, 0 by default): they move money
  between players and never make any.

Default prices (coins, buy / sell; `-` not traded that way):

| Category | Items |
|---|---|
| Farming | wheat 3/1, wheat seeds 2/-, carrot 3/1, potato 3/1, beetroot 3/1, sugar cane 3/1, pumpkin 8/3, melon slice 2/1, cactus 3/1, bone meal 3/- |
| Mining | coal 8/2, copper ingot 6/2, iron ingot 30/8, gold ingot 40/10, redstone 4/1, lapis 8/2, quartz 8/2, emerald 80/20, diamond 400/100 |
| Wood | the eight logs 4/1 |
| Mob drops | rotten flesh -/1, bone 3/1, string 3/1, gunpowder 8/2, spider eye 4/1, slime ball 10/2, blaze rod 30/8, leather 6/2, feather 2/1 |
| Building | cobblestone 1/-, stone 2/-, dirt 1/-, sand 2/-, gravel 2/-, glass 3/-, bricks 4/-, stone bricks 3/- |
| Food | bread 5/-, cooked beef 6/-, cooked porkchop 6/-, cooked chicken 5/-, baked potato 4/-, apple 4/-, golden carrot 40/- |
| Tools | torch 1/-, ladder 2/-, crafting table 4/-, chest 6/-, furnace 6/- |

A `config.json` still holding the 18 prices of versions 0.1 to 0.11, never edited, gets these at the first start of
0.12.0; edited prices are kept. Only items that stack to 64 belong in the market: a shift-click buys 64, and what
doesn't fit falls at the player's feet for them.

**Nicknames.** With `ops_only` (the default since 0.22) only operators of level 3 set nicknames: their own, or a
player's with `/nick <player> <name>`; a player may take off the one given to them (`/nick off`). At each start, a
nickname a player who isn't an operator picked themselves is taken off and logged (`for operators only`). 3 to 16 letters, digits or `_`, shown with a `~` in front (`~Rocky`) so a nickname never passes for
a real name. Names are compared case-insensitively with `_` dropped and look-alikes folded (`0`→o, `1`/`i`→l, `3`→e,
`4`→a, `5`→s, `7`→t, `8`→b, `9`→g). Refused: a nickname equal to, or containing (from 4 letters), the name of anyone
who has played here (the economy's accounts and `usercache.json`), an operator (`ops.json`), the Team (`protected`)
or another player's nickname; one that reads as a reserved word (`reserved`: server, console, admin, mod, owner,
staff, op, horizon, kira, notch, mojang, herobrine...); one the server's chat filter refuses. A player needs
`min_playtime_minutes` of play before their first nickname (operators don't). At each start, nicknames that break
these rules are taken off, each logged (`[smp] nickname X of <uuid> taken off: why`). The change is made at the next
second. The nickname shows in chat (messages, `/me`, `/msg`) and the Tab list, and hovering it gives the real name;
the real name stays everywhere else: the server's log, the anticheat, selectors, the Discord bridge. `/pay`, `/tpa`,
`/trade`, `/playtime`, `/tpaccept`, `/tpdeny` and `/eco` take a nickname (with or without `~`) wherever they take a
player's name. The player's profile (name, UUID) never changes.
`config.json`'s `nicks`: `enabled`, `ops_only` (true), `cooldown_seconds` (600), `min_playtime_minutes` (60), `protected` (the Team's
names), `reserved` (the words above).

**The shop.** `/shop` opens a 6-row chest menu on the market's prices: the categories of `config.json`'s `shop` (an
item in none goes under "Other"), each item named with its prices. A click buys one, a shift-click 64; a click on a
stack in your own inventory, below the menu, sells it; "Sell all" sells everything the market buys. The bottom row
shows your balance and today's selling limit. It goes through the market's own rules (survival or adventure only,
items then money, logged, the daily limits); the items move by the console's `clear` and `give`, measured on the
inventory before and after, so the money follows what really moved.

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
as any teleport does. Teleports cost a fee to the Treasury (`places.fees`: `spawn` 0, `home` 5, `warp` 10, `back` 10,
`tpa` 10, paid by the one who moves), logged as `travel_fee`. It is checked when the command runs and paid when the
teleport happens: one called off (moved, hurt, the money spent meanwhile) costs nothing and leaves `/back` and the
cooldown as they were. The spawn stays free so a lost or new player can always get back.

**Claims.** In a claimed chunk, only its owner, the players they trust and operators of level 3 break blocks or right
click blocks: doors, chests, levers, buttons, placing a block, a bucket or flint on a block. Anyone else's click does
nothing, and the action bar says whose land it is. The chunks a player may hold grow with the time played here
(`claims.chunks`: `[hours, chunks]`, by default 4, 16 from 5 hours, 36 from 20, 64 from 100); `claims.enabled`
turns them off. The world stays out too: no water or lava flows, no fire spreads or burns, and no piston pushes into
a claim from land that isn't its owner's (or a player's they trust), and explosions break no claimed block, whoever
lit them. Buckets are guarded where they fill or empty, and hitting the claim's item frames, armour stands,
paintings, boats, minecarts and animals does nothing for others (`claims.protected_entities`, entity ids; monsters
and players aren't in it). Not guarded yet: right clicking an entity (rotating a frame's item, an armour stand's
items, shearing, leads), which waits for Horizon's hook (#4648). The server's `spawn-protection` (vanilla's) guards
the spawn.

**The scoreboard.** A sidebar on the right, titled `board.title` ("Horizon SMP"): your own block first (your name, or
your nickname; your balance, `125.4k`, `3.2M`; your playtime, `2h 05m`; the players online), then the `board.top` (3)
richest players with their balance, then `board.footer` (`play.playhorizon.dev`, empty for none). The ranking is
built once for everyone, only when a balance changed (at most every `board.min_seconds`, 5) or every
`board.refresh_seconds` (60); each player's own lines are looked at on the same `board.refresh_seconds`, and a
sidebar is sent only to the players whose lines changed and to newcomers: nothing runs per tick per player. The Tab
list shows `board.tab_header` and `board.tab_footer` (`{server}`, `{tps}`, `{players}` filled in, every
`board.tab_refresh_seconds`, 5). `board.sidebar` and `board.tab` turn either off; `board.server_name` names the Tab
list. A server that kept 0.13's defaults (`Richest players`, the top 10) gets these at its first start of 0.14. The
sidebar uses one scoreboard objective of its own (`horizon:native_sidebar`) and leaves the world's scoreboard alone.

**Presence.** A player's first join is announced to everyone (`presence.first_join`, empty for none) and they read
`presence.welcome` and the rules. Each player gets `presence.daily_reward` (50) once a day (UTC) after
`presence.reward_after_minutes` (10) of play that day, logged as `daily_reward` (0 turns it off). Only seconds with a
move, a turn or a command in the minute before count, so an alt that logs in and stands still gets nothing (on
2026-11-01, 1,242 players got the reward; 323 of them had played 10 minutes in all). The count isn't kept over a
restart. A player who hasn't moved, turned or run a command for `presence.afk_minutes` (5),
or typed `/afk`, is AFK: they don't count for sleeping (vanilla's `players_sleeping_percentage`) until they move or
get in a bed; `presence.afk_announce` tells everyone instead of only them. A player runs at most
`presence.spam_commands` (8) of the SMP's commands in `presence.spam_seconds` (5); operators of level 3 aren't
limited. All of it is looked at once a second.

**The auction house.** Players sell to players: a listing is a fixed price or a timed auction. The listed items
leave the seller's inventory at once; what doesn't sell, what an auction's winner gets, and what didn't fit in a
buyer's inventory wait in the player's mail (`/ah claim`, or the menu's mail page; as much as fits, the rest stays).
A purchase takes the whole listing: the listing leaves first, the items move, then the money (the seller gets the
price less VAT, which goes to the Treasury); of two players who buy at once (the game runs every click on one
thread), the second finds it gone. Bids wait in the `auction` pool: a higher bid gives the last one back at once,
raising one's own only holds the difference, and a bid in the last `snipe_seconds` (60) moves the end back to
60 seconds away. When an auction ends, the seller is paid (less VAT) and the items go to the winner's mail. Sellers
and outbid players who are offline find a note when they come back. Everything (listings, bids, mail, notes) is
written in `economy.json` with the balances, in one step: a crash can't keep the money and lose the items, or the
reverse. Only plain items for now (not worn, named or enchanted); whole stacks with their enchantments wait for
Horizon's `native_give_stack` to be stamped. Survival and adventure players only. `config.json`'s `auction`:
`enabled`, `listing_fee_percent` (1), `max_listings` (8 a player), `listing_hours` (48), `auction_hours` (12),
`auction_hours_max` (24), `bid_step_percent` (5: the smallest raise), `snipe_seconds` (60). Ended listings are
looked at once a second (a pass over the open listings).

**The Treasury and taxes.** Nobody can make money, operators included: money enters the game only through the daily
reward and what the market buys from players (later, paid contracts, from the Treasury). Every tax, fee and house edge
goes into the Treasury, a pool in `economy.json` that belongs to no player (not in `/baltop` or the sidebar), and
operators pay only from it (`/treasury pay`, with a reason everyone sees, logged as `grant`). A refund for a bug is
such a payment, never new money. `/treasury` shows its balance, its income by kind and its spending; `/treasury log`
its last 50 movements. The tax code (`config.json`'s `taxes`):
- VAT (`vat_percent`, 5): on sales between players (the auction house, next), paid by the seller from the price.
- Land tax, every Monday 00:00 UTC: `land_tax_per_chunk` (5 coins) a week per claimed chunk past the first
  `land_free_chunks` (4). What a player can't pay is owed: no new claim until it's paid (`/eco taxes pay`, or the
  next Monday's collection). Nothing is ever unclaimed for it.
- Wealth tax, every Monday: `wealth_brackets` `[from, percent]`, by default 1% of a balance's part above 50,000,
  2% above 250,000 and 3% above 1,000,000 a week, on the balance and bank savings together (paid from the
  balance). On 2026-11-01 the richest player held 11,804 coins: nobody pays it
  yet; it keeps fortunes from growing without bound as the economy does.
- Games of chance: the coinflip's and the lottery's `fee_percent` goes to the Treasury.
The week's collection is looked at once a second (one comparison) and runs once a week; a restart never collects
twice (the week collected is kept with the balances). `enabled: false` turns the weekly taxes off.

**The bank.** `/bank` shows your savings; `/bank deposit <amount|all>` and `/bank withdraw <amount|all>` move coins
between your wallet and the bank (the `bank` pool), free and at any time. Interest (`bank.daily_basis_points`, 10:
0.1% a day) is paid at 00:00 UTC from the Treasury, on the lowest amount saved since the last payment (a deposit made
just before midnight earns nothing that night), on at most `max_interest_on` (10,000) coins of an account; a player
saves at most `max_deposit` (25,000). One night's interest for everyone is at most `max_treasury_percent` (5) of the
Treasury, shared out pro rata when that's short: the bank never makes money, it hands back part of the taxes to
those who keep coins out of circulation. Interest stays saved (logged as `bank_interest`); deposits and withdrawals
are `bank_deposit` and `bank_withdraw`. No loans: their collateral (items) can't be held safely here, and a loan
without it is free money for an alt that never pays back. Savings are kept in `economy.json` (`ext.bank`).

**Contracts.** Each day at 00:00 UTC the state posts `jobs.contracts_per_day` (4) contracts drawn from `jobs.items`
(farm, mine and mob produce the market buys): "deliver 64 wheat for 128 coins", announced to everyone. `/jobs` lists
them; `/jobs deliver <n>` takes `batch` (64) plain items from the inventory (they leave the game) and pays at once from
the Treasury, logged as `contract <count> <item>`. A delivery pays the market's sell price plus `premium_percent`
(30), and never more than `ask_percent` (90) of what the market asks for the item, checked again at delivery as
prices move: buying from the market to deliver never pays. Each contract takes `deliveries` (8) for the whole
server, a player makes at most `max_per_player` (2) a day, survival or adventure only, and one day's contracts cost
at most `budget_percent` (10) of the Treasury when posted (an empty Treasury posts none). Never AFK income: only a
command with the items pays. The board is kept in `economy.json` (`ext.jobs`): a restart neither posts twice nor
loses a delivery.

**The report.** `/eco stats` (anyone) shows the money supply (wallets, savings and money held, the Treasury, the
insurance fund, each pool), what the server made today (market sales, the daily reward) and destroyed (market
purchases, operators' `/eco take`), the day's taxes and fees, and how evenly money is spread (Gini over players
with money, the richest ten's share). At 00:00 UTC everyone reads the day before in one line (`report.daily_report`).
A snapshot of each day is kept 60 days (`report.history_days`) in `economy.json` (`ext.report`). Once a minute
`world/horizon-smp/web/economy.json` (`report.web_json`) is written off the tick thread for the site's /hub:
`{"schema": 1, "generated": <ms>, "currency", "today", "pools", "minted_today", "burned_today", "treasury": {balance,
insurance, income, spent, income_today, spent_today, log}, "taxes": {rates, last_collection}, "days": [...],
"daily_report": "<yesterday's line>"}`. The server's Discord relay posts only chat, joins, leaves and deaths: the
daily line reaches Discord through whoever reads that file (Kira).

Other parts reach the pools through `Economy::to_pool`, `from_pool`, `pool_move` and `settle_pool` (a bet against a
pool: the stake goes in, the payout comes out, never more than the pool holds), each one audit line.

The currency is earned in game only: it can't be bought with real money or cashed out (Mojang's usage guidelines).

## Data

Everything is in the world's folder, `world/horizon-smp/`, so a backup of the world carries it. A world without that
folder is untouched until the mod first runs on it; it changes no block and no player.

| File | Holds |
|---|---|
| `config.json` | `currency` (default `coins`), `starting_balance` (default 0), `market`: `enabled`, `items` (each item's `buy`, what the market asks, and `sell`, what it pays, 0 closing that side), `daily_sell_coins` (2000) and `daily_sell_items` (640), 0 for no limit; `shop`: `enabled`, `title`, `categories` (`name`, `icon`, `items`); written with the defaults the first time, and again when a newer version adds keys (your values kept) |
| `economy.json` | balances by player UUID, with the name last seen; `held`: coinflip stakes and lottery tickets being decided; `pools`: the Treasury, its insurance fund and other system pools; `treasury`: its income and spending by kind and its last movements; `ext`: other parts' state written in the same step as the money (`tax`: the week collected, land tax owed; `auction`: the listings, bids, mail and notes) |
| `places.json` | the spawn, the warps, each player's homes, and the seconds each player has played (written once a minute) |
| `nicks.json` | each player's nickname and when they last changed it |
| `market.json` | what each player sold today (coins and items), for the daily limits |
| `presence.json` | the players who have joined (no welcome again), and the day each last got the daily reward |
| `board.json` | the players who hid the sidebar (`/sidebar off`) |
| `claims.json` | each claimed chunk (`<dimension> <chunk x> <chunk z>`) with its owner, and whom each owner trusts |
| `audit.log` | one JSON line per change, written before the change: `time` (ms), `kind` (`open`, `pay`, `grant`, `land_tax`...), `amount`, `from`/`to` with their balance before, `by` for an operator, `to_pool`/`from_pool` with `pool_before` and `reason` for a pool's movement; a trade's line (`trade`: both players, their items and money) comes before its swap, `trade_failed` after one that didn't go through |

`economy.json` is written whole to a temporary file, flushed and renamed over the old one, at most once a server tick
and when the world closes: a crash leaves the old file or the new one. If it can't be read, the economy stays off (the
commands say so) rather than start from nothing.

## Made on the hook points

It hooks only Horizon's native hook points: `native_register_commands` and `native_perform_command` (API 8), the
acting player's items (API 9), the server menus and `native_exchange_items` (API 10), the players' health and
`native_run_command` (API 21), `native_destroy_block` (API 14), `native_right_click_block`, the acting player and the
action bar (API 21), `native_block_spread`, `native_piston_move`, `native_explosion_detonate`, `native_fill_bucket` and
`native_attack_entity` (API 22), `native_sidebar`, `native_tab_list` and `native_tick_stats` (API 49),
`native_set_nickname` and `native_text_allowed` (API 62), the server tick and the world lifecycle.
`compatibility.json` names `"hooks": 58`.

| File | What it is |
|---|---|
| `mixins/smp/horizon-mixin.toml` | The bundle: sixteen modules and the HEAD hooks |
| `mixins/smp/smp_store.rs`, `smp_economy.rs`, `smp_market.rs`, `smp_recipes.rs`, `smp_shop.rs`, `smp_nick.rs`, `smp_trade.rs`, `smp_games.rs`, `smp_lottery.rs`, `smp_places.rs`, `smp_claims.rs`, `smp_board.rs`, `smp_presence.rs`, `smp_tax.rs`, `smp_auction.rs`, `smp_bank.rs`, `smp_report.rs`, `smp_jobs.rs` | The rules, no game types (a trade's stacks are a type parameter): added to the game as `game::smp_store`, `game::smp_economy`, `game::smp_market`, `game::smp_recipes`, `game::smp_shop`, `game::smp_nick`, `game::smp_trade`, `game::smp_games`, `game::smp_lottery`, `game::smp_places`, `game::smp_claims`, `game::smp_board`, `game::smp_presence`, `game::smp_tax`, `game::smp_auction`, `game::smp_bank`, `game::smp_report`, `game::smp_jobs`; also this crate's modules (`src/lib.rs`), for their tests |
| `mixins/smp/glue.rs` | The state for the open world and the edges to the game's types (`game::smp_glue`) |
| `mixins/smp/{register,command,tick,lifecycle,menu_clicked,menu_closed,destroy_block,right_click_block,block_spread,piston_move,explosion_detonate,fill_bucket,attack_entity}.rs` | One small helper per hook |
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
