//! Upstream `mixin/client/OptionsMixin.java`, 1:1.
/*
 * Copyright (c) 2022 Space Walker. MIT; see the crate root for the full
 * notice.
 */

use crate::PackCompatibility;

/// `@Mixin(Options.class) public class OptionsMixin`.
pub struct OptionsMixin;

impl OptionsMixin {
    /// `@Redirect(method = "updateResourcePacks", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;
    /// isCompatible()Z")) private boolean
    /// onUpdateResourcePacksRedirectIsCompatible(PackCompatibility
    /// compatibility)`.
    pub fn on_update_resource_packs_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }

    /// `@Redirect(method = "loadSelectedResourcePacks", at = @At(value =
    /// "INVOKE", ordinal = 0, target =
    /// "Lnet/minecraft/server/packs/repository/PackCompatibility;
    /// isCompatible()Z")) private boolean
    /// onLoadSelectedResourcePacksRedirectIsCompatible(PackCompatibility
    /// compatibility)`.
    pub fn on_load_selected_resource_packs_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }
}

/// The redirect target the mixin replaces: upstream
/// `PackCompatibility.isCompatible()`, as the engine surface reads it.
pub trait PackCompatibilitySeam {
    /// `isCompatible()` on the compatibility instance the call site holds.
    fn is_compatible(&self, compatibility: PackCompatibility) -> bool;
}
