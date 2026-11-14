# vmp-fabric (Horizon port)

Slice 1 of a Horizon port of [Very Many Players (Fabric)](https://github.com/RelativityMC/VMP-fabric)
by ishland (MIT, upstream read at ver/26.1.1, commit 7e89476). Credits and
license preserved; the upstream LICENSE is carried verbatim.

## What slice 1 holds (1:1 logic core, 27 passing tests)

- **Config model** (`config.rs`): the complete `vmp.properties` set with
  upstream defaults, per-key fallback to the default on a missing or
  malformed value, the normalized write-back under the "Configuration file
  for VMP" comment, the java.util.Properties reader subset (comments,
  =/:/whitespace separators, escapes, continuations), and the
  `vmp:incompatibleConfig` forcing pass with upstream's exact warning
  wording and the `vmp.ignoreIncompatibleConfig` override.
- **Coordinate keys** (`mcutil.rs`): `MCUtil.getCoordinateKey/X/Z` and the
  `ChunkPos` packed helpers the watching maps run on.
- **SimpleObjectPool** (`pool.rs`): the fixed-size pool state machine
  (alloc reuse, oversized fallback without the initializer, release no-op
  when full). Upstream synchronizes access; this port is single-threaded,
  wrap at the call site.
- **AreaMap** (`area_map.rs`): the spatial index behind player watching,
  with add/remove/update diffing, per-cell object sets, the sorted listener
  walk by Chebyshev distance, and the same update-diff asymmetry upstream
  ships (adds iterate the new square minus the old, removals the old minus
  the new). Upstream's pooled insertion-ordered identity sets run on plain
  sets here with identical observable behavior.
- **AreaPlayerChunkWatchingManager** (`chunk_watching.rs`): the two-map
  bookkeeping (player map at view distance, general map at
  `GENERAL_PLAYER_AREA_MAP_DISTANCE` = 6, computed from the upstream
  `ceil(max immediate despawn range / 16.0)` formula over the vanilla
  SpawnGroup table), add/remove/move and on-watch-distance-change.

## Seams (server-coupled surfaces, slice 2+)

- `tick()`'s client view-distance change signal and the `ChunkFilter` set
  are server glue; the same bookkeeping runs through `move_player`.
- The player map's view distance comes from the server chunk loader
  (upstream TACS hook) plus one; the port takes it per call.
- Netty event loops, async chunks on login/commands, async portals, async
  logging, entity tracking lookups, the ticket propagator, biome access
  fast paths, spawn density cap, and the collections rewrites are
  Minecraft-runtime-coupled; they land as the runtime surface allows.
