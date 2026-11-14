//! Upstream mixin/client/Miscellaneous package, 1:1.

pub mod sdl_event_handler_mixin {
    //! Upstream SDLEventHandlerMixin.java.
    //!
    //! The @WrapOperation'd SDL_PollEventWrap is dead code upstream (its
    //! annotation is commented out, the method body remains); it is ported 1:1
    //! with the same note. The live members are handleMouseWheelEvent
    //! (@Inject HEAD) and PollEventsTail (@WrapMethod tail).
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::seams::{self, SdlEvent};
    use crate::smooth_sc::SmoothSc;

    /// Upstream mixin instance fields: lastScrollY, lastScrollYTime, inertia.
    pub struct SdlEventHandlerMixin {
        pub last_scroll_y: f64,
        pub last_scroll_y_time: i64,
        pub inertia: f64,
    }

    impl Default for SdlEventHandlerMixin {
        fn default() -> Self {
            Self { last_scroll_y: 0.0, last_scroll_y_time: 0, inertia: 0.0 }
        }
    }

    impl SdlEventHandlerMixin {
        /// Upstream SDL_PollEventWrap: dead code upstream (the @WrapOperation
        /// annotation is commented out), ported anyway.
        #[allow(dead_code)]
        pub fn sdl_poll_event_wrap(
            &mut self,
            event: &SdlEvent,
            operation: &mut dyn FnMut(&SdlEvent) -> bool,
        ) -> bool {
            let ret = operation(event);
            if event.event_type != 32512 {
                SmoothSc::printt(&[
                    String::from("poll: "),
                    event.event_type.to_string(),
                    event.wheel_which.to_string(),
                ]);
            }
            ret
        }

        /// Upstream handleMouseWheelEvent @Inject(method =
        /// "handleMouseWheelEvent", at = @At("HEAD")).
        pub fn handle_mouse_wheel_event(&mut self, event: &SdlEvent) {
            self.inertia = 0.0;
            // checks, whether it should consider this event touchpad-scroll
            // if the previous event was considered as such and was within
            // touchpadTimeThreshold nanoseconds, it is also considered as
            // touchpad-scroll; if the scroll amount is low enough, it is
            // considered as touchpad-scroll; if the previous event was within
            // touchpadTimeThreshold nanoseconds and is of different absolute
            // amount, it is considered as touchpad-scroll (upstream comment)
            use crate::sm_sc_cfg::vars;
            let y = event.wheel_y;
            let time_delta = (seams::util_nanos() as i64).wrapping_sub(self.last_scroll_y_time);
            if y.abs() < vars::touchpad_threshold()
                || ((time_delta as f32) < vars::touchpad_time_threshold()
                    && (crate::smooth_sc::touchpad_scrolled_y()
                        || (!vars::expect_merged() && self.last_scroll_y.abs() != (y as f64).abs())))
            {
                crate::smooth_sc::set_touchpad_scrolled_y(true);
                if y.abs() > vars::inertia_threshold() {
                    self.inertia = y as f64;
                }
            } else {
                crate::smooth_sc::set_touchpad_scrolled_y(false);
            }
            self.last_scroll_y = y as f64;
            self.last_scroll_y_time = seams::util_nanos() as i64;
        }

        /// Upstream PollEventsTail @WrapMethod(method = "pollEvents"): the
        /// wrapped operation runs first, then the tail. Takes the mixin as an
        /// Rc so the deferred execute lambda reads this.inertia when it runs,
        /// exactly as the upstream closure captures `this`.
        pub fn poll_events_tail(this: &Rc<RefCell<Self>>, operation: &mut dyn FnMut()) {
            operation();
            let time_delta =
                (seams::util_nanos() as i64).wrapping_sub(this.borrow().last_scroll_y_time);
            if time_delta == 0 {
                return;
            }
            if this.borrow().inertia.abs() < crate::sm_sc_cfg::vars::inertia_stop() as f64 {
                return;
            }
            // TODO inertia will be applied every frame, it needs to account for
            // that (upstream comment)
            this.borrow_mut().inertia *= (crate::sm_sc_cfg::vars::inertia_deceleration() as f64)
                .powf(SmoothSc::get_last_frame_duration() as f64);
            // SmoothSc.mc.execute(() -> SmoothSc.mc.mouseHandler.onScroll(
            //     SmoothSc.mc.getWindow().handle(), 0, inertia * getLastFrameDuration()))
            let mixin = Rc::clone(this);
            seams::mouse_host().execute(Box::new(move || {
                let host = seams::mouse_host();
                let window = host.window_handle();
                let frame = SmoothSc::get_last_frame_duration() as f64;
                host.on_scroll(window, 0.0, mixin.borrow().inertia * frame);
            }));
        }
    }
}

pub mod transformation_mixin {
    //! Upstream TransformationMixin.java.
    use crate::seams::ScreenRectangle;
    use crate::transformation_access::Parameters;

    /// Upstream mixin state: the @Shadow fields (scissor, pose, opacity).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TransformationMixin {
        pub scissor: Option<ScreenRectangle>,
        /// Upstream Matrix3x2fc pose.
        pub pose: [f32; 6],
        pub opacity: f32,
    }

    impl TransformationMixin {
        /// Upstream withScissorWrap @WrapMethod(method = "withScissor").
        /// TODO document why this is needed (upstream comment). `operation` is
        /// the host's original withScissor.
        pub fn with_scissor_wrap(
            &self,
            scissor: Option<ScreenRectangle>,
            operation: &mut dyn FnMut(Option<ScreenRectangle>) -> Parameters,
        ) -> Parameters {
            if scissor.is_none() {
                if self.scissor.is_some() {
                    return Parameters::new(self.pose, self.opacity, None);
                }
                if self.scissor.is_none() {
                    // upstream: return (Parameters) (Object) this
                    return Parameters::new(self.pose, self.opacity, self.scissor);
                }
            }
            operation(scissor)
        }
    }
}

pub mod minecraft_client_mixin {
    //! Upstream MinecraftClientMixin.java.
    //!
    //! Upstream setScreenT (@Inject setScreen TAIL, catching the ItemPickerMenu
    //! and resetting creativeScreenScrollOffset) is commented out; ported as a
    //! note only. The live member is onResReload.

    /// Upstream onResReload @Inject(method = "reloadResourcePacks", at = @At("HEAD")).
    pub fn on_res_reload() {
        crate::smooth_sc::SmoothSc::read_config();
    }
}

pub mod render_system_mixin {
    //! Upstream RenderSystemMixin.java: its only injection
    //! (initBackendSystemTail) is commented out upstream, so the class
    //! contributes no behavior. Kept as an empty module for the 1:1 file map.
}

pub mod render_pass_mixin {
    //! Upstream RenderPassMixin.java.
    use crate::seams::RenderArea;

    /// Upstream @Shadow private @Final RenderArea renderArea.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RenderPassMixin {
        pub render_area: RenderArea,
    }

    impl RenderPassMixin {
        /// Upstream ignoreArgumentException @WrapMethod(method =
        /// "enableScissor"). The scissor values here are almost always
        /// calculated and have safety margins, so the area can be invalid
        /// (out of bounds); upstream clamps the values so the invalid-argument
        /// exception never occurs instead of clamping its own scissors
        /// everywhere (upstream comment, abridged).
        pub fn ignore_argument_exception(
            &self,
            mut x: i32,
            mut y: i32,
            mut width: i32,
            mut height: i32,
            operation: &mut dyn FnMut(i32, i32, i32, i32),
        ) {
            let ra = &self.render_area;
            x = x.clamp(ra.x, ra.x + ra.width);
            y = y.clamp(ra.y, ra.y + ra.height);
            width = (width + x).clamp(ra.x, ra.x + ra.width) - x;
            height = (height + y).clamp(ra.y, ra.y + ra.height) - y;
            if width == 0 || height == 0 {
                width = 1;
                height = 1;
                x = ra.x;
                y = ra.y;
            }
            operation(x, y, width, height);
        }
    }
}
