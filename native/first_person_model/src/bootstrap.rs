//! Upstream FirstPersonBootstrap.java + FirstPersonModelMod.java, 1:1. The
//! forge and neoforge bootstrap variants are preprocessor blocks upstream
//! gates off on fabric and are not carried; the fabric entry point is
//! FirstPersonModelMod.onInitializeClient.

use std::rc::Rc;

use crate::first_person_model_core::FirstPersonModelCore;

/// dev.tr7zw.firstperson.FirstPersonModelMod.
pub struct FirstPersonModelMod;

impl FirstPersonModelMod {
    pub fn on_initialize_client() {
        FirstPersonModelCore::instance().shared_setup();
    }

    /// The whole fabric bootstrap: construct the core and run sharedSetup.
    pub fn bootstrap() -> Rc<FirstPersonModelCore> {
        let core = FirstPersonModelCore::instance();
        core.shared_setup();
        core
    }
}
