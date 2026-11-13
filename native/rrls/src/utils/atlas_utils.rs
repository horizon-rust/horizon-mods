/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `AtlasUtils.java`.
//!
//! Minecraft's sprite-atlas pipeline (`AtlasManager.AtlasEntry`,
//! `SpriteLoader.Preparations`, `readyForUpload`, `atlas().upload`) is the
//! Horizon host seam; the pipeline's control flow is preserved exactly:
//! single-flight keyed by the atlas definition location, upload on the
//! client thread, removal on completion and on failure. The host drives the
//! async chain via `drive` where the upstream executors would.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::texture_utils::{log_error, log_info, log_warn, Identifier};

/// `SpriteLoader.Preparations` after `readyForUpload`.
#[derive(Debug, Clone, PartialEq)]
pub enum Preparations {
    Ready,
    Failed,
}

/// `CompletableFuture<SpriteLoader.Preparations>` state.
#[derive(Debug, Clone, PartialEq)]
pub enum AtlasLoad {
    Pending,
    Done(Preparations),
}

/// The parts of `AtlasManager.AtlasEntry` the logic touches.
pub struct AtlasEntry {
    /// `config().definitionLocation()`
    pub definition_location: Identifier,
    /// `atlas().upload(preps)` — the host side effect.
    pub upload: Box<dyn Fn(&Preparations) + Send>,
    /// `atlasManager.maxMipmapLevels`
    pub max_mipmap_levels: u8,
    /// `entry.scheduleLoad(resourceManager, executor, mipmap)` + the
    /// `readyForUpload().thenApply(_ -> preps)` hop; `Err` models an
    /// upstream exception anywhere in the chain.
    pub schedule_load: Box<dyn Fn(u8) -> Result<Preparations, ()> + Send>,
}

#[derive(Default)]
pub struct PendingAtlases {
    map: Mutex<HashMap<Identifier, Arc<Mutex<AtlasLoad>>>>,
}

impl PendingAtlases {
    /// `reloadAtlas`: single-flight keyed by the definition location
    /// (`computeIfAbsent`). The future is returned while pending; upload
    /// and removal happen when the chain completes.
    pub fn reload_atlas(&self, entry: &AtlasEntry) -> Arc<Mutex<AtlasLoad>> {
        let existing = self
            .map
            .lock()
            .expect("rrls pending atlases")
            .get(&entry.definition_location)
            .cloned();
        if let Some(f) = existing {
            return f;
        }
        log_info(&format!(
            "Scheduling atlas reload: '{}'!",
            entry.definition_location
        ));
        let f = Arc::new(Mutex::new(AtlasLoad::Pending));
        self.map
            .lock()
            .expect("rrls pending atlases")
            .insert(entry.definition_location.clone(), f.clone());
        f
    }

    /// The host step for the async chain: scheduleLoad ->
    /// readyForUpload -> upload (client thread) -> remove entry;
    /// exceptionally logs, removes, returns null. A done future is a no-op.
    pub fn drive(&self, entry: &AtlasEntry) -> Arc<Mutex<AtlasLoad>> {
        let future = self.reload_atlas(entry);
        {
            let mut state = future.lock().expect("rrls atlas load");
        if matches!(*state, AtlasLoad::Pending) {
            let outcome = (entry.schedule_load)(entry.max_mipmap_levels).map_err(|_| ());
            match outcome {
                Ok(p) => {
                    (entry.upload)(&p);
                    *state = AtlasLoad::Done(Preparations::Ready);
                }
                Err(()) => {
                    log_error(&format!(
                        "Failed to force-reload atlas '{}'!",
                        entry.definition_location
                    ));
                    *state = AtlasLoad::Done(Preparations::Failed);
                }
            }
            drop(state);
            self.map
                .lock()
                .expect("rrls pending atlases")
                .remove(&entry.definition_location);
            }
        }
        future
    }

    /// `reloadAtlasSync(atlasManager, resourceManager, entry)`: warn, then
    /// block on the future (`managedBlock` upstream).
    pub fn reload_atlas_sync(&self, entry: &AtlasEntry) -> Preparations {
        log_warn(&format!(
            "Force-reloading atlas: '{}'!",
            entry.definition_location
        ));
        let f = self.drive(entry);
        match f.lock().expect("rrls atlas load").clone() {
            AtlasLoad::Done(p) => p,
            other => panic!("managedBlock returned while load is not done: {:?}", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uploads: Arc<Mutex<u32>>) -> AtlasEntry {
        AtlasEntry {
            definition_location: "minecraft:blocks".into(),
            upload: Box::new(move |_| *uploads.lock().unwrap() += 1),
            max_mipmap_levels: 4,
            schedule_load: Box::new(|_| Ok(Preparations::Ready)),
        }
    }

    #[test]
    fn single_flight_dedupes_while_pending() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        let f1 = p.reload_atlas(&entry(uploads.clone()));
        let f2 = p.reload_atlas(&entry(uploads));
        assert!(Arc::ptr_eq(&f1, &f2), "computeIfAbsent returns the live future");
    }

    #[test]
    fn drive_uploads_once_and_removes_entry() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        let f1 = p.drive(&entry(uploads.clone()));
        assert_eq!(*uploads.lock().unwrap(), 1);
        assert_eq!(*f1.lock().unwrap(), AtlasLoad::Done(Preparations::Ready));
        // Entry removed on completion: a new call schedules a fresh load.
        let f2 = p.drive(&entry(uploads.clone()));
        assert!(!Arc::ptr_eq(&f1, &f2));
        assert_eq!(*uploads.lock().unwrap(), 2);
    }

    #[test]
    fn failure_logs_removes_and_reports_failed() {
        let p = PendingAtlases::default();
        let bad = AtlasEntry {
            definition_location: "minecraft:blocks".into(),
            upload: Box::new(|_| panic!("upload must not run on failure")),
            max_mipmap_levels: 4,
            schedule_load: Box::new(|_| Err(())),
        };
        let out = p.reload_atlas_sync(&bad);
        assert_eq!(out, Preparations::Failed);
    }

    #[test]
    fn sync_warns_then_waits() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        assert_eq!(p.reload_atlas_sync(&entry(uploads)), Preparations::Ready);
    }
}
