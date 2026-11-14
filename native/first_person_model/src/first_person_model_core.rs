//! Upstream FirstPersonModelCore.java, 1:1. The //? preprocessor branches
//! collapse to the fabric / >= 1.21.2 paths (Horizon's 26.3 tree); the forge
//! and neoforge event wiring is the loader-specific block upstream gates off
//! on fabric and is not carried.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::access::{PlayerRendererAccessImpl, PlayerRendererAccessTrait};
use crate::api::{FirstPersonAPI, PlayerOffsetHandler};
use crate::keybind_holder::KeybindHolder;
use crate::logic_handler::LogicHandler;
use crate::modsupport::{ModSupportLoader, PlayerAnimatorSupport};
use crate::seams::{general_util, CameraType};
use crate::first_person_base::FirstPersonBase;

/// dev.tr7zw.firstperson.FirstPersonModelCore.
pub struct FirstPersonModelCore {
    pub base: Rc<FirstPersonBase>,
    pub logic_handler: RefCell<Option<Rc<LogicHandler>>>,
    /// WorldRendererMixin/RenderDispatcherMixin renderingPlayerPost flag.
    pub rendering_player_post: Cell<bool>,
    is_held: Cell<bool>,
    late_init: Cell<bool>,
    last_camera_type: RefCell<Option<CameraType>>,
    tick_counter_workaround: Cell<i32>,
}

impl FirstPersonModelCore {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            base: FirstPersonBase::new(),
            logic_handler: RefCell::new(None),
            is_held: Cell::new(false),
            late_init: Cell::new(true),
            last_camera_type: RefCell::new(None),
            tick_counter_workaround: Cell::new(0),
            rendering_player_post: Cell::new(false),
        })
    }

    /// FirstPersonModelCore.instance.
    pub fn instance() -> Rc<Self> {
        thread_local! {
            static INSTANCE: Rc<FirstPersonModelCore> = FirstPersonModelCore::new();
        }
        INSTANCE.with(|i| i.clone())
    }

    pub fn shared_setup(self: &Rc<Self>) {
        crate::seams::log::info("Loading FirstPerson Mod");
        *self.logic_handler.borrow_mut() = Some(LogicHandler::new(self.clone()));

        self.base.load_config();

        KeybindHolder::instance().register_keybinds();
        crate::RUNTIME.with(|r| {
            r.loader.register_client_tick_start_listener();
        });
        crate::RUNTIME.with(|r| r.loader.disable_display_test());
        crate::RUNTIME.with(|r| r.loader.register_config_screen());

        ModSupportLoader::load_support();
    }

    fn late_init(&self) {
        // isValidClass("dev.kosmx.playerAnim.core.impl.AnimationProcessor")
        if FirstPersonBase::is_valid_class("dev.kosmx.playerAnim.core.impl.AnimationProcessor") {
            crate::seams::log::info("Loading PlayerAnimator support!");
            FirstPersonAPI::register_player_offset_handler(Rc::new(PlayerAnimatorSupport));
        } else {
            crate::seams::log::info("PlayerAnimator not found!");
        }

        if let Some(lh) = self.logic_handler.borrow().as_ref() {
            lh.register_default_handlers();
            lh.reload_auto_vanilla_hands_settings();
        }
        self.update_player_layers();
    }

    pub fn on_tick(&self) {
        if self.late_init.get() {
            self.late_init.set(false);
            self.late_init();
        }
        let down = crate::RUNTIME.with(|_| KeybindHolder::instance().key_binding.is_down());
        if down {
            if self.is_held.get() {
                return;
            }
            self.is_held.set(true);
            self.set_enabled(!self.is_enabled());
        } else {
            self.is_held.set(false);
        }
        self.tick_counter_workaround.set(self.tick_counter_workaround.get() + 1);
    }

    pub fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.base.set_enabled(enabled);
    }

    pub fn is_rendering_player(&self) -> bool {
        // super.isRenderingPlayer() || cameraEntityExtract
        self.base.is_rendering_player() || self.camera_entity_extract()
    }

    /// FirstPersonModelCore.cameraEntityExtract: true while extraction runs.
    pub fn camera_entity_extract(&self) -> bool {
        crate::RUNTIME.with(|rt| rt.loader.camera_entity_extract.get())
    }

    pub fn is_rendering_player_post(&self) -> bool {
        self.rendering_player_post.get()
    }

    pub fn set_rendering_player_post(&self, is_rendering_player_post: bool) {
        self.rendering_player_post.set(is_rendering_player_post);
    }

    pub fn is_rendering_player_raw(&self) -> bool {
        self.base.is_rendering_player()
    }

    pub fn set_rendering_player(&self, is_rendering_player: bool) {
        self.base.set_rendering_player(is_rendering_player);
        // TODO: ugly sideffect, find better way
        // TODO2: even worse workaround with counting ticks
        let current = crate::RUNTIME.with(|rt| rt.client.borrow().options_get_camera_type());
        let changed = self.last_camera_type.borrow().as_ref() != Some(&current)
            || self.tick_counter_workaround.get() % 21 == 0;
        if changed {
            self.tick_counter_workaround.set(self.tick_counter_workaround.get() + 1);
            *self.last_camera_type.borrow_mut() = Some(current);
            self.update_player_layers();
        }
    }

    pub fn config(&self) -> crate::config::FirstPersonSettings {
        self.base
            .config
            .borrow()
            .as_ref()
            .expect("config loaded")
            .clone()
    }

    pub fn config_x_offset(&self) -> i32 {
        self.config().x_offset
    }

    pub fn config_sneak_x_offset(&self) -> i32 {
        self.config().sneak_x_offset
    }

    pub fn config_sit_x_offset(&self) -> i32 {
        self.config().sit_x_offset
    }

    pub fn update_player_layers(&self) {
        if self.config().hidden_layers.is_empty() {
            return;
        }
        // PlayerRendererAccess access = null;
        // >= 1.21.6 branch:
        crate::RUNTIME.with(|rt| {
            let player = general_util::get_player(&rt.client.borrow());
            let access: Option<Rc<RefCell<PlayerRendererAccessImpl>>> = match &player {
                Some(p) => rt.client.borrow().get_entity_render_dispatcher_renderer(p),
                None => return,
            };
            if let Some(access) = access {
                let third = self.last_camera_type.borrow().as_ref()
                    != Some(&CameraType::FirstPerson);
                access.borrow().update_parts_list(third);
            }
        });
    }
}
