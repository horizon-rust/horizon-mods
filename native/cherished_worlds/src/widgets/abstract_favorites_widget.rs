//! 1:1 port of client/favorites/AbstractFavoritesWidget.

use crate::constants::MOD_ID;
use crate::seams::{Component, Screen, WidgetBase};

// java: public static final Identifier FAVORITED_ICON = Identifier
//           .fromNamespaceAndPath(MOD_ID, "textures/gui/favorite.png");
pub fn favorited_icon() -> String {
    format!("{MOD_ID}:textures/gui/favorite.png")
}

pub fn empty_icon() -> String {
    format!("{MOD_ID}:textures/gui/empty.png")
}

pub fn hovered_favorited_icon() -> String {
    format!("{MOD_ID}:textures/gui/hovered_favorite.png")
}

pub fn hovered_empty_icon() -> String {
    format!("{MOD_ID}:textures/gui/hovered_empty.png")
}

pub fn focused_outline() -> String {
    format!("{MOD_ID}:textures/gui/focused_outline.png")
}

// java: public static final int TEXTURE_SIZE = 9;
pub const TEXTURE_SIZE: i32 = 9;

// java: public abstract class AbstractFavoritesWidget extends AbstractWidget
pub struct AbstractFavoritesWidget {
    pub base: WidgetBase,
    // java: protected final Screen parentScreen;
    pub parent_screen: Screen,
}

impl AbstractFavoritesWidget {
    // java: public AbstractFavoritesWidget(Screen parentScreen, int width, int height)
    //       { super(0, 0, width, height, Component.empty());
    //         this.parentScreen = parentScreen; }
    pub fn new(parent_screen: Screen, width: i32, height: i32) -> AbstractFavoritesWidget {
        AbstractFavoritesWidget {
            base: WidgetBase::new(0, 0, width, height),
            parent_screen,
        }
    }

    // java: public AbstractFavoritesWidget(Screen parentScreen)
    //       { this(parentScreen, 0, 0); }
    pub fn with_screen(parent_screen: Screen) -> AbstractFavoritesWidget {
        Self::new(parent_screen, 0, 0)
    }

    // java: protected boolean areCoordinatesInIcon(int iconX, int iconY,
    //           int height, double x, double y)
    pub fn are_coordinates_in_icon(
        icon_x: i32,
        icon_y: i32,
        height: i32,
        x: f64,
        y: f64,
    ) -> bool {
        x >= icon_x as f64
            && y >= icon_y as f64
            && x < (icon_x + TEXTURE_SIZE) as f64
            && y < (icon_y + height) as f64
    }

    // Test seam mirroring the Component.empty() base-class message.
    pub fn base_message() -> Component {
        Component::empty()
    }
}
