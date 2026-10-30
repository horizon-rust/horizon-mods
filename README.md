# Horizon mods

The registry of what is made for [Horizon](https://github.com/horizon-rust/horizon): mods (`.hmod`), modpacks (`.hpack`)
and resource packs. Each project is reviewed once, when it is listed; its later releases are checked by machine. Browse them at
[playhorizon.dev/mods](https://playhorizon.dev/mods) or in the launcher (Mods, then Horizon beside Modrinth).

Players install from here through the launcher, which checks every file's SHA-256 and, when the author signs, its
signature before the game sees it. Sandboxed `.hmod` mods run in Horizon's sandbox,
with no file, network or clock access, and can only do what their page lists.

Native source mods (`.hnmod`, kind `native-mod`) are built into an isolated game by
a launcher with native-mod support. They run with the player’s account permissions.
Their bundles pin the exact Horizon and Horizon Mixins revisions used for the build.
Other revisions have not been validated. Registry checks inspect the source bundle without
executing native code; reviewers must also examine the source and pinned-build
validation. Publishing these requires the native-mod changes to `cargo-horizon-mod`
and the launcher to have shipped first.

**Who is responsible.** Except the projects marked Official (made by the Horizon project), everything here is made and
owned by its own authors, not by the Horizon project or its maintainers. A listing is not an endorsement. See the
[mod policy](MOD-POLICY.md): what authors promise when they submit (ports of other mods included) and how to have
a project taken down.

## Submit

A project is listed by **a link to its GitHub repository and a page**. Use the form at
[playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit) (sign in with GitHub), or open a pull request adding
`mods/<id>.toml` yourself:

```toml
id = "my_mod"
kind = "native-mod"           # mod | native-mod | modpack | resourcepack | shader
repo = "you/my-mod"           # your GitHub repository
owners = ["you"]              # GitHub logins responsible for the project
key = "RWT..."                # optional: your minisign public key, see Signing
# The page, here or in horizon-registry.toml at the root of your repository (these win):
name = "My mod"
summary = "One line."
license = "MIT"
```

A reviewer (see [`.github/CODEOWNERS`](.github/CODEOWNERS)) reads the entry, your repository and its source, and the
checks' report, and asks for changes in the pull request if needed. **That is the review: a project is reviewed when it
is listed.** Merging lists it; it appears on the site and in the launcher as soon as it has a release.

What the registry takes now: mods, native mods, modpacks of registry mods, and resource packs. Shader packs open when
Horizon's own shader pack format is out (Iris and OptiFine packs are on Modrinth). Before you release, run what the
checks run:

```sh
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod
cargo horizon-mod check target/horizon-mod/my_mod.hmod
```

## Releasing a version

**Publish a GitHub release in your repository. That is all.** Attach the file (`.hmod`, `.hnmod`, `.hpack`,
`.hshader`, or a resource pack's `.zip`; exactly one per release), and its `.minisig` beside it if you sign. Not a
draft, not a pre-release. Within the hour:

1. The registry downloads the asset and runs the checks a submission runs: the file's own (its manifest, its content),
   its signature when your entry has a key, and that its id is your entry's. Nothing is run: a native mod's code is
   read, not built, by the registry.
2. If they pass, the version is listed. The version, the game and a native mod's pins are the file's own manifest (the
   tag when it names none); the date and the changelog are the release's, its notes. The SHA-256 is computed by the
   registry, and the launcher downloads the asset from your repository and refuses other bytes.
3. If they fail, the release is **skipped** and an issue here says why. Nothing is published. Publish a fixed release
   with a new tag.

No pull request, no reviewer. **Later releases are checked automatically, not reviewed by a person.** A published
version never changes: publish a new one. Tags and asset names use letters, digits, `.`, `-`, `_`, `+`. A bad release is
taken down in [`revoked.toml`](revoked.toml).

**Your page** can live in your repository: a `horizon-registry.toml` at its root (read at the newest release's tag) with
`name`, `summary`, `description`, `authors` and `license`. The entry's own fields win. Owners, key, kind and icon are
changed by a pull request here.

**Signing** is optional and recommended: put your [minisign](https://jedisct1.github.io/minisign/) public key in your
entry, and every release must carry a `.minisig` made with it, so nobody who gets into your repository's releases can
publish an update in your name without your key.

Projects listed before releases were read have `[[versions]]` hosted in this repository; they keep working, and a
release newer than the last of them is read like any other.

## Report

Something wrong with a project? [Report it](../../issues/new?template=report.yml). Your work published without your
permission? [Ask for a takedown](../../issues/new?template=takedown.yml) ([mod policy](MOD-POLICY.md)). A project that does harm is taken
down in [`revoked.toml`](revoked.toml): the launcher then refuses it and turns installed copies off, telling players
why.

## Layout

| Path | What |
| --- | --- |
| `mods/<id>.toml` | One project: its repository link and page (and, for older projects, hosted versions pinned by SHA-256) |
| `versions/<id>.json` | On the `versions` branch, written by the publish job: the releases read from each repository, as the index lists them |
| `icons/<id>.png` | Its icon, at most 256 KiB |
| `revoked.toml` | Files taken down |
| `.github/workflows/check.yml` | A pull request's checks |
| `.github/workflows/publish.yml` | On merge and every hour: reads new releases, then `index.json` on Pages |
| `.github/workflows/cleanup.yml` | Weekly: staging files nobody needs any more |
| `.github/workflows/native.yml` | Horizon's native mods: each changed one's tests, `.hnmod` and game check, all of them together, and weekly on the newest Nightly |
| `native/<name>/` | Horizon's own native mods (Zoom, FPS Counter, Keystrokes, Minimap, Recipes, Coordinates HUD): the source of their `.hnmod` files |
| `native/tools/` | Shared by them: apply on Horizon, check, build, package, prove (`native/tools/native.py`) |
| `scripts/` | What the workflows share (`sync_releases.py` reads the releases) |

The format of the entries and of `index.json` is
[docs/api/mod-index.md](https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-index.md) in Horizon's repository;
the checks are `cargo horizon-mod check` and `cargo horizon-mod registry`, from Horizon at the commit in the
repository variable `HORIZON_REV` (else `main`).

The index: `https://horizon-rust.github.io/horizon-mods/index.json`.

## Horizon's native mods

`native/<name>/` holds the source of Horizon's official mods, as
[horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundles on Horizon's native hook points
([docs/native-mods.md](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md)). Each folder keeps the
history of the repository it came from. A mod builds on any Horizon whose hook points are at least the version its
`compatibility.json` names; `horizon_revision` is the commit it was last tested on.

```sh
python3 native/tools/native.py check zoom          # apply on the tested Horizon and cargo check the game
python3 native/tools/native.py check all           # every native mod together
python3 native/tools/native.py package all         # native/target/<id>-<version>.hnmod, for a registry entry
python3 native/tools/native.py proof zoom --resources /path/to/.local/horizon-client/26.3
```

To make your own, start from [horizon-native-mod-template](https://github.com/horizon-rust/horizon-native-mod-template).
