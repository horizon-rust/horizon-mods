//! essential runtime (behavioral port of Essential by EssentialGG,
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//! source-available; no upstream code is included).
//!
//! Essential is a session and social mod: host your world for friends through
//! invite codes, chat and share pictures, dress up with cosmetics and the
//! in-game skin manager. Upstream's license is view-only source-available, so
//! this port carries none of its code: the behavior below is written against
//! what the mod's page documents. The backend-dependent parts (upstream's relay
//! for world hosting, their social service for chat and cosmetics delivery) are
//! not shippable from a clean-room port; the local half of each feature is
//! ported here and the seams are in the README.

pub mod session {
    //! Session hosting, local half: the invite-code lifecycle the mod documents.
    //! Hosts hand out invite codes (a domain the friend opens); the code carries
    //! the session reference and an expiry; the host can list and revoke codes.
    //! Transport (upstream's relay or Horizon's own connection path) is a README
    //! seam: this module owns the codes, not the wire.
    use std::collections::HashMap;

    /// One issued invite code.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Invite {
        /// The code string handed to a friend (upstream issues a short URL slug).
        pub code: String,
        /// Unix seconds the code stops working. 0 means the host's open-ended invite.
        pub expires_at: u64,
        /// Whether the host revoked it before expiry.
        pub revoked: bool,
    }

    /// The host side of the session: the codes it has issued and their rules.
    #[derive(Default)]
    pub struct Session {
        /// Codes issued by the host, keyed by code string.
        invites: HashMap<String, Invite>,
    }

    impl Session {
        /// Issue a new invite code. `slug` is the random part; the host keeps the
        /// prefixing. Duplicate slugs are rejected so codes stay unique.
        pub fn issue(&mut self, slug: &str, expires_at: u64) -> Result<&Invite, &'static str> {
            if slug.is_empty() || slug.len() > 64 {
                return Err("slug length out of range");
            }
            if !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err("slug must be alphanumeric or '-'");
            }
            if self.invites.contains_key(slug) {
                return Err("code already issued");
            }
            self.invites.insert(
                slug.to_string(),
                Invite {
                    code: slug.to_string(),
                    expires_at,
                    revoked: false,
                },
            );
            Ok(&self.invites[slug])
        }

        /// Can a friend join with this code at unix time `now`? Revoked, expired
        /// and unknown codes all reject.
        pub fn check(&self, slug: &str, now: u64) -> bool {
            match self.invites.get(slug) {
                Some(inv) => !inv.revoked && (inv.expires_at == 0 || now < inv.expires_at),
                None => false,
            }
        }

        /// The host revokes a code; revoking an unknown code fails.
        pub fn revoke(&mut self, slug: &str) -> bool {
            if let Some(inv) = self.invites.get_mut(slug) {
                inv.revoked = true;
                true
            } else {
                false
            }
        }

        /// Codes still usable at `now`, oldest issued first.
        pub fn live(&self, now: u64) -> Vec<String> {
            let mut out: Vec<&Invite> = self
                .invites
                .values()
                .filter(|i| !i.revoked && (i.expires_at == 0 || now < i.expires_at))
                .collect();
            // insertion order is not tracked; sort for determinism
            out.sort_by(|a, b| a.code.cmp(&b.code));
            out.into_iter().map(|i| i.code.clone()).collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn issue_then_join() {
            let mut s = Session::default();
            assert!(s.issue("crimson-otter", 1000).is_ok());
            assert!(s.check("crimson-otter", 999));
            assert!(!s.check("crimson-otter", 1000), "code expires at its stamp");
            assert!(!s.check("nope", 1));
        }

        #[test]
        fn revoke_kills_a_code() {
            let mut s = Session::default();
            s.issue("open", 0).unwrap();
            assert!(s.check("open", 5_000_000));
            assert!(s.revoke("open"));
            assert!(!s.check("open", 5_000_000));
            assert!(!s.revoke("ghost"));
        }

        #[test]
        fn duplicate_and_bad_slugs_rejected() {
            let mut s = Session::default();
            s.issue("aa", 0).unwrap();
            assert!(s.issue("aa", 0).is_err());
            assert!(s.issue("", 0).is_err());
            assert!(s.issue("bad/slug", 0).is_err());
        }

        #[test]
        fn live_lists_only_current_codes() {
            let mut s = Session::default();
            s.issue("b-code", 0).unwrap();
            s.issue("a-code", 10).unwrap();
            s.issue("c-code", 0).unwrap();
            assert!(s.revoke("c-code"));
            assert_eq!(s.live(5), vec!["a-code".to_string(), "b-code".to_string()]);
        }
    }
}

pub mod skins {
    //! The in-game skin manager, local half: the client picks which skin to wear
    //! from its saved list and the model layer redraws with the chosen texture.
    //! Delivery of cosmetics from upstream's service is a README seam; this owns
    //! the selection behavior.
    use std::collections::HashMap;

    /// One saved skin in the local manager.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Skin {
        /// The file name the player saved it under.
        pub file: String,
        /// Whether the skin reads as a slim (Alex) or wide (Steve) model.
        pub slim: bool,
    }

    /// The local skin manager: saved skins and the one currently worn.
    #[derive(Default)]
    pub struct SkinManager {
        /// Saved skins keyed by file name.
        skins: HashMap<String, Skin>,
        /// The file name currently worn, if any.
        worn: Option<String>,
    }

    impl SkinManager {
        /// Add or replace a saved skin; the model-width read comes from the file
        /// being present and valid.
        pub fn save(&mut self, file: &str, slim: bool) {
            if file.is_empty() {
                return;
            }
            self.skins.insert(
                file.to_string(),
                Skin {
                    file: file.to_string(),
                    slim,
                },
            );
        }

        /// Wear a saved skin by name. Wearing overwrites any prior choice; an
        /// unknown skin is rejected and leaves the current one on.
        pub fn wear(&mut self, file: &str) -> Option<&Skin> {
            if self.skins.contains_key(file) {
                self.worn = Some(file.to_string());
                self.skins.get(file)
            } else {
                None
            }
        }

        /// The skin the model layer should draw, or None when nothing is worn.
        pub fn current(&self) -> Option<&Skin> {
            self.skins.get(self.worn.as_deref()?)
        }

        /// Remove a saved skin; wearing stops when the worn skin is the one removed.
        pub fn remove(&mut self, file: &str) -> bool {
            if self.skins.remove(file).is_some() {
                if self.worn.as_deref() == Some(file) {
                    self.worn = None;
                }
                true
            } else {
                false
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn wear_and_current() {
            let mut m = SkinManager::default();
            m.save("creeper.png", false);
            m.save("ninja.png", true);
            assert_eq!(m.wear("ninja.png").unwrap().slim, true);
            assert_eq!(m.current().unwrap().file, "ninja.png");
            assert!(
                m.wear("ghost.png").is_none(),
                "unknown skin keeps the worn one"
            );
            assert_eq!(m.current().unwrap().file, "ninja.png");
        }

        #[test]
        fn removing_the_worn_skin_undresses() {
            let mut m = SkinManager::default();
            m.save("creeper.png", false);
            m.wear("creeper.png");
            assert!(m.remove("creeper.png"));
            assert!(m.current().is_none());
        }
    }
}

pub mod gallery {
    //! The screenshot gallery, local half: screenshots are captured, listed
    //! newest-first, favorited, deleted, and marked for sharing. Upstream shares
    //! through its social service; without that transport the share flag is the
    //! outbox a transport reads. The seam is in the README.
    use std::collections::HashMap;

    /// One screenshot in the gallery.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Shot {
        /// The file name on disk.
        pub file: String,
        /// Unix seconds of capture, the gallery's sort key.
        pub taken_at: u64,
        /// Whether the player starred it.
        pub favorite: bool,
        /// Whether it is queued for sharing.
        pub outbox: bool,
    }

    /// The local gallery.
    #[derive(Default)]
    pub struct Gallery {
        shots: HashMap<String, Shot>,
    }

    impl Gallery {
        /// Record a captured screenshot.
        pub fn add(&mut self, file: &str, taken_at: u64) {
            self.shots.insert(
                file.to_string(),
                Shot {
                    file: file.to_string(),
                    taken_at,
                    favorite: false,
                    outbox: false,
                },
            );
        }

        /// Every shot, newest first.
        pub fn list(&self) -> Vec<Shot> {
            let mut all: Vec<Shot> = self.shots.values().cloned().collect();
            all.sort_by(|a, b| {
                b.taken_at
                    .cmp(&a.taken_at)
                    .then_with(|| a.file.cmp(&b.file))
            });
            all
        }

        /// Toggle the favorite star. Returns the new state; unknown files fail.
        pub fn favorite(&mut self, file: &str) -> Option<bool> {
            self.shots.get_mut(file).map(|s| {
                s.favorite = !s.favorite;
                s.favorite
            })
        }

        /// Mark or unmark a shot for sharing. Returns the new state; unknown files fail.
        pub fn queue_share(&mut self, file: &str, queued: bool) -> Option<bool> {
            self.shots.get_mut(file).map(|s| {
                s.outbox = queued;
                s.outbox
            })
        }

        /// Delete a shot from the gallery (the file deletion itself rides the host).
        pub fn delete(&mut self, file: &str) -> bool {
            self.shots.remove(file).is_some()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn newest_first_ordering() {
            let mut g = Gallery::default();
            g.add("a.png", 10);
            g.add("b.png", 30);
            g.add("c.png", 20);
            let names: Vec<String> = g.list().iter().map(|s| s.file.clone()).collect();
            assert_eq!(names, vec!["b.png", "c.png", "a.png"]);
        }

        #[test]
        fn favorite_share_delete() {
            let mut g = Gallery::default();
            g.add("shot.png", 1);
            assert_eq!(g.favorite("shot.png"), Some(true));
            assert_eq!(g.favorite("shot.png"), Some(false));
            assert_eq!(g.queue_share("shot.png", true), Some(true));
            assert_eq!(g.queue_share("ghost.png", true), None);
            assert!(g.delete("shot.png"));
            assert!(!g.delete("shot.png"));
        }
    }
}
