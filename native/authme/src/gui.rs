
//! Exact 1:1 of me.axieum.mcmod.authme.api.gui.screen.AuthScreen: the
//! abstract base for authentication screens. The Minecraft Screen, widget
//! and toast APIs are host seams with the same shapes; the carrier's GUI
//! wiring drives the same init/tick/onClose sequence.

/// Which screen navigation shows next (Java: Screen references; the port
/// carries the screen kinds the mod opens).
#[derive(Debug, Clone, PartialEq)]
pub enum NextScreen {
    /// The screen to be returned to after a successful login.
    SuccessScreen,
    /// The parent (or last) screen that opened this screen.
    ParentScreen,
    /// The authentication method selection screen.
    AuthMethodScreen,
    /// The Microsoft login screen with its `selectAccount` flag.
    MicrosoftAuthScreen { select_account: bool },
    /// The offline login screen.
    OfflineAuthScreen,
    /// The Microsoft URL warning confirm screen (setDelay(40)).
    ConfirmMicrosoftWarning,
    /// The Mojang migration FAQ confirm-link screen.
    ConfirmLinkMojang,
}

/// AuthScreen base state: parent, success screen, and the success flags.
#[derive(Debug, Clone)]
pub struct AuthScreenBase {
    pub parent_screen: NextScreen,
    pub success_screen: NextScreen,
    /// True if the login task completed successfully.
    pub success: bool,
    /// True if the screen should be closed on success state (via render thread).
    pub close_on_success: bool,
}

impl AuthScreenBase {
    /// Java: `AuthScreen(title, parentScreen, successScreen)` (success and
    /// closeOnSuccess default to false).
    pub fn new(parent_screen: NextScreen, success_screen: NextScreen) -> Self {
        AuthScreenBase { parent_screen, success_screen, success: false, close_on_success: false }
    }

    /// Java: `tick()` - optionally close the screen if the login task
    /// completed successfully. Returns Some(next) when the screen closes.
    pub fn tick(&mut self) -> Option<NextScreen> {
        if self.success && self.close_on_success {
            return Some(self.on_close());
        }
        None
    }

    /// Java: `onClose()` - shows the success screen on success, else parent.
    pub fn on_close(&self) -> NextScreen {
        if self.success { self.success_screen.clone() } else { self.parent_screen.clone() }
    }

    /// Java: `centerPosition(element, screen, xOffset, yOffset)` - centers
    /// the position of a LayoutElement, shifted vertically 22 up.
    pub fn center_position(el_w: i32, el_h: i32, screen_w: i32, screen_h: i32, x_offset: i32, y_offset: i32) -> (i32, i32) {
        (
            screen_w / 2 - el_w / 2 + x_offset,
            screen_h / 2 - el_h / 2 - 22 + y_offset,
        )
    }
}

/// The widget metrics the layout code needs (Java: LayoutElement).
#[derive(Debug, Clone)]
pub struct LayoutElement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl LayoutElement {
    pub fn new(width: i32, height: i32) -> Self {
        LayoutElement { x: 0, y: 0, width, height }
    }

    /// Applies AuthScreen#centerPosition for a screen of the given size.
    pub fn center_in(&mut self, screen_w: i32, screen_h: i32, x_offset: i32, y_offset: i32) {
        let (x, y) = AuthScreenBase::center_position(self.width, self.height, screen_w, screen_h, x_offset, y_offset);
        self.x = x;
        self.y = y;
    }
}

/// A button's bounds (Java: Button.builder(...).bounds(x, y, w, h)).
#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Bounds { x, y, width, height }
    }
}
