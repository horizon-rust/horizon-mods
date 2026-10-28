// Each frame: the item panel follows the world and the mod's on/off.
fn official_recipes_frame(game: &mut crate::game::Game<'_>) {
    crate::game::official_recipes_glue::frame(game);
}
