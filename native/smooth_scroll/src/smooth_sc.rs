//! Upstream SmoothScrolling.java (main entry, empty), SmoothSc.java (client
//! entry + shared statics + helpers) and Globals.java, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use crate::seams::{self, GuiGraphicsSeam, UVPair};
use crate::sm_sc_cfg::SmScCfg;

/// Upstream SmoothScrolling.java implements ModInitializer with an empty
/// onInitialize and only the id/logger.
pub mod smooth_scrolling {
    pub const LOGGER_NAME: &str = "smoothscroll";

    pub fn on_initialize() {
        // empty upstream
    }
}

/// Upstream SmoothSc.java.
pub struct SmoothSc;

impl SmoothSc {
    /// public static final Logger LOGGER ("Smooth Scrolling")
    pub const LOGGER_NAME: &'static str = "Smooth Scrolling";
}

// Java statics that mixins read live in globals.
thread_local! {
    /// Upstream SmoothSc.cfg (set in onInitializeClient).
    pub static CFG: RefCell<Option<Rc<RefCell<SmScCfg>>>> = const { RefCell::new(None) };
    /// Upstream SmoothSc.isSmoothScrollingRefurbishedLoaded.
    pub static IS_SMOOTH_SCROLLING_REFURBISHED_LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Upstream Globals.touchpadScrolledY.
    pub static TOUCHPAD_SCROLLED_Y: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn is_smooth_scrolling_refurbished_loaded() -> bool {
    IS_SMOOTH_SCROLLING_REFURBISHED_LOADED.get()
}

impl SmoothSc {
    /// public static boolean isWindows = System.getProperty("os.name").contains("Windows")
    pub fn is_windows() -> bool {
        std::env::consts::OS == "windows"
    }

    pub fn on_initialize_client() {
        IS_SMOOTH_SCROLLING_REFURBISHED_LOADED
            .set(seams::is_mod_loaded("smoothscrollingrefurbished"));
        SmoothSc::printt(&[
            SmoothSc::is_windows().to_string(),
            std::env::consts::OS.to_string(),
        ]);
        let cfg = SmScCfg::new();
        CFG.with(|slot| *slot.borrow_mut() = Some(cfg));
    }

    /// SmoothSc.cfg access, as the Java field would give.
    pub fn cfg() -> Rc<RefCell<SmScCfg>> {
        CFG.with(|slot| {
            slot.borrow()
                .clone()
                .expect("SmoothSc.cfg used before onInitializeClient")
        })
    }

    pub fn print(s: &str) {
        seams::log_info(s);
    }

    /// printt(Object... s) -> joinPrint(", ", s)
    pub fn printt(parts: &[String]) {
        SmoothSc::join_print(", ", parts);
    }

    pub fn join_print(join: &str, parts: &[String]) {
        let mut a = String::new();
        for object in parts {
            a.push_str(object);
            a.push_str(join);
        }
        seams::log_info(&a);
    }

    pub fn read_config() {
        SmoothSc::cfg().borrow().load_and_save();
    }

    pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
        val.max(min).min(max)
    }

    pub fn get_last_frame_duration() -> f32 {
        seams::client_env().get_game_time_delta_ticks()
    }

    pub fn debug_text_draw(graphics: &dyn GuiGraphicsSeam, s: &str, x: i32, y: i32) {
        graphics.text(
            &seams::client_env().font(),
            s,
            x,
            y,
            seams::argb_color(255, 0, 255, 255),
            true,
        );
    }

    pub fn vec2f_add(a: UVPair, b: UVPair) -> UVPair {
        UVPair::new(a.u + b.u, a.v + b.v)
    }
    pub fn vec2f_sub(a: UVPair, b: UVPair) -> UVPair {
        UVPair::new(a.u - b.u, a.v - b.v)
    }
    pub fn vec2f_mul(a: UVPair, b: f32) -> UVPair {
        UVPair::new(a.u * b, a.v * b)
    }
}

/// Upstream Globals.touchpadScrolledY read/write.
pub fn touchpad_scrolled_y() -> bool {
    TOUCHPAD_SCROLLED_Y.get()
}

pub fn set_touchpad_scrolled_y(v: bool) {
    TOUCHPAD_SCROLLED_Y.set(v);
}
