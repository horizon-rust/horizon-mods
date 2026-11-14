use malilib::config::{ConfigManager, ConfigOption};
use malilib::hotkeys::KeybindMulti;

pub fn register_litematica_configs(mgr: &mut ConfigManager) {
    // Generic
    mgr.register(ConfigOption::string("toolItem", "Generic", "Item used to control tool modes", "minecraft:stick"));
    mgr.register(ConfigOption::boolean("easyPlaceMode", "Generic", "Automatically select and place blocks", true));
    mgr.register(ConfigOption::boolean("renderPlacementBoxes", "Generic", "Render wireframe bounding boxes", true));

    // Visuals
    mgr.register(ConfigOption::double("ghostBlockAlpha", "Visuals", "Transparency of ghost blocks", 0.5));
    mgr.register(ConfigOption::integer("renderDistanceChunks", "Visuals", "Maximum schematic render distance in chunks", 16));

    // Colors
    mgr.register(ConfigOption::color("colorMissingBlock", "Colors", "Missing block highlight color", 0x00B4FF));
    mgr.register(ConfigOption::color("colorWrongBlock", "Colors", "Wrong block highlight color", 0xFF1E1E));
    mgr.register(ConfigOption::color("colorWrongState", "Colors", "Wrong state highlight color", 0xFFDC00));
    mgr.register(ConfigOption::color("colorExtraBlock", "Colors", "Extra block highlight color", 0xFF00DC));
}

pub fn register_litematica_hotkeys(hotkeys: &mut malilib::hotkeys::HotkeyManager) {
    hotkeys.register("openMainMenu", KeybindMulti::parse("M"), "Open Litematica Main Menu");
    hotkeys.register("toggleEasyPlace", KeybindMulti::parse("ALT, E"), "Toggle Easy Place Mode");
    hotkeys.register("nextLayer", KeybindMulti::parse("PAGE_UP"), "Move layer up");
    hotkeys.register("prevLayer", KeybindMulti::parse("PAGE_DOWN"), "Move layer down");
    hotkeys.register("cycleToolMode", KeybindMulti::parse("M, TAB"), "Cycle through tool modes");
}
