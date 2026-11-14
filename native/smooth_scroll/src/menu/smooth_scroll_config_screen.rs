//! Upstream menu/SmoothScrollConfigScreen.java, 1:1. The Minecraft layout
//! classes (HeaderAndFooterLayout, GridLayout, ScrollableLayout, StringWidget)
//! are host seams; the widget generation, ordering and callbacks are the
//! translated logic.

use std::cell::RefCell;
use std::rc::Rc;

use crate::cfg_value::{CfgValue, WidgetHandle};
use crate::smooth_sc::SmoothSc;
use crate::seams::{Button, Component};

/// Host seam for net.minecraft.client.gui.screens.Screen: collects renderable
/// widgets and knows the parent screen to return to.
pub trait ScreenHost {
    fn add_renderable_widget(&mut self, widget: WidgetHandle);
    /// minecraft.gui.setScreen(parent)
    fn set_screen(&mut self, parent: u64);
    /// layout.getContentHeight() as the host computed it.
    fn content_height(&self) -> i32;
    /// The two-column row helper appends pairs; the host positions them.
    fn add_row_pair(&mut self, left: WidgetHandle, right: WidgetHandle);
    fn add_wide(&mut self, widget: WidgetHandle, span: i32);
}

pub struct SmoothScrollConfigScreen {
    pub parent: u64,
    /// The buttons the init() step creates.
    pub button1: RefCell<Option<Rc<RefCell<Button>>>>,
    pub button2: RefCell<Option<Rc<RefCell<Button>>>>,
    /// The widget rows generated from the config tree.
    pub rows: RefCell<Vec<Row>>,
    /// Header title: Component.translatable("smoothscroll.config.title")
    pub title: Component,
    /// The widest tracked scrollable height for reposition_elements.
    pub scrollable_max_height: RefCell<i32>,
}

pub enum Row {
    Pair(WidgetHandle, WidgetHandle),
    Wide(WidgetHandle, i32),
}

impl SmoothScrollConfigScreen {
    pub fn new(parent: u64) -> Self {
        Self {
            parent,
            button1: RefCell::new(None),
            button2: RefCell::new(None),
            rows: RefCell::new(Vec::new()),
            title: Component::translatable("smoothscroll.config.title", &[]),
            scrollable_max_height: RefCell::new(0),
        }
    }

    pub fn init(&self, host: &mut dyn ScreenHost) {
        SmoothSc::cfg().borrow().load_and_save();

        let root = SmoothSc::cfg().borrow().get_config_for_modifying();
        let top_level = match CfgValue::get_list(&root) {
            Some(l) => l,
            None => return,
        };
        for cfg_value in top_level {
            let cfglist = CfgValue::get_list(&cfg_value);
            if let Some(cfglist) = cfglist {
                let display = CfgValue::get_display_name(&cfg_value);
                // StringWidget(display, font) spanning both columns
                host.add_wide(
                    WidgetHandle::Label(display),
                    2,
                );
                for inner_cfg_value in cfglist {
                    let a = CfgValue::generate_widget(&inner_cfg_value);
                    if let Some(a0) = &a[0] {
                        a0.set_width(150 * 2);
                        if let Some(a1) = &a[1] {
                            a1.set_width(50);
                            self.rows.borrow_mut().push(Row::Pair(a0.clone(), a1.clone()));
                            host.add_row_pair(a0.clone(), a1.clone());
                        }
                    }
                }
                host.add_wide(WidgetHandle::Label(Component::empty()), 2);
            }
        }

        let weak_cfg = Rc::downgrade(&SmoothSc::cfg());
        let button1 = Button::builder(
            Component::translatable("smoothscroll.config.save", &[]),
            move |_button| {
                if let Some(cfg) = weak_cfg.upgrade() {
                    let root = cfg.borrow().get_config_for_modifying();
                    CfgValue::recursive_save_temp_value(&root);
                    cfg.borrow().save();
                }
                // this.onClose() runs from the host after save
            },
        )
        .build();
        let button2 = Button::builder(
            Component::translatable("smoothscroll.config.exit", &[]),
            move |_button| {
                // this.onClose()
            },
        )
        .build();
        *self.button1.borrow_mut() = Some(button1.clone());
        *self.button2.borrow_mut() = Some(button2.clone());
        host.add_row_pair(WidgetHandle::Button(button1), WidgetHandle::Button(button2));
    }

    pub fn on_close(&self, host: &mut dyn ScreenHost) {
        host.set_screen(self.parent);
    }

    pub fn extract_render_state(&self, _mouse_x: i32, _mouse_y: i32, _delta: f32) {
        let root = SmoothSc::cfg().borrow().get_config_for_modifying();
        CfgValue::refresh_disable_recursive(&root);
        // super.extractRenderState(...) is the host render pass
    }

    pub fn reposition_elements(&self, content_height: i32) {
        *self.scrollable_max_height.borrow_mut() = content_height.max(0);
        // layout.arrangeElements() is the host pass
    }
}
