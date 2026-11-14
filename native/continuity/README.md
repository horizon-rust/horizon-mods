# continuity

Exact 1:1 Horizon Rust port of [Continuity](https://github.com/PepperCode1/Continuity)
by PepperCode1 (LGPL-3.0-only). Every upstream Java class maps to one Rust
module with the same names, logic, defaults, and control flow; the Minecraft
client / Fabric renderer surfaces the engine binds sit in `src/seams.rs` with
one seam per upstream call.

## What it carries

- The full properties layer: `BaseCtmProperties` and the Connecting /
  Compact / Random / Repeat / Overlay property families with the exact
  parsing order, defaults, and error paths of the upstream classes.
- The full `CtmLoaderRegistry`: all 25 loader registrations (`ctm`+`glass`,
  `ctm_compact`, `horizontal`/`bookshelf`, `vertical`, the `h+v`/`v+h`
  aliases, `top`, `random`, `repeat`, `fixed`, and the 10 overlay variants)
  with the `createLoader`/`createCustomLoader` helper chain and the two
  built-in resource packs.
- The processor layer: Simple / Compact / Top / StandardOverlay /
  SimpleOverlay quad processors with caching and processing predicates,
  DirectionMaps, QuadUtil, MathUtil, RandomIndexProvider, and the five
  sprite providers.
- The model layer: `CtmBakedModel` + `EmissiveBakedModel` with the
  `ModelWrappingHandler` bake wrap, `ModelObjectsContainer`, and the
  feature states the entity renderer mixins toggle.
- The resource layer: `CtmPropertiesLoader` (pack priority order,
  `java.util.Properties` parsing), `ResourceRedirectHandler`, 
  `CustomBlockLayers` + `EmissiveSuffixLoader` reload listeners, and the
  `BakedModelManagerReloadExtension` with the atlas/sprite loader contexts
  (AtlasLoader/SpriteLoader/BakedModelManager mixins, emissive control,
  stitch carry).
- The config screen, the ModMenu entry, and the falling-block / piston
  entity renderer mixins.

The engine-bound surfaces (block model rendering, atlas json loading, GUI)
sit behind seams; `mixins/continuity/runtime.rs` is the native mod entry
wiring the upstream `ContinuityClient` init and reload order.

Port: sparticle. Upstream license and credit carried: the crate is
LGPL-3.0-only, copyright (c) PepperCode1. The license text ships with the
source.

## Tests

134 tests cover the property parsing, processor math and split logic,
resource redirect, atlas/sprite context seams, and the loader registry.
