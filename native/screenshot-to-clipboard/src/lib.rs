// screenshot-to-clipboard slice 1 (port of comp500/ScreenshotToClipboard,
// MIT (c) 2018 comp500; upstream read at commit 8ea7131, mod 1.0.10).
//
// Logic-only core: the screenshot handling pipeline (mac gate, RGBA
// format gate, byte extraction, the RGB clipboard image the AWT path
// builds), the macOS pasteboard flow, the headless setup decision, and
// the fabrishot hook shape. The OS clipboard, AWT and objc calls are
// seams noted in the README.

pub mod awt_setup;
pub mod macos_compat;
pub mod screenshot_to_clipboard;

/// Upstream NativeImage.Format values the format gate checks against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Rgba,
    Rgb,
    Bgra,
    Luminance,
    LuminanceAlpha,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_id_matches_upstream() {
        assert_eq!(screenshot_to_clipboard::MOD_ID, "screenshotclipboard");
    }
}
