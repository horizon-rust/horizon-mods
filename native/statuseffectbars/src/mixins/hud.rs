//! `io.github.a5b84.statuseffectbars.mixin.HudMixin`, 1:1.
//! Renders the duration bars under status effects.

use crate::mob_effect_instance_duck::MobEffectInstanceHost;
use crate::status_effect_bar_renderer::{render, DeltaTrackerHost, GraphicsHost};
use crate::status_effect_bars;

/// `@Unique private static final int ICON_SIZE = 24;`
pub const ICON_SIZE: i32 = 24;

/// `onExtractEffects` (`extractEffects` at the first `getMobEffectSprite`
/// INVOKE, locals `instance`/`x`/`y`).
pub fn on_extract_effects(
    graphics: &dyn GraphicsHost,
    delta_tracker: Option<&dyn DeltaTrackerHost>,
    instance: &impl MobEffectInstanceHost,
    x: i32,
    y: i32,
) {
    // Copy the layout out of the config static: the Mutex behind the static is
    // not reentrant and render() reads the static again.
    let hud_layout = status_effect_bars::config().hud_layout.clone();
    render(
        graphics,
        delta_tracker,
        instance,
        x,
        y,
        ICON_SIZE,
        ICON_SIZE,
        &hud_layout,
    );
}
