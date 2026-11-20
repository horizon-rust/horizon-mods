# Horizon mods

The registry of what is made for [Horizon](https://github.com/horizon-rust/horizon): mods (`.hmod`), modpacks (`.hpack`)
and resource packs. Every new project and every mod version is reviewed before it is published. Browse them at
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

**Cutover activation gate:** do not merge/activate this catalogue removal, change `.github/horizon-rev`, or deploy the new
submission service for public use until the matching signed game, SDK and launcher are published and real native
and wasm staging/index/install canaries pass. The current published Nightly is not proof of that gate.

**Who is responsible.** Except the projects marked Official (made by the Horizon project), everything here is made and
owned by its own authors, not by the Horizon project or its maintainers. A listing is not an endorsement. See the
[mod policy](MOD-POLICY.md): what authors promise when they submit (ports of other mods included), the licences
it accepts, and how to have a project taken down.

## Submit

Submit with a pull request:

1. Attach the file to a GitHub release of your own repository, under the name the entry gives it.
2. Open a pull request that adds or updates `mods/<id>.toml` with a new `[[versions]]` record: its `version`, `file`
   name and the file's `sha256` (`sha256sum <file>`). Link the release's file in the pull request's description:
   `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>`.

Then:

1. The `stage` workflow downloads each linked file, makes sure its name and SHA-256 are a new record's, runs the
   checker on it and puts it in the `staging` prerelease as `<sha256>.<extension>`. If the check failed because the
   file was missing, it re-runs the check once. It reads the pull request only as data and runs none of it. At most
   3 files a run, 64 MiB each.
2. Checks compare the entry with main, verify package structure, hashes and signatures, and build the candidate
   index. They never compile or execute submitted code. Their report and candidate index are in the pull request.
3. A reviewer (see [`.github/CODEOWNERS`](.github/CODEOWNERS)) reads the entry, the source and the report (a native
   mod's source also gets a security review), and asks for changes in the pull request if needed. An owner's update
   of a pack skips this step (see Updates below).
4. Merging publishes it: the file becomes a release, the project appears on the site and in the launcher within
   minutes.

The form at [playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit) is legacy: it can't take current
packages ([#1289](../../issues/1289)).

Accepted kinds include content/wasm mods, native source mods, modpacks of current registry mods, resource/data packs
and shader packs supported by the checker. Before submitting, use the checker for the reviewed engine revision:

```sh
cargo install --locked --git https://github.com/horizon-rust/horizon --rev "$(cat .github/horizon-rev)" cargo-horizon-mod
cargo horizon-mod check target/horizon-mod/my_mod.hmod
```

**Updates** come the same way: a pull request adds a version to your entry, and a reviewer reads it before it is
published, except an owner's update of a pack (below). Only an entry's `owners` can add versions (Horizon's own former
extras are the exception, see the archive section below), and every version
of a mod, native or not, is reviewed, owner or not: a native mod's code runs with the player's account permissions.

An owner's update of a `kind = "resourcepack"`, `"shader"` (`.hshader`) or `"datapack"` entry publishes without a
reviewer once its checks pass (`owner-update.yml`); a `"shaderpack"` (a zip) is reviewed. The pull request is opened
by an account in the entry's `owner_ids` and only adds `[[versions]]` records to that one live entry, every other
field unchanged, and the project is not taken down. A new record goes above any record with the same `published`
day: the index keeps a same-day tie in the entry's order, and the launcher takes the first as the newest. New
projects, page changes and every mod version still get a person. A published file never changes: publish a new
version.

The checker reads a resource or data pack's `pack.mcmeta` and paths, so other files in it, `.wasm` or Rust sources
included, are accepted but never loaded (a shader pack holding them is refused). In
[horizon](https://github.com/horizon-rust/horizon) at `.github/horizon-rev`:

- the kind decides where the launcher installs the file: `resourcepacks/`, `shaderpacks/`, or the zip copied into a
  world's `datapacks/` (`launcher/src/browser/horizon.rs:830-837`, `launcher/src/datapacks.rs:92-106`);
- the game loads mods, and so code, only from `horizon-mods/` and `horizon-native-mods/`
  (`crates/horizon-mod-package/src/mod_list.rs:16-19`, `crates/horizon-client/src/game/modding/mods/mod.rs:1557-1558`);
- the file must have its kind's extension, `.zip` or `.hshader`, or the index build refuses it
  (`crates/horizon-mod-package/src/index.rs:113-120`, `623-627`);
- an owner update can't change the kind (`scripts/owner_update.py`).

**Signing** is optional and recommended: put your [minisign](https://jedisct1.github.io/minisign/) public key in your
entry, and every later version must be signed with it, so nobody else can publish an update in your name.

## Staging by hand

A file linked somewhere `stage` doesn't take (not a GitHub release download, or more than 3 files) is staged by a
maintainer, as is any file once `staging` holds 900 (a release takes 1000; `cleanup.yml` makes room). From a checkout of main they trust, signed in to `gh` as a maintainer, they run
`scripts/stage.sh <pull request> <base sha> <head sha> <link>` with the head they reviewed and
`git merge-base origin/main <head sha>`. To have both commits without checking the pull request out, first run
`git fetch origin main refs/pull/<pull request>/head`. The script reads the pull request as data and runs none of its
code; it stages the file once its name and SHA-256 match a version the pull request adds. Then they re-run the
check, and merge that reviewed head once its required checks pass.

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
| `.github/workflows/stage.yml` | A pull request's new files, into `staging` |
| `.github/workflows/check.yml` | A pull request's checks |
| `.github/workflows/owner-update.yml` | After the check: merges an owner's update of a resource, shader or data pack |
| `.github/workflows/publish.yml` | On merge: releases, then `index.json` on Pages |
| `.github/workflows/cleanup.yml` | Weekly: staging files nobody needs any more |
| `.github/workflows/checker.yml` | When the pin changes: builds the checker once, attested, as release `checker@<rev>` |
| `archive/native-v1/mods/` | Frozen original native-project histories, including earlier `.hmod` releases, and their dependent modpack; never installed or validated as current packages |
| `archive/api-0.1/mods/` | Frozen shader API 0.1 packs and content mods with pre-SDK manifests, whole projects with no version the current tool reads; same rules as native-v1 |
| `native/<name>/` | Frozen pre-SDK official-mod source history; not migrated mods or current build automation |
| `scripts/catalogue.py` | Byte-preserving archival review and the explicitly incompatible historical catalogue page |
| `scripts/` | What the workflows share |

The entry and index format is
[the registry reference](https://github.com/horizon-rust/horizon/blob/main/docs/modding/src/reference/registry-index.md).
Checks and publishing use `cargo horizon-mod` from **the exact reviewed 40-character engine commit** in
`.github/horizon-rev`. Each job reads it from its trusted checkout (a pull request's check from main, never from the
pull request), so fork pull requests get the same tool. Changing the pin is a reviewed commit to main. A live project leaves the index only by a move listed in
`.github/archive-moves.txt` on main.
Missing, abbreviated or floating revisions fail closed; there is no `main` fallback.
Deploy `horizon-mod-registry` from that coordinated engine release too: an old deployed service does not understand
the new package contract. First land `scripts/catalogue.py` and its unit tests alone as inert, reviewed tooling,
without changing workflows or the catalogue. The cutover PR can then use that trusted base script normally;
never execute the pull request's copy or bypass required checks to bootstrap the change.

The index: `https://horizon-rust.github.io/horizon-mods/index.json`.

## Frozen native-v1 history

The pre-SDK catalogue is preserved byte-for-byte in `archive/native-v1/mods/`, including original owners, signing
keys, hashes, signatures, source links and mixed `.hmod`/`.hnmod` version histories. All 137 old native projects and Atomic Industry (both published modpack
versions require old native bundles) are outside the live index. Shader packs that need shader API 0.1 and content
mods whose manifest predates the SDK are frozen the same way in `archive/api-0.1/mods/`, with their own
[historical catalogue](https://horizon-rust.github.io/horizon-mods/archive/api-0.1/). This is an explicit clean-break incompatibility, **not completed ports**. No `.hnmod`
validation/runtime path or transparent legacy fallback remains. Published releases and Git history are not deleted
or rewritten; cleanup also preserves archived staging copies.

Publishing generates [the historical catalogue](https://horizon-rust.github.io/horizon-mods/archive/native-v1/),
with original metadata and direct release links. Its `catalogue.json` is labelled non-installable, not a runtime
index. Modpack dependencies in the current index must all refer to current packages; a dependent historical pack
is archived rather than offered with missing dependencies.

Archived IDs are reserved by the pull request check: a `mods/<id>.toml` for an archived id passes only when the
pull request was opened by an account whose numeric GitHub id is in the archived entry's `owner_ids` (after a
reviewed transfer, below, the new account's id instead of the old one's). Anything else
fails, including an archived entry without `owner_ids` and every pull request the submission form opens (as its bot).
That failure comes last: every other check still runs, and the final step says "owner not confirmed; every other check
passed" (or that other checks failed too). A maintainer may then admin-merge a reactivation only after verifying the
owner by hand, and only when every step except that final owner step is green on the exact head they reviewed.
The form service itself still checks only live ids. Owners reactivate a project through a manually reviewed `mods/<id>.toml` with a genuinely new `.hmod`
version, preserving kind, owners, numeric owner IDs and signing key. An archived id moves to another account only
with evidence that it is the same person (a deleted account, say), or, for Horizon's own former extras, to the
`horizon-rust` organization: a maintainer adds a line to `.github/owner-transfers.txt` on main, and from then on the
new account is the owner. Those extras (`authors = ["Horizon"]`: Zoom, FPS Counter, Coordinates HUD, Keystrokes,
Minimap and Recipes) belong to the organization rather than a person. An organization never opens a pull request, so
anyone may open one on GitHub with a new version of them, and a maintainer confirms the owner step by hand; the
submission form refuses them, because it accepts only an owner's own account. The entry may keep the archived
owners until it is changed, or name the new login and id together. The one kind change allowed is a legacy
`native-mod` coming back as a `mod` (content or sandboxed wasm), which can do less than native code. The frozen entry stays unchanged. That review checks the port, its source and
exact-SDK proof, and the normal package/index checks still run. Consumer-mod ports are not part of this cutover.

## Build and publication trust

The old `native.yml` and `native-compat.yml`, their source-application/check/trace/package tools and mutable
`native-health` index overlay are retired. There is no scheduled third-party Rust build, shared trusted Actions
cache, combined-game publisher or signing step in this repository. This replaces the unsafe context in #390;
the old combined-game proposal #381 is not revived.

Trusted validation and publication jobs restore no Actions cache. They take the pinned checker from release
`checker@<rev>` only when GitHub's build attestation shows `checker.yml` on main built that file for that exact pin,
and otherwise build it from source. So that hourly
runs don't count as players' downloads, each publish uploads the files it verified as a short-lived artifact, and
the next one, and every pull request's check, takes the matching ones from the last successful publish run this
repository started itself (a push, the schedule or a dispatch on main, never a pull request's run), outside the
checkout, and downloads the rest; every byte is rehashed against
its entry, kept or downloaded, and anything missing or mismatched is downloaded again from the original version
release or staging. The ordinary index builder then checks package structure, metadata and author
signatures before anything is published. Existing SDK release signature/provenance authority stays in the engine's
protected release workflow; registry packages do not acquire invented signing keys.

Any future third-party build must run without write/signing credentials and outside any cache scope trusted
publishers restore; only bounded, digest-checked data may cross into a separate publisher. It must never execute
uploaded artifacts or scripts in that publisher. There is no such build job to enable in this cutover.
