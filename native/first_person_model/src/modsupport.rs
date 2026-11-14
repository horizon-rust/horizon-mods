//! Upstream modsupport/*, 1:1.

use std::rc::Rc;

use crate::api::{ActivationHandler, PlayerOffsetHandler};
use crate::first_person_base::FirstPersonBase;
use crate::seams::{Entity, Vec3};

/// modsupport/ModSupportLoader.java.
pub struct ModSupportLoader;

impl ModSupportLoader {
    pub fn load_support() {
        crate::RUNTIME.with(|rt| {
            // try {
            if rt.loader.is_mod_loaded("freecam") {
                FirstPersonAPI_reg_freecam();
                crate::seams::log::info("Freecam support loaded.");
            }
            // } catch (Throwable ex) { LOGGER.warn(...) } - the port's support
            // classes cannot throw at load time.
        });
    }
}

fn FirstPersonAPI_reg_freecam() {
    crate::api::FirstPersonAPI::register_activation_handler(Rc::new(FreecamSupport));
}

/// modsupport/FreecamSupport.java: making sure the constructor throws an
/// exception, in case something is wrong, is the Java-side guard; the port's
/// Freecam.isEnabled() seam is total.
pub struct FreecamSupport;

impl ActivationHandler for FreecamSupport {
    fn prevent_firstperson(&self) -> bool {
        // Freecam.isEnabled(): a mod-loaded check on this port.
        crate::RUNTIME.with(|rt| rt.loader.is_mod_loaded("freecam"))
    }
}

/// modsupport/PlayerAnimatorSupport.java: the transform math upstream ships is
/// fully commented out (FIXME: PartKey missing), so the handler returns
/// current, 1:1.
pub struct PlayerAnimatorSupport;

impl PlayerOffsetHandler for PlayerAnimatorSupport {
    fn apply_offset(
        &self,
        _entity: &Rc<std::cell::RefCell<Entity>>,
        _delta: f32,
        _original: Vec3,
        current: Vec3,
    ) -> Vec3 {
        current
    }
}
