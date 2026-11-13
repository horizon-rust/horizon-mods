/*
 * Port of Remove Reloading Screen (rrls) to Rust for Horizon.
 *
 * Copyright 2023 - 2026 dima_dencep (upstream, OSL-3.0); Rust translation 2026 sparticle.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 *
 * Exact 1:1 translation of the upstream Java. Where an
 * upstream file binds to a Minecraft/Fabric API Horizon does not expose, the
 * same shape is kept and the host side is a seam (see README).
 */

pub mod rrls;

pub mod config;
pub mod utils;
pub mod services;
pub mod screens;
pub mod mixins;
