use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_hex(hex: u32) -> Self {
        Self {
            a: ((hex >> 24) & 0xFF) as u8,
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    pub fn to_hex(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub const WHITE: Color = Color::rgba(255, 255, 255, 255);
    pub const BLACK: Color = Color::rgba(0, 0, 0, 255);
    pub const RED: Color = Color::rgba(255, 0, 0, 255);
    pub const GREEN: Color = Color::rgba(0, 255, 0, 255);
    pub const BLUE: Color = Color::rgba(0, 120, 255, 255);
    pub const YELLOW: Color = Color::rgba(255, 255, 0, 255);
    pub const CYAN: Color = Color::rgba(0, 255, 255, 255);
    pub const MAGENTA: Color = Color::rgba(255, 0, 255, 255);
}

pub trait Widget: std::fmt::Debug {
    fn bounds(&self) -> Rect;
    fn set_bounds(&mut self, bounds: Rect);
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    fn on_click(&mut self, mouse_x: f32, mouse_y: f32, button: i32) -> bool {
        let _ = (mouse_x, mouse_y, button);
        false
    }

    fn on_key(&mut self, key: &str, pressed: bool) -> bool {
        let _ = (key, pressed);
        false
    }
}
