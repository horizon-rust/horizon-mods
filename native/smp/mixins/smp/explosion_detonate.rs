// An explosion's blocks: claimed land is spared.
fn smp_explosion_detonate(
    game: &crate::game::Game<'_>,
    dimension: &str,
    blocks: &mut Vec<[i32; 3]>,
) {
    crate::game::smp_glue::explosion(game, dimension, blocks);
}
