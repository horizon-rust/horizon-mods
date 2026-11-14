//! Simple Music Control ported to Horizon as a native mod.
//! Slice 1: config, situational music picker, jukebox tracker, keybinds,
//! music-note color tick, and sound-engine pause control. 1:1 from
//! pajicadvance/simple-music-control commit d59f4b9 (MIT (c) 2026 pajic).
//!
//! Game-facing calls (music manager start/stop, sound engine channels, key
//! input, screens, level state) sit behind the `MusicManager` trait and
//! `ClientView` snapshot; Horizon surfaces for those are tracked in the
//! repo README and the horizon-rust/horizon degradation issue.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MOD_ID: &str = "simple_music_control";
pub const CONFIG_FILE: &str = "simple_music_control.json";

/// Mirrors SMC.id(path).
pub fn id(path: &str) -> String {
    format!("{MOD_ID}:{path}")
}

/// Mirrors SMC.debugLog: logs only in a dev environment.
pub fn debug_log(_message: &str) {
    // No-op in release; dev-env logging is a platform seam.
}

// ---------------------------------------------------------------------------
// WidgetPosition (gui/WidgetPosition.java)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WidgetPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl WidgetPosition {
    pub fn prefix() -> &'static str {
        "simple_music_control.config.widgetPosition"
    }

    /// WidgetPosition.getName(): translatable key "prefix"."NAME".
    pub fn translation_key(&self) -> String {
        let name = match self {
            WidgetPosition::TopLeft => "TOP_LEFT",
            WidgetPosition::TopRight => "TOP_RIGHT",
            WidgetPosition::BottomLeft => "BOTTOM_LEFT",
            WidgetPosition::BottomRight => "BOTTOM_RIGHT",
        };
        format!("{}.{}", Self::prefix(), name)
    }
}

// ---------------------------------------------------------------------------
// Music ids (util/Constants.java wraps SoundEvents / Musics constants)
// ---------------------------------------------------------------------------

/// The `Music` values the upstream constants wrap: situational lists are
/// built from `Musics.createGameMusic(SoundEvents.MUSIC_BIOME_* /
/// MUSIC_UNDER_WATER)`, plus `Musics.CREATIVE` and `Musics.GAME`. Variant
/// names mirror the upstream SoundEvents constant; mapping each to its real
/// vanilla sound id happens at the audio seam.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MusicId {
    MUSIC_BIOME_DEEP_DARK,
    MUSIC_BIOME_DRIPSTONE_CAVES,
    MUSIC_BIOME_GROVE,
    MUSIC_BIOME_JAGGED_PEAKS,
    MUSIC_BIOME_LUSH_CAVES,
    MUSIC_BIOME_SWAMP,
    MUSIC_BIOME_FOREST,
    MUSIC_BIOME_OLD_GROWTH_TAIGA,
    MUSIC_BIOME_MEADOW,
    MUSIC_BIOME_CHERRY_GROVE,
    MUSIC_BIOME_FROZEN_PEAKS,
    MUSIC_BIOME_SNOWY_SLOPES,
    MUSIC_BIOME_STONY_PEAKS,
    MUSIC_BIOME_FLOWER_FOREST,
    MUSIC_BIOME_DESERT,
    MUSIC_BIOME_BADLANDS,
    MUSIC_BIOME_JUNGLE,
    MUSIC_BIOME_SPARSE_JUNGLE,
    MUSIC_BIOME_BAMBOO_JUNGLE,
    MUSIC_UNDER_WATER,
    MUSIC_BIOME_BASALT_DELTAS,
    MUSIC_BIOME_CRIMSON_FOREST,
    MUSIC_BIOME_NETHER_WASTES,
    MUSIC_BIOME_SOUL_SAND_VALLEY,
    MUSIC_BIOME_WARPED_FOREST,
    CREATIVE,
    GAME,
}

/// Constants.OVERWORLD_SITUATIONAL_MUSIC, in upstream order (20 entries).
pub fn overworld_situational_music() -> Vec<MusicId> {
    vec![
        MusicId::MUSIC_BIOME_DEEP_DARK,
        MusicId::MUSIC_BIOME_DRIPSTONE_CAVES,
        MusicId::MUSIC_BIOME_GROVE,
        MusicId::MUSIC_BIOME_JAGGED_PEAKS,
        MusicId::MUSIC_BIOME_LUSH_CAVES,
        MusicId::MUSIC_BIOME_SWAMP,
        MusicId::MUSIC_BIOME_FOREST,
        MusicId::MUSIC_BIOME_OLD_GROWTH_TAIGA,
        MusicId::MUSIC_BIOME_MEADOW,
        MusicId::MUSIC_BIOME_CHERRY_GROVE,
        MusicId::MUSIC_BIOME_FROZEN_PEAKS,
        MusicId::MUSIC_BIOME_SNOWY_SLOPES,
        MusicId::MUSIC_BIOME_STONY_PEAKS,
        MusicId::MUSIC_BIOME_FLOWER_FOREST,
        MusicId::MUSIC_BIOME_DESERT,
        MusicId::MUSIC_BIOME_BADLANDS,
        MusicId::MUSIC_BIOME_JUNGLE,
        MusicId::MUSIC_BIOME_SPARSE_JUNGLE,
        MusicId::MUSIC_BIOME_BAMBOO_JUNGLE,
        MusicId::MUSIC_UNDER_WATER,
    ]
}

/// Constants.NETHER_SITUATIONAL_MUSIC, in upstream order (5 entries).
pub fn nether_situational_music() -> Vec<MusicId> {
    vec![
        MusicId::MUSIC_BIOME_BASALT_DELTAS,
        MusicId::MUSIC_BIOME_CRIMSON_FOREST,
        MusicId::MUSIC_BIOME_NETHER_WASTES,
        MusicId::MUSIC_BIOME_SOUL_SAND_VALLEY,
        MusicId::MUSIC_BIOME_WARPED_FOREST,
    ]
}

// ---------------------------------------------------------------------------
// Dimension (Level.OVERWORLD / Level.NETHER comparisons in ModUtil)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dimension {
    Overworld,
    Nether,
    Other,
}

// ---------------------------------------------------------------------------
// Config (config/ModConfig.java). fzzy_config field defaults + ValidatedInt
// ranges; keys are the Java field names, camelCase.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub stop_music_on_jukebox_use: bool,
    pub creative_music_in_survival: bool,
    pub situational_music_in_creative: bool,
    pub music_min_delay: i32,
    pub music_max_delay: i32,
    pub play_music_when_paused: bool,
    pub now_playing_widget: bool,
    pub now_playing_widget_duration: i32,
    pub show_now_playing_widget_in_pause_menu: bool,
    pub show_now_playing_widget_on_title_screen: bool,
    pub pause_widget_position: WidgetPosition,
    pub title_widget_position: WidgetPosition,
    pub unlock_situational_music: bool,
    pub situational_music_chance: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stop_music_on_jukebox_use: true,
            creative_music_in_survival: true,
            situational_music_in_creative: true,
            music_min_delay: 300,
            music_max_delay: 600,
            play_music_when_paused: true,
            now_playing_widget: true,
            now_playing_widget_duration: 6,
            show_now_playing_widget_in_pause_menu: true,
            show_now_playing_widget_on_title_screen: true,
            pause_widget_position: WidgetPosition::TopLeft,
            title_widget_position: WidgetPosition::TopLeft,
            unlock_situational_music: false,
            situational_music_chance: 70,
        }
    }
}

impl Config {
    /// Mirrors fzzy_config's validateAndSet clamping: musicMinDelay
    /// [0, 2400], musicMaxDelay [10, 2400], nowPlayingWidgetDuration
    /// [1, 30], situationalMusicChance [0, 100] (per the ValidatedInt
    /// constructors and the Sodium options' setRange calls).
    pub fn clamp_ranges(&mut self) {
        self.music_min_delay = self.music_min_delay.clamp(0, 2400);
        self.music_max_delay = self.music_max_delay.clamp(10, 2400);
        self.now_playing_widget_duration = self.now_playing_widget_duration.clamp(1, 30);
        self.situational_music_chance = self.situational_music_chance.clamp(0, 100);
    }
}

/// Config load: broken or absent file falls back to upstream defaults.
pub fn load_config(json: &str) -> Config {
    let mut config: Config = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(_) => Config::default(),
    };
    config.clamp_ranges();
    config
}

// ---------------------------------------------------------------------------
// Keybinds (keybind/ModKeybinds.java)
// ---------------------------------------------------------------------------

/// KeyMapping.clickCount / consumeClick semantics.
#[derive(Default, Debug)]
pub struct KeyMapping {
    pub name: &'static str,
    pub default_key: i32,
    clicks: i32,
}

impl KeyMapping {
    pub fn new(name: &'static str, default_key: i32) -> Self {
        KeyMapping {
            name,
            default_key,
            clicks: 0,
        }
    }

    pub fn press(&mut self) {
        self.clicks += 1;
    }

    pub fn consume_click(&mut self) -> bool {
        if self.clicks > 0 {
            self.clicks -= 1;
            true
        } else {
            false
        }
    }
}

/// InputConstants.KEY_M; TOGGLE_MUSIC starts unbound (UNKNOWN).
pub const KEY_M: i32 = 77;
pub const KEY_UNKNOWN: i32 = -1;

pub fn next_music_track_key() -> KeyMapping {
    KeyMapping::new("key.simple_music_control.next_music_track", KEY_M)
}

pub fn toggle_music_key() -> KeyMapping {
    KeyMapping::new("key.simple_music_control.toggle_music", KEY_UNKNOWN)
}

// ---------------------------------------------------------------------------
// Music-manager seam: the game-facing surface (MusicManager stop/start,
// getSituationalMusic, and the SoundEngine MusicPauseControl mixin).
// ---------------------------------------------------------------------------

pub trait MusicManager {
    /// Minecraft.getMusicManager().stopPlaying()
    fn stop_playing(&mut self);
    /// Minecraft.getMusicManager().startPlaying(music)
    fn start_playing(&mut self, music: MusicId);
    /// Minecraft.getSituationalMusic(); None == null
    fn situational_music(&self) -> Option<MusicId>;
    /// MusicManager.getCurrentMusicTranslationKey() != null
    fn has_current_music(&self) -> bool;
    /// SoundEngineMixin.smc$setMusicPaused (MusicPauseControl)
    fn set_music_paused(&mut self, paused: bool);
}

// ---------------------------------------------------------------------------
// SoundEngine (mixin/client/SoundEngineMixin.java, >=26.1 path)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SoundSource {
    Music,
    Other,
}

/// A ChannelAccess.ChannelHandle's underlying audio channel, reduced to the
/// paused state SoundEngineMixin drives.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub source: SoundSource,
    pub paused: bool,
}

/// SoundEngine state: `loaded` and instanceToChannel, reduced to MUSIC vs
/// other sources.
pub struct SoundEngine {
    pub loaded: bool,
    pub channels: Vec<Channel>,
}

impl Default for SoundEngine {
    fn default() -> Self {
        SoundEngine {
            loaded: false,
            channels: Vec::new(),
        }
    }
}

impl SoundEngine {
    /// SoundEngineMixin.smc$setMusicPaused: skip when not loaded; when
    /// unpausing, skip while the client is paused and
    /// playMusicWhenPaused is false; otherwise pause/unpause every
    /// MUSIC-source channel.
    pub fn smc_set_music_paused(&mut self, paused: bool, config: &Config, client_paused: bool) {
        if !self.loaded {
            return;
        }
        if !paused && client_paused && !config.play_music_when_paused {
            return;
        }
        for channel in self.channels.iter_mut() {
            if channel.source == SoundSource::Music {
                channel.paused = paused;
            }
        }
    }

    /// SoundEngineMixin.pauseMusicIfConfigured (>=26.1): wrap of
    /// pauseAllExcept(ignoredSources). When playMusicWhenPaused is false,
    /// MUSIC is removed from the ignored list so the music channels are
    /// paused along with everything else.
    pub fn pause_all_except(&mut self, ignored: &[SoundSource], config: &Config) {
        let ignored: Vec<SoundSource> = if config.play_music_when_paused {
            ignored.to_vec()
        } else {
            ignored
                .iter()
                .copied()
                .filter(|s| *s != SoundSource::Music)
                .collect()
        };
        for channel in self.channels.iter_mut() {
            if !ignored.contains(&channel.source) {
                channel.paused = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JukeboxTracker (util/JukeboxTracker.java). BlockPos keys are [x, y, z].
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct JukeboxTracker {
    /// Object2BooleanOpenHashMap<BlockPos>: true == playing in range.
    pub jukeboxes: HashMap<[i64; 3], bool>,
}

impl JukeboxTracker {
    /// onJukeboxPlay(level, minecraft, pos): only when the level is present,
    /// record the jukebox as playing and stop game music if configured.
    pub fn on_jukebox_play(
        &mut self,
        level_present: bool,
        pos: [i64; 3],
        config: &Config,
        music: &mut dyn MusicManager,
    ) {
        if level_present {
            self.jukeboxes.insert(pos, true);
            if config.stop_music_on_jukebox_use {
                music.stop_playing();
            }
        }
    }

    pub fn on_jukebox_stop(&mut self, pos: [i64; 3]) {
        self.jukeboxes.remove(&pos);
    }

    /// noJukeboxesInRange(): true when the feature is off or no tracked
    /// jukebox is marked playing-in-range.
    pub fn no_jukeboxes_in_range(&self, config: &Config) -> bool {
        !config.stop_music_on_jukebox_use || self.jukeboxes.values().all(|&playing| !playing)
    }
}

// ---------------------------------------------------------------------------
// ModUtil (util/ModUtil.java): globalPause, situational picker, client tick.
// ---------------------------------------------------------------------------

/// The slice of LocalPlayer/Level state the picker reads.
pub struct PlayerCtx {
    pub dimension: Dimension,
    pub creative: bool,
}

/// level.getRandom(): nextFloat() and nextInt(bound).
pub trait LevelRng {
    fn next_float(&mut self) -> f32;
    fn next_int(&mut self, bound: usize) -> usize;
}

/// A small deterministic stand-in for level.getRandom() used in tests.
pub struct VanillaRng {
    state: u64,
}

impl VanillaRng {
    pub fn new(seed: u64) -> Self {
        VanillaRng { state: seed | 1 }
    }
}

impl LevelRng for VanillaRng {
    fn next_float(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 40) as f32) / (1u64 << 24) as f32
    }

    fn next_int(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        let f = self.next_float();
        let v = (f * bound as f32) as usize;
        v.min(bound - 1)
    }
}

/// The slice of Minecraft state onClientTick reads each tick.
pub struct ClientView<'a, M: MusicManager + ?Sized = dyn MusicManager> {
    /// client.player position; None == no player.
    pub player_pos: Option<[f64; 3]>,
    pub now_ms: u64,
    pub pause_screen_open: bool,
    pub level_present: bool,
    pub music: &'a mut M,
}

/// gui/MusicNoteColorManager state. musicNoteColor starts at -1 upstream
/// (no note rendered until the first color tick).
pub struct MusicNoteColorManager {
    music_note_color_tick: u32,
    last_music_note_color_change: u64,
    pub music_note_color: i32,
}

impl Default for MusicNoteColorManager {
    fn default() -> Self {
        MusicNoteColorManager {
            music_note_color_tick: 0,
            last_music_note_color_change: 0,
            music_note_color: -1,
        }
    }
}

impl MusicNoteColorManager {
    /// tickMusicNotes(client): while current music is playing, advance the
    /// color at most once per 25 ms.
    pub fn tick_music_notes(&mut self, has_current_music: bool, now_ms: u64) {
        if has_current_music {
            if now_ms > self.last_music_note_color_change + 25 {
                self.music_note_color_tick += 1;
                self.last_music_note_color_change = now_ms;
                self.music_note_color = get_lerped_color(self.music_note_color_tick as f32);
            }
        }
    }

    pub fn music_note_color_tick(&self) -> u32 {
        self.music_note_color_tick
    }
}

/// getLerpedColor: vanilla >=26.1 calls
/// ColorLerper.getLerpedColor(ColorLerper.Type.MUSIC_NOTE, f). Horizon has
/// no equivalent surface yet, so this returns the pre-init sentinel -1
/// until the seam lands (tracked in the degradation issue); the mod's own
/// gate logic above is complete.
pub fn get_lerped_color(f: f32) -> i32 {
    let _ = f;
    -1
}

/// gui/NowPlayingWidget state slice needed by the toggle path (rendering is
/// a later slice).
pub struct WidgetState {
    pub now_playing_timer_ms: i64,
    pub toggle_timer_ms: i64,
    pub centered: bool,
}

impl Default for WidgetState {
    fn default() -> Self {
        WidgetState {
            now_playing_timer_ms: 0,
            toggle_timer_ms: 0,
            centered: true,
        }
    }
}

impl WidgetState {
    /// displayToggleNotification(): when no pause screen is open, arm the
    /// toggle notification for the configured duration; centered tracks
    /// whether a level is loaded.
    pub fn display_toggle_notification(
        &mut self,
        pause_screen_open: bool,
        level_present: bool,
        duration_s: i32,
    ) {
        if !pause_screen_open {
            self.centered = level_present;
            self.toggle_timer_ms = duration_s as i64 * 1000;
        }
    }
}

/// Core mod state: SMC.CONFIG + the statics the client tick touches.
pub struct Smc {
    pub config: Config,
    pub jukeboxes: JukeboxTracker,
    pub next_music_track: KeyMapping,
    pub toggle_music: KeyMapping,
    /// ModUtil.globalPause
    pub global_pause: bool,
    pub note_color: MusicNoteColorManager,
    pub widget: WidgetState,
}

impl Default for Smc {
    fn default() -> Self {
        Smc {
            config: Config::default(),
            jukeboxes: JukeboxTracker::default(),
            next_music_track: next_music_track_key(),
            toggle_music: toggle_music_key(),
            global_pause: false,
            note_color: MusicNoteColorManager::default(),
            widget: WidgetState::default(),
        }
    }
}

impl Smc {
    /// ModUtil.pickRandomSituationalMusic, 1:1.
    pub fn pick_random_situational_music(
        &self,
        player: &PlayerCtx,
        rng: &mut dyn LevelRng,
    ) -> Option<MusicId> {
        if self.config.unlock_situational_music {
            let situational =
                rng.next_float() < self.config.situational_music_chance as f32 / 100.0;
            if player.dimension == Dimension::Overworld {
                if situational {
                    let list = overworld_situational_music();
                    return Some(list[rng.next_int(list.len())]);
                } else if player.creative {
                    return Some(MusicId::CREATIVE);
                } else if self.config.creative_music_in_survival {
                    return Some(MusicId::CREATIVE);
                } else {
                    return Some(MusicId::GAME);
                }
            } else if situational && player.dimension == Dimension::Nether {
                let list = nether_situational_music();
                return Some(list[rng.next_int(list.len())]);
            }
        }
        None
    }

    /// ModKeybinds.onNextMusicTrack, 1:1: only when no jukebox is playing in
    /// range, restart situational music.
    pub fn on_next_music_track<M: MusicManager + ?Sized>(&self, music: &mut M) {
        if self.jukeboxes.no_jukeboxes_in_range(&self.config) {
            music.stop_playing();
            if let Some(m) = music.situational_music() {
                music.start_playing(m);
            }
        }
    }

    /// ModKeybinds.onToggleMusic, 1:1: flip the global pause, push it to the
    /// sound engine, and arm the toggle notification.
    pub fn on_toggle_music<M: MusicManager + ?Sized>(&mut self, view: &mut ClientView<'_, M>) {
        self.global_pause = !self.global_pause;
        view.music.set_music_paused(self.global_pause);
        self.widget.display_toggle_notification(
            view.pause_screen_open,
            view.level_present,
            self.config.now_playing_widget_duration,
        );
    }

    /// ModUtil.onClientTick, 1:1: jukebox range sweep, keybind consumes,
    /// then the music-note color tick.
    pub fn on_client_tick<M: MusicManager + ?Sized>(&mut self, view: &mut ClientView<'_, M>) {
        if self.config.stop_music_on_jukebox_use {
            if let Some(player) = view.player_pos {
                if !self.jukeboxes.jukeboxes.is_empty() {
                    let positions: Vec<[i64; 3]> =
                        self.jukeboxes.jukeboxes.keys().copied().collect();
                    for pos in positions {
                        // Vec3.atCenterOf(pos): +0.5 on each axis.
                        let dx = player[0] - (pos[0] as f64 + 0.5);
                        let dy = player[1] - (pos[1] as f64 + 0.5);
                        let dz = player[2] - (pos[2] as f64 + 0.5);
                        let dist_sqr = dx * dx + dy * dy + dz * dz;
                        if dist_sqr > 4096.0 {
                            self.jukeboxes.jukeboxes.insert(pos, false);
                        } else {
                            self.jukeboxes.jukeboxes.insert(pos, true);
                            view.music.stop_playing();
                        }
                    }
                }
            }
        }
        if self.next_music_track.consume_click() {
            self.on_next_music_track(view.music);
        }
        if self.toggle_music.consume_click() {
            self.on_toggle_music(view);
        }
        // MusicNoteColorManager.tickMusicNotes(client)
        let has_current = view.music.has_current_music();
        self.note_color.tick_music_notes(has_current, view.now_ms);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedRng {
        floats: Vec<f32>,
        i: usize,
    }

    impl ScriptedRng {
        fn new(floats: Vec<f32>) -> Self {
            ScriptedRng { floats, i: 0 }
        }
        fn next(&mut self) -> f32 {
            let f = self.floats[self.i % self.floats.len().max(1)];
            self.i += 1;
            f
        }
    }

    impl LevelRng for ScriptedRng {
        fn next_float(&mut self) -> f32 {
            self.next()
        }
        fn next_int(&mut self, bound: usize) -> usize {
            let f = self.next();
            if bound == 0 {
                return 0;
            }
            ((f * bound as f32) as usize).min(bound - 1)
        }
    }

    #[derive(Default)]
    struct TestMusic {
        stopped: u32,
        started: Vec<MusicId>,
        situational: Option<MusicId>,
        current: bool,
        paused: Option<bool>,
    }

    impl TestMusic {
        fn new(situational: Option<MusicId>) -> Self {
            TestMusic {
                situational,
                ..Default::default()
            }
        }
    }

    impl MusicManager for TestMusic {
        fn stop_playing(&mut self) {
            self.stopped += 1;
        }
        fn start_playing(&mut self, music: MusicId) {
            self.started.push(music);
        }
        fn situational_music(&self) -> Option<MusicId> {
            self.situational
        }
        fn has_current_music(&self) -> bool {
            self.current
        }
        fn set_music_paused(&mut self, paused: bool) {
            self.paused = Some(paused);
        }
    }

    fn config_with(f: impl FnOnce(&mut Config)) -> Config {
        let mut c = Config::default();
        f(&mut c);
        c
    }

    #[test]
    fn config_defaults_match_upstream() {
        let c = Config::default();
        assert!(c.stop_music_on_jukebox_use);
        assert!(c.creative_music_in_survival);
        assert!(c.situational_music_in_creative);
        assert_eq!(c.music_min_delay, 300);
        assert_eq!(c.music_max_delay, 600);
        assert!(c.play_music_when_paused);
        assert!(c.now_playing_widget);
        assert_eq!(c.now_playing_widget_duration, 6);
        assert!(c.show_now_playing_widget_in_pause_menu);
        assert!(c.show_now_playing_widget_on_title_screen);
        assert_eq!(c.pause_widget_position, WidgetPosition::TopLeft);
        assert_eq!(c.title_widget_position, WidgetPosition::TopLeft);
        assert!(!c.unlock_situational_music);
        assert_eq!(c.situational_music_chance, 70);
    }

    #[test]
    fn config_roundtrips_camel_case_keys() {
        let json = r#"{
            "stopMusicOnJukeboxUse": false,
            "creativeMusicInSurvival": false,
            "situationalMusicInCreative": false,
            "musicMinDelay": 120,
            "musicMaxDelay": 900,
            "playMusicWhenPaused": false,
            "nowPlayingWidget": false,
            "nowPlayingWidgetDuration": 12,
            "showNowPlayingWidgetInPauseMenu": false,
            "showNowPlayingWidgetOnTitleScreen": false,
            "pauseWidgetPosition": "BOTTOM_RIGHT",
            "titleWidgetPosition": "TOP_RIGHT",
            "unlockSituationalMusic": true,
            "situationalMusicChance": 33
        }"#;
        let c = load_config(json);
        assert!(!c.stop_music_on_jukebox_use);
        assert!(!c.creative_music_in_survival);
        assert!(!c.situational_music_in_creative);
        assert_eq!(c.music_min_delay, 120);
        assert_eq!(c.music_max_delay, 900);
        assert!(!c.play_music_when_paused);
        assert!(!c.now_playing_widget);
        assert_eq!(c.now_playing_widget_duration, 12);
        assert!(!c.show_now_playing_widget_in_pause_menu);
        assert!(!c.show_now_playing_widget_on_title_screen);
        assert_eq!(c.pause_widget_position, WidgetPosition::BottomRight);
        assert_eq!(c.title_widget_position, WidgetPosition::TopRight);
        assert!(c.unlock_situational_music);
        assert_eq!(c.situational_music_chance, 33);
    }

    #[test]
    fn config_broken_json_falls_back_to_defaults() {
        let c = load_config("{not json at all");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn config_empty_json_falls_back_to_defaults() {
        assert_eq!(load_config("{}"), Config::default());
    }

    #[test]
    fn config_clamps_to_validated_ranges() {
        let mut c = config_with(|c| {
            c.music_min_delay = -5;
            c.music_max_delay = 99999;
            c.now_playing_widget_duration = 0;
            c.situational_music_chance = 150;
        });
        c.clamp_ranges();
        assert_eq!(c.music_min_delay, 0);
        assert_eq!(c.music_max_delay, 2400);
        assert_eq!(c.now_playing_widget_duration, 1);
        assert_eq!(c.situational_music_chance, 100);
        let mut c = config_with(|c| {
            c.music_min_delay = -1;
        });
        c.clamp_ranges();
        assert_eq!(c.music_min_delay, 0);
    }

    #[test]
    fn widget_position_serde_uses_upstream_variant_names() {
        assert_eq!(
            serde_json::to_string(&WidgetPosition::TopLeft).unwrap(),
            "\"TOP_LEFT\""
        );
        assert_eq!(
            serde_json::to_string(&WidgetPosition::BottomLeft).unwrap(),
            "\"BOTTOM_LEFT\""
        );
        assert_eq!(
            serde_json::to_string(&WidgetPosition::BottomRight).unwrap(),
            "\"BOTTOM_RIGHT\""
        );
    }

    #[test]
    fn widget_position_translation_key_matches_prefix() {
        assert_eq!(
            WidgetPosition::TopRight.translation_key(),
            "simple_music_control.config.widgetPosition.TOP_RIGHT"
        );
    }

    #[test]
    fn keybinding_defaults_match_upstream() {
        let next = next_music_track_key();
        assert_eq!(next.name, "key.simple_music_control.next_music_track");
        assert_eq!(next.default_key, KEY_M);
        let toggle = toggle_music_key();
        assert_eq!(toggle.name, "key.simple_music_control.toggle_music");
        assert_eq!(toggle.default_key, KEY_UNKNOWN);
    }

    #[test]
    fn consume_click_semantics() {
        let mut key = next_music_track_key();
        assert!(!key.consume_click());
        key.press();
        assert!(key.consume_click());
        assert!(!key.consume_click());
        key.press();
        key.press();
        assert!(key.consume_click());
        assert!(key.consume_click());
        assert!(!key.consume_click());
    }

    #[test]
    fn constants_lists_match_upstream_sizes_and_order() {
        let overworld = overworld_situational_music();
        let nether = nether_situational_music();
        assert_eq!(overworld.len(), 20);
        assert_eq!(nether.len(), 5);
        assert_eq!(overworld[0], MusicId::MUSIC_BIOME_DEEP_DARK);
        assert_eq!(overworld[19], MusicId::MUSIC_UNDER_WATER);
        assert_eq!(nether[0], MusicId::MUSIC_BIOME_BASALT_DELTAS);
        assert_eq!(nether[4], MusicId::MUSIC_BIOME_WARPED_FOREST);
        let mut all = overworld.clone();
        all.extend(nether);
        let mut sorted = all.clone();
        sorted.sort_by_key(|m| format!("{m:?}"));
        sorted.dedup_by_key(|m| format!("{m:?}"));
        assert_eq!(all.len(), sorted.len(), "no duplicate entries");
    }

    #[test]
    fn jukebox_play_records_and_stops_music_when_configured() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        smc.jukeboxes
            .on_jukebox_play(true, [1, 64, -3], &smc.config, &mut music);
        assert_eq!(smc.jukeboxes.jukeboxes.get(&[1, 64, -3]), Some(&true));
        assert_eq!(music.stopped, 1);
    }

    #[test]
    fn jukebox_play_without_stop_music_config_leaves_music_alone() {
        let mut smc = Smc::default();
        smc.config.stop_music_on_jukebox_use = false;
        let mut music = TestMusic::new(None);
        smc.jukeboxes
            .on_jukebox_play(true, [0, 0, 0], &smc.config, &mut music);
        assert_eq!(smc.jukeboxes.jukeboxes.get(&[0, 0, 0]), Some(&true));
        assert_eq!(music.stopped, 0);
    }

    #[test]
    fn jukebox_play_ignored_without_level() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        smc.jukeboxes
            .on_jukebox_play(false, [0, 0, 0], &smc.config, &mut music);
        assert!(smc.jukeboxes.jukeboxes.is_empty());
        assert_eq!(music.stopped, 0);
    }

    #[test]
    fn jukebox_stop_removes_entry() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        smc.jukeboxes
            .on_jukebox_play(true, [5, 5, 5], &smc.config, &mut music);
        smc.jukeboxes.on_jukebox_stop([5, 5, 5]);
        assert!(smc.jukeboxes.jukeboxes.is_empty());
    }

    #[test]
    fn no_jukeboxes_in_range_follows_tracker_and_config() {
        let mut smc = Smc::default();
        assert!(
            smc.jukeboxes.no_jukeboxes_in_range(&smc.config),
            "empty map"
        );
        smc.jukeboxes.jukeboxes.insert([0, 0, 0], true);
        assert!(!smc.jukeboxes.no_jukeboxes_in_range(&smc.config));
        smc.jukeboxes.jukeboxes.insert([0, 0, 0], false);
        assert!(smc.jukeboxes.no_jukeboxes_in_range(&smc.config));
        smc.config.stop_music_on_jukebox_use = false;
        smc.jukeboxes.jukeboxes.insert([1, 1, 1], true);
        assert!(
            smc.jukeboxes.no_jukeboxes_in_range(&smc.config),
            "feature off"
        );
    }

    #[test]
    fn client_tick_marks_far_jukebox_inactive_without_stopping_music() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        // Jukebox at 65 blocks away: distance^2 = 4225 > 4096.
        smc.jukeboxes.jukeboxes.insert([65, 0, 0], true);
        let mut view = ClientView {
            player_pos: Some([0.5, 0.5, 0.5]),
            now_ms: 0,
            pause_screen_open: false,
            level_present: true,
            music: &mut music,
        };
        smc.on_client_tick(&mut view);
        assert_eq!(smc.jukeboxes.jukeboxes.get(&[65, 0, 0]), Some(&false));
        assert_eq!(view.music.stopped, 0);
    }

    #[test]
    fn client_tick_marks_near_jukebox_active_and_stops_music() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        // Exactly at the 64-block boundary: 4096 is not > 4096, so it stops.
        smc.jukeboxes.jukeboxes.insert([64, 0, 0], true);
        let mut view = ClientView {
            player_pos: Some([0.5, 0.5, 0.5]),
            now_ms: 0,
            pause_screen_open: false,
            level_present: true,
            music: &mut music,
        };
        smc.on_client_tick(&mut view);
        assert_eq!(smc.jukeboxes.jukeboxes.get(&[64, 0, 0]), Some(&true));
        assert_eq!(view.music.stopped, 1);
    }

    #[test]
    fn client_tick_without_player_leaves_jukeboxes_untouched() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        smc.jukeboxes.jukeboxes.insert([0, 0, 0], true);
        let mut view = ClientView {
            player_pos: None,
            now_ms: 0,
            pause_screen_open: false,
            level_present: true,
            music: &mut music,
        };
        smc.on_client_tick(&mut view);
        assert_eq!(smc.jukeboxes.jukeboxes.get(&[0, 0, 0]), Some(&true));
    }

    #[test]
    fn client_tick_consumes_keybinds_in_upstream_order() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let mut music = TestMusic::new(Some(MusicId::GAME));
        smc.next_music_track.press();
        smc.toggle_music.press();
        let mut view = ClientView {
            player_pos: None,
            now_ms: 0,
            pause_screen_open: false,
            level_present: true,
            music: &mut music,
        };
        smc.on_client_tick(&mut view);
        assert_eq!(view.music.started, vec![MusicId::GAME]);
        assert_eq!(view.music.paused, Some(true));
        assert_eq!(smc.widget.toggle_timer_ms, 6000);
        // second tick consumes nothing further
        smc.on_client_tick(&mut view);
        assert_eq!(view.music.started, vec![MusicId::GAME]);
    }

    #[test]
    fn next_music_track_with_no_jukebox_restarts_situational_music() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(Some(MusicId::MUSIC_BIOME_FOREST));
        smc.on_next_music_track(&mut music);
        assert_eq!(music.stopped, 1);
        assert_eq!(music.started, vec![MusicId::MUSIC_BIOME_FOREST]);
    }

    #[test]
    fn next_music_track_skipped_while_jukebox_playing() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(Some(MusicId::GAME));
        smc.jukeboxes.jukeboxes.insert([0, 0, 0], true);
        smc.on_next_music_track(&mut music);
        assert_eq!(music.stopped, 0);
        assert!(music.started.is_empty());
    }

    #[test]
    fn next_music_track_with_null_situational_music_only_stops() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        smc.on_next_music_track(&mut music);
        assert_eq!(music.stopped, 1);
        assert!(music.started.is_empty());
    }

    #[test]
    fn toggle_music_flips_pause_and_notifies_engine_and_widget() {
        let mut smc = Smc::default();
        smc.config.now_playing_widget_duration = 8;
        let mut music = TestMusic::new(None);
        let mut view = ClientView {
            player_pos: None,
            now_ms: 0,
            pause_screen_open: false,
            level_present: true,
            music: &mut music,
        };
        smc.on_toggle_music(&mut view);
        assert!(smc.global_pause);
        assert_eq!(view.music.paused, Some(true));
        assert_eq!(smc.widget.toggle_timer_ms, 8000);
        assert!(smc.widget.centered);
        smc.on_toggle_music(&mut view);
        assert!(!smc.global_pause);
        assert_eq!(view.music.paused, Some(false));
        assert_eq!(smc.widget.toggle_timer_ms, 8000);
    }

    #[test]
    fn toggle_music_on_pause_screen_arms_no_notification() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        let mut view = ClientView {
            player_pos: None,
            now_ms: 0,
            pause_screen_open: true,
            level_present: true,
            music: &mut music,
        };
        smc.on_toggle_music(&mut view);
        assert!(smc.global_pause);
        assert_eq!(view.music.paused, Some(true));
        assert_eq!(smc.widget.toggle_timer_ms, 0);
    }

    #[test]
    fn toggle_music_without_level_recenters_widget() {
        let mut smc = Smc::default();
        let mut music = TestMusic::new(None);
        let mut view = ClientView {
            player_pos: None,
            now_ms: 0,
            pause_screen_open: false,
            level_present: false,
            music: &mut music,
        };
        smc.on_toggle_music(&mut view);
        assert!(!smc.widget.centered);
    }

    #[test]
    fn sound_engine_set_music_paused_only_touches_music_channels() {
        let mut engine = SoundEngine {
            loaded: true,
            channels: vec![
                Channel {
                    source: SoundSource::Music,
                    paused: false,
                },
                Channel {
                    source: SoundSource::Other,
                    paused: false,
                },
                Channel {
                    source: SoundSource::Music,
                    paused: false,
                },
            ],
        };
        let config = Config::default();
        engine.smc_set_music_paused(true, &config, false);
        assert!(engine.channels[0].paused);
        assert!(!engine.channels[1].paused);
        assert!(engine.channels[2].paused);
        engine.smc_set_music_paused(false, &config, false);
        assert!(!engine.channels[0].paused);
    }

    #[test]
    fn sound_engine_ignores_pause_requests_before_load() {
        let mut engine = SoundEngine {
            loaded: false,
            channels: vec![Channel {
                source: SoundSource::Music,
                paused: false,
            }],
        };
        let config = Config::default();
        engine.smc_set_music_paused(true, &config, false);
        assert!(!engine.channels[0].paused);
    }

    #[test]
    fn sound_engine_unpause_blocked_while_client_paused_without_config() {
        let mut engine = SoundEngine {
            loaded: true,
            channels: vec![Channel {
                source: SoundSource::Music,
                paused: true,
            }],
        };
        let mut config = Config::default();
        config.play_music_when_paused = false;
        engine.smc_set_music_paused(false, &config, true);
        assert!(engine.channels[0].paused, "unpause refused");
        engine.smc_set_music_paused(false, &config, false);
        assert!(
            !engine.channels[0].paused,
            "unpause allowed when not paused"
        );
    }

    #[test]
    fn pause_all_except_drops_music_from_ignored_when_config_says_so() {
        let mut engine = SoundEngine {
            loaded: true,
            channels: vec![
                Channel {
                    source: SoundSource::Music,
                    paused: false,
                },
                Channel {
                    source: SoundSource::Other,
                    paused: false,
                },
            ],
        };
        let mut config = Config::default();
        config.play_music_when_paused = false;
        engine.pause_all_except(&[SoundSource::Music], &config);
        assert!(
            engine.channels[0].paused,
            "music paused despite being ignored"
        );
        assert!(engine.channels[1].paused);
        // With playMusicWhenPaused true, MUSIC stays in the ignored list.
        let mut engine = SoundEngine {
            loaded: true,
            channels: vec![Channel {
                source: SoundSource::Music,
                paused: false,
            }],
        };
        config.play_music_when_paused = true;
        engine.pause_all_except(&[SoundSource::Music], &config);
        assert!(
            !engine.channels[0].paused,
            "music left playing when ignored"
        );
    }

    #[test]
    fn situational_picker_disabled_returns_none() {
        let smc = Smc::default();
        let mut rng = ScriptedRng::new(vec![0.0]);
        let player = PlayerCtx {
            dimension: Dimension::Overworld,
            creative: false,
        };
        assert_eq!(
            smc.pick_random_situational_music(&player, &mut rng),
            None,
            "unlockSituationalMusic defaults to false"
        );
    }

    #[test]
    fn situational_overworld_picks_from_overworld_list() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let mut rng = ScriptedRng::new(vec![0.0, 0.5]); // situational, mid-list pick
        let player = PlayerCtx {
            dimension: Dimension::Overworld,
            creative: false,
        };
        let picked = smc
            .pick_random_situational_music(&player, &mut rng)
            .unwrap();
        assert!(overworld_situational_music().contains(&picked));
    }

    #[test]
    fn situational_overworld_non_situational_creative_returns_creative_music() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let mut rng = ScriptedRng::new(vec![0.99]); // >= chance
        let player = PlayerCtx {
            dimension: Dimension::Overworld,
            creative: true,
        };
        assert_eq!(
            smc.pick_random_situational_music(&player, &mut rng),
            Some(MusicId::CREATIVE)
        );
    }

    #[test]
    fn situational_overworld_survival_follows_creative_music_in_survival() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let mut rng = ScriptedRng::new(vec![0.99]);
        let player = PlayerCtx {
            dimension: Dimension::Overworld,
            creative: false,
        };
        assert_eq!(
            smc.pick_random_situational_music(&player, &mut rng),
            Some(MusicId::CREATIVE),
            "creativeMusicInSurvival default true"
        );
        smc.config.creative_music_in_survival = false;
        let mut rng = ScriptedRng::new(vec![0.99]);
        assert_eq!(
            smc.pick_random_situational_music(&player, &mut rng),
            Some(MusicId::GAME)
        );
    }

    #[test]
    fn situational_nether_paths() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let player = PlayerCtx {
            dimension: Dimension::Nether,
            creative: false,
        };
        let mut rng = ScriptedRng::new(vec![0.0, 0.9]);
        let picked = smc
            .pick_random_situational_music(&player, &mut rng)
            .unwrap();
        assert!(nether_situational_music().contains(&picked));
        let mut rng = ScriptedRng::new(vec![0.99]); // not situational
        assert_eq!(smc.pick_random_situational_music(&player, &mut rng), None);
    }

    #[test]
    fn situational_other_dimension_never_picks() {
        let mut smc = Smc::default();
        smc.config.unlock_situational_music = true;
        let player = PlayerCtx {
            dimension: Dimension::Other,
            creative: true,
        };
        let mut rng = ScriptedRng::new(vec![0.0]);
        assert_eq!(smc.pick_random_situational_music(&player, &mut rng), None);
    }

    #[test]
    fn note_color_tick_gated_at_25ms() {
        let mut notes = MusicNoteColorManager::default();
        assert_eq!(notes.music_note_color, -1);
        notes.tick_music_notes(true, 0);
        assert_eq!(
            notes.music_note_color_tick(),
            0,
            "first tick at t=0 no-ops (0 > 0+25 false)"
        );
        notes.tick_music_notes(true, 24);
        assert_eq!(notes.music_note_color_tick(), 0, "inside the 25ms window");
        notes.tick_music_notes(true, 26);
        assert_eq!(
            notes.music_note_color_tick(),
            1,
            "past the window advances once"
        );
        assert_eq!(notes.music_note_color, get_lerped_color(1.0));
        notes.tick_music_notes(true, 30);
        assert_eq!(
            notes.music_note_color_tick(),
            1,
            "still inside the next window"
        );
        notes.tick_music_notes(true, 60);
        assert_eq!(notes.music_note_color_tick(), 2);
    }

    #[test]
    fn note_color_tick_requires_current_music() {
        let mut notes = MusicNoteColorManager::default();
        notes.tick_music_notes(false, 1000);
        assert_eq!(notes.music_note_color_tick(), 0);
    }

    #[test]
    fn widget_toggle_notification_arms_centered_state() {
        let mut w = WidgetState::default();
        assert!(w.centered, "upstream initializes centered = true");
        w.display_toggle_notification(false, false, 6);
        assert!(!w.centered);
        assert_eq!(w.toggle_timer_ms, 6000);
    }

    #[test]
    fn id_matches_mod_namespace() {
        assert_eq!(id("keys"), "simple_music_control:keys");
    }
}
