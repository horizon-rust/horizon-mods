pub mod keybind;
pub mod manager;

pub use keybind::{KeyAction, KeyModifier, KeybindMulti};
pub use manager::{HotkeyManager, RegisteredHotkey};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_parsing_and_trigger() {
        let mut mgr = HotkeyManager::new();
        let bind = KeybindMulti::parse("M, C");
        assert_eq!(bind.keys, vec!["M", "C"]);

        mgr.register("openConfig", bind, "Open Config GUI");

        let mods = KeyModifier::NONE;
        // Press M
        let t1 = mgr.handle_key_event("M", true, mods);
        assert!(t1.is_empty());

        // Press C while M is held
        let t2 = mgr.handle_key_event("C", true, mods);
        assert_eq!(t2, vec!["openConfig"]);

        // Release C
        let t3 = mgr.handle_key_event("C", false, mods);
        assert!(t3.is_empty());
    }
}
