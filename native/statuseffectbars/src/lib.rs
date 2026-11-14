//! Status Effect Bars (Horizon Rust port), 1:1.
/*
 * Copyright (c) 2026 A5b84.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License (LGPL-3.0-or-later, per upstream fabric.mod.json), or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this program. If not, see
 * <https://www.gnu.org/licenses/>.
 */
//!
//! Upstream: https://github.com/A5b84/status-effect-bars @7c00964 (2026-06-18).
//! Ported 1:1 per the port policy; credit A5b84. The cloth-config GUI
//! annotations (@PrefixText/@Tooltip/@ColorPicker/@EnumHandler/@Category/
//! @TransitiveObject) are config-screen metadata; the config screen
//! (ModMenu entrypoint) is a later batch like the other ports.

pub mod config;
pub mod i18n;
pub mod mth;
pub mod mixins;
pub mod mob_effect_instance_duck;
pub mod status_effect_bar_renderer;
pub mod status_effect_bars;
pub mod tests;
