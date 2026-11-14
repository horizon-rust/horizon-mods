//! `io.github.a5b84.statuseffectbars.StatusEffectBarRenderer`, 1:1.

use crate::config::status_effect_bars_config::LayoutConfig;
use crate::mth;
use crate::mob_effect_instance_duck::MobEffectInstanceHost;
use crate::status_effect_bars;

/// `net.minecraft.client.gui.GuiGraphicsExtractor` host surface (the one call
/// the renderer makes).
pub trait GraphicsHost {
    fn fill(&self, min_x: i32, min_y: i32, max_x: i32, max_y: i32, color: i32);
}

/// `net.minecraft.client.DeltaTracker` host surface.
pub trait DeltaTrackerHost {
    /// `getGameTimeDeltaPartialTick(false)`.
    fn get_game_time_delta_partial_tick(&self, partially_running_tick: bool) -> f32;
}

/// 1:1 `render` (the `@SuppressWarnings("SuspiciousNameCombination")`
/// width/height swap included).
#[allow(clippy::too_many_arguments)]
pub fn render(
    graphics: &dyn GraphicsHost,
    delta_tracker: Option<&dyn DeltaTrackerHost>,
    effect: &impl MobEffectInstanceHost,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    layout_config: &LayoutConfig,
) {
    // Special cases where the bar is hidden

    if !layout_config.enabled {
        return;
    }

    if effect.get_duration() > status_effect_bars::config().max_remaining_duration
        || effect.is_infinite_duration()
    {
        return; // Too much time remaining
    }

    let age = effect.status_effect_bars_get_max_duration() - effect.get_duration();
    if effect.is_ambient() && age < status_effect_bars::config().min_ambient_age {
        return; // Beacon effect too recent (will probably be refreshed soon)
    }

    let (width, height) = if layout_config.direction.swap_xy() {
        (height, width)
    } else {
        (width, height)
    };

    // start--------+-----end
    // |            |       |
    // +---------middle-----+
    // or
    // start----+
    // |        |
    // |        |
    // +---middle
    // |        |
    // end------+

    let (sx, ex) = (
        layout_config.collinear_offset + layout_config.collinear_padding,
        layout_config.collinear_offset + width - layout_config.collinear_padding,
    );
    let (start_x, end_x) = if layout_config.direction.reverse_axis() {
        (ex, sx)
    } else {
        (sx, ex)
    };

    let tick_delta = match delta_tracker {
        Some(delta_tracker) => delta_tracker.get_game_time_delta_partial_tick(false),
        None => 0.0,
    };
    let progress = (effect.get_duration() as f32 - tick_delta)
        / effect.status_effect_bars_get_max_duration() as f32;
    let middle_x = mth::lerp_int(progress, start_x, end_x);

    let mut start_y = layout_config.orthogonal_offset;
    if layout_config.relative_to_end {
        start_y = height - (start_y + layout_config.thickness);
    }
    let middle_y = start_y + layout_config.thickness;
    let end_y = start_y;

    let (mut start_x, mut middle_x, mut end_x) = (start_x, middle_x, end_x);
    let (mut start_y, mut middle_y, mut end_y) = (start_y, middle_y, end_y);

    if layout_config.direction.swap_xy() {
        // Swapping X and Y to make the bar vertical instead of horizontal
        let mut tmp;

        tmp = start_x;
        start_x = start_y;
        start_y = tmp;

        tmp = middle_x;
        middle_x = middle_y;
        middle_y = tmp;

        tmp = end_x;
        end_x = end_y;
        end_y = tmp;
    }

    start_x += x;
    middle_x += x;
    end_x += x;
    start_y += y;
    middle_y += y;
    end_y += y;

    let config = status_effect_bars::config();
    graphics.fill(start_x, start_y, middle_x, middle_y, config.get_color(effect));
    graphics.fill(middle_x, middle_y, end_x, end_y, config.background_color);
}
