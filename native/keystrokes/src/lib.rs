//! Horizon's official Keystrokes: the movement keys, the mouse buttons and the space bar in a
//! corner of the HUD, lit while held, with clicks per second under the mouse buttons, drawn as the
//! extra and the 1.x and 2.x mods drew them, and giving way to F3's debug screen as 2.0.0 did. Its
//! settings keep the files of earlier versions (`horizon-mods/.horizon-mod-data/keystrokes/corner`,
//! `cps`); the extra's own (`keystrokesCorner`, `keystrokesCps` in `horizon-extras.txt`) seed them
//! once.
use std::collections::VecDeque;
use std::sync::Mutex;

use mod_api::text::TextContent;
use mod_api::{
    ClientHost, ClientResourcesReloadedEvent, ConfigPath, Corner, Flow, Hooks, MouseButtonPreEvent,
    Registrar, RegistrationContext, RenderFramePreEvent, RenderGuiPostEvent, SettingSpec, Text,
};

/// The mod's id: its settings folder.
pub const ID: &str = "keystrokes";
/// The extra's colours: a key up is translucent black with white text, a key held is light with
/// dark text (text in `argb`, opaque).
pub const BACKGROUND: u32 = 0x7800_0000;
pub const HELD: u32 = 0xB0FF_FFFF;
pub const INK_HELD: u32 = 0xFF14_1414;
pub const INK: u32 = 0xFFFF_FFFF;
/// A key's side and the gap between keys, in GUI pixels.
const KEY: f32 = 22.0;
const GAP: f32 = 2.0;
/// The box's size.
pub const SIZE: (f32, f32) = (3.0 * KEY + 2.0 * GAP, 3.0 * KEY + 3.0 * GAP + 10.0);
/// The mappings of the top two rows, in the order they are laid out.
pub const MOVES: [&str; 4] = ["key.forward", "key.left", "key.back", "key.right"];
/// The mouse buttons' mappings: the left box's, then the right's.
pub const BUTTONS: [&str; 2] = ["key.attack", "key.use"];
/// The corners in the order of the setting's options, with the names earlier versions stored.
pub const CORNERS: [(Corner, &str); 4] = [
    (Corner::TopLeft, "top_left"),
    (Corner::TopRight, "top_right"),
    (Corner::BottomLeft, "bottom_left"),
    (Corner::BottomRight, "bottom_right"),
];
/// The file in the mod's data folder (`horizon-mod-data/keystrokes/`) that says the extra's old
/// settings were read: they are read once, never again.
pub const EXTRAS_READ: &str = "extras-read";
/// GLFW's left and right mouse buttons, as `mouse_button_pre` names them.
const GLFW_LEFT: i32 = 0;
const GLFW_RIGHT: i32 = 1;

/// The settings the player picks on Keystrokes' page in Options > Mods.
pub fn settings() -> [SettingSpec; 2] {
    [
        SettingSpec::choice(
            "corner",
            "keystrokes.setting.corner",
            CORNERS.map(|(_, name)| format!("keystrokes.corner.{name}")),
            0,
        )
        .tooltip("keystrokes.setting.corner.tooltip"),
        SettingSpec::toggle("cps", "keystrokes.setting.cps", true)
            .tooltip("keystrokes.setting.cps.tooltip"),
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

/// The mod's settings the extra's old ones (`official_extra_settings`) give, as the settings
/// store them: `keystrokesCorner` (`top_left`) as `corner`'s index, `keystrokesCps` as `cps`.
/// `keystrokes`, the extra's switch, is left out: the mod's switch in Options > Mods is the
/// player's.
pub fn from_extra(old: &[(String, String)]) -> Vec<(&'static str, String)> {
    old.iter()
        .filter_map(|(key, value)| match key.as_str() {
            "keystrokesCorner" => CORNERS
                .iter()
                .position(|(_, name)| *name == value.trim())
                .map(|i| ("corner", i.to_string())),
            "keystrokesCps" => match value.trim() {
                v @ ("true" | "false") => Some(("cps", v.to_owned())),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// What the one read of the extra's old settings needs from the game; the tests stand in for it.
pub trait ExtraSource {
    /// Whether the data folder says the extras were never read (`extras-read` isn't there). False
    /// when it is there, or when the folder can't be asked.
    fn unread(&self) -> bool;
    /// The extra's old settings (`official_extra_settings`).
    fn extra(&self) -> Vec<(String, String)>;
    /// Whether the mod has a file for the setting already.
    fn has_setting(&self, key: &str) -> bool;
    /// Keeps the setting; false when it didn't stick (it reads back as something else).
    fn set_setting(&self, key: &str, value: &str) -> bool;
    /// Writes `extras-read`; false when it couldn't.
    fn mark_read(&self) -> bool;
}

/// The extra's old corner and clicks per second become the mod's own, for each the mod has no
/// file for yet, so a choice made in an earlier version (or on the settings page) stays. The
/// marker is written after the settings, and only when the game gave the extra's settings and
/// every one of them was kept: with none (a player who never changed the extras, a file that
/// couldn't be read) or a write that didn't stick, the next run asks again (and keeps what did
/// stick). True when the marker was written.
pub fn seed_from_extra(source: &dyn ExtraSource) -> bool {
    if !source.unread() {
        return false;
    }
    let old = source.extra();
    if old.is_empty() {
        return false;
    }
    let mut kept = true;
    for (key, value) in from_extra(&old) {
        if !source.has_setting(key) {
            kept &= source.set_setting(key, &value);
        }
    }
    kept && source.mark_read()
}

/// A key's name as Controls shows it, in plain text: literal text as it is, a translation in the
/// game's language (`key.keyboard.w` is `W`), its children after it.
pub fn plain(text: &Text, translate: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    plain_into(text, translate, &mut out, 0);
    out
}

fn plain_into(
    text: &Text,
    translate: &dyn Fn(&str) -> Option<String>,
    out: &mut String,
    depth: u32,
) {
    // A key name is one or two nodes; anything deeper is not worth walking.
    if depth > 8 {
        return;
    }
    match &text.content {
        TextContent::Literal(s) => out.push_str(s),
        TextContent::Translatable {
            key,
            fallback,
            arguments,
        } => {
            let template = translate(key)
                .or_else(|| fallback.clone())
                .unwrap_or_else(|| key.clone());
            let mut args = arguments.iter();
            let mut parts = template.split("%s");
            if let Some(first) = parts.next() {
                out.push_str(first);
            }
            for part in parts {
                if let Some(arg) = args.next() {
                    plain_into(arg, translate, out, depth + 1);
                }
                out.push_str(part);
            }
        }
        TextContent::Keybind(name) => {
            out.push_str(&translate(name).unwrap_or_else(|| name.clone()));
        }
        _ => {}
    }
    for child in &text.children {
        plain_into(child, translate, out, depth + 1);
    }
}

/// What the box shows: each movement key (held, label), the space bar, the mouse buttons, and the
/// clicks per second when shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub moves: [(bool, String); 4],
    pub jump: bool,
    pub attack: bool,
    pub use_: bool,
    pub cps: Option<(u32, u32)>,
}

/// One thing to draw, relative to the box's top left corner.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// x, y, w, h and argb.
    Fill([f32; 4], u32),
    /// The text, its x and y, argb, and a shadow.
    Label(String, [f32; 2], u32, bool),
}

/// A key's box with its lines centred; `width` measures text.
fn key_box(
    out: &mut Vec<Shape>,
    [x, y, w, h]: [f32; 4],
    held: bool,
    lines: &[&str],
    width: &dyn Fn(&str) -> f32,
) {
    let (fill, ink) = if held {
        (HELD, INK_HELD)
    } else {
        (BACKGROUND, INK)
    };
    out.push(Shape::Fill([x, y, w, h], fill));
    let total = lines.len() as f32 * 9.0 - 1.0;
    for (i, line) in lines.iter().enumerate() {
        let at = [
            (x + (w - width(line)) / 2.0).round(),
            (y + (h - total) / 2.0).round() + i as f32 * 9.0,
        ];
        out.push(Shape::Label((*line).to_owned(), at, ink, !held));
    }
}

/// The box, as the extra laid it out: W, then A S D, then the mouse buttons with their clicks per
/// second, then the space bar.
pub fn shapes(v: &View, width: &dyn Fn(&str) -> f32) -> Vec<Shape> {
    let (w, _) = SIZE;
    let step = KEY + GAP;
    let mut out = Vec::with_capacity(24);
    let [forward, left, back, right] = &v.moves;
    key_box(
        &mut out,
        [step, 0.0, KEY, KEY],
        forward.0,
        &[&forward.1],
        width,
    );
    for (i, key) in [left, back, right].into_iter().enumerate() {
        key_box(
            &mut out,
            [i as f32 * step, step, KEY, KEY],
            key.0,
            &[&key.1],
            width,
        );
    }
    let mw = (w - GAP) / 2.0;
    let my = 2.0 * step;
    let buttons = [
        (v.attack, "LMB", v.cps.map(|c| c.0)),
        (v.use_, "RMB", v.cps.map(|c| c.1)),
    ];
    for (i, (held, label, cps)) in buttons.into_iter().enumerate() {
        let rate = cps.map(|n| format!("{n} CPS"));
        let lines: Vec<&str> = std::iter::once(label).chain(rate.as_deref()).collect();
        key_box(
            &mut out,
            [i as f32 * (mw + GAP), my, mw, KEY],
            held,
            &lines,
            width,
        );
    }
    let sy = my + step;
    key_box(&mut out, [0.0, sy, w, 10.0], v.jump, &[], width);
    let bar = if v.jump { INK_HELD } else { INK };
    out.push(Shape::Fill([w / 2.0 - 12.0, sy + 4.0, 24.0, 2.0], bar));
    out
}

/// The clicks of the two mouse buttons in the last second, as the game counted them for the extra
/// (`extras::Cps`): each press of the left or right button while playing. The window's own presses
/// come from `mouse_button_pre`, counted on the button pressed (attack on the right button counts
/// on the right). A press that reaches the attack or use mapping another way (a key, a
/// controller's trigger, `horizon-ctl mouse left down`) shows as the mapping going down in a frame
/// with no window press to stand for it, as the game's counter took those too. A mapping that goes
/// down in a frame the window pressed a button is that press, whichever button it was, but each
/// window press stands for one mapping only: a left click and a key bound to use in the same frame
/// are two clicks.
#[derive(Debug, Default)]
pub struct Clicks {
    /// Press times in seconds, oldest first, per button.
    times: [VecDeque<f64>; 2],
    /// Which of the window's buttons were pressed since the last frame.
    pressed: [bool; 2],
    /// Whether the mapping was held at the last frame.
    was_down: [bool; 2],
}

impl Clicks {
    /// More than any hand clicks in a second: the queue never grows past it.
    const KEEP: usize = 64;

    fn add(&mut self, i: usize, now: f64) {
        let times = &mut self.times[i];
        if times.len() == Self::KEEP {
            times.pop_front();
        }
        times.push_back(now);
    }

    /// A press of the window's left (0) or right (1) button.
    pub fn press(&mut self, i: usize, now: f64) {
        self.add(i, now);
        self.pressed[i] = true;
    }

    /// Once a frame: whether the attack and use mappings are held. A mapping that went down is
    /// the window's press of its own button if there was one, else of the other button if that
    /// press isn't the other mapping's (a swapped binding); with no press left for it, it is a
    /// click of its own.
    pub fn frame(&mut self, down: [bool; 2], now: f64) {
        let mut pressed = std::mem::take(&mut self.pressed);
        let went_down: [bool; 2] = std::array::from_fn(|i| down[i] && !self.was_down[i]);
        self.was_down = down;
        // First each mapping takes its own button's press, then what is left over.
        let unmatched: [bool; 2] =
            std::array::from_fn(|i| went_down[i] && !std::mem::take(&mut pressed[i]));
        for (i, _) in unmatched.iter().enumerate().filter(|(_, u)| **u) {
            if !std::mem::take(&mut pressed[1 - i]) {
                self.add(i, now);
            }
        }
    }

    /// The presses of each button in the second up to `now`.
    pub fn per_second(&mut self, now: f64) -> (u32, u32) {
        let [l, r] = self.times.each_mut().map(|times| {
            while times.front().is_some_and(|&t| now - t >= 1.0) {
                times.pop_front();
            }
            times.len() as u32
        });
        (l, r)
    }
}

/// The key names drawn, kept while the game's label for the key stays the same (rebinding a key
/// changes it); a resource reload (a new language) empties it.
#[derive(Debug, Default)]
pub struct Labels(Vec<(Text, String)>);

impl Labels {
    /// More than the five keys drawn: old bindings never pile up past it.
    const KEEP: usize = 16;

    /// The plain name kept for `label`.
    pub fn get(&self, label: &Text) -> Option<String> {
        self.0
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, n)| n.clone())
    }

    /// Keeps `name` for `label`.
    pub fn put(&mut self, label: Text, name: String) {
        if self.0.len() >= Self::KEEP {
            self.0.clear();
        }
        self.0.push((label, name));
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Default)]
struct State {
    clicks: Clicks,
    labels: Labels,
    /// Whether this run looked at the stored corner once (to rewrite an earlier version's name).
    corner_read: bool,
    /// Whether this run looked for the extra's old settings.
    extras_checked: bool,
}

#[derive(Default)]
struct Keystrokes {
    // Never held across a host call: a host call can fire another of this mod's hooks.
    state: Mutex<State>,
}

/// Seconds on the game's clock.
fn now(host: &dyn ClientHost) -> f64 {
    host.millis() as f64 / 1000.0
}

/// Whether a vanilla mapping is held, with no name looked up.
fn down(host: &dyn ClientHost, name: &str) -> bool {
    matches!(host.mapping(name), Ok(Some(m)) if m.down)
}

/// The game itself, for the one read of the extra's settings.
struct HostExtras<'a> {
    host: &'a dyn ClientHost,
    marker: ConfigPath,
}

impl ExtraSource for HostExtras<'_> {
    fn unread(&self) -> bool {
        matches!(self.host.data_files().stat(&self.marker), Ok(None))
    }
    fn extra(&self) -> Vec<(String, String)> {
        self.host.official_extra_settings()
    }
    fn has_setting(&self, key: &str) -> bool {
        self.host.setting(ID, key).is_some()
    }
    fn set_setting(&self, key: &str, value: &str) -> bool {
        // The host's write says nothing back: a read shows whether it was kept.
        self.host.set_setting(ID, key, value);
        self.host.setting(ID, key).as_deref().map(str::trim) == Some(value)
    }
    fn mark_read(&self) -> bool {
        self.host
            .data_files()
            .write(&self.marker, b"done\n")
            .is_ok()
    }
}

impl Keystrokes {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// At most once a run, until the data folder remembers it was done: the extra's old corner
    /// and clicks per second become the mod's own (`seed_from_extra`).
    fn read_extra_once(&self, host: &dyn ClientHost) {
        if std::mem::replace(&mut self.state().extras_checked, true) {
            return;
        }
        if let Ok(marker) = ConfigPath::new(EXTRAS_READ) {
            seed_from_extra(&HostExtras { host, marker });
        }
    }

    /// A vanilla mapping: whether it is held, and its key's name in plain text.
    fn mapping(&self, host: &dyn ClientHost, name: &str) -> (bool, String) {
        match host.mapping(name) {
            Ok(Some(m)) => {
                // Not under the lock while translating: a host call can fire another hook.
                let cached = self.state().labels.get(&m.label);
                let name = cached.unwrap_or_else(|| {
                    let name = plain(&m.label, &|key| host.translate(key));
                    self.state().labels.put(m.label, name.clone());
                    name
                });
                (m.down, name)
            }
            _ => (false, String::new()),
        }
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
impl Hooks for Keystrokes {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for setting in settings() {
            // Only a malformed spec is refused, and the tests check every spec.
            let _ = r.register_setting(setting);
        }
    }

    /// The window's own presses of the left and right buttons while playing: the clicks the
    /// game's counter took. The press goes on to the game.
    fn mouse_button_pre(&self, host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
        let i = match event.button {
            GLFW_LEFT => 0,
            GLFW_RIGHT => 1,
            _ => return Flow::Continue,
        };
        if event.pressed && host.playing() && host.on(ID) {
            let now = now(host);
            self.state().clicks.press(i, now);
        }
        Flow::Continue
    }

    /// A resource reload (a new language among them): the key names are translated again.
    fn client_resources_reloaded(
        &self,
        _: &mut dyn ClientHost,
        _: &mut ClientResourcesReloadedEvent,
    ) {
        self.state().labels.clear();
    }

    /// Every frame: presses of the attack and use mappings that came another way.
    fn render_frame_pre(&self, host: &mut dyn ClientHost, _: &mut RenderFramePreEvent) {
        if !host.on(ID) {
            return;
        }
        let host = &*host;
        let playing = host.playing();
        let held = BUTTONS.map(|name| playing && down(host, name));
        let now = now(host);
        self.state().clicks.frame(held, now);
    }

    /// The keys in their corner, below (or above) what the corner holds already; none while F3's
    /// debug screen takes the corner.
    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) {
            return;
        }
        self.read_extra_once(host);
        let corner = CORNERS[self.corner(host)].0;
        if !host.native_corner_shown(corner) {
            return;
        }
        let cps = host.setting_bool("cps").unwrap_or(true).then(|| {
            let now = now(host);
            self.state().clicks.per_second(now)
        });
        let view = View {
            moves: MOVES.map(|name| self.mapping(host, name)),
            jump: down(host, "key.jump"),
            attack: down(host, BUTTONS[0]),
            use_: down(host, BUTTONS[1]),
            cps,
        };
        let draw = &mut *event.draw;
        let shapes = shapes(&view, &|text| draw.text_width(text));
        let (w, h) = SIZE;
        let (x, y) = event.corners.place(corner, w, h);
        for shape in shapes {
            match shape {
                Shape::Fill([dx, dy, w, h], argb) => draw.fill([x + dx, y + dy, w, h], argb),
                Shape::Label(text, [dx, dy], argb, shadow) => {
                    draw.text(&text, [x + dx, y + dy], argb, shadow)
                }
            }
        }
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
    fn the_stored_files_of_earlier_versions_are_read_back() {
        // 1.x and 2.x wrote the corner's name; 3.x writes the option's index.
        for (i, (_, name)) in CORNERS.iter().enumerate() {
            assert_eq!(stored_corner(name), Some(i), "{name}");
            assert_eq!(stored_corner(&format!("{name}\n")), Some(i), "{name}");
        }
        assert_eq!(CORNERS.map(|c| c.1)[3], "bottom_right");
        assert_eq!(stored_corner("3"), Some(3));
        // A corner it does not know is the extra's default (top left).
        assert_eq!(stored_corner("middle"), None);
        assert_eq!(stored_corner("4"), None);
        // The settings read what earlier versions wrote, and write it as they did.
        let [corner, cps] = settings();
        use mod_api::SettingValue::{Choice, Toggle};
        assert_eq!(corner.read(None), Choice(0), "top left, as the extra");
        assert_eq!(corner.read(Some("3")), Choice(3));
        assert_eq!(corner.store(Choice(3)).as_deref(), Some("3"));
        assert_eq!(cps.read(None), Toggle(true), "shown, as the extra");
        assert_eq!(cps.read(Some("false")), Toggle(false));
        assert_eq!(cps.read(Some("true")), Toggle(true));
        // Anything but `false` showed them in 1.x and 2.x, and still does.
        assert_eq!(cps.read(Some("yes")), Toggle(true));
        assert_eq!(cps.read(Some("false\n")), Toggle(false));
        assert_eq!(cps.store(Toggle(false)).as_deref(), Some("false"));
        assert_eq!(cps.store(Toggle(true)).as_deref(), Some("true"));
    }

    #[test]
    fn takes_the_extras_corner_and_cps_once() {
        let old = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        assert_eq!(
            from_extra(&old(&[
                ("keystrokes", "true"),
                ("keystrokesCorner", "bottom_right"),
                ("keystrokesCps", "false"),
            ])),
            [("corner", "3".to_owned()), ("cps", "false".to_owned())]
        );
        assert_eq!(
            from_extra(&old(&[
                ("keystrokesCorner", "top_right"),
                ("keystrokesCps", "true")
            ])),
            [("corner", "1".to_owned()), ("cps", "true".to_owned())]
        );
        // Nothing else is taken: not the switch, not another extra's keys, not odd values.
        assert!(
            from_extra(&old(&[
                ("keystrokesCorner", "middle"),
                ("keystrokesCps", "yes")
            ]))
            .is_empty()
        );
        assert!(
            from_extra(&old(&[("keystrokes", "false"), ("fpsCorner", "top_right")])).is_empty()
        );
        assert!(from_extra(&[]).is_empty());
        // What it gives is what the settings read back.
        let [corner, cps] = settings();
        use mod_api::SettingValue::{Choice, Toggle};
        assert_eq!(corner.read(Some("3")), Choice(3));
        assert_eq!(cps.read(Some("false")), Toggle(false));
        assert!(ConfigPath::new(EXTRAS_READ).is_ok());
    }

    /// The game as the one read of the extra's settings sees it, with what was done in order.
    #[derive(Default)]
    struct FakeGame {
        marker: std::cell::Cell<bool>,
        /// The folder can't be asked (stat fails).
        broken: bool,
        /// The marker can't be written.
        mark_fails: bool,
        /// This setting can't be written.
        set_fails: Option<&'static str>,
        extra: Vec<(String, String)>,
        settings: std::cell::RefCell<Vec<(String, String)>>,
        log: std::cell::RefCell<Vec<String>>,
    }

    impl ExtraSource for FakeGame {
        fn unread(&self) -> bool {
            !self.broken && !self.marker.get()
        }
        fn extra(&self) -> Vec<(String, String)> {
            self.log.borrow_mut().push("extra".into());
            self.extra.clone()
        }
        fn has_setting(&self, key: &str) -> bool {
            self.settings.borrow().iter().any(|(k, _)| k == key)
        }
        fn set_setting(&self, key: &str, value: &str) -> bool {
            self.log.borrow_mut().push(format!("set {key}={value}"));
            if self.set_fails == Some(key) {
                return false;
            }
            self.settings.borrow_mut().push((key.into(), value.into()));
            true
        }
        fn mark_read(&self) -> bool {
            self.log.borrow_mut().push("marker".into());
            self.marker.set(!self.mark_fails);
            !self.mark_fails
        }
    }

    fn extra(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_extras_settings_are_written_before_the_marker() {
        let game = FakeGame {
            extra: extra(&[
                ("keystrokesCorner", "bottom_right"),
                ("keystrokesCps", "false"),
            ]),
            ..FakeGame::default()
        };
        assert!(seed_from_extra(&game));
        assert_eq!(
            *game.log.borrow(),
            ["extra", "set corner=3", "set cps=false", "marker"]
        );
        // The next run finds the marker and asks nothing.
        game.log.borrow_mut().clear();
        assert!(!seed_from_extra(&game));
        assert!(game.log.borrow().is_empty());
    }

    #[test]
    fn a_setting_that_did_not_stick_leaves_no_marker_and_is_tried_again() {
        let mut game = FakeGame {
            extra: extra(&[
                ("keystrokesCorner", "bottom_right"),
                ("keystrokesCps", "false"),
            ]),
            set_fails: Some("corner"),
            ..FakeGame::default()
        };
        assert!(!seed_from_extra(&game));
        assert_eq!(
            *game.log.borrow(),
            ["extra", "set corner=3", "set cps=false"],
            "no marker after a failed write"
        );
        assert!(!game.marker.get());
        // The next run writes only what is still missing, then the marker.
        game.set_fails = None;
        game.log.borrow_mut().clear();
        assert!(seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra", "set corner=3", "marker"]);
    }

    #[test]
    fn the_extras_never_overwrite_the_players_own_setting() {
        let game = FakeGame {
            extra: extra(&[
                ("keystrokesCorner", "bottom_right"),
                ("keystrokesCps", "false"),
            ]),
            settings: vec![("corner".into(), "1".into())].into(),
            ..FakeGame::default()
        };
        assert!(seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra", "set cps=false", "marker"]);
        assert_eq!(
            *game.settings.borrow(),
            [
                ("corner".into(), "1".into()),
                ("cps".into(), "false".into())
            ]
        );
    }

    #[test]
    fn no_extras_leave_no_marker_and_are_asked_again_next_run() {
        // A player who never changed the extras, or a file the game couldn't read this time.
        let game = FakeGame::default();
        assert!(!seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra"]);
        assert!(!game.marker.get());
        // The next run asks again; once the game gives them they are taken.
        let game = FakeGame {
            extra: extra(&[("keystrokesCps", "false")]),
            ..game
        };
        assert!(seed_from_extra(&game));
        assert!(game.marker.get());
        // Only the extra's switch: nothing to take, but the extras were read.
        let game = FakeGame {
            extra: extra(&[("keystrokes", "false")]),
            ..FakeGame::default()
        };
        assert!(seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra", "marker"]);
        // A folder that can't be asked: nothing read, nothing written.
        let game = FakeGame {
            broken: true,
            extra: extra(&[("keystrokesCps", "false")]),
            ..FakeGame::default()
        };
        assert!(!seed_from_extra(&game));
        assert!(game.log.borrow().is_empty());
    }

    #[test]
    fn a_marker_that_cant_be_written_changes_no_setting_twice() {
        let game = FakeGame {
            mark_fails: true,
            extra: extra(&[("keystrokesCps", "false")]),
            ..FakeGame::default()
        };
        assert!(!seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra", "set cps=false", "marker"]);
        // The next run tries again, but the setting has its file now: nothing is changed.
        game.log.borrow_mut().clear();
        game.settings.borrow_mut()[0].1 = "true".into();
        assert!(!seed_from_extra(&game));
        assert_eq!(*game.log.borrow(), ["extra", "marker"]);
        assert_eq!(*game.settings.borrow(), [("cps".into(), "true".into())]);
    }

    #[test]
    fn key_names_are_translated_once_per_binding() {
        let mut labels = Labels::default();
        let w = Text::translate("key.keyboard.w", vec![]);
        assert_eq!(labels.get(&w), None);
        labels.put(w.clone(), "W".into());
        assert_eq!(labels.get(&w).as_deref(), Some("W"));
        // A new binding is a new label.
        let z = Text::translate("key.keyboard.z", vec![]);
        assert_eq!(labels.get(&z), None);
        // The box stays small.
        for i in 0..40 {
            labels.put(Text::literal(i.to_string()), i.to_string());
        }
        assert!(labels.0.len() <= Labels::KEEP);
        labels.clear();
        assert_eq!(labels.get(&Text::literal("39")), None);
    }

    #[test]
    fn lays_the_keys_out_as_the_extra_did() {
        let mut v = View {
            moves: ["W", "A", "S", "D"].map(|k| (false, k.to_owned())),
            cps: Some((3, 0)),
            ..View::default()
        };
        v.moves[0].0 = true;
        let width = |t: &str| t.len() as f32 * 6.0;
        let out = shapes(&v, &width);
        // W lit: light fill, dark text with no shadow, centred in its key.
        assert_eq!(out[0], Shape::Fill([24.0, 0.0, 22.0, 22.0], HELD));
        assert_eq!(
            out[1],
            Shape::Label("W".into(), [32.0, 7.0], INK_HELD, false)
        );
        // A, S, D in the second row, up.
        assert_eq!(out[2], Shape::Fill([0.0, 24.0, 22.0, 22.0], BACKGROUND));
        assert_eq!(out[6], Shape::Fill([48.0, 24.0, 22.0, 22.0], BACKGROUND));
        // The mouse buttons: half the width each, the label and the clicks per second.
        assert_eq!(out[8], Shape::Fill([0.0, 48.0, 34.0, 22.0], BACKGROUND));
        assert_eq!(out[9], Shape::Label("LMB".into(), [8.0, 51.0], INK, true));
        assert!(out.contains(&Shape::Label("3 CPS".into(), [2.0, 60.0], INK, true)));
        assert!(out.contains(&Shape::Fill([36.0, 48.0, 34.0, 22.0], BACKGROUND)));
        // The space bar and its line.
        assert_eq!(
            out[out.len() - 2],
            Shape::Fill([0.0, 72.0, 70.0, 10.0], BACKGROUND)
        );
        assert_eq!(
            out[out.len() - 1],
            Shape::Fill([23.0, 76.0, 24.0, 2.0], INK)
        );
        assert_eq!(SIZE, (70.0, 82.0));
        // Without clicks per second the button labels sit in the middle.
        v.cps = None;
        let out = shapes(&v, &width);
        assert!(out.contains(&Shape::Label("LMB".into(), [8.0, 7.0 + 48.0], INK, true)));
        // The space bar held: light, with a dark line.
        v.jump = true;
        let out = shapes(&v, &width);
        assert_eq!(
            out[out.len() - 2],
            Shape::Fill([0.0, 72.0, 70.0, 10.0], HELD)
        );
        assert_eq!(
            out[out.len() - 1],
            Shape::Fill([23.0, 76.0, 24.0, 2.0], INK_HELD)
        );
    }

    #[test]
    fn key_names_read_as_controls_shows_them() {
        let translate = |key: &str| match key {
            "key.keyboard.w" => Some("W".to_owned()),
            "key.mouse.left" => Some("Left Button".to_owned()),
            _ => None,
        };
        assert_eq!(plain(&Text::literal("W"), &translate), "W");
        assert_eq!(
            plain(&Text::translate("key.keyboard.w", vec![]), &translate),
            "W"
        );
        assert_eq!(
            plain(&Text::translate("key.keyboard.unknown", vec![]), &translate),
            "key.keyboard.unknown"
        );
        assert_eq!(
            plain(
                &Text::literal("Shift + ").append(Text::translate("key.keyboard.w", vec![])),
                &translate
            ),
            "Shift + W"
        );
    }

    #[test]
    fn counts_clicks_as_the_games_counter() {
        let mut c = Clicks::default();
        assert_eq!(c.per_second(0.0), (0, 0));
        // Three window presses of the left button: the frame after sees the mapping held and
        // doesn't count it again.
        c.press(0, 0.1);
        c.frame([true, false], 0.11);
        c.frame([false, false], 0.2);
        c.press(0, 0.3);
        c.press(0, 0.35);
        c.frame([true, false], 0.36);
        assert_eq!(c.per_second(0.4), (3, 0));
        // A controller's trigger (or horizon-ctl) holds the use mapping with no window press:
        // one click, however long it is held.
        c.frame([false, true], 0.5);
        c.frame([false, true], 0.6);
        c.frame([false, false], 0.7);
        assert_eq!(c.per_second(0.7), (3, 1));
        // A second later the presses have left the window.
        assert_eq!(c.per_second(1.31), (1, 1));
        assert_eq!(c.per_second(1.6), (0, 0));
        // The queue stays small.
        for i in 0..200 {
            c.press(1, 2.0 + i as f64 * 0.001);
        }
        assert_eq!(c.per_second(2.3), (0, 64));
    }

    #[test]
    fn a_click_counts_once_on_the_button_pressed() {
        // Left-handed: attack on the right button. The window's right press is the click; the
        // attack mapping going down in that frame is the same click.
        let mut c = Clicks::default();
        c.press(1, 0.1);
        c.frame([true, false], 0.11);
        c.frame([false, false], 0.2);
        assert_eq!(c.per_second(0.3), (0, 1));
        // And use on the left button.
        let mut c = Clicks::default();
        c.press(0, 0.1);
        c.frame([false, true], 0.11);
        assert_eq!(c.per_second(0.3), (1, 0));
        // Both buttons in one frame, each bound as usual: one click each.
        let mut c = Clicks::default();
        c.press(0, 0.1);
        c.press(1, 0.1);
        c.frame([true, true], 0.11);
        assert_eq!(c.per_second(0.3), (1, 1));
    }

    #[test]
    fn a_key_click_in_the_same_frame_as_a_mouse_click_counts_too() {
        // A left click and use on a key in one frame: two clicks, not one.
        let mut c = Clicks::default();
        c.press(0, 0.1);
        c.frame([true, true], 0.11);
        assert_eq!(c.per_second(0.3), (1, 1));
        // A right click and attack on a key.
        let mut c = Clicks::default();
        c.press(1, 0.1);
        c.frame([true, true], 0.11);
        assert_eq!(c.per_second(0.3), (1, 1));
        // Swapped buttons, both clicked: still one each.
        let mut c = Clicks::default();
        c.press(1, 0.1);
        c.press(0, 0.1);
        c.frame([true, true], 0.11);
        assert_eq!(c.per_second(0.3), (1, 1));
    }

    #[test]
    fn a_mapping_press_with_no_mouse_press_counts() {
        // Attack on a key: no window press, the mapping goes down, one click on the left.
        let mut c = Clicks::default();
        c.frame([true, false], 0.1);
        c.frame([true, false], 0.2);
        c.frame([false, false], 0.3);
        c.frame([true, false], 0.4);
        assert_eq!(c.per_second(0.5), (2, 0));
        // A controller's trigger on use, then a mouse click on the next frame: one each.
        let mut c = Clicks::default();
        c.frame([false, true], 0.1);
        c.press(0, 0.15);
        c.frame([true, true], 0.2);
        assert_eq!(c.per_second(0.5), (1, 1));
        // A window press is forgotten after its frame: a later key press counts.
        let mut c = Clicks::default();
        c.press(1, 0.1);
        c.frame([false, false], 0.11);
        c.frame([true, false], 0.2);
        assert_eq!(c.per_second(0.5), (1, 1));
    }
}
