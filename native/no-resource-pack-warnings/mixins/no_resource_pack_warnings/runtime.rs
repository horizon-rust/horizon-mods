// Upstream no-resource-pack-warnings, 1:1 splice carrier.
//
// Copyright (c) 2022 Space Walker. MIT. Ported 1:1 per the port policy;
// credit Space Walker. Upstream is 2 mixins, 69 lines: all four
// `PackCompatibility.isCompatible()` call sites the mod cares about are
// redirected to return `true`, so outdated resource/data packs load and
// select with no warning screen or red border. Target methods:
// `Options.updateResourcePacks`, `Options.loadSelectedResourcePacks`,
// `PackEntry.extractContent`, `PackEntry.handlePackSelection`.

/// `net.minecraft.server.packs.repository.PackCompatibility` (token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackCompatibility(pub u64);

/// `@Mixin(Options.class) public class OptionsMixin`.
pub struct OptionsMixin;

impl OptionsMixin {
    /// `@Redirect(method = "updateResourcePacks", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;isCompatible()Z"))
    /// private boolean onUpdateResourcePacksRedirectIsCompatible(PackCompatibility
    /// compatibility)`.
    pub fn on_update_resource_packs_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }

    /// `@Redirect(method = "loadSelectedResourcePacks", at = @At(value =
    /// "INVOKE", ordinal = 0, target =
    /// "Lnet/minecraft/server/packs/repository/PackCompatibility;isCompatible()Z"))
    /// private boolean onLoadSelectedResourcePacksRedirectIsCompatible(PackCompatibility
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

/// `@Mixin(PackEntry.class) public class PackEntryMixin`.
pub struct PackEntryMixin;

impl PackEntryMixin {
    /// `@Redirect(method = "extractContent", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;isCompatible()Z"))
    /// private boolean onRenderRedirectIsCompatible(PackCompatibility
    /// compatibility)`.
    pub fn on_render_redirect_is_compatible(
        &self,
        _compatibility: PackCompatibility,
    ) -> bool {
        true
    }

    /// `@Redirect(method = "handlePackSelection", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/server/packs/repository/PackCompatibility;isCompatible()Z"))
    /// private boolean onMouseClickedRedirectIsCompatible(PackCompatibility
    /// compatibility)`.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn compat(id: u64) -> PackCompatibility {
        PackCompatibility(id)
    }

    #[test]
    fn redirects_return_true_for_any_verdict() {
        let options = OptionsMixin;
        let entry = PackEntryMixin;
        for id in [0, 1, 2, 3, u64::MAX] {
            assert!(options.on_update_resource_packs_redirect_is_compatible(compat(id)));
            assert!(options.on_load_selected_resource_packs_redirect_is_compatible(compat(id)));
            assert!(entry.on_render_redirect_is_compatible(compat(id)));
            assert!(entry.on_mouse_clicked_redirect_is_compatible(compat(id)));
            assert!(entry.is_compatible(compat(id)));
        }
    }
}
