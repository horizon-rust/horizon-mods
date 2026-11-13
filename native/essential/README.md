# Essential for Horizon (behavioral Rust port)

A behavioral port of [Essential](https://modrinth.com/mod/essential) by EssentialGG
(source-available license, view-only): no upstream code is included. Ported 1:1
against the behavior the mod's page documents, under CC0-1.0.

Essential's headline features live behind its own hosted backend, which a clean-room
port cannot ship against; this port covers the local half of each feature and files
the rest.

## What is ported (the behavior the mod page documents)

- **Session invite codes, local half**: the host issues short alphanumeric invite
  codes with an optional expiry, lists the still-live ones, and revokes them;
  revoked, expired and unknown codes all reject at join time.
- **In-game skin manager, local half**: save skins (slim/wide model flag), wear one
  by name, and undress by removing the worn skin; unknown skins are rejected and
  leave the current one on.
- **Screenshot gallery, local half**: screenshots are listed newest-first,
  favorited, deleted, and flagged into a share outbox.

Tests: 8 (`cargo test`), all passing.

## Degradations (backend-dependent, not shippable from a clean-room port)

1. **World hosting transport** - upstream hosts worlds through its own peer-to-peer
   relay service. The port owns the invite-code lifecycle; binding redemption to an
   actual connection path needs a hosting seam (same shape as the e4mc port's relay
   wiring on Horizon's own connection path).
2. **Social chat, groups, picture sharing** - upstream's social service carries all
   of it; the port keeps the local share outbox and needs a friend/messaging
   transport to deliver.
3. **Cosmetics, outfits, emotes** - delivered from upstream's servers; no local
   equivalent is shippable without licensing the assets.

Host seams (need Horizon native-mod API support):

1. A keybind/UI seam for the skin-manager and gallery screens (screen overlay hook).
2. A client skin-texture override hook for the model layers.
3. A screenshot-capture hook so captures feed the gallery.
