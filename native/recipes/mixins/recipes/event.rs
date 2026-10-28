// The player's click, search or key on the panel, when it is this mod's.
fn official_recipes_event(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    event: &crate::game::native_hooks::NativePanelEvent,
) {
    use crate::game::official_recipes as m;
    if mod_id != m::ID || !game.native_on(m::ID) {
        return;
    }
    crate::game::official_recipes_glue::event(game, event);
}
