# Recipes

Horizon's official recipe viewer: every item beside your inventory and container screens, with a search box. R over
an item (in the panel, in any slot, or in a recipe) shows its recipes, and U shows its uses; a click does the same, a
left click for recipes and a right click for uses. A recipe page has a tab for each station (crafting table, furnace,
blast furnace, smoker, campfire, stonecutter, smithing table), and Back returns through the pages you opened. The + under
a crafting recipe you know fills the crafting grid, as the recipe book does. In creative mode, turn on "Click gives
items": a click on an item in the panel gives you one, and a right click a stack.

## Install

In the Horizon launcher: **Mods**, find Recipes, **Install**. It is a WebAssembly mod, so nothing is built on your
computer. It needs a game with `RecipesClientHost::panel_recipes` (horizon-rust/horizon#19392), which draws its
recipe pages.

## Search

Words in the item's name or id (`oak plank`), `@minecraft` for a namespace, `#logs` for the items of the tags whose
name has `logs`. The tags the search knows are those the game's recipes name (`#minecraft:logs`, `#minecraft:planks`,
`#minecraft:wool`...).

## Settings and keys

**Options > Mods > Recipes > Config:** "Click gives items (creative)", off by default. Kept in
`horizon-mods/.horizon-mod-data/recipes/give` (`true`/`false`), the file earlier versions used. The keys are on
Options > Controls under Recipes ("Show recipes", R; "Show uses", U), kept as `key.recipes.recipe` and
`key.recipes.uses` in `horizon-mods.txt` as before.

## What it reads

The game's items and their names when a world opens, and the recipes of the stations' types (`recipes_of_type`) the
first time you use the panel; again when the server sends recipes or tags, or resources reload. Nothing is done per
frame. On a server, the + and its greying follow the server's recipe book (`known_recipes`, `recipe_book_ids`); the
recipes shown are the game's own 26.3 ones and the content mods', unless a mod has the server send its types.

## Build

With [`cargo horizon-mod`](https://github.com/horizon-rust/horizon/tree/main/docs/modding):

```sh
cargo test                       # the viewer: search, pages, tabs, history, giving, grids
cargo horizon-mod build --wasm   # target/horizon-mod/recipes.hmod
cargo horizon-mod check
```

## Earlier versions (legacy)

2.0.0 was a native `.hnmod` built into the game by the launcher (a horizon-mixins bundle), and 1.0.0 an API 0.1
WebAssembly mod. Launchers from 0.2.70 on can't load either, and their install and build steps no longer apply. Their
source is in this folder's history.

## Licence

MIT, see [LICENSE](LICENSE).
