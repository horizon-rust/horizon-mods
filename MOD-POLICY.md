# Mod policy

This registry lists mods, modpacks and packs for [Horizon](https://github.com/horizon-rust/horizon). This page says
who makes them, who is responsible for them, and how to have one removed. By submitting to the registry you accept it.

## Who makes the mods

- **Mods made by the Horizon project** are marked **Official**: their author is "Horizon" and their source is in a
  [horizon-rust](https://github.com/horizon-rust) repository. They are the only mods the Horizon
  project makes and is responsible for.
- **Every other mod is made, owned and maintained by its own authors**, named on its page. The Horizon project and
  its maintainers are not their authors. They don't own these mods, sell them, or license them to you; each mod's
  licence is the one its authors chose.
- **A listing is not an endorsement.** Reviewers check that an entry is complete, that its files match their pinned
  hashes, that it builds or loads against the Horizon version it names, and that nothing obviously harmful is in it.
  That review is not a legal review, a security audit or a guarantee. Mods are provided as they are, without
  warranty, by their authors.
- **A project is reviewed when it is listed, not release by release.** Its repository is linked from its entry, and the
  releases it publishes there afterwards are listed after automatic checks only (the file's format, its hash, its
  signature when the project has a key); no person reads them. The authors are responsible for every release; the
  Horizon project can take any of them down.
- **Native mods** (built into the game with [horizon-mixins](https://github.com/horizon-rust/horizon-mixins)) run
  with the same access to your computer as the game. Install them only from authors you trust.

## What authors promise when they submit

Listing a project, through the form or a pull request, means you confirm that it, and every release you publish in
the repository you link, meets these promises:

1. You made it, or you have the right to publish it, under the licence its entry names.
2. **If it is a port or a translation of another mod** (for example from Java Edition), the original's licence allows
   that, or its authors gave you permission. You keep the original's copyright and licence notices, and you credit
   its authors and link the original on your page. A port of a mod whose licence reserves all rights needs its
   authors' written permission.
3. It contains no code, assets or data from Minecraft, Mojang or Microsoft beyond what their
   [usage guidelines](https://www.minecraft.net/usage-guidelines) and EULA allow. Horizon players bring their own
   copy of the game.
4. It contains nothing that harms players: no malware, no stealing of accounts or data, no hidden network access.
5. The `source` link (your repository) has the code the released files were built from.

You alone are responsible for what you publish and for any claim about it. The Horizon project may refuse, hide or
remove any project, at any time, without having to give a reason.

## Copyright and takedowns

If a project in this registry uses your work without your permission:

- [Open a takedown request](../../issues/new?template=takedown.yml) (public), or
- write privately to the maintainer through [lu4ovic.com](https://lu4ovic.com/#contact).

Say which project, which of your work it uses, and how to reach you. A claim that holds gets the project removed and
listed in [`revoked.toml`](revoked.toml): the registry stops listing it and the launcher stops offering it and turns
installed copies off. Clear claims are acted on quickly. The project's author is told, and can answer the claim in the
same issue.

Authors whose project was removed by mistake can ask for it back in that issue.

## Changes

This policy can change. The version in this repository's `main` branch is the one that applies.
