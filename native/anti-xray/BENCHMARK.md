# Dedicated-server verification

The native mod and optimized dedicated runtime were built against Horizon
`dd1509b82d4804a234fbc0da3263a5454948e9c4` with Rust 1.99.1 on Linux x86-64. The host was an
AMD Ryzen 5 5600X with 6 cores, 12 logical CPUs and 31 GiB of RAM. Server and wire clients ran on
the same workstation over loopback. The runtime used prepared Minecraft Java 26.3 data. The server and each
run's homes and world folder were disposable.

## Workload and measurements

Use the [README](README.md#verify) to build the export and run the tools. The full comparison rotates the
order of modes 0, 1, 2 and 3 over three rounds. Mode 0 loads the same native mod with no terrain views enabled.
Every trial starts from a copy of the same saved world, with seed 87387, view distance 6, simulation distance 3,
four clients, no other anticheat, no mob spawning, no random block ticks, and a 256-byte compression threshold.

Each phase lasts 12 seconds.

- **Join.** All four clients join at the same spawn.
- **Cold.** Clients travel at 18 blocks per second in four directions into new terrain from the same coordinates.
- **Warm.** Each client retraces its route, receiving previously seen columns.
- **Mining.** Four clients mine parallel rows in one shared region. The fixture spaces cuts far enough apart that
  one break's extra radius does not pre-reveal the next measured ore.

Chunk delivery counts packets actually received during the phase, including refreshes. The supplementary cold
test counts distinct and previously unseen columns per player as well. The benchmark counts complete compressed
packet frames, including length headers, and excludes unrelated packets. CPU is server process CPU time sampled
from Linux `/proc`. Peak RSS is the maximum sampled during the phase. Per-trial p95 and p99 are medians of the
server's `tick query` snapshots, with each snapshot covering the latest 100 ticks. They are rolling-window
estimates and can include ticks from the preceding phase. The report retains each snapshot and every phase's
delivered-chunk total.

The mining script matches the accepted block state change, the acknowledged action sequence and the actual
ore state on the wire. When the accepted change and the reveal are in one section update, they were queued
together. Where they occupy separate frames, the script records client receipt latency.

## Interleaved comparison

Each cell is the median across three trials, with its minimum and maximum in brackets. Throughput is
delivered packet rate. CPU is seconds across all server threads in the 12-second phase.

| Mode | Phase | p95 ms | p99 ms | Chunks/s | Bytes/chunk | CPU seconds | Peak MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | join | 4.95 [3.45–6.50] | 27.90 [9.85–60.60] | 71.0 [71.0–71.0] | 4,441 [4,440–4,441] | 6.27 [4.49–6.34] | 525 [523–532] |
| 0 | cold | 7.05 [3.80–8.80] | 13.30 [6.70–20.55] | 67.5 [67.5–67.5] | 5,276 [5,274–5,288] | 14.49 [13.62–15.54] | 632 [630–635] |
| 0 | warm | 7.50 [5.35–7.80] | 18.25 [7.30–26.55] | 65.0 [65.0–65.0] | 4,477 [4,477–4,477] | 1.43 [1.02–2.12] | 637 [636–642] |
| 0 | mining | 2.90 [1.35–3.35] | 5.30 [1.90–6.75] | 16.2 [16.2–17.5] | 4,044 [4,044–4,064] | 0.59 [0.38–0.64] | 592 [588–606] |
| 1 | join | 3.20 [2.85–7.00] | 32.70 [9.80–53.40] | 79.3 [71.0–79.7] | 4,243 [4,243–4,253] | 6.32 [4.64–6.48] | 520 [517–526] |
| 1 | cold | 7.70 [4.60–7.75] | 14.10 [9.10–20.35] | 107.2 [105.8–107.8] | 5,089 [5,083–5,173] | 15.36 [13.30–16.19] | 627 [626–636] |
| 1 | warm | 10.00 [4.10–10.05] | 14.85 [7.75–21.35] | 65.0 [65.0–67.7] | 4,268 [4,268–4,327] | 1.52 [1.20–2.44] | 640 [636–642] |
| 1 | mining | 2.20 [1.25–3.65] | 3.70 [1.80–6.50] | 16.2 [16.2–16.2] | 3,870 [3,870–3,870] | 0.62 [0.40–0.65] | 592 [592–592] |
| 2 | join | 3.45 [2.70–3.70] | 47.00 [11.90–49.90] | 79.0 [71.0–79.3] | 12,168 [12,149–12,168] | 6.43 [5.26–6.68] | 535 [534–539] |
| 2 | cold | 7.85 [4.65–8.30] | 13.20 [9.50–20.45] | 111.1 [107.1–111.1] | 12,849 [12,847–12,894] | 16.91 [15.77–18.11] | 656 [653–660] |
| 2 | warm | 8.10 [5.75–10.75] | 17.55 [9.30–24.75] | 65.2 [65.0–68.7] | 12,249 [12,242–12,306] | 2.30 [1.38–3.51] | 670 [668–670] |
| 2 | mining | 4.75 [1.30–5.70] | 9.20 [1.95–23.30] | 16.2 [16.2–17.5] | 11,543 [11,534–11,561] | 0.67 [0.45–0.69] | 629 [624–630] |
| 3 | join | 4.70 [2.25–4.95] | 34.45 [9.95–62.05] | 77.5 [71.0–81.0] | 4,425 [4,407–4,425] | 6.44 [4.84–6.91] | 532 [528–541] |
| 3 | cold | 7.50 [4.15–9.35] | 15.35 [8.70–18.70] | 107.6 [82.6–111.6] | 5,020 [4,953–5,124] | 15.35 [13.54–17.67] | 647 [622–651] |
| 3 | warm | 7.80 [5.45–11.55] | 13.35 [10.40–21.70] | 67.7 [65.0–75.1] | 4,550 [4,481–4,603] | 2.87 [1.34–4.89] | 658 [656–660] |
| 3 | mining | 5.75 [2.00–6.75] | 8.00 [6.10–14.50] | 16.2 [16.2–17.5] | 3,856 [3,843–3,893] | 0.67 [0.63–0.73] | 610 [607–617] |

Part of these runs shared the workstation with another job's release builds (load average about 11 on 12
logical CPUs), so latency percentiles are noisier than a quiet host would give, and mode 0 itself varies by a factor
of two between trials. Against mode 0, mode 1's median p95 was lower at join (4.95 to 3.20 ms) and in mining
(2.90 to 2.20 ms) but higher in cold exploration (7.05 to 7.70 ms, about 9%) and warm (7.50 to 10.00 ms, about 33%).
The cold and warm results miss the 5% target, and the spread in both rows overlaps. Cold delivery volume increased,
which includes the refresh packets the engine sends when adjacent terrain becomes resident. The distinct-column check
below isolates first-time terrain delivery.

The scripts attempted 120 to 124 breaks in each trial, depending on how many fit in the window. Every accepted break
in every protected mode was acknowledged. Each ordinary reveal the script saw arrived in the same section update as the
accepted air change, meeting the next-tick queue target. Mode 1 recorded all 364 reveals. Modes 2 and 3 recorded 308 and
236 of 364, because the engine now sends a neighboring block only when the viewer's picture of it changes. The
benchmark's decoy list contains diamond ore, so an ore whose decoy is also diamond ore is already shown correctly and
sends nothing. Mode 3 chooses one decoy per chunk layer, and the missing reveals fall in the same whole columns for all
four clients. Request-to-receipt time includes client, input scheduling and server time. The maximum was 90.6 ms for
mode 1, 96.5 ms for mode 2 and 91.0 ms for mode 3.

For the matched traffic run on 49 identical saved columns, mode 2 sent 611,374 framed bytes and mode 3 sent
225,840. Mode 3 used **63.1% fewer bytes** in that sample. In the interleaved cold run its median compressed
bytes per delivered chunk fell from 12,849 to 5,020.

## First-time exploration delivery

Four disabled and mode 1 trials used the same saved snapshot in two orders, mode 0 first and mode 1 first.
Every trial delivered 810 first-time columns in 12 seconds, or **67.5 per second**, except one mode 1 trial
at 67.4. Mode 1 met the 10% first-time throughput target. It sent 424–496 additional refreshes per trial, compared
with zero in the disabled trials. Total framed chunk traffic rose from a median 4.28 MB to 6.62 MB, **54.7% more**
for the same first-time delivery. Per-chunk averages alone hide the refresh cost.

Phase-wide percentiles include the transition out of the preceding phase. Across the four trials cold p95
ranged from 5.45 to 6.30 ms disabled and from 5.15 to 7.10 ms in mode 1, so the ranges overlap.

## Disabled overhead against the plain engine

Two more interleaved pairs ran the same export with its native mod list emptied (plain) beside the disabled mod.
The plain export delivered 71.0 chunks/s at join and 67.5 first-time chunks/s in cold exploration in both trials.
The disabled export also delivered 71.0 at join, and 67.5 and 66.3 first-time chunks/s in its two cold trials.
Cold CPU medians were 15.19 s plain and 15.13 s disabled. Cold p95 ranged from 7.20 to 7.90 ms plain and
5.85 to 10.10 ms disabled, so the ranges overlap and the sample does not establish tick-time neutrality. Cold peak
RSS medians were 591 MiB plain and 627 MiB disabled.

## Packet and gameplay results

The unmodified Minecraft Java 26.3 client joined the protected dedicated export and rendered its live terrain
in a fresh Xvfb session. The remaining checks used stock protocol clients to observe individual packets.

- Two protected users saw deepslate at the lit ore and east-facing chest below Y=0 and stone at the ore above Y=0.
  Their initial columns omitted the concealed chest record. The operator saw the authoritative states and the
  chest record. Removing and restoring operator status refreshed that client's projection both ways.
- Glass, a single slab and water left nearby ore visible. Lava left it visible with lava occlusion off and concealed
  it with lava occlusion on.
- The out-of-range dig was acknowledged without making the ore visible. A valid break at the chunk border
  exposed the real ore and chest to both protected clients. Resealing and fill edits reapplied the projection.
  The saved Anvil chunk kept the authoritative lit ore, the east-facing chest and three diamonds in its inventory.
  Destroying the chest dropped the diamonds. A survival dig with a diamond pickaxe dropped redstone.
- The saved far-ring column (12, 0) delivered fake deepslate and no chest record to the protected client and the
  real chest and record to the operator. Both stayed at spawn. The engine's own decoded packet tests in Horizon #19063 also
  passed for resident neighbor arrival invalidating a saved projection and for a wrapped cross-dimension view.
- Mode 2 varied within all 59 sampled layers. Mode 3 was uniform in each of the 59 layers. Both used the configured
  decoys, suppressed the replacement chest's metadata, respected the rounded Y=0 limit, and repeated the
  projection after unload, revisit and a Nether round trip. The Nether used netherrack for buried ancient debris.
  Selecting a chest as a mode 2 decoy named the dimension and setting at startup and refused login.

The sustained mining run reached the **1,024-entry** extra reveal cap after break 57. All 72 ordinary adjacent ore
reveals still reached both viewers, including after capacity. Before capacity, the still-covered chest's true state
was sent only to the miner. After capacity, another still-covered chest received no extra state. Breaking its
immediate seal then sent its true state to both viewers. At this engine revision every chest state that reached a
viewer carried its chest update tag in the same delivery, and `dedicated.py --require-chest-tag` passed.
