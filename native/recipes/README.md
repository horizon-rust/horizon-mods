# Recipes (Horizon native mod)

Horizon's official recipe viewer: every item beside your inventory, with a search box (`#tag` searches a tag). R over an
item shows its recipes, one tab per station; U shows its uses. The + under a crafting recipe you know fills the
crafting grid, as the recipe book does. In creative mode, turn on "Click gives items" (Options > Mods > Recipes >
Settings): a click on an item in the panel gives you one, a right click a stack. Change R and U on the mod's page in
Options > Mods.

It is a [horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundle (`mixins/recipes/`) that the Horizon
launcher builds into the game. It keeps the settings of earlier versions: the same file
(`horizon-mods/.horizon-mod-data/recipes/give`) and the same keys (`key.recipes.recipe`, `key.recipes.uses` in
`horizon-mods.txt`).

## Install

In the Horizon launcher: Mods → Recipes → Install, then Settings → Game → Native mods (experimental) → Build. The
launcher downloads its own Rust and builds the game with your native mods; see Horizon's
[native mods](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md).

## Made on the hook points

The bundle hooks only Horizon's native hook points (`crates/horizon-client/src/game/native_hooks.rs`), which keep
their names and signatures across releases. It needs the item panel and the recipe tables, so `compatibility.json`
names version 2 (`"hooks": 2`): the launcher builds Recipes on any Horizon with that version or a later one, the
player's own game by default, together with any other mod. `horizon_revision` is only the commit it was last tested
on.

| File | What it is |
|---|---|
| `mixins/recipes/horizon-mixin.toml` | The bundle: two modules and the HEAD hooks |
| `mixins/recipes/runtime.rs` | The viewer: search, pages per station, history, and its tests; added to the game as `game::official_recipes`, and this crate's library |
| `mixins/recipes/glue.rs` | The viewer's edges in the game (`game::official_recipes_glue`): the game's tables and panel events in, the panel, give and place out |
| `mixins/recipes/*.rs` | One small helper per hook |
| `proof.py` (with `../tools/game.py`) | The proof: the built game driven with `horizon-ctl --offscreen` on fresh, temporary data |

## Build and prove

```sh
(cd native/recipes && cargo test)                    # the viewer
python3 native/tools/native.py check recipes         # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py check all             # with the other native mods, as a player's set
python3 native/tools/native.py package recipes       # native/target/recipes-<version>.hnmod for the registry
python3 native/tools/native.py proof recipes --resources /path/to/.local/horizon-client/26.3
```

The proof needs resources prepared from your own Minecraft 26.3 (`horizon-prepare`); none are in this repository.
It seeds an earlier version's setting ("Click gives items" on) in a temporary game folder, checks that Options > Mods
shows the mod, its keys and the setting, then opens the inventory: the panel shows items, a click on one in creative
mode gives it, and with the setting turned off the same click opens its recipe page.

License: MIT.
