//! Java: compat/modmenu/ModMenuIntegration (1:1): the config screen factory
//! is ArmorConfigScreen::createConfigScreen.
pub fn get_mod_config_screen_factory() -> fn(Option<crate::gui::framework::WidgetNode>) -> crate::gui::ArmorConfigScreen {
    crate::gui::ArmorConfigScreen::create_config_screen
}
