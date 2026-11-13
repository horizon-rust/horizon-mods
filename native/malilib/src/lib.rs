pub mod nbt;
pub mod util;
pub mod config;
pub mod hotkeys;
pub mod gui;

pub use nbt::{NbtTag, NbtParser, NbtWriter};
pub use util::{BlockPos, Vec3d, Vec3f, IntBoundingBox, BlockRotation, BlockMirror, transform_block_state, LayerMode, LayerRange};
pub use config::{ConfigManager, ConfigOption, ConfigValue};
pub use hotkeys::{HotkeyManager, KeybindMulti, KeyModifier, KeyAction, RegisteredHotkey};
pub use gui::{Rect, Color, Widget, GuiScreen, Button, Slider, TextField, Label};

use std::sync::{Mutex, OnceLock};

static GLOBAL_CONFIG: OnceLock<Mutex<ConfigManager>> = OnceLock::new();
static GLOBAL_HOTKEYS: OnceLock<Mutex<HotkeyManager>> = OnceLock::new();

pub fn global_config() -> &'static Mutex<ConfigManager> {
    GLOBAL_CONFIG.get_or_init(|| Mutex::new(ConfigManager::new("malilib")))
}

pub fn global_hotkeys() -> &'static Mutex<HotkeyManager> {
    GLOBAL_HOTKEYS.get_or_init(|| Mutex::new(HotkeyManager::new()))
}

pub fn init() {
    log::info!("Initialized MaLiLib (Rust / Horizon)");
}
