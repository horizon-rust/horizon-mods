use super::widget::{Color, Rect, Widget};

#[derive(Debug, Clone)]
pub struct Button {
    pub bounds: Rect,
    pub text: String,
    pub visible: bool,
    pub enabled: bool,
    pub is_hovered: bool,
}

impl Button {
    pub fn new(x: f32, y: f32, width: f32, height: f32, text: impl Into<String>) -> Self {
        Self {
            bounds: Rect::new(x, y, width, height),
            text: text.into(),
            visible: true,
            enabled: true,
            is_hovered: false,
        }
    }
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn on_click(&mut self, mouse_x: f32, mouse_y: f32, button: i32) -> bool {
        if self.visible && self.enabled && self.bounds.contains(mouse_x, mouse_y) && button == 0 {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct Slider {
    pub bounds: Rect,
    pub label: String,
    pub min_value: f64,
    pub max_value: f64,
    pub current_value: f64,
    pub visible: bool,
    pub enabled: bool,
    pub is_dragging: bool,
}

impl Slider {
    pub fn new(x: f32, y: f32, width: f32, height: f32, label: impl Into<String>, min: f64, max: f64, current: f64) -> Self {
        Self {
            bounds: Rect::new(x, y, width, height),
            label: label.into(),
            min_value: min,
            max_value: max,
            current_value: current.clamp(min, max),
            visible: true,
            enabled: true,
            is_dragging: false,
        }
    }

    pub fn set_value_from_mouse_x(&mut self, mouse_x: f32) {
        let ratio = ((mouse_x - self.bounds.x) / self.bounds.width).clamp(0.0, 1.0) as f64;
        self.current_value = self.min_value + ratio * (self.max_value - self.min_value);
    }
}

impl Widget for Slider {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn on_click(&mut self, mouse_x: f32, mouse_y: f32, button: i32) -> bool {
        if self.visible && self.enabled && self.bounds.contains(mouse_x, mouse_y) && button == 0 {
            self.is_dragging = true;
            self.set_value_from_mouse_x(mouse_x);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextField {
    pub bounds: Rect,
    pub text: String,
    pub placeholder: String,
    pub visible: bool,
    pub enabled: bool,
    pub is_focused: bool,
    pub cursor_pos: usize,
}

impl TextField {
    pub fn new(x: f32, y: f32, width: f32, height: f32, placeholder: impl Into<String>) -> Self {
        Self {
            bounds: Rect::new(x, y, width, height),
            text: String::new(),
            placeholder: placeholder.into(),
            visible: true,
            enabled: true,
            is_focused: false,
            cursor_pos: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.cursor_pos <= self.text.len() {
            self.text.insert(self.cursor_pos, c);
            self.cursor_pos += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 && self.cursor_pos <= self.text.len() {
            self.cursor_pos -= 1;
            self.text.remove(self.cursor_pos);
        }
    }
}

impl Widget for TextField {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn on_click(&mut self, mouse_x: f32, mouse_y: f32, button: i32) -> bool {
        if self.visible && self.enabled && button == 0 {
            self.is_focused = self.bounds.contains(mouse_x, mouse_y);
            self.is_focused
        } else {
            false
        }
    }

    fn on_key(&mut self, key: &str, pressed: bool) -> bool {
        if !self.is_focused || !pressed {
            return false;
        }
        match key {
            "BACKSPACE" => {
                self.backspace();
                true
            }
            "LEFT" => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                true
            }
            "RIGHT" => {
                if self.cursor_pos < self.text.len() {
                    self.cursor_pos += 1;
                }
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub bounds: Rect,
    pub text: String,
    pub color: Color,
    pub visible: bool,
}

impl Label {
    pub fn new(x: f32, y: f32, text: impl Into<String>, color: Color) -> Self {
        Self {
            bounds: Rect::new(x, y, 100.0, 14.0),
            text: text.into(),
            color,
            visible: true,
        }
    }
}

impl Widget for Label {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn is_enabled(&self) -> bool {
        false
    }

    fn set_enabled(&mut self, _enabled: bool) {}
}
