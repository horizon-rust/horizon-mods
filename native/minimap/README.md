# Minimap (Horizon native mod)

Horizon's official Minimap: a map of the terrain around you in a corner, in map colours, square or round, north up or turning with you, drawn by Horizon's own minimap. Its settings page (Options > Mods > Minimap > Settings) picks the corner, size, shape, zoom and turning.

It is a [horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundle (`mixins/minimap/`) that the Horizon
launcher builds into the game. It keeps the settings of earlier versions: the same files (`horizon-mods/.horizon-mod-data/minimap/corner`, `size`, `shape`, `zoom`, `rotate`).

## Install

In the Horizon launcher: Mods → Minimap → Install, then **Build** on the same page (once built, switch it
on or off in the game under Options → Mods). The
launcher downloads its own Rust and builds the game with your native mods; see Horizon's
[native mods](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md).

## Made on the hook points

The bundle hooks only Horizon's native hook points (`crates/horizon-client/src/game/native_hooks.rs`), which keep
their names and signatures across releases. `compatibility.json` names the version it needs (`"hooks": 1`), so the
launcher builds Minimap on any Horizon with that version or a later one, the player's own game by default, together with any other
mod. `horizon_revision` is only the commit it was last tested on.

| File | What it is |
|---|---|
| `mixins/minimap/horizon-mixin.toml` | The bundle: one module and the HEAD hooks |
| `mixins/minimap/runtime.rs` | The mod's logic and state, added to the game as `game::official_minimap`; also this crate's library, for its tests |
| `mixins/minimap/*.rs` | One small helper per hook |
| `../tools/native.py` | Shared by the native mods: apply on Horizon (from GitHub, by commit), check, build, package the `.hnmod`, run the proof |
| `proof.py` (with `../tools/game.py`) | The proof: the built game driven with `horizon-ctl --offscreen` on fresh, temporary data |

## Build and prove

```sh
(cd native/minimap && cargo test)                    # the logic
python3 native/tools/native.py check minimap       # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py check minimap --horizon latest  # ... on the newest Nightly
python3 native/tools/native.py check all           # with the other native mods, as a player's set
python3 native/tools/native.py package minimap     # native/target/minimap-<version>.hnmod for the registry
python3 native/tools/native.py proof minimap --resources /path/to/.local/horizon-client/26.3
```

The proof needs resources prepared from your own Minecraft 26.3 (`horizon-prepare`); none are in this repository.
It seeds an earlier version's settings (bottom right, round, 2 blocks per pixel) in a temporary game folder, then checks that Options > Mods shows them, and that the Corner button moves the map to the top left and writes the same file. CI runs `cargo test`, packages the `.hnmod` and checks the game on the tested commit, and every Monday on the newest Nightly.

License: MIT.
