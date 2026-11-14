//! Upstream cfg/CfgValueBuilder.java, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use crate::cfg_value::{CfgValue, MapKey, Value};
use crate::seams::Component;

pub struct CfgValueBuilder {
    pub(crate) cfg_val: Rc<RefCell<CfgValue>>,
}

impl CfgValueBuilder {
    pub fn new(name: &str, default_val: Value) -> Self {
        Self {
            cfg_val: Rc::new(RefCell::new(CfgValue::new(name, default_val))),
        }
    }

    pub fn step(self, step: f64) -> Self {
        self.cfg_val.borrow().step.set(step);
        self
    }
    pub fn min_max(self, min: f32, max: f32) -> Self {
        let b = self.cfg_val.borrow();
        b.min_val.set(min);
        b.max_val.set(max);
        drop(b);
        self
    }
    pub fn tooltip(self, t: Component) -> Self {
        *self.cfg_val.borrow().tooltiptxt.borrow_mut() = Some(t);
        self
    }
    pub fn translatable(self, key: &str) -> Self {
        CfgValue::set_translation_key(&self.cfg_val, key);
        self
    }
    /// map(Object from, String to). Booleans map by Boolean key; numbers go in
    /// as Java doubles, which can never match a stored Float value on lookup.
    pub fn map(self, from: MapKey, to: &str) -> Self {
        self.cfg_val
            .borrow()
            .translation_map
            .borrow_mut()
            .insert(from, to.to_string());
        self
    }
    pub fn format(self, s: &str) -> Self {
        CfgValue::set_format_key(&self.cfg_val, s);
        self
    }
    pub fn disable_when(
        self,
        condition: impl Fn(&Rc<RefCell<CfgValue>>) -> bool + 'static,
    ) -> Self {
        *self.cfg_val.borrow().disable_when.borrow_mut() = Some(Rc::new(condition));
        self
    }

    pub fn build(self) -> Rc<RefCell<CfgValue>> {
        CfgValue::link_children(&self.cfg_val);
        self.cfg_val
    }
}

/// Convenience: the Java double literals the builder is called with.
pub fn map_f64(from: f64) -> MapKey {
    MapKey::D(from)
}
pub fn map_bool(from: bool) -> MapKey {
    MapKey::B(from)
}
