# FPS Counter (Horizon native mod)

Horizon's official FPS counter: frames per second in a box in a corner of the screen, and, if you like, the slowest frame and the 1% low of the last 5 seconds. Its settings page (Options > Mods > FPS Counter > Settings) picks the corner and the lows.

It is a [horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundle (`mixins/fps_overlay/`) that the Horizon
launcher builds into the game. It keeps the settings of earlier versions: the same files (`horizon-mods/.horizon-mod-data/fps_overlay/corner`, `lows`).

## Install

In the Horizon launcher: Mods → FPS Counter → Install, then **Build** on the same page (once built, switch it
on or off in the game under Options → Mods). The
launcher downloads its own Rust and builds the game with your native mods; see Horizon's
[native mods](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md).

## Made on the hook points

The bundle hooks only Horizon's native hook points (`crates/horizon-client/src/game/native_hooks.rs`), which keep
their names and signatures across releases. `compatibility.json` names the version it needs (`"hooks": 1`), so the
launcher builds FPS Counter on any Horizon with that version or a later one, the player's own game by default, together with any other
mod. `horizon_revision` is only the commit it was last tested on.

| File | What it is |
|---|---|
| `mixins/fps_overlay/horizon-mixin.toml` | The bundle: one module and the HEAD hooks |
| `mixins/fps_overlay/runtime.rs` | The mod's logic and state, added to the game as `game::official_fps_overlay`; also this crate's library, for its tests |
| `mixins/fps_overlay/*.rs` | One small helper per hook |
| `../tools/native.py` | Shared by the native mods: apply on Horizon (from GitHub, by commit), check, build, package the `.hnmod`, run the proof |
| `proof.py` (with `../tools/game.py`) | The proof: the built game driven with `horizon-ctl --offscreen` on fresh, temporary data |

## Build and prove

```sh
(cd native/fps-overlay && cargo test)                    # the logic
python3 native/tools/native.py check fps-overlay   # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py check fps-overlay --horizon latest  # ... on the newest Nightly
python3 native/tools/native.py check all           # with the other native mods, as a player's set
python3 native/tools/native.py package fps-overlay # native/target/fps_overlay-<version>.hnmod for the registry
python3 native/tools/native.py proof fps-overlay --resources /path/to/.local/horizon-client/26.3
```

The proof needs resources prepared from your own Minecraft 26.3 (`horizon-prepare`); none are in this repository.
It seeds an earlier version's settings (bottom right, lows on) in a temporary game folder, then checks that Options > Mods shows them, and that the Corner button moves the box to the top left and writes the same file. CI runs `cargo test`, packages the `.hnmod` and checks the game on the tested commit, and every Monday on the newest Nightly.

License: MIT.
