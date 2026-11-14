//! no-resource-pack-warnings (Horizon Rust port), 1:1.
/*
 * Copyright (c) 2022 Space Walker.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
//!
//! Upstream: https://github.com/SpaceWalkerRS/no-resource-pack-warnings
//! (MIT). Ported 1:1 per the port policy; credit Space Walker.
//!
//! Upstream is 2 mixins, 69 lines: every one of the four
//! `PackCompatibility.isCompatible()` call sites the mod cares about is
//! redirected to return `true`, so outdated resource/data packs load and
//! select with no warning screen or red border. The mixin target methods
//! (`Options.updateResourcePacks`, `Options.loadSelectedResourcePacks`,
//! `PackEntry.extractContent`, `PackEntry.handlePackSelection`) are
//! represented by their redirect handlers running against the
//! [`PackCompatibilitySeam`] token the hosts wire in.
pub mod mixins;

/// `net.minecraft.server.packs.repository.PackCompatibility` (token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackCompatibility(pub u64);

#[cfg(test)]
mod tests;
