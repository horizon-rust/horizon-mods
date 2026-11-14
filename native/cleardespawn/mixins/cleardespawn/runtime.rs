//! cleardespawn (Horizon Rust port), slice 1: the despawn filter model.
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//!
//! 1:1 port of the upstream logic core (MIT): when an item entity despawns,
//! its poof particles are suppressed, except for entity ids on the ignore
//! list. Sound and render seams land in the next slice.
pub mod despawn {
    //! Port of upstream despawn-particle suppression and the ignore list.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Settings {
        /// Entity ids (upstream "ignoreList") that keep their vanilla despawn
        /// particles.
        pub ignore_list: Vec<String>,
    }

    impl Default for Settings {
        fn default() -> Self {
            Settings {
                ignore_list: Vec::new(),
            }
        }
    }

    impl Settings {
        /// Upstream stores the list lower-cased, one id per line.
        pub fn normalize(&mut self) {
            self.ignore_list = self
                .ignore_list
                .iter()
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        }

        pub fn ignores(&self, entity_id: &str) -> bool {
            self.ignore_list
                .iter()
                .any(|e| e == &entity_id.to_ascii_lowercase())
        }
    }

    /// Upstream decision: an entity's despawn particles are cleared unless its id
    /// is on the ignore list.
    pub fn should_clear_despawn_particles(settings: &Settings, entity_id: &str) -> bool {
        !settings.ignores(entity_id)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn clears_particles_for_normal_items() {
            let s = Settings::default();
            assert!(should_clear_despawn_particles(&s, "minecraft:item"));
            assert!(should_clear_despawn_particles(&s, "minecraft:arrow"));
        }

        #[test]
        fn ignored_entities_keep_their_particles() {
            let mut s = Settings {
                ignore_list: vec!["minecraft:shulker_bullet".into()],
            };
            s.normalize();
            assert!(!should_clear_despawn_particles(
                &s,
                "minecraft:shulker_bullet"
            ));
            assert!(should_clear_despawn_particles(&s, "minecraft:item"));
        }

        #[test]
        fn matching_is_case_insensitive_like_upstream_storage() {
            let mut s = Settings {
                ignore_list: vec!["MINECRAFT:ITEM".into()],
            };
            s.normalize();
            assert_eq!(s.ignore_list, vec!["minecraft:item"]);
            assert!(s.ignores("minecraft:ITEM"));
        }

        #[test]
        fn normalize_drops_blank_lines() {
            let mut s = Settings {
                ignore_list: vec!["  ".into(), "a".into(), "".into()],
            };
            s.normalize();
            assert_eq!(s.ignore_list, vec!["a"]);
        }
    }
}
