# Coordinates HUD (Horizon native mod)

Your block position and the direction you face in a box in the top-left corner of the screen.

- `/where` puts your coordinates and dimension in chat.
- `/coords` hides or shows the box, and the game keeps your choice (also Options > Mods > Coordinates HUD > Settings).

It is a [horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundle (`mixins/coords_hud/`) that the Horizon
launcher builds into the game. It is a small example mod.
The time of day line is gone: Horizon's hook points don't give the game's clock yet.

## Install

In the Horizon launcher: Mods → Coordinates HUD → Install, then **Build** on the same page (once built, switch it
on or off in the game under Options → Mods). The
launcher downloads its own Rust and builds the game with your native mods; see Horizon's
[native mods](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md).

## Made on the hook points

The bundle hooks only Horizon's native hook points (`crates/horizon-client/src/game/native_hooks.rs`), which keep
their names and signatures across releases. `compatibility.json` names the version it needs (`"hooks": 1`), so the
launcher builds it on any Horizon with that version or a later one, together with any other mod. `horizon_revision` is
only the commit it was last tested on.

| File | What it is |
|---|---|
| `mixins/coords_hud/horizon-mixin.toml` | The bundle: one module and the HEAD hooks |
| `mixins/coords_hud/runtime.rs` | The mod's logic and state, added to the game as `game::official_coords_hud`; also this crate's library, for its tests |
| `mixins/coords_hud/*.rs` | One small helper per hook |
| `proof.py` (with `../tools/game.py`) | The proof: the built game driven with `horizon-ctl --offscreen` on fresh, temporary data |

## Build and prove

```sh
(cd native/coords-hud && cargo test)                     # the logic
python3 native/tools/native.py check coords-hud          # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py check all                 # with the other native mods, as a player's set
python3 native/tools/native.py package coords-hud        # native/target/coords_hud-<version>.hnmod for the registry
python3 native/tools/native.py proof coords-hud --resources /path/to/.local/horizon-client/26.3
```

The proof needs resources prepared from your own Minecraft 26.3 (`horizon-prepare`); none are in this repository.
It checks the row and the settings page in Options > Mods, that the box shows in the top left, that `/coords` hides it
and writes `horizon-mods/.horizon-mod-data/coords_hud/shown`, and takes a screenshot of `/where`'s line in chat.

License: MIT.
