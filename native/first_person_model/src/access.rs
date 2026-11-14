//! Upstream access/*, 1:1: the duck interfaces the mixins inject into engine
//! types, plus the concrete renderer state the seams hand to them.

use std::cell::RefCell;
use std::rc::Rc;

/// access/AgeableListModelAccess.java.
pub trait AgeableListModelAccess {
    fn first_person_head_parts_getter(&self) -> Vec<Rc<RefCell<crate::seams::ModelPart>>>;
}

/// access/LivingEntityRenderStateAccess.java.
pub trait LivingEntityRenderStateAccess {
    fn set_is_camera_entity(&mut self, value: bool);
    fn is_camera_entity(&self) -> bool;

    /// The default method upstream declares.
    fn set_hide_arms(&mut self, value: bool) {
        self.set_hide_left_arm(value);
        self.set_hide_right_arm(value);
    }

    fn set_hide_right_arm(&mut self, value: bool);
    fn set_hide_left_arm(&mut self, value: bool);
    fn hide_left_arm(&self) -> bool;
    fn hide_right_arm(&self) -> bool;
    fn set_hide_body(&mut self, value: bool);
    fn hide_body(&self) -> bool;
    fn set_arm_offset(&mut self, offset: f32);
    fn get_arm_offset(&self) -> f32;
}

/// access/PlayerModelAccess.java: the < 1.21.3 cloak getter is the whole
/// interface; the carried >= 1.21.3 variant keeps it empty.
pub trait PlayerModelAccess {}

/// access/PlayerRendererAccess.java.
pub trait PlayerRendererAccessTrait {
    fn get_render_layers(&self) -> Vec<String>;
    fn update_parts_list(&self, thirdperson: bool);
}

/// The concrete renderer state behind PlayerRendererAccessImpl: the render
/// layer list and the parts visibility the mixins drive.
#[derive(Default)]
pub struct PlayerRendererAccessImpl {
    pub render_layers: RefCell<Vec<String>>,
    /// updatePartsList state: whether parts are hidden for third person.
    pub thirdperson: std::cell::Cell<bool>,
}

impl PlayerRendererAccessTrait for PlayerRendererAccessImpl {
    fn get_render_layers(&self) -> Vec<String> {
        self.render_layers.borrow().clone()
    }

    fn update_parts_list(&self, thirdperson: bool) {
        self.thirdperson.set(thirdperson);
    }
}
