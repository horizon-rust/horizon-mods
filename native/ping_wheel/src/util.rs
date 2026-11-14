//! Upstream: common/util/ (1:1). InputUtils and DirectionalSoundInstance follow the resource/screen phase.
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// util/RateLimiter.java (1:1)
// ---------------------------------------------------------------------------

static TIME_TO_REGENERATE: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);
static TIME_WINDOW: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);

pub fn set_rates(ms_to_regenerate: i64, limit: i64) {
    *TIME_TO_REGENERATE.lock().unwrap() = Some(Duration::from_millis(ms_to_regenerate.max(0) as u64));
    *TIME_WINDOW.lock().unwrap() = Some(Duration::from_millis((ms_to_regenerate * limit).max(0) as u64));
}

#[derive(Default)]
pub struct RateLimiter {
    start_time: Option<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self { start_time: None }
    }

    pub fn check_exceeded(&mut self) -> bool {
        let time_to_regenerate = *TIME_TO_REGENERATE.lock().unwrap();
        let time_window = *TIME_WINDOW.lock().unwrap();

        let (Some(time_to_regenerate), Some(time_window)) = (time_to_regenerate, time_window) else {
            return false;
        };

        if self.start_time.is_none() {
            // Instant.now().minus(timeWindow).plus(timeToRegenerate)
            self.start_time = Some(Instant::now() - time_window + time_to_regenerate);
            return false;
        }

        let start = self.start_time.unwrap();
        let now = Instant::now();
        let mut elapsed = now.duration_since(start);

        if elapsed > time_window {
            elapsed = time_window;
        }

        match elapsed.checked_sub(time_to_regenerate) {
            // leftOver.isNegative()
            None => true,
            Some(left_over) => {
                self.start_time = Some(now - left_over);
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Key mapping tokens (upstream: net.minecraft.client.KeyMapping); the register/set/
// consumeClick/same operations are host-side, the port holds the identity.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct KeyBindingToken {
    pub name: String,
    /// InputConstants.Type: MOUSE or KEYSYM
    pub input_type: &'static str,
    pub key: i32,
    pub category: String,
}

impl KeyBindingToken {
    pub fn new(name: &str, input_type: &'static str, key: i32, category: &str) -> Self {
        Self { name: name.to_string(), input_type, key, category: category.to_string() }
    }

    pub fn set_key(&mut self, key: i32) {
        self.key = key;
    }
}

// ---------------------------------------------------------------------------
// util/InputUtils.java (1:1; key bindings live as tokens, the consumeClick /
// same() behavior stays host-side through the seam trait).
// ---------------------------------------------------------------------------

pub const GLFW_MOUSE_BUTTON_5: i32 = 4;

pub struct InputUtils;

impl InputUtils {
    pub fn key_binding_ping() -> KeyBindingToken {
        KeyBindingToken::new(
            &crate::resource::LanguageUtils::key_of("key", &["ping_location"]),
            "MOUSE",
            GLFW_MOUSE_BUTTON_5,
            &crate::resource::LanguageUtils::key_of("key.category", &["name"]),
        )
    }

    pub fn key_binding_settings() -> KeyBindingToken {
        KeyBindingToken::new(
            &crate::resource::LanguageUtils::key_of("key", &["open_settings"]),
            "KEYSYM",
            -1,
            &crate::resource::LanguageUtils::key_of("key.category", &["name"]),
        )
    }

    /// consumePingHotkey: the pick-item overlap workaround is host-side; the port consumes
    /// the ping binding through the key-mapping seam.
    pub fn consume_ping_hotkey() -> bool {
        crate::seams::consume_click_key(&InputUtils::key_binding_ping())
    }
}
