// The end of each server tick: the SMP's accounts and files.
fn smp_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    if matches!(
        phase,
        crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { .. }
    ) {
        crate::game::smp_glue::tick(game);
    }
}
