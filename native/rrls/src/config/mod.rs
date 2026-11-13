/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `config` package: `AprilFool`, `DoubleLoad`,
//! `HideType`, `Type`, and the `RrlsConfig` service interface.

pub mod april_fool;
pub mod double_load;
pub mod hide_type;
pub mod rrls_config;
pub mod rrls_type;

pub use april_fool::AprilFool;
pub use double_load::DoubleLoad;
pub use hide_type::HideType;
pub use rrls_config::RrlsConfig;
pub use rrls_type::Type;

/// Upstream types carry `Component.translatable("rrls.configuration....")`
/// names; Horizon has no translation registry, so the key is kept verbatim
/// and rendering it is the host seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translatable {
    pub key: &'static str,
}

impl Translatable {
    pub const fn new(key: &'static str) -> Self {
        Self { key }
    }
}
