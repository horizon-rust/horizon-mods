//! Upstream api/*, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use crate::seams::{Entity, Vec3};

/// api/ActivationHandler.java.
pub trait ActivationHandler {
    fn prevent_firstperson(&self) -> bool;
}

/// api/PlayerOffsetHandler.java.
pub trait PlayerOffsetHandler {
    /// The current value is the value pre-calculated by the mod (or handlers
    /// running before). If the handler doesn't apply to the current situation,
    /// the expected behavior is to just return `current`.
    fn apply_offset(
        &self,
        entity: &Rc<RefCell<Entity>>,
        delta: f32,
        original: Vec3,
        current: Vec3,
    ) -> Vec3;
}

/// api/FirstPersonAPI.java: the static lists become thread-locals.
pub struct FirstPersonAPI;

impl FirstPersonAPI {
    thread_local! {
        static PLAYER_OFFSET_HANDLERS: RefCell<Vec<Rc<dyn PlayerOffsetHandler>>> =
            const { RefCell::new(Vec::new()) };
        static ACTIVATION_HANDLERS: RefCell<Vec<Rc<dyn ActivationHandler>>> =
            const { RefCell::new(Vec::new()) };
    }

    /// The current status of the FirstPerson Mod.
    pub fn is_enabled() -> bool {
        crate::first_person_model_core::FirstPersonModelCore::instance().is_enabled()
    }

    /// Change the enabled status. Simulates pressing the (by default) F6 hotkey.
    pub fn set_enabled(enabled: bool) {
        crate::first_person_model_core::FirstPersonModelCore::instance().set_enabled(enabled);
    }

    /// Returns true when called during the first person player rendering.
    /// While this is true, no heads/helmets/hats should be rendered!
    pub fn is_rendering_player() -> bool {
        crate::first_person_model_core::FirstPersonModelCore::instance().is_rendering_player()
    }

    /// Adds a new handler. Supports PlayerOffsetHandler and ActivationHandler.
    pub fn register_player_handler(handler: Rc<dyn PlayerOffsetHandler>) {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    /// The PlayerOffsetHandler arm of upstream's registerPlayerHandler.
    pub fn register_player_offset_handler(handler: Rc<dyn PlayerOffsetHandler>) {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    pub fn register_activation_handler(handler: Rc<dyn ActivationHandler>) {
        Self::ACTIVATION_HANDLERS.with(|l| l.borrow_mut().push(handler));
    }

    pub fn get_player_offset_handlers() -> Vec<Rc<dyn PlayerOffsetHandler>> {
        Self::PLAYER_OFFSET_HANDLERS.with(|l| l.borrow().clone())
    }

    pub fn get_activation_handlers() -> Vec<Rc<dyn ActivationHandler>> {
        Self::ACTIVATION_HANDLERS.with(|l| l.borrow().clone())
    }
}
