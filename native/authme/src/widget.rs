
//! Exact 1:1 of me.axieum.mcmod.authme.api.gui.widget.AuthButtonWidget:
//! the textured button for opening the authentication screens, with
//! click-on-release handling, drag movement clamped to the screen, and the
//! session-status icon. The Minecraft rendering/mouse APIs are host seams.

use crate::screens::session_status_icon_u;
use crate::session_utils::{get_status, SessionStatus};

/// A functional interface for widget relocation callback (Java: MoveAction).
pub type MoveAction = Box<dyn Fn(&AuthButtonWidgetLogic) + Send + Sync>;

/// What the button's press action opens (Java: OnPress, wired by the
/// carrier through AuthMethodScreen/AuthButtonWidget use sites).
pub type PressAction = Box<dyn Fn() + Send + Sync>;

/// Java: ImageButton base: a 20x20 button at (x, y) with a press action.
pub struct AuthButtonWidgetLogic {
    /// Java: field x / getY().
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// The screen used to constrain the button movements to (Java: Screen).
    pub screen: Option<(i32, i32)>,
    /// Callback for after the button has been dragged.
    pub move_action: Option<MoveAction>,
    /// True if the button was just dragged.
    pub did_drag: bool,
    /// The last known status of the Minecraft session.
    pub session_status: SessionStatus,
    /// Whether the button accepts left clicks (Java: isValidClickButton).
    pub active: bool,
}

impl AuthButtonWidgetLogic {
    /// Java: `AuthButtonWidget(x, y, pressAction)` - a fixed (no drag)
    /// authentication button; the press action opens the auth screens.
    pub fn fixed(x: i32, y: i32) -> Self {
        AuthButtonWidgetLogic::movable(None, x, y, None)
    }

    /// Java: `AuthButtonWidget(screen, x, y, pressAction, moveAction)` - a
    /// movable (with drag) authentication button.
    pub fn movable(screen: Option<(i32, i32)>, x: i32, y: i32, move_action: Option<MoveAction>) -> Self {
        let mut w = AuthButtonWidgetLogic {
            x, y, width: 20, height: 20,
            screen,
            move_action,
            did_drag: false,
            session_status: SessionStatus::UNKNOWN,
            active: true,
        };
        w.refresh_session_status();
        w
    }

    /// Java: `mouseClicked(event, doubleClick)` - with a move action, the
    /// default behaviour is adjusted to NOT trigger a click (dragging starts
    /// on mouse down; the click fires on mouse up/release).
    pub fn mouse_clicked(&self) -> bool {
        if self.move_action.is_some() {
            return self.active; // Java: isValidClickButton(buttonInfo)
        }
        true // Java: super.mouseClicked(event, doubleClick)
    }

    /// Java: `mouseReleased(event)` - with a move action, the click triggers
    /// on mouse up: a finished drag invokes the move callback, otherwise it
    /// continues as a button press.
    pub fn mouse_released(&mut self) -> WidgetClick {
        if self.move_action.is_none() {
            return WidgetClick::Press; // Java: super.mouseReleased(event)
        }
        if !self.active {
            return WidgetClick::None; // Java: !isValidClickButton -> false
        }
        // Check if the user just finished dragging
        if self.did_drag {
            // Invoke any callbacks and reset the dragging status
            let cb = self.move_action.as_ref().unwrap();
            cb(self);
            self.did_drag = false;
            return WidgetClick::Moved;
        }
        // Else, continue as a button press
        WidgetClick::Press
    }

    /// Java: `onDrag(event, deltaX, deltaY)` - moves the button with the
    /// drag, constraining within the screen's bounds.
    pub fn on_drag(&mut self, event_x: f64, event_y: f64) {
        if self.move_action.is_some() {
            // Flag the widget as in a dragging state
            self.did_drag = true;
            // Move the button with the drag, constraining within the screen's bounds
            match self.screen {
                Some((sw, sh)) => {
                    self.set_position(
                        (event_x as i32 - self.width / 2).clamp(0, sw - self.width),
                        (event_y as i32 - self.height / 2).clamp(0, sh - self.height),
                    );
                }
                None => {
                    self.set_position(event_x as i32 - self.width / 2, event_y as i32 - self.height / 2);
                }
            }
        }
    }

    /// Java: `setPosition(x, y)`.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Java: `getX()`, `getY()` through the public fields.

    /// Java: `getSessionStatus()`.
    pub fn get_session_status(&self) -> SessionStatus {
        self.session_status
    }

    /// Java: `refreshSessionStatus()` - resets to UNKNOWN and adopts the
    /// status the in-flight status check resolves to.
    pub fn refresh_session_status(&mut self) {
        self.session_status = SessionStatus::UNKNOWN;
        self.session_status = get_status().join();
    }

    /// Java: `setSessionStatus(sessionStatus)`.
    pub fn set_session_status(&mut self, session_status: SessionStatus) {
        self.session_status = session_status;
    }

    /// Java: `extractContents(...)` after the cascade - renders the current
    /// session status icon: blit(SESSION_STATUS_TEXTURE,
    /// getX() + width - 6, getY() - 1, u, 0, 8, 8, 24, 8).
    pub fn session_status_blit(&self) -> (i32, i32, i32) {
        (self.x + self.width - 6, self.y - 1, session_status_icon_u(self.session_status))
    }
}

/// The outcome of a widget click sequence (Java: which path ran).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetClick {
    /// Java: the press action fires.
    Press,
    /// Java: the move callback fired and dragging reset.
    Moved,
    /// Java: neither.
    None,
}

/// The textures this widget renders (Java: BUTTON_TEXTURES /
/// SESSION_STATUS_TEXTURE), re-exported for the carrier.
pub use crate::screens::{BUTTON_TEXTURES as AUTH_BUTTON_TEXTURES, SESSION_STATUS_TEXTURE as AUTH_SESSION_STATUS_TEXTURE};
