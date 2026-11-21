//! Chat colours and the few builders the SMP's lines use, over the API's `Text`.

pub use mod_api::Text;

pub const RED: &str = "red";
pub const GOLD: &str = "gold";
pub const GREEN: &str = "green";
pub const GRAY: &str = "gray";
pub const WHITE: &str = "white";
pub const DARK_GRAY: &str = "dark_gray";
pub const AQUA: &str = "aqua";
pub const YELLOW: &str = "yellow";

pub trait TextExt {
    fn with_color(self, color: &str) -> Text;
    fn bold(self) -> Text;
}

impl TextExt for Text {
    fn with_color(mut self, color: &str) -> Text {
        self.style.color = Some(color.to_owned());
        self
    }
    fn bold(mut self) -> Text {
        self.style.bold = Some(true);
        self
    }
}
