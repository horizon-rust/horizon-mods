# Emojiful

Emojiful ported to Horizon as a native Rust mod.  Upstream: https://github.com/InnovativeOnlineIndustries/Emojiful at commit 6442d9d75ddc39308d8cdce8f14e0a9aa3011519 (2024-06-12), MIT (c) 2022 Innovative Online Industries. 1:1 feature port of the client core: config, emoji model (with Github/Custom/Twemoji sources), regex + text matching, profanity filter, twemoji and custom-package loading, emoji indexing for the selection GUI, and the datapack emoji flow.  Game-side seams (documented in README.md): font renderer swap and emoji quad rendering (EmojiFontRenderer, EmojiUtil.createRenderType/renderEmoji), texture upload and GIF frame upload (Emoji.DownloadImageData and the Twitmoji/Emojipedia variants), asset HTTP fetch (CommonClass.readStringFromURL, Emoji.loadTextureFromServer), recipe-manager lookup (onRecipesUpdated) and

Port by sparticle. Upstream: https://github.com/InnovativeOnlineIndustries/Emojiful (MIT).
