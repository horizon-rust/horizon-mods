# horizon-rrls (1:1 port)

Exact 1:1 Rust translation of [Remove Reloading Screen](https://github.com/dima-dencep/rrls)
by dima_dencep (OSL-3.0), per Danil's port policy (2026-11-12): never clean-room,
always exact code. License + upstream credit carried.

**Status: complete.** Translated (98/98 tests passing):

- `Rrls.java` -> `rrls.rs` (MOD_ID, logger, MOD_RELOADERS_FUTURE as an
  epoch-locked slot with set/complete semantics)
- `config/` -> `config/` (AprilFool with Calendar month/day math,
  DoubleLoad.is_load, HideType.affect_initial + can_hide matrix, Type,
  RrlsConfig defaults)
- `utils/Ease.java` -> `utils/ease.rs` (34 variants, upstream enum ids incl.
  the 2..=5 gap, easeIn/easeOut/easeInOut wrappers)
- `utils/Easing.java` -> `utils/easing.rs` (sine/cubic/quadratic/pow/exp/
  circle/back/elastic/bounce/step with the binary-search loop/catmullRom,
  formulas transcribed literally)
- `utils/RainbowUtils.java` -> `utils/rainbow.rs` (makeChannel, rainbowColor
  with nanos injectable, ARGB packing)
- `utils/WaitingSharedState.java` -> `utils/waiting_shared_state.rs`
  (failed-future fallback for future-typed keys)

Complete: every upstream file is translated 1:1, including
`utils/DummyGuiGraphics`, `utils/GuiGraphicsGenerator`, `utils/OverlayHelper`,
`utils/TextureUtils`, `utils/AtlasUtils`, the `services/` reloader,
`screens/`, and the full mixin set (core, animation, compat, workaround
including the vulkan/opengl/textures subgroups, accessors). 98 unit tests on
the translated logic. The mixins/rrls bundle inlines the modules into one
runtime.rs, compile-equivalent, for the single-module .hnmod format.
