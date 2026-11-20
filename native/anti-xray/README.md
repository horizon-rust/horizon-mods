# Anti-Xray

Anti-Xray is a native mod for Horizon dedicated servers. It conceals enclosed blocks in outgoing Minecraft Java
terrain packets. A vanilla 26.3 client can join without installing anything. The server still mines, drops and saves
the authoritative blocks. Protection is off until enabled in the server configuration.

## Build and install

This source targets Horizon engine and SDK commit `dd1509b82d4804a234fbc0da3263a5454948e9c4` from
[Horizon PR #19063](https://github.com/horizon-rust/horizon/pull/19063), Minecraft Java 26.3 and Rust 1.99.1.
Keep the engine checkout separate from this repository. Prepare game resources from your own 26.3 install or server
jar using that checkout's [hosting guide](https://github.com/horizon-rust/horizon/blob/dd1509b82d4804a234fbc0da3263a5454948e9c4/docs/hosting.md#prepare-the-game-files).

The Cargo project expects the SDK API sources under `.horizon/sdk/api/` in this directory. With a signed SDK for this
build, use `cargo horizon-mod check` to fetch the pin and `cargo horizon-mod build` to create the source package.
`HORIZON_SDK_BASE` can point to a signed mirror with the same layout. The pinned PR build has no public signed SDK
release. To reproduce the Linux build from its exact source:

```sh
MOD=$(pwd) # run from horizon-mods/native/anti-xray
ENGINE=/absolute/path/to/separate/horizon-checkout
REV=dd1509b82d4804a234fbc0da3263a5454948e9c4
git -C "$ENGINE" fetch origin main
git -C "$ENGINE" switch --detach "$REV"
rustup toolchain install 1.99.1
rustup target add --toolchain 1.99.1 wasm32-unknown-unknown
export RUSTUP_TOOLCHAIN=1.99.1
BUILD=$(mktemp -d)

mkdir -p "$MOD/.horizon/sdk"
(cd "$ENGINE" && python3 tools/sdk_api_source.py build "$MOD/.horizon/sdk/api")
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
(cd "$ENGINE" && python3 tools/prebuilt_game.py build "$BUILD/sdk" --server)
python3 tests/export_local.py --engine "$ENGINE" --prebuilt "$BUILD/sdk" --output "$BUILD/export"
```

For a source package check without an SDK release, build the package tool from the pinned engine and stage the
Cargo project inside its ignored `target/` directory. The package tool recognizes the local engine source and
skips fetching a signed SDK. It excludes the `.horizon/` link from the package:

```sh
(cd "$ENGINE" && cargo build --locked --release -p cargo-horizon-mod)
STAGE=$(mktemp -d "$ENGINE/target/anti-xray-package.XXXXXX")
cp -R Cargo.toml Cargo.lock horizon-mod.toml LICENSE src "$STAGE"
mkdir -p "$STAGE/.horizon/sdk"
ln -s "$ENGINE" "$STAGE/.horizon/sdk/api"
TOOL="$ENGINE/target/release/cargo-horizon-mod"
(cd "$STAGE" && "$TOOL" build && "$TOOL" check target/horizon-mod/anti_xray.hmod --kind native-mod)
```

The local export uses the exact SDK's native entry template and compiler metadata, and verifies the hashes and
compiler identity in the SDK before linking. It does not need a signed development release. Keep the export folder
together. With a signed SDK and matching launcher version, the regular export command is
`horizon-launcher native-build "$MOD" --server-output "$BUILD/export" --version TAG --trust-native-code`.

Copy an example into the dedicated server folder as `config/anti_xray.toml`. Point a prepared standalone server at
the exported runtime and restart it using `horizon-server` from the matching engine revision:

```sh
mkdir -p "$SERVER/config"
cp examples/anti_xray.toml.example "$SERVER/config/anti_xray.toml"
horizon-server --dir "$SERVER" config set engine.path="$BUILD/export/horizon-engine"
horizon-server --dir "$SERVER" config set engine.resources="$RESOURCES"
horizon-server --dir "$SERVER" run
```

For a scratch server with an accepted EULA and `server.properties`, you can also launch the engine directly from the
scratch server directory with
`"$BUILD/export/horizon-engine" --game-dir "$PWD" --resources "$RESOURCES" --dedicated --nogui`.

## Configure

`[defaults]` sets common values. Each `[dimensions."minecraft:overworld"]`,
`[dimensions."minecraft:the_nether"]` or `[dimensions."minecraft:the_end"]` table can override them, with lists
replacing the inherited list. Restart to apply changes. Startup errors identify the setting or TOML location.
The server refuses logins if it could not validate or register the configuration.

| Setting | Default | Behavior |
| --- | --- | --- |
| `enabled` | `false` | Protects the configured dimension when there are eligible blocks. |
| `mode` | `1` | Mode 1 replaces enclosed `hidden_blocks` with stone at and above Y=0, deepslate below Y=0, netherrack in the Nether or end stone in the End. Mode 2 selects a decoy per block. Mode 3 selects one per chunk and horizontal Y layer. |
| `hidden_blocks` | `[]` | Specifies eligible blocks in all modes and weighted decoys in modes 2 and 3. Block names expand to all their states for eligibility. Each occurrence contributes the block's default state to decoy selection. The three air blocks are ignored. A block entity can go here in mode 1. |
| `replacement_blocks` | `[]` | Adds eligible states in modes 2 and 3, including blocks with block entities. Air is ignored. |
| `max_block_height` | `64` | Exclusive world Y limit. Rounds down to a multiple of 16, including negative values. |
| `update_radius` | `2` | Extra reveal distance, clamped to 0–2. At 0, newly exposed blocks still receive ordinary updates. |
| `lava_obscures` | `false` | Treats every lava state as covering a face when enabled. Lava surfaces can expose the visual substitution. |
| `permission_bypass` | `false` | Allows server operator level 2 or greater to receive authoritative terrain for this dimension. |

Modes 2 and 3 require 1–256 non-air decoy entries, including duplicates. Putting a block with a block entity in
`hidden_blocks` is a configuration error in these modes. Use `replacement_blocks` to conceal it with other decoys.
Full opaque cubes cover adjacent faces. Glass, partial shapes and water leave them exposed. Lava follows its setting.
An unavailable face initially counts as covered. The engine refreshes tracked columns when it learns a neighbor's
states. Decoys stay stable across resends in one server run. The choice seed comes from the process, independently
of the world generation seed.

The SDK exposes operator levels. It has no Paper permission node equivalent. The mod assigns protected views for all
enabled dimensions at login. It checks the resolved operator level after placement and refreshes its decision on
subsequent server ticks. Until a level is known, the player stays protected. The engine resends tracked columns
following a view change. Revoking bypass cannot retract terrain the client already received. Horizon also withholds its
additional distant-terrain LOD in protected dimensions for protected players.

With the pinned build, extra Manhattan-radius reveals go only to the player who successfully broke the block.
Rejected interactions do not trigger that reveal. Paper documents extra updates on block interaction. The pinned
engine remembers at most 1,024 still-covered positions per player and dimension view. Once full, it skips new extra
reveals until tracking or view changes free entries. Ordinary geometric exposure still applies.

The behavior references are Paper's public [Anti-Xray guide](https://docs.papermc.io/paper/anti-xray/) and
[world configuration](https://docs.papermc.io/paper/reference/world-configuration/#anti-xray). The implementation uses
Horizon's public [terrain SDK contract](https://github.com/horizon-rust/horizon/blob/dd1509b82d4804a234fbc0da3263a5454948e9c4/docs/mod-api/terrain-view.md).

## Verify

The acceptance scripts use disposable game directories and homes, plus offline clients that use the vanilla wire
protocol. Give them the exact separate engine checkout, the standalone export and prepared resources:

```sh
RUNS=$(mktemp -d)
COMMON=(--engine "$ENGINE" --binary "$BUILD/export/horizon-engine" --resources "$RESOURCES" --out "$RUNS")
python3 tests/dedicated.py "${COMMON[@]}"
python3 tests/dedicated.py "${COMMON[@]}" --lava-obscures
python3 tests/patterns.py "${COMMON[@]}" --mode 2
python3 tests/patterns.py "${COMMON[@]}" --mode 3
python3 tests/sustained.py "${COMMON[@]}"
python3 tests/far.py "${COMMON[@]}"
python3 tests/benchmark.py "${COMMON[@]}" --modes 0,1,2,3 --rounds 3 --seconds 12 --bots 4
```

Use `--require-chest-tag` on the dedicated test to enforce state-before-tag delivery: every chest state sent after a
reveal must arrive with its chest update tag. The pinned engine does this, and the script records any missing tag
when the flag is left off.

The benchmark writes a JSON report and its starting snapshot under `--out`; the reusable world is its `snapshot/world`
folder. Use `--snapshot PATH --traffic-only`
to compare identical columns in modes 2 and 3, or `--snapshot PATH --cold-only --modes 0,1` to repeat exploration.
To compare disabled overhead against the same engine without any mod, copy the export, replace
`native-mods.json` in the copy with `{"libraries": []}` and delete its `libanti_xray.so` and `horizon-native-mods`.
Then run the benchmark against that plain runtime:

```sh
python3 tests/benchmark.py "${COMMON[@]}" --plain-binary "$PLAIN/horizon-engine" \
  --modes=-1,0 --cold-only --rounds 2 --seconds 12 --bots 4
```

See [the verification and benchmark report](BENCHMARK.md) for the measurements.

To try the unmodified Java client on Linux, prepare its original game files with the engine's `tools/lab.py`.
Run the smoke check on its own display with Java 25 and Xvfb:

```sh
python3 "$ENGINE/tools/ci/with_xvfb.py" python3 tests/vanilla_smoke.py "${COMMON[@]}" \
  --vanilla-root "$VANILLA_CHECKOUT" --assets "$MINECRAFT_ASSETS"
```
