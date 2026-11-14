//! Upstream menu/CustomSlider.java, 1:1.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::cfg_value::{CfgValue, Value, WidgetHandle};
use crate::seams::{Component, SliderBase};

pub struct CustomSlider {
    /// AbstractSliderButton superclass state.
    pub base: SliderBase,
    pub(crate) txt: RefCell<String>,
    /// The CfgValue this slider edits (a weak ref; the value holds the widget).
    pub(crate) entry: RefCell<Weak<RefCell<CfgValue>>>,
}

impl CustomSlider {
    pub fn new(cfg_value: &Rc<RefCell<CfgValue>>) -> Rc<RefCell<CustomSlider>> {
        // super(0, 0, 150, 20, Component.literal(""), 0)
        let slider = Rc::new(RefCell::new(CustomSlider {
            base: SliderBase::new(0, 0, 150, 20, Component::literal(""), 0.0),
            txt: RefCell::new(String::new()),
            entry: RefCell::new(Rc::downgrade(cfg_value)),
        }));
        CfgValue::reset_temp_value(cfg_value);
        let value = match CfgValue::get_value(cfg_value) {
            Value::Num(v) => v as f64,
            _ => 0.0,
        };
        slider.borrow().base.value.set(CfgValue::de_min_max(cfg_value, value));
        *slider.borrow().txt.borrow_mut() = CfgValue::get_name(cfg_value);
        slider.borrow().update_message();
        slider
    }

    pub fn refresh_value(&self) {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let temp = match CfgValue::get_temp_value(&entry) {
            Value::Num(v) => v as f64,
            _ => 0.0,
        };
        self.base.value.set(CfgValue::de_min_max(&entry, temp));
        self.update_message();
    }

    fn make_text(&self) -> Component {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let display = CfgValue::get_display_name(&entry);
        let display_text = display.text();
        let shown = CfgValue::try_translate(
            &entry,
            &Value::Num(CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get())) as f32),
        );
        Component::translatable(
            &CfgValue::get_format_key(&entry),
            &[&display_text, &shown.text()],
        )
    }

    /// applyValue(): on a drag, write the stepped value back.
    pub fn apply_value(&self) {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let stepped = CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get()));
        CfgValue::set_temp_value(&entry, Value::Num(stepped as f32));
        self.base
            .value
            .set(CfgValue::de_min_max(&entry, CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get()))));
    }

    pub fn update_message(&self) {
        let text = self.make_text();
        self.base.set_message(text);
    }

    /// AbstractWidget.active, used by refreshDisableRecursive.
    pub fn set_active(slider: &Rc<RefCell<CustomSlider>>, active: bool) {
        slider.borrow().base.base.active.set(active);
    }
}

/// The AbstractWidget surface CfgValue sees on the slider handle.
pub fn slider_widget(slider: &Rc<RefCell<CustomSlider>>) -> WidgetHandle {
    let _ = Cell::new(0u8);
    WidgetHandle::Slider(slider.clone())
}
