# Horizon mods

The registry of what is made for [Horizon](https://github.com/horizon-rust/horizon): mods (`.hmod`), modpacks (`.hpack`)
and resource packs, each one reviewed before it is published. Browse them at
[playhorizon.dev/mods](https://playhorizon.dev/mods) or in the launcher (Mods, then Horizon beside Modrinth).

Players install through the launcher, which verifies each reviewed package's SHA-256 and, when the project has a
minisign key, its signature. `kind = "mod"` includes content and sandboxed wasm `.hmod` packages. Wasm is not a
separate catalogue kind.

Native source packages are also `.hmod`, with `kind = "native-mod"`. They contain `horizon-mod.toml`, the root Cargo
project and sources, and any resources. The launcher builds against the **signed SDK for the exact pinned game
build**, host target, side and Rust toolchain; it does not rebuild a game per mod or choose a nearby build. Native
code runs with the player's permissions. Registry checks inspect packages without compiling or executing submitted
Rust, build scripts, proc macros or wasm. Reviewers must examine sources and the author's exact-SDK build proof.
Registry-built native binaries and combined-game sets are not needed by this launcher.

**Cutover activation gate:** do not merge/activate this catalogue removal, change `HORIZON_REV`, or deploy the new
submission service for public use until the matching signed game, SDK and launcher are published and real native
and wasm staging/index/install canaries pass. The current published Nightly is not proof of that gate.

**Who is responsible.** Except the projects marked Official (made by the Horizon project), everything here is made and
owned by its own authors, not by the Horizon project or its maintainers. A listing is not an endorsement. See the
[mod policy](MOD-POLICY.md): what authors promise when they submit (ports of other mods included), the licences
it accepts, and how to have a project taken down.

## Submit

Use the form at [playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit): sign in with GitHub, pick the file
and fill in the page. The form checks the file, uploads it here and opens a pull request for you. Then:

1. Checks compare the entry with main, verify package structure, hashes and signatures, and build the candidate
   index. They never compile or execute submitted code. Their report and candidate index are in the pull request.
2. A reviewer (see [`.github/CODEOWNERS`](.github/CODEOWNERS)) reads the entry, the source and the report, and asks
   for changes in the pull request if needed.
3. Merging publishes it: the file becomes a release, the project appears on the site and in the launcher within
   minutes.

Accepted kinds include content/wasm mods, native source mods, modpacks of current registry mods, resource/data packs
and shader packs supported by the checker. Before submitting, use the checker for the reviewed engine revision:

```sh
cargo install --locked --git https://github.com/horizon-rust/horizon --rev "$HORIZON_REV" cargo-horizon-mod
cargo horizon-mod check target/horizon-mod/my_mod.hmod
```

**Updates** come the same way: the form adds a version to your entry, and a reviewer reads it before it is
published. Only an entry's `owners` can add versions, and every version of a native mod is reviewed, owner or
not: its code runs with the player's account permissions. A published file never changes: publish a new version.

**Signing** is optional and recommended: put your [minisign](https://jedisct1.github.io/minisign/) public key in your
entry, and every later version must be signed with it, so nobody else can publish an update in your name.

## Report

Something wrong with a project? [Report it](../../issues/new?template=report.yml). Your work published without your
permission? [Ask for a takedown](../../issues/new?template=takedown.yml) ([mod policy](MOD-POLICY.md)). A project that does harm is taken
down in [`revoked.toml`](revoked.toml): the launcher then refuses it and turns installed copies off, telling players
why.

## Layout

| Path | What |
| --- | --- |
| `mods/<id>.toml` | One project: its page and its versions, each pinned by SHA-256 |
| `icons/<id>.png` | Its icon, at most 256 KiB |
| `revoked.toml` | Files taken down |
| `.github/workflows/check.yml` | A pull request's checks |
| `.github/workflows/publish.yml` | On merge: releases, then `index.json` on Pages |
| `.github/workflows/cleanup.yml` | Weekly: staging files nobody needs any more |
| `archive/native-v1/mods/` | Frozen original native-project histories, including earlier `.hmod` releases, and their dependent modpack; never installed or validated as current packages |
| `native/<name>/` | Frozen pre-SDK official-mod source history; not migrated mods or current build automation |
| `scripts/catalogue.py` | Byte-preserving archival review and the explicitly incompatible historical catalogue page |
| `scripts/` | What the workflows share |

The entry and index format is
[the registry reference](https://github.com/horizon-rust/horizon/blob/main/docs/modding/src/reference/registry-index.md).
Checks and publishing use `cargo horizon-mod` from **the exact reviewed 40-character engine commit** in repository
variable `HORIZON_REV`. Missing, abbreviated or floating revisions fail closed; there is no `main` fallback.
Deploy `horizon-mod-registry` from that coordinated engine release too: an old deployed service does not understand
the new package contract. First land `scripts/catalogue.py` and its unit tests alone as inert, reviewed tooling,
without changing workflows or the catalogue. The cutover PR can then use that trusted base script normally;
never execute the pull request's copy or bypass required checks to bootstrap the change.

The index: `https://horizon-rust.github.io/horizon-mods/index.json`.

## Frozen native-v1 history

The pre-SDK catalogue is preserved byte-for-byte in `archive/native-v1/mods/`, including original owners, signing
keys, hashes, signatures, source links and mixed `.hmod`/`.hnmod` version histories. All 137 old native projects and Atomic Industry (both published modpack
versions require old native bundles) are outside the live index. The 12 content/wasm projects, 13 shaders and two
resource packs remain live. This is an explicit clean-break incompatibility, **not completed ports**. No `.hnmod`
validation/runtime path or transparent legacy fallback remains. Published releases and Git history are not deleted
or rewritten; cleanup also preserves archived staging copies.

Publishing generates [the historical catalogue](https://horizon-rust.github.io/horizon-mods/archive/native-v1/),
with original metadata and direct release links. Its `catalogue.json` is labelled non-installable, not a runtime
index. Modpack dependencies in the current index must all refer to current packages; a dependent historical pack
is archived rather than offered with missing dependencies.

Archived IDs are reserved. The submission service refuses new listings for them; their owners can reactivate a
project through a manually reviewed `mods/<id>.toml` with a genuinely new `.hmod` version, preserving kind, owners,
numeric owner IDs and signing key. The frozen entry stays unchanged. That review checks the port, its source and
exact-SDK proof, and the normal package/index checks still run. Consumer-mod ports are not part of this cutover.

## Build and publication trust

The old `native.yml` and `native-compat.yml`, their source-application/check/trace/package tools and mutable
`native-health` index overlay are retired. There is no scheduled third-party Rust build, shared trusted Actions
cache, combined-game publisher or signing step in this repository. This replaces the unsafe context in #390;
the old combined-game proposal #381 is not revived.

Trusted validation and publication jobs build the pinned checker fresh and restore no Actions cache. Files come
from content-addressed `registry-files` copies, original version releases or staging, and every byte is rehashed,
including files already on disk. The ordinary index builder then checks package structure, metadata and author
signatures before anything is published. Existing SDK release signature/provenance authority stays in the engine's
protected release workflow; registry packages do not acquire invented signing keys.

Any future third-party build must run without write/signing credentials and outside any cache scope trusted
publishers restore; only bounded, digest-checked data may cross into a separate publisher. It must never execute
uploaded artifacts or scripts in that publisher. There is no such build job to enable in this cutover.
