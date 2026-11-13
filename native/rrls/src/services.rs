/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `services/RrlsReloaderService.java`.

use crate::rrls::PreparableReloadListener;
use crate::rrls::ResourceManager;

/// Upstream: `org.redlance.common.services.AdvancedService`. The shared
/// library is not part of this port; the marker trait keeps the contract
/// (`isServiceActive` is its only member upstream).
pub trait AdvancedService {
    fn is_service_active(&self) -> bool;
}

/// Upstream: `RrlsReloaderService` interface. `collectEarlyReloaders` runs
/// before the game's own reload listener collection; `ResourceManager` and
/// the consumer are the host seam (Horizon supplies both at the call site).
pub trait RrlsReloaderService: AdvancedService + Send + Sync {
    fn collect_early_reloaders(
        &self,
        manager: &ResourceManager,
        consumer: &mut dyn FnMut(PreparableReloadListener),
    );
}

/// Upstream: `ServiceUtils.loadService(RrlsReloaderService.class, () -> new
/// RrlsReloaderService() { ... })`. Java walks the ServiceLoader first and
/// falls back to the anonymous no-op below; here the host provider seam is
/// consulted at first use and the same fallback is the default.
pub struct DefaultRrlsReloaderService;

impl AdvancedService for DefaultRrlsReloaderService {
    fn is_service_active(&self) -> bool {
        true
    }
}

impl RrlsReloaderService for DefaultRrlsReloaderService {
    fn collect_early_reloaders(
        &self,
        _manager: &ResourceManager,
        _consumer: &mut dyn FnMut(PreparableReloadListener),
    ) {
        // no-op
    }
}

pub struct RrlsReloaderServiceHandle {
    provider: Option<&'static dyn RrlsReloaderService>,
}

impl RrlsReloaderServiceHandle {
    pub const fn new() -> Self {
        Self { provider: None }
    }

    /// `ServiceUtils.loadService`: a host-registered provider wins; otherwise
    /// the anonymous default impl above serves.
    pub fn instance(&mut self) -> &'static dyn RrlsReloaderService {
        if self.provider.is_none() {
            self.provider = crate::rrls::service_provider::<dyn RrlsReloaderService>()
                .or(Some(&DefaultRrlsReloaderService));
        }
        self.provider.unwrap()
    }
}

static INSTANCE: std::sync::Mutex<RrlsReloaderServiceHandle> =
    std::sync::Mutex::new(RrlsReloaderServiceHandle::new());

/// Upstream: `RrlsReloaderService.INSTANCE`.
pub fn instance() -> &'static dyn RrlsReloaderService {
    INSTANCE.lock().expect("rrls reloader service").instance()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn manager() -> ResourceManager {
        ResourceManager
    }

    struct Counting {
        collected: AtomicU32,
    }

    impl AdvancedService for Counting {
        fn is_service_active(&self) -> bool {
            true
        }
    }

    impl RrlsReloaderService for Counting {
        fn collect_early_reloaders(
            &self,
            _manager: &ResourceManager,
            consumer: &mut dyn FnMut(PreparableReloadListener),
        ) {
            for _ in 0..2 {
                consumer(PreparableReloadListener);
            }
            self.collected.fetch_add(2, Ordering::SeqCst);
        }
    }

    #[test]
    fn default_service_is_active_and_noop() {
        assert!(DefaultRrlsReloaderService.is_service_active());
        let mut collected = 0usize;
        DefaultRrlsReloaderService.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 0);
    }

    #[test]
    fn instance_falls_back_to_default_impl() {
        // No host provider registered in tests: upstream's fallback wins.
        let inst = instance();
        let mut collected = 0usize;
        inst.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 0);
        assert!(inst.is_service_active());
    }

    #[test]
    fn host_provider_receives_the_collection_call() {
        let counting = Counting { collected: AtomicU32::new(0) };
        let mut collected = 0usize;
        counting.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 2);
        assert_eq!(counting.collected.load(Ordering::SeqCst), 2);
        assert!(counting.is_service_active());
    }
}
