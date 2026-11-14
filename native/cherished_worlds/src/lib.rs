//! 1:1 Horizon Rust port of Cherished Worlds (Illusive Soulworks,
//! LGPL-3.0-or-later).
//! Upstream: https://github.com/illusivesoulworks/cherishedworlds
//!
//! Every Java class maps to one module below, same names, same logic, same
//! defaults, same control flow. Host seams (seams.rs) carry the Minecraft
//! client / SpectreLib / MixinExtras surfaces the engine binds; where an
//! upstream type is engine-bound, the seam mirrors its Java calls one to one.
//!
//! Copyright (C) 2018-2026 Illusive Soulworks (upstream, LGPL-3.0-or-later);
//! Rust translation 2026 sparticle, under the same license.
//!
//! This program is free software: you can redistribute it and/or modify it
//! under the terms of the GNU Lesser General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or (at your
//! option) any later version. This program is distributed in the hope that it
//! will be useful, but WITHOUT ANY WARRANTY; without even the implied
//! warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
//! GNU Lesser General Public License for more details. You should have
//! received a copy of the GNU Lesser General Public License along with this
//! program; if not, see <https://www.gnu.org/licenses/>.

pub mod seams;

pub mod common_mod;
pub mod config;
pub mod constants;
pub mod favorites_list;
pub mod mixin_hooks;
pub mod mixins;
pub mod screen_event_hooks;
pub mod services;
pub mod widgets;

// Re-exports under the Java class names so call sites read 1:1.
pub use favorites_list::FavoritesList;
