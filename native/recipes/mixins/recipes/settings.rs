// The mod's settings page: the WebAssembly mod's one button.
fn official_recipes_page(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    use crate::game::official_recipes as m;
    if mod_id != m::ID {
        return;
    }
    buttons.push(m::give_label(m::gives(game.native_setting(m::ID, "give"))));
}
