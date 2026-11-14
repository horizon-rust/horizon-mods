/*
 * Copyright (C) 2020-2022 Illusive Soulworks
 *
 * Polymorph is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published
 * by the Free Software Foundation, either version 3 of the License, or
 * any later version.
 *
 * Polymorph is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR PARTICULAR PURPOSE.  See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with Polymorph.  If not, see <https://www.gnu.org/licenses/>.
 *
 * Rust translation 2026 sparticle, exact 1:1 of the upstream Java.
 */

//! Exact 1:1 translation of Polymorph (upstream Java, LGPL-3.0-or-later).
//! Where Minecraft/Fabric APIs do not exist on Horizon, the counterpart is a
//! documented host seam with the same shape and semantics.

pub mod recipe_pair;
pub mod recipe_cache;
pub mod block_entity_recipe_data;
pub mod player_recipe_data;
pub mod recipe_data;
pub mod recipe_manager;
pub mod block_entity_ticker;
pub mod network;
pub mod wrappers;
pub mod client_widget;
