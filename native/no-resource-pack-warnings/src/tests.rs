use crate::mixins::options::{OptionsMixin, PackCompatibilitySeam};
use crate::mixins::pack_entry::PackEntryMixin;
use crate::PackCompatibility;

fn compat(id: u64) -> PackCompatibility {
    PackCompatibility(id)
}

/// Upstream `onUpdateResourcePacksRedirectIsCompatible`: any verdict reads
/// as compatible.
#[test]
fn options_update_resource_packs_redirect_returns_true() {
    let mixin = OptionsMixin;
    for id in [0, 1, 2, 3, u64::MAX] {
        assert!(mixin.on_update_resource_packs_redirect_is_compatible(compat(id)));
    }
}

/// Upstream `onLoadSelectedResourcePacksRedirectIsCompatible` (ordinal 0
/// site).
#[test]
fn options_load_selected_resource_packs_redirect_returns_true() {
    let mixin = OptionsMixin;
    for id in [0, 1, 2, 3, u64::MAX] {
        assert!(mixin.on_load_selected_resource_packs_redirect_is_compatible(compat(id)));
    }
}

/// Upstream `onRenderRedirectIsCompatible` (the `extractContent` site).
#[test]
fn pack_entry_extract_content_redirect_returns_true() {
    let mixin = PackEntryMixin;
    for id in [0, 1, 2, 3, u64::MAX] {
        assert!(mixin.on_render_redirect_is_compatible(compat(id)));
    }
}

/// Upstream `onMouseClickedRedirectIsCompatible` (the
/// `handlePackSelection` site).
#[test]
fn pack_entry_handle_pack_selection_redirect_returns_true() {
    let mixin = PackEntryMixin;
    for id in [0, 1, 2, 3, u64::MAX] {
        assert!(mixin.on_mouse_clicked_redirect_is_compatible(compat(id)));
    }
}

/// The mixin's redirected call surface (`PackCompatibility.isCompatible()`)
/// also answers true through the seam, any verdict in.
#[test]
fn seam_is_compatible_always_true() {
    let mixin = PackEntryMixin;
    for id in [0, 1, 2, 3, u64::MAX] {
        assert!(mixin.is_compatible(compat(id)));
    }
}
