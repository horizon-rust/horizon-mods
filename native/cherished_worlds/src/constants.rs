//! 1:1 port of CherishedWorldsConstants.

use crate::seams::Logger;

// java: public static final String MOD_ID = "cherishedworlds";
pub const MOD_ID: &str = "cherishedworlds";

// java: public static final String MOD_NAME = "Cherished Worlds";
pub const MOD_NAME: &str = "Cherished Worlds";

// java: public static final Logger LOG = LoggerFactory.getLogger(MOD_NAME);
pub fn log() -> Logger {
    Logger
}
