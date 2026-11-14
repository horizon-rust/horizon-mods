//! Upstream KeybindHolder.java, 1:1.

use std::cell::Cell;
use std::rc::Rc;

use crate::seams::{create_key_mapping, KeyMapping};

/// dev.tr7zw.firstperson.KeybindHolder.
pub struct KeybindHolder {
    pub key_binding: Rc<KeyMapping>,
    initialized: Cell<bool>,
}

impl KeybindHolder {
    /// KeybindHolder.INSTANCE.
    pub fn instance() -> Rc<Self> {
        thread_local! {
            static INSTANCE: Rc<KeybindHolder> = Rc::new(KeybindHolder {
                key_binding: create_key_mapping("key.firstperson.toggle", 295, "firstperson:keybind"),
                initialized: Cell::new(false),
            });
        }
        INSTANCE.with(|i| i.clone())
    }

    pub fn register_keybinds(&self) {
        if self.initialized.get() {
            return;
        }
        self.initialized.set(true);
        // ModLoaderUtil.registerKeybind(keyBinding);
        crate::RUNTIME.with(|r| r.loader.register_keybind(&self.key_binding));
    }
}
