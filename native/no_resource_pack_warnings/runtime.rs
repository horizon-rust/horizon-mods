// No Resource Pack Warnings for Horizon: every resource-pack
// compatibility check reports compatible, so incompatible-format packs
// load and select without the warning screen.
//
// A Rust port of no-resource-pack-warnings (SpaceWalkerRS, MIT,
// https://github.com/SpaceWalkerRS/no-resource-pack-warnings). The
// original's behavior is kept:
//
// - The Options path (`updateResourcePacks` and `loadSelectedResourcePacks`)
//   reads `PackCompatibility.isCompatible` as always true (upstream's
//   OptionsMixin, two redirects).
// - The pack-selection path (`PackEntry.extractContent` and
//   `handlePackSelection`) reads the same call as always true (upstream's
//   PackEntryMixin, two redirects).
// - Nothing else is touched: packs still get a compatibility verdict from
//   the repository layer, the verdict is simply never honored as a block.
//
// Horizon note: the engine takes packs through the configuration adapter
// (`horizon-protocol`), with no pack-selection screen or options-side
// pack reload yet, so both upstream paths funnel into one verdict the
// adapter applies. The override covers that path; when a selection UI
// lands, this module extends to it unchanged.

/// What every compatibility check reads once this port is active.
pub const ALWAYS_COMPATIBLE: bool = true;

/// The verdict the adapter applies to a pack's format compatibility.
/// Upstream returns `PackCompatibility::isCompatible()`; this port
/// always answers true.
pub fn pack_is_compatible(_compatibility: PackCompatibility) -> bool {
    ALWAYS_COMPATIBLE
}

/// The verdict, as the engine's repository layer reports it upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackCompatibility {
    Compatible,
    TooNew,
    TooOld,
    Broken,
}

/// A pack passes compatibility under this port whatever its verdict.
pub fn suppress_warning(compatibility: PackCompatibility) -> bool {
    pack_is_compatible(compatibility)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verdict_passes() {
        for verdict in [
            PackCompatibility::Compatible,
            PackCompatibility::TooNew,
            PackCompatibility::TooOld,
            PackCompatibility::Broken,
        ] {
            assert!(suppress_warning(verdict));
        }
    }
}
