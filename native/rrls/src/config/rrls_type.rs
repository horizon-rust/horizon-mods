/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Type.java` (upstream class name `Type`; the module is
//! `rrls_type` because `type` is reserved).

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Progress,
    Text,
    TextWithBackground,
    None,
}

impl Type {
    pub fn translated_name(&self) -> Translatable {
        match self {
            Type::Progress => Translatable::new("rrls.configuration.type.progress"),
            Type::Text => Translatable::new("rrls.configuration.type.text"),
            Type::TextWithBackground => Translatable::new("rrls.configuration.type.textbg"),
            Type::None => Translatable::new("rrls.configuration.type.none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_keys_match_upstream() {
        assert_eq!(Type::Progress.translated_name().key, "rrls.configuration.type.progress");
        assert_eq!(Type::Text.translated_name().key, "rrls.configuration.type.text");
        assert_eq!(Type::TextWithBackground.translated_name().key, "rrls.configuration.type.textbg");
        assert_eq!(Type::None.translated_name().key, "rrls.configuration.type.none");
    }
}
