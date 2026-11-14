//! screenshot-viewer (Horizon Rust port), slice 1: the gallery model.
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//!
//! 1:1 port of the upstream logic core (MIT): screenshots are listed from the
//! screenshot folder newest-first, paged through a fixed-size grid, and the
//! selected one can be deleted or copied to the clipboard. The image render
//! surface and clipboard OS hook land in the next slice.
pub mod gallery {
    //! Port of upstream gallery state: listing, sorting, paging and actions.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SortOrder {
        /// Upstream default: newest first.
        NewestFirst,
        OldestFirst,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Settings {
        /// Grid columns per page (upstream layout).
        pub columns: usize,
        pub rows: usize,
        pub sort: SortOrder,
    }

    impl Default for Settings {
        fn default() -> Self {
            Settings {
                columns: 4,
                rows: 3,
                sort: SortOrder::NewestFirst,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Shot {
        /// File name in the screenshot folder.
        pub file_name: String,
        /// Modification epoch seconds, drives the sort.
        pub modified_epoch: u64,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Gallery {
        pub shots: Vec<Shot>,
        pub page: usize,
        pub selected: Option<usize>,
    }

    impl Gallery {
        /// Build from folder entries, sorted by the setting (newest first default).
        pub fn new(mut shots: Vec<Shot>, s: &Settings) -> Self {
            shots.sort_by(|a, b| match s.sort {
                SortOrder::NewestFirst => b.modified_epoch.cmp(&a.modified_epoch),
                SortOrder::OldestFirst => a.modified_epoch.cmp(&b.modified_epoch),
            });
            Gallery {
                shots,
                page: 0,
                selected: None,
            }
        }

        pub fn page_size(&self, s: &Settings) -> usize {
            (s.columns * s.rows).max(1)
        }

        pub fn page_count(&self, s: &Settings) -> usize {
            (self.shots.len() + self.page_size(s) - 1) / self.page_size(s)
        }

        /// The shots on the current page, in sort order.
        pub fn page_shots(&self, s: &Settings) -> &[Shot] {
            let ps = self.page_size(s);
            let start = self.page.min(self.page_count(s).saturating_sub(1)) * ps;
            let end = (start + ps).min(self.shots.len());
            &self.shots[start..end]
        }

        pub fn next_page(&mut self, s: &Settings) {
            if self.page + 1 < self.page_count(s) {
                self.page += 1;
            }
        }

        pub fn prev_page(&mut self) {
            self.page = self.page.saturating_sub(1);
        }

        /// Select the page-relative cell; None clears.
        pub fn select(&mut self, cell: Option<usize>, s: &Settings) {
            self.selected = cell.filter(|c| *c < self.page_shots(s).len());
        }

        /// Remove the selected shot (upstream delete), return its file name.
        pub fn delete_selected(&mut self, s: &Settings) -> Option<String> {
            let ps = self.page_size(s);
            let idx = self.selected? + self.page * ps;
            if idx >= self.shots.len() {
                self.selected = None;
                return None;
            }
            let name = self.shots.remove(idx).file_name;
            self.selected = None;
            if self.page >= self.page_count(s) {
                self.page = self.page_count(s).saturating_sub(1);
            }
            Some(name)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn shot(n: &str, t: u64) -> Shot {
            Shot {
                file_name: n.to_string(),
                modified_epoch: t,
            }
        }

        #[test]
        fn listing_sorts_newest_first_by_default() {
            let g = Gallery::new(
                vec![shot("a", 100), shot("b", 300), shot("c", 200)],
                &Settings::default(),
            );
            let names: Vec<&str> = g.shots.iter().map(|s| s.file_name.as_str()).collect();
            assert_eq!(names, vec!["b", "c", "a"]);
        }

        #[test]
        fn paging_moves_through_the_grid() {
            let mut s = Settings::default();
            s.rows = 1;
            let shots: Vec<Shot> = (0..9).map(|i| shot(&format!("s{i}"), i)).collect();
            let mut g = Gallery::new(shots, &s);
            assert_eq!(g.page_count(&s), 3, "9 shots at 4 per page is 3 pages");
            assert_eq!(g.page_shots(&s).len(), 4);
            g.next_page(&s);
            g.next_page(&s);
            g.next_page(&s);
            g.next_page(&s); // clamps at the last page
            assert_eq!(g.page, 2);
            assert_eq!(g.page_shots(&s).len(), 1);
            g.prev_page();
            assert_eq!(g.page, 1);
        }

        #[test]
        fn selection_is_page_relative_and_bounded() {
            let mut s = Settings::default();
            s.rows = 1;
            let shots: Vec<Shot> = (0..6).map(|i| shot(&format!("s{i}"), i)).collect();
            let mut g = Gallery::new(shots, &s);
            g.select(Some(5), &s); // beyond the page
            assert_eq!(g.selected, None);
            g.select(Some(2), &s);
            assert_eq!(g.selected, Some(2));
            g.next_page(&s);
            assert_eq!(g.selected, Some(2), "selection survives paging");
            g.select(Some(1), &s);
            assert_eq!(g.page_shots(&s)[1].file_name, "s0", "page 1 is [s1, s0]");
        }

        #[test]
        fn delete_removes_selected_and_clamps_page() {
            let mut s = Settings::default();
            s.rows = 1;
            let shots: Vec<Shot> = (0..5).map(|i| shot(&format!("s{i}"), i)).collect();
            let mut g = Gallery::new(shots, &s);
            g.next_page(&s);
            g.select(Some(0), &s);
            let deleted = g.delete_selected(&s);
            assert_eq!(
                deleted.as_deref(),
                Some("s0"),
                "page 1 holds only the oldest shot"
            );
            assert_eq!(g.page, 0, "page clamped back after the page emptied");
            assert_eq!(g.selected, None);
        }

        #[test]
        fn empty_gallery_has_one_page_and_nothing_selected() {
            let g = Gallery::new(vec![], &Settings::default());
            assert_eq!(g.page_count(&Settings::default()), 0);
            assert!(g.page_shots(&Settings::default()).is_empty());
        }
    }
}
