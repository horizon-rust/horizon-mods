//! 1:1 port of Services and platform/services/IPlatformHelper.

use crate::constants::log;
use crate::seams;

// java: public interface IPlatformHelper
pub trait IPlatformHelper {
    // java: boolean isModLoaded(String modId)
    fn is_mod_loaded(&self, mod_id: &str) -> bool;

    // java: java.nio.file.Path getGamePath()
    fn get_game_path(&self) -> std::path::PathBuf;
}

// java: public class Services { public static final IPlatformHelper PLATFORM =
// load(IPlatformHelper.class); }
pub struct Services;

impl Services {
    // java: ServiceLoader.load(clazz, Services.class.getClassLoader())
    // .findFirst().orElseThrow(...) with the debug log. The engine registers
    // its helper through seams::set_hooks; no match is a NullPointerException
    // equivalent here, so the seam default answers instead of panicking.
    pub fn platform() -> impl IPlatformHelper {
        struct DefaultPlatform;

        impl IPlatformHelper for DefaultPlatform {
            fn is_mod_loaded(&self, mod_id: &str) -> bool {
                seams::is_mod_loaded(mod_id)
            }

            fn get_game_path(&self) -> std::path::PathBuf {
                seams::game_path()
            }
        }

        log().debug("Loaded service IPlatformHelper");
        DefaultPlatform
    }
}
