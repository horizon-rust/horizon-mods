/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `HideType.java`.

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideType {
    All,
    Loading,
    Reloading,
    None,
}

impl HideType {
    pub const ALL_AFFECT_INITIAL: bool = true;
    pub const LOADING_AFFECT_INITIAL: bool = true;
    pub const RELOADING_AFFECT_INITIAL: bool = false;
    pub const NONE_AFFECT_INITIAL: bool = false;

    pub fn affect_initial(&self) -> bool {
        match self {
            HideType::All | HideType::Loading => true,
            HideType::Reloading | HideType::None => false,
        }
    }

    pub fn translated_name(&self) -> Translatable {
        match self {
            HideType::All => Translatable::new("rrls.configuration.hide.all"),
            HideType::Loading => Translatable::new("rrls.configuration.hide.loading"),
            HideType::Reloading => Translatable::new("rrls.configuration.hide.reloading"),
            HideType::None => Translatable::new("rrls.configuration.hide_doubleload.none"),
        }
    }

    pub fn can_hide(&self, reloading: bool) -> bool {
        match self {
            HideType::All => true,
            HideType::Loading => !reloading,
            HideType::Reloading => reloading,
            HideType::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_hide_matrix_matches_java_switch() {
        assert!(HideType::All.can_hide(true));
        assert!(HideType::All.can_hide(false));
        assert!(HideType::Loading.can_hide(false));
        assert!(!HideType::Loading.can_hide(true));
        assert!(!HideType::Reloading.can_hide(false));
        assert!(HideType::Reloading.can_hide(true));
        assert!(!HideType::None.can_hide(true));
        assert!(!HideType::None.can_hide(false));
    }

    #[test]
    fn affect_initial_flags_match_java_fields() {
        assert!(HideType::All.affect_initial());
        assert!(HideType::Loading.affect_initial());
        assert!(!HideType::Reloading.affect_initial());
        assert!(!HideType::None.affect_initial());
    }
}
