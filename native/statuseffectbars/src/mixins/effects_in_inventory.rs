//! `io.github.a5b84.statuseffectbars.mixin.EffectsInInventoryMixin`, 1:1.
//! Renders the duration bars under status effects.

use crate::mob_effect_instance_duck::MobEffectInstanceHost;
use crate::status_effect_bar_renderer::{render, GraphicsHost};
use crate::status_effect_bars;

/// `net.minecraft.client.gui.screens.inventory.EffectsInInventory` host
/// surface; `SPRITE_SQUARE_SIZE` comes from the engine at splice time.
pub trait EffectsInInventoryHost {
    const SPRITE_SQUARE_SIZE: i32;
}

/// `onDrawStatusEffectBackground` (`extractEffects` at the `extractText`
/// INVOKE, locals `y0`/`textureWidth`/`effect`).
pub fn on_draw_status_effect_background<E: EffectsInInventoryHost>(
    graphics: &dyn GraphicsHost,
    effect: &impl MobEffectInstanceHost,
    x0: i32,
    y0: i32,
    texture_width: i32,
) {
    // Copy the layout out of the config static: the Mutex behind the static is
    // not reentrant and render() reads the static again.
    let inventory_layout = status_effect_bars::config().inventory_layout.clone();
    render(
        graphics,
        None,
        effect,
        x0,
        y0,
        texture_width,
        E::SPRITE_SQUARE_SIZE,
        &inventory_layout,
    );
}
