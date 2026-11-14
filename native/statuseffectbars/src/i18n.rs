//! Stand-in for `net.minecraft.client.resources.language.I18n` (host surface).

use std::sync::RwLock;

type Provider = fn(&str) -> String;

static PROVIDER: RwLock<Option<Provider>> = RwLock::new(None);

/// `I18n.get(key)`. The carrier maps this onto the engine's I18n; the
/// standalone port falls back to the key itself.
pub fn get(key: &str) -> String {
    let provider = PROVIDER.read().unwrap();
    match *provider {
        Some(provider) => provider(key),
        None => key.to_string(),
    }
}

pub fn set_provider_for_tests(provider: Provider) {
    *PROVIDER.write().unwrap() = Some(provider);
}

pub fn clear_provider_for_tests() {
    *PROVIDER.write().unwrap() = None;
}
