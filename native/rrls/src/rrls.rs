/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Rrls.java`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const MOD_ID: &str = "rrls";

/// Upstream: `LogManager.getLogger(MOD_ID)`. Horizon has no log4j; the same
/// logger name is kept and emission is the host seam.
pub const LOGGER: &str = MOD_ID;

/// Upstream: `AtomicReference<CompletableFuture<Void>> MOD_RELOADERS_FUTURE`.
/// The Java type pairs a slot with a completable future; the same
/// set-and-swap semantics live in one lock here (`STATE_*` hold the payload,
/// `DONE` marks the future completed).
pub struct ReloadersFuture {
    slot: Mutex<Option<SharedState>>,
    epoch: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedState {
    Pending,
    Done,
}

impl ReloadersFuture {
    pub const fn new() -> Self {
        Self { slot: Mutex::new(None), epoch: AtomicU64::new(0) }
    }

    /// `compareAndSet(expect, fresh future)`: swaps in a new pending future
    /// only when the slot still holds `expect`'s epoch (or is empty when
    /// `expect` is None).
    pub fn set_new(&self, expect: Option<u64>) -> u64 {
        let mut slot = self.slot.lock().expect("rrls reloaders slot");
        let current = self.epoch.load(Ordering::SeqCst);
        let matches = match expect {
            None => slot.is_none(),
            Some(e) => slot.as_ref().is_some_and(|_| e == current),
        };
        if matches {
            *slot = Some(SharedState::Pending);
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        self.epoch.load(Ordering::SeqCst)
    }

    /// Upstream `AtomicReference.getAndSet(null)`: returns whether a future
    /// was live and clears the slot.
    pub fn get_and_set_null(&self) -> bool {
        let mut slot = self.slot.lock().expect("rrls reloaders slot");
        let had = slot.is_some();
        if had {
            *slot = None;
        }
        had
    }

    /// Complete the live future (`complete(null)` upstream).
    pub fn complete(&self) {
        if let Some(state) = self.slot.lock().expect("rrls reloaders slot").as_mut() {
            *state = SharedState::Done;
        }
    }

    pub fn current(&self) -> (u64, Option<SharedState>) {
        (
            self.epoch.load(Ordering::SeqCst),
            *self.slot.lock().expect("rrls reloaders slot"),
        )
    }
}

/// Upstream: `net.minecraft.server.packs.resources.ResourceManager`
/// (opaque host seam).
pub struct ResourceManager;

/// Upstream: `net.minecraft.server.packs.resources.PreparableReloadListener`
/// (opaque host seam).
pub struct PreparableReloadListener;

/// Upstream: `ServiceUtils.loadService` walks the Java ServiceLoader. The
/// provider registry is the host seam; with nothing registered the callers
/// fall back to their own defaults, exactly as upstream does.
pub fn service_provider<T: ?Sized>() -> Option<&'static T> {
    None
}

/// Upstream `onInitializeClient` logs the config service class.
pub fn on_initialize_client(config_service: &str) -> String {
    format!("Config service: {}", config_service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_new_swaps_pending_in() {
        let f = ReloadersFuture::new();
        let e0 = f.set_new(None);
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Pending));
        let e1 = f.set_new(Some(e0));
        assert_ne!(e0, e1);
    }

    #[test]
    fn set_new_requires_matching_epoch() {
        let f = ReloadersFuture::new();
        let e0 = f.set_new(None);
        assert_eq!(f.set_new(Some(e0 + 7)), e0, "stale expect is a no-op, current epoch stays");
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Pending), "slot untouched");
    }

    #[test]
    fn complete_marks_done() {
        let f = ReloadersFuture::new();
        f.set_new(None);
        f.complete();
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Done));
    }
}
