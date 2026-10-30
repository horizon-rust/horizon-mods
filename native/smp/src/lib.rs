//! The SMP's rules, as the bundle adds them to the game: each file is a
//! module of `game` there and of this crate here, under the same name, so
//! `super::smp_store` reaches the same code in both.
#[path = "../mixins/smp/smp_economy.rs"]
pub mod smp_economy;
#[path = "../mixins/smp/smp_market.rs"]
pub mod smp_market;
#[path = "../mixins/smp/smp_store.rs"]
pub mod smp_store;
