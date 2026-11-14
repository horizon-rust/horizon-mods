# no-resource-pack-warnings (Horizon Rust port)

A 1:1 Rust port of [no-resource-pack-warnings](https://github.com/SpaceWalkerRS/no-resource-pack-warnings)
(Space Walker, MIT). All four `PackCompatibility.isCompatible()` call sites
the mod targets are redirected to return `true`, so outdated resource/data
packs load and select with no warning screen or red border:

- `Options.updateResourcePacks` and `Options.loadSelectedResourcePacks`
  (upstream `OptionsMixin`, two redirects)
- `PackEntry.extractContent` and `PackEntry.handlePackSelection`
  (upstream `PackEntryMixin`, two redirects)

Upstream is 2 mixins, 69 lines; the port mirrors them file for file.
Credited to sparticle.
