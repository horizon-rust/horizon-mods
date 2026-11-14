//! world-play-time (Horizon Rust port), slice 1: the playtime model.
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//!
//! 1:1 port of the upstream logic core (MIT): playtime accumulates per world
//! (per save and per server) in seconds, is persisted through a JSON store the
//! upstream keeps per world, and is rendered by the same human formatting the
//! mod uses. HUD/overlay seams land in the next slice.
pub mod playtime {
    //! Port of upstream `PlayTimeWorldData`: accumulate-on-tick, per-world store,
    //! JSON round-trip and the human-readable formatter.
    use std::collections::BTreeMap;

    /// Seconds of playtime for one world.
    #[derive(Clone, Debug, PartialEq)]
    pub struct WorldEntry {
        pub world_id: String,
        pub seconds: u64,
    }

    /// The store upstream persists: world id -> total seconds played.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct PlayTimeStore {
        worlds: BTreeMap<String, u64>,
    }

    impl PlayTimeStore {
        pub fn new() -> Self {
            Self::default()
        }

        /// Upstream accumulates 1 second per second of play in that world.
        pub fn add(&mut self, world_id: &str, seconds: u64) {
            *self.worlds.entry(world_id.to_string()).or_insert(0) += seconds;
        }

        pub fn get(&self, world_id: &str) -> u64 {
            self.worlds.get(world_id).copied().unwrap_or(0)
        }

        pub fn worlds(&self) -> impl Iterator<Item = (&str, u64)> {
            self.worlds.iter().map(|(k, v)| (k.as_str(), *v))
        }

        /// JSON shape upstream saves: {"worlds": {"<id>": <seconds>}}.
        pub fn to_json(&self) -> String {
            let mut out = String::from("{\"worlds\":{");
            let mut first = true;
            for (k, v) in self.worlds.iter() {
                if !first {
                    out.push(',');
                }
                out.push_str(&format!(
                    "\"{}\":{}",
                    k.replace('\\', "\\\\").replace('"', "\\\""),
                    v
                ));
                first = false;
            }
            out.push_str("}}");
            out
        }

        /// Tolerant parse like upstream: unknown keys ignored, missing map = empty.
        /// Hand-rolled over the store's own shape {"worlds": {"<id>": <seconds>}} so
        /// the port carries no third-party deps, matching the other ports.
        pub fn from_json(json: &str) -> Result<Self, String> {
            let mut store = PlayTimeStore::new();
            let bytes: Vec<char> = json.chars().collect();
            let mut i = 0;
            // find the "worlds" key
            let needle = "\"worlds\"";
            let n: Vec<char> = needle.chars().collect();
            while i + n.len() <= bytes.len() && bytes[i..i + n.len()] != n[..] {
                i += 1;
            }
            if i + n.len() > bytes.len() {
                return Ok(store); // missing map = empty, tolerated
            }
            i += n.len();
            // skip to the inner object's opening brace
            while i < bytes.len() && bytes[i] != '{' {
                i += 1;
            }
            if i >= bytes.len() {
                return Ok(store);
            }
            i += 1;
            loop {
                // skip whitespace
                while i < bytes.len() && bytes[i].is_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() || bytes[i] == '}' {
                    break;
                }
                if bytes[i] == ',' {
                    i += 1;
                    continue;
                }
                // key string
                if bytes[i] != '"' {
                    return Err(format!("expected key at char {i}"));
                }
                i += 1;
                let mut key = String::new();
                while i < bytes.len() && bytes[i] != '"' {
                    if bytes[i] == '\\' && i + 1 < bytes.len() {
                        i += 1;
                    }
                    key.push(bytes[i]);
                    i += 1;
                }
                i += 1; // closing quote
                while i < bytes.len() && bytes[i] != ':' {
                    i += 1;
                }
                i += 1;
                while i < bytes.len() && bytes[i].is_whitespace() {
                    i += 1;
                }
                let digits_start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i == digits_start {
                    return Err(format!("non-integer seconds for {key}"));
                }
                let secs: u64 = bytes[digits_start..i]
                    .iter()
                    .collect::<String>()
                    .parse::<u64>()
                    .map_err(|e| e.to_string())?;
                store.worlds.insert(key, secs);
            }
            Ok(store)
        }
    }

    /// Upstream formatting: whole days/hours/minutes, e.g. "3d 4h 25m"; a store
    /// under a minute still reads "0s".
    pub fn format_duration(total_seconds: u64) -> String {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let mut parts = Vec::new();
        if days > 0 {
            parts.push(format!("{days}d"));
        }
        if hours > 0 {
            parts.push(format!("{hours}h"));
        }
        parts.push(format!("{minutes}m"));
        parts.join(" ")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn accumulates_per_world() {
            let mut s = PlayTimeStore::new();
            s.add("world", 100);
            s.add("world", 50);
            s.add("nether", 30);
            assert_eq!(s.get("world"), 150);
            assert_eq!(s.get("nether"), 30);
            assert_eq!(s.get("end"), 0);
        }

        #[test]
        fn json_round_trip_preserves_every_world() {
            let mut s = PlayTimeStore::new();
            s.add("overworld", 3661);
            s.add("server:mc.hypixel.net", 999);
            let j = s.to_json();
            let back = PlayTimeStore::from_json(&j).unwrap();
            assert_eq!(back, s);
            assert!(j.contains("\"overworld\":3661"));
        }

        #[test]
        fn json_parse_tolerates_unknown_keys() {
            let s = PlayTimeStore::from_json("{\"worlds\":{\"a\":10},\"version\":3}").unwrap();
            assert_eq!(s.get("a"), 10);
        }

        #[test]
        fn json_parse_rejects_bad_seconds() {
            assert!(PlayTimeStore::from_json("{\"worlds\":{\"a\":\"lots\"}}").is_err());
        }

        #[test]
        fn formatting_matches_upstream_units() {
            assert_eq!(format_duration(0), "0m");
            assert_eq!(format_duration(59), "0m");
            assert_eq!(format_duration(60), "1m");
            assert_eq!(format_duration(3600 + 60), "1h 1m");
            assert_eq!(format_duration(2 * 86400 + 3 * 3600 + 25 * 60), "2d 3h 25m");
        }
    }
}
