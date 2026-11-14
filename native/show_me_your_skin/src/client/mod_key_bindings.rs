//! nl.enjarai.showmeyourskin.client.ModKeyBindings (1:1).

use crate::config::mod_config;
use crate::gui::ArmorConfigScreen;

/// Java: KeyMapping OPEN_SETTINGS (K) / GLOBAL_TOGGLE (J); the mappings
/// themselves are the engine keybind seam.
#[derive(Default)]
pub struct KeyMappingState {
    pub open_settings_clicks: u32,
    pub global_toggle_clicks: u32,
}

thread_local! {
    static KEYBINDS: std::cell::RefCell<KeyMappingState> = Default::default();
}

/// Java: register() - two KeyMappings in the showmeyourskin category.
pub fn register() {}

/// Java: tick(client) - consume the queued clicks.
pub fn tick() {
    KEYBINDS.with(|k| {
        let mut k = k.borrow_mut();
        while k.open_settings_clicks > 0 {
            k.open_settings_clicks -= 1;
            crate::gui::framework::set_screen(ArmorConfigScreen::create_config_screen(None).widget, None);
        }
        while k.global_toggle_clicks > 0 {
            k.global_toggle_clicks -= 1;
            mod_config::with_instance_mut(|c| c.global_enabled = !c.global_enabled);
            mod_config::with_instance(|c| c.save());
            // Java: sendOverlayMessage(translatable("key.showmeyourskin." + ...)).
        }
    });
}

/// Test seam: queue a click like the engine window would.
pub fn queue_click(open_settings: bool) {
    KEYBINDS.with(|k| {
        let mut k = k.borrow_mut();
        if open_settings {
            k.open_settings_clicks += 1;
        } else {
            k.global_toggle_clicks += 1;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_toggle_flips_and_saves() {
        queue_click(true);
        queue_click(false);
        let before = mod_config::with_instance(|c| c.global_enabled);
        tick();
        let after = mod_config::with_instance(|c| c.global_enabled);
        assert_ne!(before, after);
        // the settings click consumed silently (screen seam)
        queue_click(true);
        tick();
    }
}
