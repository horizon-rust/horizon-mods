//! Upstream mixin/client/ package, 1:1.
//!
//! Mixin targets are Minecraft classes; each mixin's injected body ports 1:1
//! in its own module as the translation proceeds. MixinPlugin is the
//! conditional-application gate and is already translated.

//!   Chat/ChatScreenMixin.java, SuggestionsListMixin.java,
//!   ChatComponentMixin.java, CommandSuggestionsMixin.java
//!   CreativeScreen/CreativeModeInventoryScreenMixin.java,
//!   ItemPickerMenuMixin.java, AbstractContainerScreenMixin.java,
//!   ItemPickerMenuAccessor.java

/// Upstream mixin/client/MixinPlugin.java.
pub mod mixin_plugin {
    use crate::seams::is_mod_loaded;

    pub fn should_apply_mixin(target_class_name: &str, mixin_class_name: &str) -> bool {
        let _ = target_class_name;
        if mixin_class_name.contains("CreativeScreenHandlerMixin_CC") {
            return is_mod_loaded("condensed_creative");
        }

        true
    }
}

pub mod abstract_scroll_area;
pub mod chat_component;
pub mod chat_screen;
pub mod creative_screen;
pub mod item_picker_menu;
pub mod abstract_container_screen;
pub mod command_suggestions;
pub mod suggestions_list;
pub mod edit_box;
pub mod hud;
pub mod miscellaneous;
pub mod mouse_handler;
pub mod precise_scissor;
