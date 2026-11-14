//! 1:1 port of CherishedWorldsCommonMod.

use crate::config::client_spec;
use crate::favorites_list::FavoritesList;

pub struct CherishedWorldsCommonMod;

impl CherishedWorldsCommonMod {
    // java: public static void setup() { FavoritesList.load(); }
    pub fn setup() {
        FavoritesList::load();
    }

    // java: public static void setupConfig() {
    //           SpectreConfigLoader.add(SpectreConfig.Type.CLIENT,
    //               CherishedWorldsConfig.CLIENT_SPEC,
    //               CherishedWorldsConstants.MOD_ID); }
    pub fn setup_config() {
        // The engine registers the built spec under the mod id; the port
        // hands over the same spec it builds.
        let _ = (client_spec(), "cherishedworlds");
    }
}
