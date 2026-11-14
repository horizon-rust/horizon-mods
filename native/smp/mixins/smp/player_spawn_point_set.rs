// A spawn point is set (a bed, an anchor, a command) or reset: a player with one never starts at a random spot.
fn smp_player_spawn_point_set(game: &mut crate::game::Game<'_>, player: u128, cause: &str) {
    let kind: &'static str = if cause == "death_reset" { "death_reset" } else { "set" };
    crate::game::smp_glue::spawn_news(game, player, kind);
}
