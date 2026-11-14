// Port of AWTHackMixin and the init() clipboard check in
// ScreenshotToClipboard. At startup on non-macOS the java.awt.headless
// property is forced off as early as possible (with the upstream println)
// so AWT can load alongside GLFW; init() then verifies the system
// clipboard is reachable and warns on HeadlessException. The JVM system
// property and the Toolkit call are the glue; the decisions and exact
// messages stay here.

/// Upstream AWTHackMixin.awtHack: Ok(Some(text)) prints and forces
/// headless off on non-macOS; Ok(None) leaves the JVM untouched on macOS.
pub fn awt_hack(
    os_name: &str,
    set_headless: &mut dyn FnMut(bool),
) -> Result<Option<&'static str>, std::convert::Infallible> {
    if !os_name.to_lowercase().contains("mac") {
        set_headless(false);
        Ok(Some(
            "[Screenshot to Clipboard] Setting java.awt.headless to false",
        ))
    } else {
        Ok(None)
    }
}

/// Upstream init(): non-macOS only, a test fetch of the system clipboard;
/// a HeadlessException surfaces the upstream warning.
pub fn init(is_mac: bool, clipboard_available: bool) -> Option<&'static str> {
    if !is_mac && !clipboard_available {
        return Some("java.awt.headless property was not set properly!");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn non_mac_forces_headless_off_with_the_upstream_line() {
        let forced = Cell::new(true);
        let printed = awt_hack("Windows 11", &mut |v| forced.set(v)).unwrap();
        assert!(!forced.get());
        assert_eq!(
            printed,
            Some("[Screenshot to Clipboard] Setting java.awt.headless to false")
        );
    }

    #[test]
    fn mac_leaves_headless_alone() {
        let forced = Cell::new(true);
        let printed = awt_hack("Mac OS X", &mut |v| forced.set(v)).unwrap();
        assert!(forced.get());
        assert_eq!(printed, None);
    }

    #[test]
    fn init_warns_only_off_mac_without_a_clipboard() {
        assert_eq!(
            init(false, false),
            Some("java.awt.headless property was not set properly!")
        );
        assert_eq!(init(false, true), None);
        assert_eq!(init(true, false), None); // mac skips the AWT check
    }
}
