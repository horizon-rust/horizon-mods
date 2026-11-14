# first-person-model (Horizon Rust port)

A 1:1 Rust port of [First-person Model](https://modrinth.com/mod/first-person-model)
(upstream tr7zw/FirstPersonModel, MIT, @6b5859f, fabric >= 1.21.9 / 26.3
variant paths). Renders the player model in first person instead of the
vanilla floating hands.

Upstream sources the port carries 1:1:

- `FirstPersonBase` (camera entity handoff, `getOffset`)
- `Constants` (body offsets: sneak 0.27, swim up 0.60, swim down 0.50, vehicle 0.20)
- `config` (`FirstPersonSettings`, `ConfigUpgrader` v0 -> v2, vanilla hands and toggles)
- `mixinbase` (`ModelPartBase` z=5000 body-hide trick)
- `InventoryUtil`, `KeybindHolder`
- `LogicHandler` (offset state machine: swim/crawl/vehicle/inventory states, dynamic hands)
- `FirstPersonModelCore` (init, tick, render hooks, avatar renderer handoff)
- `access`, `api` (`FirstPersonAPI`, activation and offset handlers)
- `modsupport` (activators, `PlayerAnimatorSupport` compatibility hook)
- all 19 upstream mixins (model parts, render dispatch, held item, fishing bobber, elytra)

Minecraft/Fabric types Horizon does not expose are host seams with the same
shapes and semantics (`seams`).

Credited to sparticle; carries the upstream MIT license and credit.
