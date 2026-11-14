# Simple Music Control

Simple Music Control ported to Horizon as a native mod. Slice 1: config, situational music picker, jukebox tracker, keybinds, music-note color tick, and sound-engine pause control. 1:1 from pajicadvance/simple-music-control commit d59f4b9 (MIT (c) 2026 pajic).  Game-facing calls (music manager start/stop, sound engine channels, key input, screens, level state) sit behind the `MusicManager` trait and `ClientView` snapshot; Horizon surfaces for those are tracked in the repo README and the horizon-rust/horizon degradation issue.

Port by sparticle. Upstream: https://github.com/pajicadvance/simple-music-control (MIT).
