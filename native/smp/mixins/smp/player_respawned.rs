// A player respawned: a newcomer or a player with no spawn point starts at a random spot.
fn smp_player_respawned(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::smp_glue::spawn_news(game, player, "respawn");
}
