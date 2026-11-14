// Port of com.ishland.vmp.common.config.Config (Config.java).
// Upstream reads vmp.properties through java.util.Properties, resolves
// every key with its default on a missing or malformed value, writes the
// normalized file back under the "Configuration file for VMP" comment,
// and forces booleans that another mod lists in its
// vmp:incompatibleConfig array to disabled (unless the
// vmp.ignoreIncompatibleConfig system property is set). The Fabric
// mod-container walk is the caller's glue; the forcing logic and the
// exact warning wording are kept 1:1.

pub const TARGET_CHUNK_SEND_RATE_DEFAULT: i32 = -1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub target_chunk_send_rate: i32,
    pub use_async_logging: bool,
    pub use_optimized_entity_tracking: bool,
    pub optimized_entity_tracking_use_staging_area: bool,
    pub use_multiple_netty_event_loops: bool,
    pub use_async_portals: bool,
    pub use_async_chunks_on_login: bool,
    pub use_async_chunks_on_some_commands: bool,
    pub show_async_loading_messages: bool,
    pub show_chunk_tracking_messages: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            target_chunk_send_rate: TARGET_CHUNK_SEND_RATE_DEFAULT,
            use_async_logging: true,
            use_optimized_entity_tracking: true,
            optimized_entity_tracking_use_staging_area: true,
            use_multiple_netty_event_loops: true,
            use_async_portals: true,
            use_async_chunks_on_login: true,
            use_async_chunks_on_some_commands: false,
            show_async_loading_messages: true,
            show_chunk_tracking_messages: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incompatibility {
    pub key: String,
    pub mod_id: String,
    pub mod_version: String,
}

pub struct LoadResult {
    pub config: Config,
    /// The normalized vmp.properties text upstream writes back, with
    /// every key resolved and the forcing NOT applied (upstream writes
    /// the parsed value before the incompatibility pass).
    pub normalized_properties: String,
    /// The warning messages in upstream order.
    pub warnings: Vec<String>,
}

impl Config {
    pub fn load(
        properties_text: &str,
        incompatibilities: &[Incompatibility],
        ignore_incompatible: bool,
    ) -> LoadResult {
        let entries = parse_properties(properties_text);
        let mut normalized: Vec<(String, String)> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let mut config = Config::default();

        config.target_chunk_send_rate = get_int(
            &entries,
            "target_chunk_send_rate",
            TARGET_CHUNK_SEND_RATE_DEFAULT,
            &mut normalized,
        );
        config.use_async_logging = get_boolean(
            &entries,
            "use_async_logging",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.use_optimized_entity_tracking = get_boolean(
            &entries,
            "use_optimized_entity_tracking",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.optimized_entity_tracking_use_staging_area = get_boolean(
            &entries,
            "optimized_entity_tracking_use_staging_area",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.use_multiple_netty_event_loops = get_boolean(
            &entries,
            "use_multiple_netty_event_loops",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.use_async_portals = get_boolean(
            &entries,
            "use_async_portals",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.use_async_chunks_on_login = get_boolean(
            &entries,
            "use_async_chunks_on_login_v2",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.use_async_chunks_on_some_commands = get_boolean(
            &entries,
            "use_async_chunks_on_some_commands",
            false,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.show_async_loading_messages = get_boolean(
            &entries,
            "show_async_loading_messages",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );
        config.show_chunk_tracking_messages = get_boolean(
            &entries,
            "show_chunk_tracking_messages",
            true,
            incompatibilities,
            ignore_incompatible,
            &mut normalized,
            &mut warnings,
        );

        let mut normalized_properties = String::from("#Configuration file for VMP\n");
        for (key, value) in &normalized {
            normalized_properties.push_str(key);
            normalized_properties.push('=');
            normalized_properties.push_str(value);
            normalized_properties.push('\n');
        }

        LoadResult {
            config,
            normalized_properties,
            warnings,
        }
    }
}

fn lookup<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
    // java.util.Properties: the last assignment wins.
    entries
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn get_int(
    entries: &[(String, String)],
    key: &str,
    def: i32,
    normalized: &mut Vec<(String, String)>,
) -> i32 {
    let value = match lookup(entries, key) {
        Some(text) => match text.trim().parse::<i32>() {
            Ok(i) => i,
            Err(_) => def,
        },
        None => def,
    };
    normalized.push((key.to_string(), value.to_string()));
    value
}

fn get_boolean0(
    entries: &[(String, String)],
    key: &str,
    def: bool,
    normalized: &mut Vec<(String, String)>,
) -> bool {
    let parsed = lookup(entries, key)
        .map(|text| {
            let trimmed = text.trim();
            if trimmed.eq_ignore_ascii_case("true") {
                true
            } else if trimmed.eq_ignore_ascii_case("false") {
                false
            } else {
                def
            }
        })
        .unwrap_or(def);
    normalized.push((key.to_string(), parsed.to_string()));
    parsed
}

fn get_boolean(
    entries: &[(String, String)],
    key: &str,
    def: bool,
    incompatibilities: &[Incompatibility],
    ignore_incompatible: bool,
    normalized: &mut Vec<(String, String)>,
    warnings: &mut Vec<String>,
) -> bool {
    let mut value = get_boolean0(entries, key, def, normalized);
    for incompatibility in incompatibilities.iter().filter(|i| i.key == key) {
        if ignore_incompatible {
            warnings.push(format!(
                "Ignoring incompatibility of {} (defined in {}@{})",
                key, incompatibility.mod_id, incompatibility.mod_version
            ));
        } else {
            warnings.push(format!(
                "Forcing {} in vmp.properties to be disabled (defined in {}@{}) (you may override this with -Dvmp.ignoreIncompatibleConfig=true)",
                key, incompatibility.mod_id, incompatibility.mod_version
            ));
            value = false;
        }
    }
    value
}

/// The java.util.Properties reader subset the config needs: comment lines
/// (# or !), =/:/whitespace separators, escape sequences, and backslash
/// line continuations. Properties.store output is plain key=value lines.
fn parse_properties(text: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        // Backslash continuations: a line ends with an ODD number of
        // trailing backslashes when the last one is an escape marker.
        let mut logical = trimmed.to_string();
        while trailing_backslashes(&logical) % 2 == 1 {
            logical.pop(); // drop the continuation marker
            match lines.next() {
                Some(next) => logical.push_str(next.trim_start()),
                None => break,
            }
        }
        if let Some((key, value)) = split_entry(&logical) {
            entries.push((key, value));
        }
    }
    entries
}

fn trailing_backslashes(s: &str) -> usize {
    s.chars().rev().take_while(|c| *c == '\\').count()
}

fn split_entry(line: &str) -> Option<(String, String)> {
    let chars: Vec<char> = line.chars().collect();
    let mut key = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            if let Some(next) = chars.get(i + 1) {
                key.push(*next);
                i += 2;
                continue;
            }
            return None;
        }
        if c == '=' || c == ':' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            break;
        }
        if c.is_whitespace() {
            // whitespace separator: the value may start after more whitespace
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && chars[i] != '=' && chars[i] != ':' {
                return Some((key, unescape(&chars[i..].iter().collect::<String>())));
            }
            continue;
        }
        key.push(c);
        i += 1;
    }
    if i > chars.len() {
        return None;
    }
    let value = unescape(&chars[i..].iter().collect::<String>());
    Some((key, value))
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                out.push(char::from_u32(code).unwrap_or('\u{0}'));
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inc(key: &str) -> Incompatibility {
        Incompatibility {
            key: key.to_string(),
            mod_id: "othermod".to_string(),
            mod_version: "1.2.3".to_string(),
        }
    }

    #[test]
    fn missing_file_yields_defaults() {
        let result = Config::load("", &[], false);
        assert_eq!(result.config, Config::default());
        assert_eq!(result.config.target_chunk_send_rate, -1);
        assert!(result.config.use_async_chunks_on_login);
        assert!(!result.config.use_async_chunks_on_some_commands);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn normalized_write_back_lists_every_key() {
        let result = Config::load("# manual edit\ntarget_chunk_send_rate=5\n", &[], false);
        assert!(
            result
                .normalized_properties
                .starts_with("#Configuration file for VMP\n")
        );
        for key in [
            "target_chunk_send_rate",
            "use_async_logging",
            "use_async_chunks_on_login_v2",
        ] {
            assert!(
                result.normalized_properties.contains(&format!("{key}=")),
                "{key} missing"
            );
        }
    }

    #[test]
    fn malformed_values_fall_back_to_defaults() {
        let result = Config::load(
            "target_chunk_send_rate=notanumber\nuse_async_portals=1\nuse_async_logging=TRUE\n",
            &[],
            false,
        );
        assert_eq!(result.config.target_chunk_send_rate, -1);
        assert!(result.config.use_async_portals); // "1" is not a boolean: default
        assert!(result.config.use_async_logging); // trimmed, case-insensitive true
    }

    #[test]
    fn last_assignment_wins_like_java_properties() {
        let result = Config::load(
            "target_chunk_send_rate=1\ntarget_chunk_send_rate=2\n",
            &[],
            false,
        );
        assert_eq!(result.config.target_chunk_send_rate, 2);
    }

    #[test]
    fn incompatibility_forces_boolean_off_with_upstream_wording() {
        let result = Config::load("", &[inc("use_async_portals")], false);
        assert!(!result.config.use_async_portals);
        assert_eq!(
            result.warnings,
            vec!["Forcing use_async_portals in vmp.properties to be disabled (defined in othermod@1.2.3) (you may override this with -Dvmp.ignoreIncompatibleConfig=true)".to_string()]
        );
        // the write-back keeps the parsed value, as upstream does
        assert!(
            result
                .normalized_properties
                .contains("use_async_portals=true")
        );
    }

    #[test]
    fn ignore_flag_skips_the_forcing() {
        let result = Config::load("", &[inc("use_async_portals")], true);
        assert!(result.config.use_async_portals);
        assert_eq!(
            result.warnings,
            vec![
                "Ignoring incompatibility of use_async_portals (defined in othermod@1.2.3)"
                    .to_string()
            ]
        );
    }

    #[test]
    fn properties_reader_handles_separators_and_comments() {
        let entries =
            parse_properties("a=1\n! comment\n# comment\n  b : 2\n\tc 3\nd=spaced value\n");
        let find = |k: &str| {
            entries
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(find("a").as_deref(), Some("1"));
        assert_eq!(find("b").as_deref(), Some("2"));
        assert_eq!(find("c").as_deref(), Some("3"));
        assert_eq!(find("d").as_deref(), Some("spaced value"));
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn escapes_and_continuations_parse() {
        let entries = parse_properties("path=C\\:\\\\dir\nmulti=one\\\n  two\n");
        let find = |k: &str| {
            entries
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(find("path").as_deref(), Some("C:\\dir"));
        assert_eq!(find("multi").as_deref(), Some("onetwo"));
    }
}
