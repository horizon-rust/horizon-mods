// Port of link.infra.screenshotclipboard.common.ScreenshotToClipboard.
// The AWT copy pipeline: only RGBA screenshots are accepted, the alpha
// channel is dropped when the clipboard image is built (JDK-8204187), and
// the copy work runs on a background thread upstream. The clipboard write
// itself is the sink seam here.

use crate::ImageFormat;
use crate::macos_compat;

pub const MOD_ID: &str = "screenshotclipboard";

/// Upstream NativeImage.Format values the gate checks against; only RGBA
/// is accepted for the direct path.
pub use crate::ImageFormat as NativeImageFormat;

/// A captured screenshot as the mod receives it from the screenshot
/// hook: format plus raw pixel bytes (the pointer-backed buffer in
/// upstream, an owned buffer here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenshotImage {
    pub format: ImageFormat,
    pub width: usize,
    pub height: usize,
    /// None mirrors the upstream pointer fetch returning null
    /// (memByteBufferSafe on an invalid pointer).
    pub bytes: Option<Vec<u8>>,
}

impl ScreenshotImage {
    /// Upstream handleScreenshotAWT(NativeImage).
    pub fn handle_awt(&self, is_mac: bool) -> Result<ClipboardImage, CopyError> {
        if is_mac {
            return Err(CopyError::Skipped);
        }

        // Only allow RGBA
        if self.format != ImageFormat::Rgba {
            return Err(CopyError::WrongFormat);
        }

        let expected = self.width * self.height * 4;
        let bytes = match &self.bytes {
            Some(bytes) if bytes.len() == expected => bytes.clone(),
            Some(_) | None => return Err(CopyError::InvalidImage), // "Invalid image"
        };

        self.handle_awt_bytes(&bytes, self.width, self.height, 4, is_mac)
    }

    /// Upstream handleScreenshotAWT(ByteBuffer, int, int, int); used with
    /// components=4 from the screenshot hook and components=3 from the
    /// fabrishot framebuffer hook.
    pub fn handle_awt_bytes(
        &self,
        byte_buffer: &[u8],
        width: usize,
        height: usize,
        components: usize,
        is_mac: bool,
    ) -> Result<ClipboardImage, CopyError> {
        if is_mac {
            return Err(CopyError::Skipped);
        }

        let expected = height * width * components;
        // Upstream: array-backed buffers pass through unchanged; otherwise
        // an exact w*h*components read is performed.
        if byte_buffer.len() < expected {
            return Err(CopyError::InvalidImage);
        }
        let array = &byte_buffer[..expected];

        Ok(do_copy(array, width, height, components))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyError {
    /// macOS goes through the file-path pasteboard path instead.
    Skipped,
    /// "Failed to capture screenshot: wrong format"
    WrongFormat,
    /// "Invalid image"
    InvalidImage,
}

impl CopyError {
    pub fn warning_text(&self) -> Option<&'static str> {
        match self {
            CopyError::Skipped => None,
            CopyError::WrongFormat => Some("Failed to capture screenshot: wrong format"),
            CopyError::InvalidImage => Some("Invalid image"),
        }
    }
}

/// The image the clipboard receives, mirroring the AWT construction in
/// upstream doCopy: ComponentColorModel over CS_sRGB with 8 bits per
/// channel, no alpha (dropped due to JDK-8204187), band offsets
/// {0, 1, 2}, interleaved raster with scanline stride
/// width * components and pixel stride components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    /// Row-major RGB bytes: the first three bytes of every pixel.
    pub rgb: Vec<u8>,
}

fn do_copy(image_data: &[u8], width: usize, height: usize, components: usize) -> ClipboardImage {
    // Upstream builds the image and hands it to the system clipboard on a
    // dedicated "Screenshot to Clipboard Copy" thread; the sink seam takes
    // its place and the model is built inline here.
    let mut rgb = Vec::with_capacity(width * height * 3);
    for pixel in image_data.chunks_exact(components) {
        rgb.extend_from_slice(&pixel[..3]);
    }
    ClipboardImage { width, height, rgb }
}

/// Upstream getTransferableImage: the transferable exposes exactly one
/// flavor, the image flavor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFlavor {
    ImageFlavor,
    Other(&'static str),
}

pub fn transferable_flavors() -> Vec<DataFlavor> {
    vec![DataFlavor::ImageFlavor]
}

pub fn is_flavor_supported(flavor: &DataFlavor) -> bool {
    matches!(flavor, DataFlavor::ImageFlavor)
}

/// Upstream macOS flow: the mixin injects after the file is saved and
/// hands the absolute path to MacOSCompat.doCopyMacOS.
pub fn handle_saved_file_macos(
    path: &str,
    is_mac: bool,
    pasteboard: &mut dyn macos_compat::Pasteboard,
) {
    macos_compat::do_copy_mac_os(path, is_mac, pasteboard);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(w: usize, h: usize) -> Vec<u8> {
        (0..w * h * 4).map(|i| (i % 256) as u8).collect()
    }

    #[test]
    fn mac_skips_the_awt_paths() {
        let img = ScreenshotImage {
            format: ImageFormat::Rgba,
            width: 1,
            height: 1,
            bytes: Some(vec![1, 2, 3, 4]),
        };
        assert_eq!(img.handle_awt(true), Err(CopyError::Skipped));
        assert_eq!(
            ScreenshotImage {
                format: ImageFormat::Rgba,
                width: 1,
                height: 1,
                bytes: Some(vec![1, 2, 3, 4]),
            }
            .handle_awt_bytes(&[1, 2, 3, 4], 1, 1, 4, true),
            Err(CopyError::Skipped)
        );
    }

    #[test]
    fn only_rgba_is_accepted() {
        for format in [ImageFormat::Rgb, ImageFormat::Luminance, ImageFormat::Bgra] {
            let img = ScreenshotImage {
                format,
                width: 2,
                height: 2,
                bytes: Some(rgba(2, 2)),
            };
            assert_eq!(img.handle_awt(false), Err(CopyError::WrongFormat));
            assert_eq!(
                CopyError::WrongFormat.warning_text(),
                Some("Failed to capture screenshot: wrong format")
            );
        }
    }

    #[test]
    fn rgba_passes_and_drops_alpha() {
        let img = ScreenshotImage {
            format: ImageFormat::Rgba,
            width: 2,
            height: 1,
            bytes: Some(vec![10, 20, 30, 99, 40, 50, 60, 98]),
        };
        let out = img.handle_awt(false).unwrap();
        assert_eq!(out.width, 2);
        assert_eq!(out.height, 1);
        // alpha bytes 99/98 are dropped (JDK-8204187)
        assert_eq!(out.rgb, vec![10, 20, 30, 40, 50, 60]);
    }

    #[test]
    fn short_buffer_is_invalid_image() {
        let img = ScreenshotImage {
            format: ImageFormat::Rgba,
            width: 2,
            height: 2,
            bytes: Some(vec![0; 10]),
        };
        assert_eq!(img.handle_awt(false), Err(CopyError::InvalidImage));
        let no_pointer = ScreenshotImage {
            format: ImageFormat::Rgba,
            width: 1,
            height: 1,
            bytes: None,
        };
        assert_eq!(no_pointer.handle_awt(false), Err(CopyError::InvalidImage));
    }

    #[test]
    fn fabrishot_three_component_path() {
        let img = ScreenshotImage {
            format: ImageFormat::Rgba,
            width: 2,
            height: 1,
            bytes: Some(rgba(2, 1)),
        };
        // fabrishot hands over an RGB framebuffer with components=3
        let out = img
            .handle_awt_bytes(&[1, 2, 3, 4, 5, 6], 2, 1, 3, false)
            .unwrap();
        assert_eq!(out.rgb, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn transferable_exposes_only_the_image_flavor() {
        assert_eq!(transferable_flavors(), vec![DataFlavor::ImageFlavor]);
        assert!(is_flavor_supported(&DataFlavor::ImageFlavor));
        assert!(!is_flavor_supported(&DataFlavor::Other("text/plain")));
    }
}
