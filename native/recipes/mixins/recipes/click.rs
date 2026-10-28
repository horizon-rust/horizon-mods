// A click on the mod's settings page: "Click gives items" flips, kept where
// the WebAssembly mod kept it.
fn official_recipes_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::official_recipes as m;
    if mod_id != m::ID || button != 0 {
        return;
    }
    let give = !m::gives(game.native_setting(m::ID, "give"));
    game.native_set_setting(m::ID, "give", if give { "true" } else { "false" });
}
