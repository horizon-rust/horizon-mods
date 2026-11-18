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

impl State {
    /// The key is `down` or up now. A hold starts only while `playing` (no screen open), at the
    /// starting zoom `read` gives; letting go ends it.
    pub fn key(&mut self, down: bool, playing: bool, read: impl FnOnce() -> Settings) {
        if !down {
            self.held = None;
        } else if self.held.is_none() && playing {
            let settings = read();
            self.held = Some((settings.level.clamp(MIN, MAX), settings));
        }
    }

    /// Whole wheel steps (up is positive): true when the key is held and they zoom.
    pub fn wheel(&mut self, steps: i32) -> bool {
        let Some((zoom, _)) = &mut self.held else {
            return false;
        };
        *zoom = (*zoom * STEP.powi(steps)).clamp(MIN, MAX);
        true
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

    /// Reads the key, and the settings when a hold starts.
    fn follow_key(&self, host: &dyn ClientHost) {
        let down = host.is_down(ID, KEY);
        let held = self.state().held.is_some();
        if down && !held {
            if !(host.playing() && host.on(ID)) {
                return;
            }
            let settings = Settings {
                level: host.setting_f64("level").unwrap_or(LEVEL_DEFAULT) as f32,
                smooth: host.setting_bool("smooth").unwrap_or(true),
                sensitivity: host.setting_bool("sensitivity").unwrap_or(true),
            };
            self.state().key(true, true, || settings);
        } else if !down && held {
            self.state().key(false, false, Settings::default);
        }
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
    fn mouse_scrolling(&self, _: &mut dyn ClientHost, event: &mut MouseScrollingEvent) -> Flow {
        if self.state().wheel(event.accumulated[1]) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(level: f32) -> impl FnOnce() -> Settings {
        move || Settings {
            level,
            ..Settings::default()
        }
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
        s.key(true, false, at(4.0));
        assert_eq!(s.view(), None, "not while a screen is open");
        assert!(!s.wheel(1));
        s.key(true, true, at(4.0));
        assert_eq!(
            s.view(),
            Some(mod_api::Zoom {
                factor: 4.0,
                smooth: true,
                turn: 0.25
            })
        );
        assert!(s.wheel(1));
        assert_eq!(s.view().map(|z| z.factor), Some(5.0));
        assert!(s.wheel(0), "a part step is still the zoom's");
        assert!(s.wheel(40));
        assert_eq!(s.view().map(|z| z.factor), Some(10.0));
        assert!(s.wheel(-40));
        assert_eq!(s.view().map(|z| z.factor), Some(1.25));
        // Held on: the hold keeps its zoom, not the starting one.
        s.key(true, true, at(8.0));
        assert_eq!(s.view().map(|z| z.factor), Some(1.25));
        s.key(false, false, at(4.0));
        assert_eq!(s.view(), None);
        assert!(!s.wheel(1));
    }

    #[test]
    fn slower_turning_off_turns_at_full_speed() {
        let mut s = State::default();
        s.key(true, true, || Settings {
            level: 8.0,
            smooth: false,
            sensitivity: false,
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
