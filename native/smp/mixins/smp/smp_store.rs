// The SMP's files under the world's folder (`world/horizon-smp/`): JSON
// documents written whole and atomically (a temporary file, flushed, renamed
// over the old one), and the audit log, one JSON line per transaction,
// appended before the change it records. Added to the game as
// `game::smp_store`; also a module of this crate, for its tests.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// The folder of the SMP's files in a world's folder.
pub const DIR: &str = "horizon-smp";

/// Writes `bytes` to `path` so that a crash leaves either the old file or the
/// new one: a temporary file beside it, flushed to disk, then renamed over it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temp = dir.join(format!(".{name}.tmp"));
    {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    // The rename itself reaches the disk with the folder's entry (Unix).
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// A JSON document: its default when the file doesn't exist, an error when it
/// exists but can't be read, so that a damaged file is never replaced by an
/// empty one.
pub fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Writes a JSON document atomically, pretty-printed.
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_atomic(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The audit log: `audit.log`, one JSON object a line.
pub struct Audit {
    path: PathBuf,
    file: Option<File>,
    /// Lines appended since the last sync.
    unsynced: bool,
}

impl Audit {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: None,
            unsynced: false,
        }
    }
    /// Appends one line; the change it records waits for it.
    pub fn append(&mut self, entry: &serde_json::Value) -> Result<(), String> {
        let mut line = entry.to_string();
        line.push('\n');
        let path = &self.path;
        if self.file.is_none() {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            self.file = Some(file);
        }
        let file = self.file.as_mut().expect("opened above");
        file.write_all(line.as_bytes())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        self.unsynced = true;
        Ok(())
    }
    /// Flushes what was appended to disk (at the end of a tick that appended,
    /// and when the world closes); nothing when nothing was appended, so a
    /// quiet tick makes no system call.
    pub fn sync(&mut self) {
        if self.unsynced
            && let Some(f) = &self.file
        {
            let _ = f.sync_data();
            self.unsynced = false;
        }
    }
}

/// `UUID.toString()`: 8-4-4-4-12 lowercase hex digits.
pub fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// The UUID a [`uuid_string`] wrote.
pub fn parse_uuid(s: &str) -> Option<u128> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32)
        .then(|| u128::from_str_radix(&hex, 16).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smp-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn atomic_writes_replace_the_file_and_leave_no_temporary() {
        let dir = scratch("atomic");
        let path = dir.join("economy.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["economy.json"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_document_is_an_error_not_an_empty_one() {
        let dir = scratch("damaged");
        let path = dir.join("economy.json");
        let missing: Vec<u32> = read_json(&path).unwrap();
        assert!(missing.is_empty());
        write_atomic(&path, b"{ not json").unwrap();
        assert!(read_json::<Vec<u32>>(&path).is_err());
        write_json(&path, &vec![1u32, 2]).unwrap();
        assert_eq!(read_json::<Vec<u32>>(&path).unwrap(), [1, 2]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_audit_log_appends_lines() {
        let dir = scratch("audit");
        let mut audit = Audit::new(dir.join("audit.log"));
        audit
            .append(&serde_json::json!({"kind": "pay", "amount": 5}))
            .unwrap();
        audit
            .append(&serde_json::json!({"kind": "pay", "amount": 6}))
            .unwrap();
        audit.sync();
        let text = fs::read_to_string(dir.join("audit.log")).unwrap();
        assert_eq!(
            text,
            "{\"amount\":5,\"kind\":\"pay\"}\n{\"amount\":6,\"kind\":\"pay\"}\n"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn uuids_round_trip() {
        let u = 0x0000_0000_0000_4000_8000_0000_0000_0001u128;
        assert_eq!(uuid_string(u), "00000000-0000-4000-8000-000000000001");
        assert_eq!(parse_uuid(&uuid_string(u)), Some(u));
        assert_eq!(parse_uuid("nope"), None);
    }
}
