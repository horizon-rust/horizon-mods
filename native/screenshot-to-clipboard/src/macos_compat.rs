// Port of link.infra.screenshotclipboard.common.MacOSCompat.
// macOS doesn't allow GLFW and AWT to load at the same time, so the copy
// goes through the native pasteboard from the saved file. The objc chain
// (NSURL fileURLWithPath -> NSImage initWithContentsOfURL -> NSArray
// arrayByAddingObject -> NSPasteboard generalPasteboard) is the glue; the
// flow, the mac gate and the failure log stay here.

pub trait Pasteboard {
    fn clear_contents(&mut self);
    /// NSPasteboard writeObjects: with the image on it.
    fn write_objects(&mut self, image_path: &str) -> bool;
}

/// Upstream doCopyMacOS. Ok(()) when the pasteboard write reported
/// success; Err carries the upstream error-log text.
pub fn do_copy_mac_os(
    path: &str,
    is_mac: bool,
    pasteboard: &mut dyn Pasteboard,
) -> Result<(), &'static str> {
    if !is_mac {
        return Ok(());
    }

    pasteboard.clear_contents();
    let was_successful = pasteboard.write_objects(path);
    if !was_successful {
        return Err("Failed to write image to pasteboard!");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePasteboard {
        clear_calls: usize,
        write_result: bool,
        last_path: Option<String>,
    }

    impl Pasteboard for FakePasteboard {
        fn clear_contents(&mut self) {
            self.clear_calls += 1;
        }
        fn write_objects(&mut self, image_path: &str) -> bool {
            self.last_path = Some(image_path.to_string());
            self.write_result
        }
    }

    #[test]
    fn non_mac_is_a_no_op() {
        let mut pb = FakePasteboard {
            clear_calls: 0,
            write_result: true,
            last_path: None,
        };
        assert!(do_copy_mac_os("/tmp/shot.png", false, &mut pb).is_ok());
        assert_eq!(pb.clear_calls, 0);
        assert!(pb.last_path.is_none());
    }

    #[test]
    fn mac_flow_clears_then_writes_the_file() {
        let mut pb = FakePasteboard {
            clear_calls: 0,
            write_result: true,
            last_path: None,
        };
        assert!(do_copy_mac_os("/screenshots/2026-11-06.png", true, &mut pb).is_ok());
        assert_eq!(pb.clear_calls, 1);
        assert_eq!(pb.last_path.as_deref(), Some("/screenshots/2026-11-06.png"));
    }

    #[test]
    fn failed_write_reports_the_upstream_error() {
        let mut pb = FakePasteboard {
            clear_calls: 0,
            write_result: false,
            last_path: None,
        };
        assert_eq!(
            do_copy_mac_os("/screenshots/shot.png", true, &mut pb),
            Err("Failed to write image to pasteboard!")
        );
        assert_eq!(pb.clear_calls, 1);
    }
}
