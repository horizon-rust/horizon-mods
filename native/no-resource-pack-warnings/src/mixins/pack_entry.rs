//! Upstream `mixin/client/PackEntryMixin.java`, 1:1.
/*
 * Copyright (c) 2022 Space Walker. MIT; see the crate root for the full
 * notice.
 */

use crate::PackCompatibility;
use crate::mixins::options::PackCompatibilitySeam;

/// `@Mixin(PackEntry.class) public class PackEntryMixin`.
pub struct PackEntryMixin;

impl PackEntryMixin {
    /// `@Redirect(method = "extractContent", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;
    /// isCompatible()Z")) private boolean onRenderRedirectIsCompatible
    /// (PackCompatibility compatibility)`.
    pub fn on_render_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }

    /// `@Redirect(method = "handlePackSelection", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;
    /// isCompatible()Z")) private boolean
    /// onMouseClickedRedirectIsCompatible(PackCompatibility compatibility)`.
    pub fn on_mouse_clicked_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }
}

impl PackCompatibilitySeam for PackEntryMixin {
    fn is_compatible(&self, _compatibility: PackCompatibility) -> bool {
        true
    }
}
