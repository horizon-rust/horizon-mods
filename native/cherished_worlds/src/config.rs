//! 1:1 port of CherishedWorldsConfig (SpectreLib client config).

use crate::constants::MOD_ID;
use std::sync::{Mutex, OnceLock};

// java: org.apache.commons.lang3.tuple.Pair<Client, SpectreConfigSpec> built
// by SpectreConfigSpec.Builder.configure(Client::new).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupType {
    All,
    Favorited,
    None,
}

// java: SpectreConfigSpec.EnumValue<BackupType> (the live client config value)
#[derive(Debug)]
pub struct EnumValue {
    value: Mutex<BackupType>,
}

impl EnumValue {
    // java: EnumValue.get()
    pub fn get(&self) -> BackupType {
        *self.value.lock().unwrap()
    }

    // java: SpectreConfigSpec.Builder.defineEnum(String, BackupType) default
    pub fn set(&self, value: BackupType) {
        *self.value.lock().unwrap() = value;
    }
}

// java: SpectreConfigSpec.Builder.comment(...).translation(...).defineEnum(...)
// The comment and translation key are preserved verbatim as the config entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumSpec {
    pub comment: &'static str,
    pub translation: &'static str,
    pub default: BackupType,
}

// java: SpectreConfigSpec.CLIENT_SPEC (the built spec)
pub struct ClientSpec {
    pub backup_world_type: EnumSpec,
}

// java: public static class Client { public final SpectreConfigSpec.EnumValue<BackupType> backupWorldType; }
pub struct Client {
    pub backup_world_type: EnumValue,
}

impl Client {
    // java: public Client(SpectreConfigSpec.Builder builder)
    fn new() -> Client {
        Client {
            backup_world_type: EnumValue {
                value: Mutex::new(BackupType::All),
            },
        }
    }
}

// java: CONFIG_PREFIX = "gui." + MOD_ID + ".config."
const CONFIG_PREFIX: &str = "gui.cherishedworlds.config.";

fn spec() -> ClientSpec {
    ClientSpec {
        backup_world_type: EnumSpec {
            comment: "Determines which worlds prompt for a backup when loaded with a newer Minecraft version.",
            translation: "gui.cherishedworlds.config.backupWorldType",
            default: BackupType::All,
        },
    }
}

// java: public static final Pair built in the static initializer
pub fn client_spec() -> ClientSpec {
    spec()
}

// java: public static final Client CLIENT
pub fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::new)
}

// java: public enum BackupType (already above) — key name kept for the
// config prefix wiring.
pub fn config_prefix() -> String {
    format!("gui.{MOD_ID}.config.")
}

#[allow(dead_code)]
fn _unused() {
    let _ = CONFIG_PREFIX;
}
