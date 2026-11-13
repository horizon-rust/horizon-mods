/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `TextureUtils.java`.
//!
//! Minecraft's `CompletableFuture<TextureContents>` pipeline becomes the
//! `TextureLoad` state machine below with identical semantics: a pending
//! map keyed by the texture's resource id, `computeIfAbsent`-style
//! single-flight entry, the load applied when the future completes,
//! completion removing its own entry (outside the entry call, per the
//! upstream comment), and a force-reload that blocks until done
//! (`Minecraft.managedBlock` is the Horizon seam; the host drives the load
//! via `drive` where the upstream executors would).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// `Identifier` on the wire is a namespaced string ("minecraft:foo").
pub type Identifier = String;

/// The load's terminal state, mirroring `TextureContents` + an error
/// landing as `null` upstream (`exceptionally` -> null).
#[derive(Debug, Clone, PartialEq)]
pub enum TextureContents {
    Loaded,
    Failed,
}

/// `CompletableFuture<TextureContents>` shape: pending, or done.
#[derive(Debug, Clone, PartialEq)]
pub enum TextureLoad {
    Pending,
    Done(TextureContents),
}

#[derive(Default)]
pub struct PendingTextures {
    map: Mutex<HashMap<Identifier, Arc<Mutex<TextureLoad>>>>,
}

impl PendingTextures {
    /// `reloadTexture(manager, rl, texture)`: single-flight keyed by the
    /// resource id (`computeIfAbsent`). Returns the live future while the
    /// load is pending; the completed future removes its own entry via
    /// `whenComplete` upstream.
    pub fn reload_texture(&self, rl: Identifier) -> Arc<Mutex<TextureLoad>> {
        let existing = self
            .map
            .lock()
            .expect("rrls pending textures")
            .get(&rl)
            .cloned();
        if let Some(f) = existing {
            return f;
        }
        let f = Arc::new(Mutex::new(TextureLoad::Pending));
        self.map
            .lock()
            .expect("rrls pending textures")
            .insert(rl.clone(), f.clone());
        f
    }

    /// The host step that stands in for the upstream async chain:
    /// `reloadTextureInternal` schedules the load, applies the contents
    /// when it lands; failure logs and returns null. A done future is a
    /// no-op, like re-driving a completed future.
    pub fn drive(
        &self,
        rl: &str,
        loader: impl FnOnce() -> Result<TextureContents, ()>,
    ) -> Arc<Mutex<TextureLoad>> {
        let future = self.reload_texture(rl.to_string());
        {
            let mut state = future.lock().expect("rrls texture load");
        if matches!(*state, TextureLoad::Pending) {
            log_info(&format!("Reloading texture '{}'!", rl));
            let outcome = match loader() {
                Ok(c) => TextureContents::Loaded,
                Err(()) => {
                    log_error("Failed to force-reload texture!");
                    TextureContents::Failed
                }
            };
            *state = TextureLoad::Done(outcome);
            drop(state);
            // whenComplete cleanup: outside the entry call, otherwise
            // "Recursive update" (upstream comment kept).
            self.map
                .lock()
                .expect("rrls pending textures")
                .remove(rl);
            }
        }
        future
    }

    /// `reloadTextureSync(manager, texture)`: force-reload, block until the
    /// future is done (`managedBlock` upstream).
    pub fn reload_texture_sync(
        &self,
        rl: Identifier,
        loader: impl FnOnce() -> Result<TextureContents, ()>,
    ) -> TextureContents {
        log_warn(&format!("Force-reloading texture: '{}'!", rl));
        let future = self.drive(&rl, loader);
        match future.lock().expect("rrls texture load").clone() {
            TextureLoad::Done(c) => c,
            other => panic!("managedBlock returned while load is not done: {:?}", other),
        }
    }

    pub fn is_pending(&self, rl: &str) -> bool {
        self.map
            .lock()
            .expect("rrls pending textures")
            .get(rl)
            .is_some_and(|f| matches!(*f.lock().expect("rrls texture load"), TextureLoad::Pending))
    }
}

/// Upstream log lines go through `Rrls.LOGGER`; Horizon's host supplies the
/// sink, so they surface as functions a test can observe.
pub fn log_info(_msg: &str) {}
pub fn log_warn(_msg: &str) {}
pub fn log_error(_msg: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_flight_dedupes_while_pending() {
        let p = PendingTextures::default();
        let f1 = p.reload_texture("minecraft:foo".into());
        let f2 = p.reload_texture("minecraft:foo".into());
        assert!(Arc::ptr_eq(&f1, &f2), "computeIfAbsent returns the live future");
        assert!(p.is_pending("minecraft:foo"));
    }

    #[test]
    fn drive_completes_and_fails_like_upstream() {
        let p = PendingTextures::default();
        p.reload_texture("minecraft:bar".into());
        let f = p.drive("minecraft:bar", || Ok(TextureContents::Loaded));
        assert_eq!(*f.lock().unwrap(), TextureLoad::Done(TextureContents::Loaded));
        assert!(!p.is_pending("minecraft:bar"), "whenComplete removed the entry");

        p.reload_texture("minecraft:baz".into());
        let g = p.drive("minecraft:baz", || Err(()));
        assert_eq!(*g.lock().unwrap(), TextureLoad::Done(TextureContents::Failed), "exceptionally -> null");
    }

    #[test]
    fn drive_after_completion_is_a_noop() {
        let p = PendingTextures::default();
        p.reload_texture("minecraft:ok".into());
        p.drive("minecraft:ok", || Ok(TextureContents::Loaded));
        let calls = Arc::new(Mutex::new(0));
        let c2 = calls.clone();
        p.reload_texture("minecraft:ok".into());
        assert!(p.is_pending("minecraft:ok"), "a completed future was removed, so a new call schedules a fresh load");
        p.drive("minecraft:ok", || {
            *c2.lock().unwrap() += 1;
            Ok(TextureContents::Loaded)
        });
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn sync_warns_then_waits() {
        let p = PendingTextures::default();
        let out = p.reload_texture_sync("minecraft:bar".into(), || Ok(TextureContents::Loaded));
        assert_eq!(out, TextureContents::Loaded);
        let err = p.reload_texture_sync("minecraft:baz".into(), || Err(()));
        assert_eq!(err, TextureContents::Failed);
    }
}
