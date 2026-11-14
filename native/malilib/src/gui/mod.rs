pub mod widget;
pub mod widgets;

pub use widget::{Color, Rect, Widget};
pub use widgets::{Button, Slider, TextField, Label};

pub trait GuiScreen: std::fmt::Debug {
    fn title(&self) -> &str;
    fn widgets(&self) -> &[Box<dyn Widget>];
    fn widgets_mut(&mut self) -> &mut [Box<dyn Widget>];

    fn handle_mouse_click(&mut self, mouse_x: f32, mouse_y: f32, button: i32) -> bool {
        for w in self.widgets_mut() {
            if w.on_click(mouse_x, mouse_y, button) {
                return true;
            }
        }
        false
    }

    fn handle_key_press(&mut self, key: &str, pressed: bool) -> bool {
        for w in self.widgets_mut() {
            if w.on_key(key, pressed) {
                return true;
            }
        }
        false
    }
}
