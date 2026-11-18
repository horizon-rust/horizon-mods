//! Horizon's official Zoom: hold the key (Z by default) to zoom in, and turn the mouse wheel while
//! holding it to zoom further in or out. Its settings keep the files of earlier versions
//! (`horizon-mods/.horizon-mod-data/zoom/level`, `smooth`, `sensitivity`) and its key keeps the line
//! `key.zoom.zoom:…` in `horizon-mods.txt`.
use std::sync::Mutex;

use mod_api::{
    ClientHost, ClientTickPostEvent, Flow, Hooks, KeyConflictContext, KeyRegistration,
    MouseScrollingEvent, RegistrationContext, Registrar, SettingSpec, ZoomEvent,
};

/// The mod's id: its settings folder and its key's line.
pub const ID: &str = "zoom";
/// The key's id within the mod.
pub const KEY: &str = "zoom";
pub const DEFAULT_KEY: &str = "key.keyboard.z";
/// The starting zoom's range, and its default; earlier versions stored one of 2, 3, 4, 6, 8, 10.
pub const LEVEL_MIN: f64 = 2.0;
pub const LEVEL_MAX: f64 = 10.0;
pub const LEVEL_DEFAULT: f64 = 4.0;
/// The wheel's limits.
const MIN: f32 = 1.25;
const MAX: f32 = 10.0;
/// Each wheel step multiplies the zoom by this.
const STEP: f32 = 1.25;

/// The settings the player picks on Zoom's page in Options > Mods.
pub fn settings() -> [SettingSpec; 3] {
    [
        SettingSpec::range(
            "level",
            "zoom.setting.level",
            LEVEL_MIN,
            LEVEL_MAX,
            1.0,
            LEVEL_DEFAULT,
        )
        .tooltip("zoom.setting.level.tooltip"),
        SettingSpec::toggle("smooth", "zoom.setting.smooth", true)
            .tooltip("zoom.setting.smooth.tooltip"),
        SettingSpec::toggle("sensitivity", "zoom.setting.sensitivity", true)
            .tooltip("zoom.setting.sensitivity.tooltip"),
    ]
}

/// The settings in effect for one hold of the key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub level: f32,
    pub smooth: bool,
    pub sensitivity: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            level: LEVEL_DEFAULT as f32,
            smooth: true,
            sensitivity: true,
        }
    }
}

/// The key's state: the zoom while it is held, none when it is up.
#[derive(Debug, Default)]
pub struct State {
    held: Option<(f32, Settings)>,
}

/// The key as the game reads it now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub down: bool,
    /// The settings a hold starts with, when one may start now (playing, no screen open, the mod
    /// on); none otherwise.
    pub start: Option<Settings>,
}

impl State {
    /// Whether a press would start a hold now: the key is up in this state.
    pub fn idle(&self) -> bool {
        self.held.is_none()
    }

    /// Follows the key: a hold starts at `key.start`'s starting zoom; letting go ends it.
    pub fn key(&mut self, key: Key) {
        if !key.down {
            self.held = None;
        } else if self.held.is_none()
            && let Some(settings) = key.start
        {
            self.held = Some((settings.level.clamp(MIN, MAX), settings));
        }
    }

    /// Scaled wheel steps (up is positive, fractions from a touchpad included): true when the
    /// key is held and they zoom.
    pub fn wheel(&mut self, steps: f32) -> bool {
        let Some((zoom, _)) = &mut self.held else {
            return false;
        };
        *zoom = (*zoom * STEP.powf(steps)).clamp(MIN, MAX);
        true
    }

    /// A wheel event: the key as it is at this event first, so a press or release since the last
    /// tick counts, then the wheel. True when it zooms instead of moving the hotbar.
    pub fn scroll(&mut self, key: Key, steps: f32) -> bool {
        self.key(key);
        self.wheel(steps)
    }

    /// The zoom in effect, none when the key is up.
    pub fn view(&self) -> Option<mod_api::Zoom> {
        let (factor, settings) = self.held?;
        Some(mod_api::Zoom {
            factor,
            smooth: settings.smooth,
            turn: if settings.sensitivity { 1.0 / factor } else { 1.0 },
        })
    }
}

#[derive(Default)]
struct Zoom {
    // Never held across a host call: a host call can fire another of this mod's hooks.
    state: Mutex<State>,
}

impl Zoom {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reads the key, and the settings only when a hold may start.
    fn read_key(&self, host: &dyn ClientHost) -> Key {
        let down = host.is_down(ID, KEY);
        // Its own statement: the lock is let go before the host calls below.
        let idle = self.state().idle();
        let start = (down && idle && host.playing() && host.on(ID)).then(|| Settings {
            level: host.setting_f64("level").unwrap_or(LEVEL_DEFAULT) as f32,
            smooth: host.setting_bool("smooth").unwrap_or(true),
            sensitivity: host.setting_bool("sensitivity").unwrap_or(true),
        });
        Key { down, start }
    }

    fn follow_key(&self, host: &dyn ClientHost) {
        let key = self.read_key(host);
        self.state().key(key);
    }
}

#[mod_api::horizon_mod]
impl Hooks for Zoom {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new(KEY, DEFAULT_KEY)
                .label("key.zoom.zoom")
                .context(KeyConflictContext::InGame),
        );
        for setting in settings() {
            // Only a malformed spec is refused, and the tests check every spec.
            let _ = r.register_setting(setting);
        }
    }

    // The zoom hook is asked only while playing; the tick also sees a release behind a screen.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        self.follow_key(host);
    }

    fn zoom(&self, host: &dyn ClientHost, _: &mut ZoomEvent) -> Option<mod_api::Zoom> {
        self.follow_key(host);
        self.state().view()
    }

    /// While the key is held, the wheel zooms instead of moving the hotbar.
    fn mouse_scrolling(&self, host: &mut dyn ClientHost, event: &mut MouseScrollingEvent) -> Flow {
        let key = self.read_key(host);
        if self.state().scroll(key, event.scroll[1] as f32) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key down while playing, a hold starting at `level`.
    fn press(level: f32) -> Key {
        Key {
            down: true,
            start: Some(Settings {
                level,
                ..Settings::default()
            }),
        }
    }
    /// The key down where no hold may start (a screen open).
    const BEHIND_SCREEN: Key = Key {
        down: true,
        start: None,
    };
    const UP: Key = Key {
        down: false,
        start: None,
    };

    fn factor(s: &State) -> Option<f32> {
        s.view().map(|z| z.factor)
    }

    #[test]
    fn every_setting_is_one_the_game_takes() {
        for setting in settings() {
            assert_eq!(setting.check(), Ok(()), "{}", setting.key);
        }
    }

    #[test]
    fn reads_the_files_earlier_versions_wrote() {
        let [level, smooth, sensitivity] = settings();
        use mod_api::SettingValue::{Range, Toggle};
        for (text, zoom) in [("2", 2.0), ("8\n", 8.0), ("10", 10.0)] {
            assert_eq!(level.read(Some(text)), Range(zoom));
            assert_eq!(level.store(Range(zoom)).as_deref(), Some(text.trim()));
        }
        assert_eq!(level.read(None), Range(4.0));
        assert_eq!(level.read(Some("99")), Range(4.0), "outside 2-10: the default");
        assert_eq!(smooth.read(Some("false")), Toggle(false));
        assert_eq!(smooth.read(None), Toggle(true));
        assert_eq!(sensitivity.read(Some("true")), Toggle(true));
        assert_eq!(sensitivity.store(Toggle(false)).as_deref(), Some("false"));
    }

    #[test]
    fn holds_zooms_with_the_wheel_and_lets_go() {
        let mut s = State::default();
        s.key(BEHIND_SCREEN);
        assert_eq!(s.view(), None, "not while a screen is open");
        assert!(!s.wheel(1.0));
        s.key(press(4.0));
        assert_eq!(
            s.view(),
            Some(mod_api::Zoom {
                factor: 4.0,
                smooth: true,
                turn: 0.25
            })
        );
        assert!(s.wheel(1.0));
        assert_eq!(factor(&s), Some(5.0));
        assert!(s.wheel(40.0));
        assert_eq!(factor(&s), Some(10.0));
        assert!(s.wheel(-40.0));
        assert_eq!(factor(&s), Some(1.25));
        // Held on: the hold keeps its zoom, not the starting one.
        s.key(press(8.0));
        assert_eq!(factor(&s), Some(1.25));
        s.key(UP);
        assert_eq!(s.view(), None);
        assert!(!s.wheel(1.0));
    }

    #[test]
    fn a_touchpad_fraction_zooms_by_its_share_of_a_step() {
        let mut s = State::default();
        s.key(press(4.0));
        assert!(s.wheel(0.5));
        let half = factor(&s).unwrap();
        assert!((half - 4.0 * STEP.sqrt()).abs() < 1e-5, "{half}");
        assert!(s.wheel(0.5));
        assert!((factor(&s).unwrap() - 5.0).abs() < 1e-5, "two halves make one step");
        assert!(s.wheel(-0.25));
        assert!(factor(&s).unwrap() < 5.0);
    }

    #[test]
    fn a_wheel_event_reads_the_key_as_it_is_then() {
        // Pressed since the last tick: the first wheel event already zooms, the hotbar stays.
        let mut s = State::default();
        assert!(s.scroll(press(4.0), 1.0));
        assert_eq!(factor(&s), Some(5.0));
        // Let go since the last tick: the wheel goes back to the hotbar at once.
        assert!(!s.scroll(UP, 1.0));
        assert_eq!(s.view(), None);
        // Down behind a screen starts nothing, so the event isn't taken.
        assert!(!s.scroll(BEHIND_SCREEN, 1.0));
    }

    #[test]
    fn slower_turning_off_turns_at_full_speed() {
        let mut s = State::default();
        s.key(Key {
            down: true,
            start: Some(Settings {
                level: 8.0,
                smooth: false,
                sensitivity: false,
            }),
        });
        assert_eq!(
            s.view(),
            Some(mod_api::Zoom {
                factor: 8.0,
                smooth: false,
                turn: 1.0
            })
        );
    }
}
