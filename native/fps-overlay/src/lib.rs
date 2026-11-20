//! Horizon's official FPS Counter: the frames of the last second in a small box in a corner of the
//! HUD, and if you like the slowest frame and the 1% low of the last 5 seconds, drawn as the extra
//! and the 1.x and 2.x mods drew it. Its settings keep the files of earlier versions
//! (`horizon-mods/.horizon-mod-data/fps_overlay/corner`, `lows`).
use std::sync::Mutex;

use mod_api::{
    ClientHost, Corner, Hooks, Registrar, RegistrationContext, RenderFramePreEvent,
    RenderGuiPostEvent, SettingSpec,
};

/// The mod's id: its settings folder.
pub const ID: &str = "fps_overlay";
/// The extra's box: translucent black (`0x78000000`, the extras' box).
pub const BACKGROUND: u32 = 0x7800_0000;
/// Opaque white text, with vanilla's shadow.
pub const INK: u32 = 0xFFFF_FFFF;
/// The box's height and the text's inset, in GUI pixels.
pub const HEIGHT: f32 = 12.0;
pub const PADDING: f32 = 2.0;
/// The corners in the order of the setting's options, with the names earlier versions stored.
pub const CORNERS: [(Corner, &str); 4] = [
    (Corner::TopLeft, "top_left"),
    (Corner::TopRight, "top_right"),
    (Corner::BottomLeft, "bottom_left"),
    (Corner::BottomRight, "bottom_right"),
];
/// Seconds the minimum and 1% low look back.
const LOW_WINDOW: f64 = 5.0;
/// Seconds between two readings.
const REFRESH: f64 = 0.5;

/// The settings the player picks on FPS Counter's page in Options > Mods.
pub fn settings() -> [SettingSpec; 2] {
    [
        SettingSpec::choice(
            "corner",
            "fps_overlay.setting.corner",
            CORNERS.map(|(_, name)| format!("fps_overlay.corner.{name}")),
            0,
        )
        .tooltip("fps_overlay.setting.corner.tooltip"),
        SettingSpec::toggle("lows", "fps_overlay.setting.lows", false)
            .tooltip("fps_overlay.setting.lows.tooltip"),
    ]
}

/// The corner kept in `corner`: the option's index, or the name versions before 3.0.0 wrote
/// (`bottom_right`). None for anything else.
pub fn stored_corner(text: &str) -> Option<usize> {
    let text = text.trim();
    CORNERS
        .iter()
        .position(|(_, name)| *name == text)
        .or_else(|| text.parse().ok().filter(|i| *i < CORNERS.len()))
}

/// What the box says: frames in the last second, then the slowest frame and the 1% low.
pub fn text(reading: Reading, lows: bool) -> String {
    let Reading { fps, min, low } = reading;
    if lows {
        format!("{fps} fps  min {min}  1% {low}")
    } else {
        format!("{fps} fps")
    }
}

/// The box around text `width` GUI pixels wide: `[x, y, w, h]` once placed.
pub fn size(width: f32) -> [f32; 2] {
    [width + 2.0 * PADDING, HEIGHT]
}

/// A reading of the frame counter, in frames per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    /// Frames that ended in the last second.
    pub fps: u32,
    /// The slowest frame of the last 5 seconds.
    pub min: u32,
    /// The 99th percentile frame time of the last 5 seconds.
    pub low: u32,
}

/// The game's frame counter (`extras::FpsCounter`), fed the same frame times: the reading is
/// recomputed every half second, as the extra's was.
#[derive(Debug)]
pub struct Counter {
    /// The frames' times added up: the clock the window runs on.
    now: f64,
    /// (end time in seconds, frame milliseconds), oldest first.
    frames: std::collections::VecDeque<(f64, f32)>,
    shown: Reading,
    computed_at: f64,
}

impl Default for Counter {
    fn default() -> Self {
        Self {
            now: 0.0,
            frames: Default::default(),
            shown: Reading::default(),
            computed_at: f64::NEG_INFINITY,
        }
    }
}

impl Counter {
    /// A frame that took `seconds`.
    pub fn frame(&mut self, seconds: f64) {
        self.now += seconds;
        let now = self.now;
        self.frames.push_back((now, (seconds * 1000.0) as f32));
        while self
            .frames
            .front()
            .is_some_and(|&(t, _)| now - t > LOW_WINDOW)
        {
            self.frames.pop_front();
        }
        if now - self.computed_at >= REFRESH {
            self.computed_at = now;
            self.shown = self.compute();
        }
    }

    fn compute(&self) -> Reading {
        let fps = self
            .frames
            .iter()
            .filter(|&&(t, _)| self.now - t < 1.0)
            .count() as u32;
        let mut times: Vec<f32> = self.frames.iter().map(|f| f.1).collect();
        if times.is_empty() {
            return Reading::default();
        }
        times.sort_by(|a, b| b.total_cmp(a));
        let to_fps = |ms: f32| (1000.0 / ms.max(0.001)).round() as u32;
        Reading {
            fps,
            min: to_fps(times[0]),
            low: to_fps(times[(times.len() - 1) / 100]),
        }
    }

    pub fn reading(&self) -> Reading {
        self.shown
    }
}

#[derive(Default)]
struct State {
    counter: Counter,
    /// Whether this run looked at the stored corner once (to rewrite an earlier version's name).
    corner_read: bool,
}

#[derive(Default)]
struct Fps {
    // Never held across a host call: a host call can fire another of this mod's hooks.
    state: Mutex<State>,
}

impl Fps {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The corner picked. The first time in a run, a name an earlier version stored is rewritten
    /// as the option's index, so the settings page shows it too.
    fn corner(&self, host: &dyn ClientHost) -> usize {
        let first = !std::mem::replace(&mut self.state().corner_read, true);
        if first
            && let Some(text) = host.setting(ID, "corner")
            && let Some(i) = stored_corner(&text)
        {
            if text.trim() != i.to_string() {
                host.set_setting(ID, "corner", &i.to_string());
            }
            return i;
        }
        host.setting_choice("corner")
            .map_or(0, |i| i as usize)
            .min(CORNERS.len() - 1)
    }
}

#[mod_api::horizon_mod]
impl Hooks for Fps {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for setting in settings() {
            // Only a malformed spec is refused, and the tests check every spec.
            let _ = r.register_setting(setting);
        }
    }

    /// Every frame, in a world or not: the frame time the game's own counter gets.
    fn render_frame_pre(&self, _: &mut dyn ClientHost, event: &mut RenderFramePreEvent) {
        self.state().counter.frame(event.elapsed.as_secs_f64());
    }

    /// The box in its corner, below (or above) what the corner holds already.
    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) {
            return;
        }
        let corner = CORNERS[self.corner(host)].0;
        let lows = host.setting_bool("lows").unwrap_or(false);
        let reading = self.state().counter.reading();
        let text = text(reading, lows);
        let draw = &mut *event.draw;
        let [w, h] = size(draw.text_width(&text));
        let (x, y) = event.corners.place(corner, w, h);
        draw.fill([x, y, w, h], BACKGROUND);
        draw.text(&text, [x + PADDING, y + PADDING], INK, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_one_the_game_takes() {
        for setting in settings() {
            assert_eq!(setting.check(), Ok(()), "{}", setting.key);
        }
    }

    #[test]
    fn reads_the_files_earlier_versions_wrote() {
        // 1.x and 2.x wrote the corner's name and `true`/`false`.
        for (text, i) in [
            ("top_left", 0),
            ("top_right", 1),
            ("bottom_left\n", 2),
            ("bottom_right", 3),
            ("3", 3),
        ] {
            assert_eq!(stored_corner(text), Some(i), "{text}");
        }
        assert_eq!(stored_corner("middle"), None);
        assert_eq!(stored_corner("4"), None);
        let [corner, lows] = settings();
        use mod_api::SettingValue::{Choice, Toggle};
        assert_eq!(corner.read(None), Choice(0), "top left, as the extra");
        assert_eq!(corner.store(Choice(3)).as_deref(), Some("3"));
        assert_eq!(lows.read(None), Toggle(false), "no lows, as the extra");
        assert_eq!(lows.read(Some("true")), Toggle(true));
        assert_eq!(lows.store(Toggle(false)).as_deref(), Some("false"));
    }

    #[test]
    fn says_what_the_extra_said() {
        let r = Reading {
            fps: 60,
            min: 40,
            low: 30,
        };
        assert_eq!(text(r, false), "60 fps");
        assert_eq!(text(r, true), "60 fps  min 40  1% 30");
        assert_eq!(size(30.0), [34.0, 12.0]);
    }

    #[test]
    fn counts_like_the_games_counter() {
        let mut c = Counter::default();
        assert_eq!(c.reading(), Reading::default());
        // Two and a half seconds at 60 fps, with one slow frame of 50 ms.
        for i in 0..150 {
            c.frame(if i == 100 { 0.05 } else { 1.0 / 60.0 });
        }
        let r = c.reading();
        assert!((59..=61).contains(&r.fps), "{r:?}");
        assert_eq!(r.min, 20);
        assert_eq!(r.low, 60, "one slow frame in 120 is under the 1%");
        // Five seconds later the slow frame has left the window.
        for _ in 0..330 {
            c.frame(1.0 / 60.0);
        }
        assert_eq!(c.reading().min, 60);
    }
}
