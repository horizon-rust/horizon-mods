// A living entity is about to be hurt: a blast spares a claim's animals.
fn smp_living_incoming_damage(entity: i32, damage_type: &str, canceled: &mut bool) {
    crate::game::smp_claims_glue::damage(entity, damage_type, canceled);
}
