// The box in its corner, as the extra drew it; the debug screen takes the top
// left.
fn fps_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::official_fps_overlay as m;
    // Turned off in Options > Mods: nothing until the next build leaves it out.
    if !game.native_on(m::ID) {
        return;
    }
    use horizon_client::extras::Corner;
    let settings = {
        let game = &*game;
        m::settings(|key| game.native_setting(m::ID, key))
    };
    let corner = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight][settings.corner];
    if !game.native_corner_shown(corner) {
        return;
    }
    let text = settings.text(game.native_frame_rate());
    let w = game.native_text_width(&text) + 4.0;
    let (x, y) = corners.place(corner, w, 12.0);
    game.native_fill(batch, [x, y, w, 12.0], m::BACKGROUND);
    game.native_text(batch, &text, [x + 2.0, y + 2.0], m::INK, true);
}
