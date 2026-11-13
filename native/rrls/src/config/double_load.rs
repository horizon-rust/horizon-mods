/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `DoubleLoad.java`. The enum constants carry translated
//! names; those live on `Translatable` here.

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleLoad {
    ForceLoad,
    Load,
    None,
}

impl DoubleLoad {
    pub fn translated_name(&self) -> Translatable {
        match self {
            DoubleLoad::ForceLoad => Translatable::new("rrls.configuration.doubleload.force_load"),
            DoubleLoad::Load => Translatable::new("rrls.configuration.doubleload.load"),
            DoubleLoad::None => Translatable::new("rrls.configuration.hide_doubleload.none"),
        }
    }

    pub fn is_load(&self) -> bool {
        matches!(self, DoubleLoad::ForceLoad | DoubleLoad::Load)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_load_matches_java_switch() {
        assert!(DoubleLoad::ForceLoad.is_load());
        assert!(DoubleLoad::Load.is_load());
        assert!(!DoubleLoad::None.is_load());
    }
}
