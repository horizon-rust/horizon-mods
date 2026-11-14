//! Upstream menu/ModMenuImpl.java, 1:1: the mod-menu integration entry.

/// Host seam for com.terraformersmc.modmenu.api.ConfigScreenFactory: builds
/// the config screen with its parent screen.
pub type ConfigScreenFactory<'a> = dyn Fn(u64) -> crate::menu::smooth_scroll_config_screen::SmoothScrollConfigScreen + 'a;

pub struct ModMenuImpl;

impl ModMenuImpl {
    pub fn get_mod_config_screen_factory<'a>(
        parent: u64,
    ) -> Box<dyn Fn() -> crate::menu::smooth_scroll_config_screen::SmoothScrollConfigScreen + 'a>
    where
        Self: 'a,
    {
        Box::new(move || crate::menu::smooth_scroll_config_screen::SmoothScrollConfigScreen::new(parent))
    }
}
