/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `WaitingSharedState.java`.
//!
//! Upstream extends `PreparableReloadListener.SharedState` and overrides
//! `get(StateKey)` so a missing/failed entry returns
//! `CompletableFuture.failedFuture(throwable)` for future-typed keys instead
//! of unwinding. Horizon has no reload state registry, so the store is
//! injected: entries hold either a value or the failure, and `get` mirrors
//! the Java fallback for future-typed keys.

use std::collections::HashMap;
use std::sync::Arc;

pub enum Entry<T> {
    Value(T),
    Failed(String),
}

pub struct WaitingSharedState<T: Clone> {
    entries: HashMap<String, Entry<T>>,
}

impl<T: Clone> WaitingSharedState<T> {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn put_value(&mut self, key: &str, value: T) {
        self.entries.insert(key.to_string(), Entry::Value(value));
    }

    pub fn put_failed(&mut self, key: &str, message: &str) {
        self.entries.insert(key.to_string(), Entry::Failed(message.to_string()));
    }

    /// Upstream `get`: a present value returns as-is; a failure becomes a
    /// failed future for future-typed keys (modeled as `Entry::Failed`
    /// handed back wrapped, never unwound); an absent key still throws.
    pub fn get(&self, key: &str, future_typed: bool) -> Result<Arc<Entry<T>>, String> {
        match self.entries.get(key) {
            Some(e @ Entry::Value(_)) => Ok(Arc::new(e.clone())),
            Some(e @ Entry::Failed(m)) if future_typed => Ok(Arc::new(e.clone())),
            Some(Entry::Failed(m)) => Err(m.clone()),
            None => Err(format!("No state for key {}", key)),
        }
    }
}

impl<T: Clone> Default for WaitingSharedState<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for Entry<T> {
    fn clone(&self) -> Self {
        match self {
            Entry::Value(v) => Entry::Value(v.clone()),
            Entry::Failed(m) => Entry::Failed(m.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_returns_as_is() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_value("k", 7);
        assert!(matches!(*s.get("k", false).unwrap(), Entry::Value(7)));
    }

    #[test]
    fn failed_becomes_failed_future_for_future_keys() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_failed("k", "boom");
        assert!(matches!(*s.get("k", true).unwrap(), Entry::Failed(_)), "no unwind");
    }

    #[test]
    fn failed_still_throws_for_plain_keys() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_failed("k", "boom");
        assert_eq!(s.get("k", false).unwrap_err(), "boom");
    }

    #[test]
    fn missing_key_throws_like_java() {
        let s = WaitingSharedState::<u32>::new();
        assert!(s.get("absent", true).is_err());
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Entry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Value(v) => f.debug_tuple("Value").field(v).finish(),
            Entry::Failed(m) => f.debug_tuple("Failed").field(m).finish(),
        }
    }
}
