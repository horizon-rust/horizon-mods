//! Upstream menu/CustomTab.java, 1:1: a Tab that does not render but provides
//! inner widgets to TabNavigationBar instances.

use std::cell::RefCell;
use std::rc::Rc;

use crate::cfg_value::WidgetHandle;
use crate::seams::{Component, WidgetBase};

/// Host seam for net.minecraft.client.gui.components.tabs.Tab.
pub trait Tab {
    fn get_tab_title(&self) -> Component;
    /// Used to load/unload children when switching tab
    fn visit_children(&self, consumer: &mut dyn FnMut(&WidgetHandle));
    fn do_layout(&self, tab_area_x: i32, tab_area_y: i32, tab_area_width: i32, tab_area_height: i32);
    fn get_tab_extra_narration(&self) -> Component;
    /// getLayout(): upstream returns null here.
    fn get_layout(&self) -> Option<()>;
}

/// Host seam for net.minecraft.client.gui.navigation.ScreenRectangle.
pub struct ScreenRectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct CustomTab {
    pub title: Component,
    pub children: RefCell<Vec<WidgetHandle>>,
    /// The AbstractWidget surface the tab's widgets share in the host screen.
    pub widget_base: WidgetBase,
}

impl CustomTab {
    pub fn new(title: Component, children: Vec<WidgetHandle>) -> Rc<RefCell<CustomTab>> {
        Rc::new(RefCell::new(CustomTab {
            title,
            children: RefCell::new(children),
            widget_base: WidgetBase::default(),
        }))
    }

    pub fn add_child(&self, child: WidgetHandle) {
        self.children.borrow_mut().push(child);
    }
}

impl Tab for CustomTab {
    fn get_tab_title(&self) -> Component {
        self.title.clone()
    }

    fn visit_children(&self, consumer: &mut dyn FnMut(&WidgetHandle)) {
        for child in self.children.borrow().iter() {
            consumer(child);
        }
    }

    fn do_layout(&self, _tab_area_x: i32, _tab_area_y: i32, _tab_area_width: i32, _tab_area_height: i32) {
        // Seems useless here
    }

    fn get_tab_extra_narration(&self) -> Component {
        Component::literal("Custom Tab Smooth Scrolling Settings")
    }

    fn get_layout(&self) -> Option<()> {
        crate::smooth_sc::SmoothSc::print("TRIED TO GET LAYOUT OF CUSTOMTAB, RETURNING NULL");
        None
    }
}
