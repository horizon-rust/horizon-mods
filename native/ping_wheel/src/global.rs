//! Upstream: common/Global.java (1:1).
pub const MOD_ID: &str = "pingwheel";
pub const MOD_PREFIX: &str = "[Ping-Wheel] ";

/// Upstream builds LOGGER with a FormattedMessageFactory that prefixes every message with MOD_PREFIX.
/// The host logging seam in `seams` applies the same prefix, so log lines stay 1:1.
pub use crate::seams::log_error;
pub use crate::seams::log_info;
pub use crate::seams::log_warn;

/// MOD_VERSION comes from IPlatformContextService.INSTANCE.getSelfModVersion() at class-load.
pub fn mod_version() -> String {
    crate::seams::context_service().get_self_mod_version()
}
