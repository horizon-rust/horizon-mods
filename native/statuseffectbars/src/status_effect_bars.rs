//! `io.github.a5b84.statuseffectbars.StatusEffectBars`, 1:1.

use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::MutexGuard;

use crate::config::status_effect_bars_config::StatusEffectBarsConfig;

pub const MOD_ID: &str = "status-effect-bars";

/// `StatusEffectBars.config` (set by `onInitializeClient`).
static CONFIG: Mutex<Option<StatusEffectBarsConfig>> = Mutex::new(None);

/// `onInitializeClient`:
/// `AutoConfig.register(StatusEffectBarsConfig.class, GsonConfigSerializer::new);
///  config = AutoConfig.getConfigHolder(StatusEffectBarsConfig.class).getConfig();`
pub fn on_initialize_client() {
    let loaded = StatusEffectBarsConfig::load_or_default();
    *CONFIG.lock().unwrap() = Some(loaded);
}

/// Live handle over the `config` static, like Java's shared reference.
pub fn config() -> ConfigRef {
    ConfigRef(CONFIG.lock().unwrap())
}

pub struct ConfigRef(MutexGuard<'static, Option<StatusEffectBarsConfig>>);

impl Deref for ConfigRef {
    type Target = StatusEffectBarsConfig;

    fn deref(&self) -> &StatusEffectBarsConfig {
        self.0.as_ref().expect("StatusEffectBars.config not initialized")
    }
}

impl DerefMut for ConfigRef {
    fn deref_mut(&mut self) -> &mut StatusEffectBarsConfig {
        self.0.as_mut().expect("StatusEffectBars.config not initialized")
    }
}

/// Resets the static for tests.
pub fn reset_vars_for_tests() {
    *CONFIG.lock().unwrap() = None;
}
