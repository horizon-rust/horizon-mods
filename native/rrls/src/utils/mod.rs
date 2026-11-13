/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `utils` package. Rendering-bound helpers
//! (`DummyGuiGraphics`, `GuiGraphicsGenerator`, `AtlasUtils`, `TextureUtils`,
//! `OverlayHelper`) bind to Minecraft's GuiGraphics/render pipeline and are
//! host seams; the pure-logic ones are translated here.

pub mod atlas_utils;
pub mod gui_graphics;
pub mod gui_graphics_generator;
pub mod ease;
pub mod overlay_helper;
pub mod texture_utils;
pub mod easing;
pub mod rainbow;
pub mod waiting_shared_state;
