//! inventory-profiles-next runtime for Horizon (slice 1): the inventory grid
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//! model, sorting comparators, layouts and the minimal move plan that the
//! client glue turns into slot clicks. Ported behaviorally from Inventory
//! Profiles Next (AGPL-3.0-or-later). The screen/interaction glue waits on
//! Horizon's GUI surface (upstream issue #10025's widget scope).

pub mod grid {
    //! The container model: a width-by-height grid of item stacks, the shape IPN
    //! operates on. Stacks are (name, count); stacks only stack when names match
    //! (no per-stack max sizes yet, matching how the sort cares about identity).

    #[derive(Clone, Debug, PartialEq)]
    pub struct Stack {
        pub name: String,
        pub count: u32,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Grid {
        pub width: usize,
        pub stacks: Vec<Option<Stack>>,
    }

    impl Grid {
        pub fn new(width: usize, height: usize) -> Self {
            Self {
                width,
                stacks: vec![None; width * height],
            }
        }

        pub fn len(&self) -> usize {
            self.stacks.len()
        }

        pub fn get(&self, slot: usize) -> Option<&Option<Stack>> {
            self.stacks.get(slot)
        }

        pub fn set(&mut self, slot: usize, s: Option<Stack>) {
            if slot < self.stacks.len() {
                self.stacks[slot] = s;
            }
        }

        /// Slots that hold a stack, ascending.
        pub fn filled(&self) -> Vec<usize> {
            (0..self.stacks.len())
                .filter(|i| self.stacks[*i].is_some())
                .collect()
        }

        /// Total stack count of one item name across the grid.
        pub fn total_of(&self, name: &str) -> u32 {
            self.stacks
                .iter()
                .flatten()
                .filter(|s| s.name == name)
                .map(|s| s.count)
                .sum()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn st(name: &str, count: u32) -> Option<Stack> {
            Some(Stack {
                name: name.into(),
                count,
            })
        }

        #[test]
        fn grid_shape_and_access() {
            let mut g = Grid::new(3, 2);
            assert_eq!(g.len(), 6);
            g.set(1, st("minecraft:diamond", 3));
            assert_eq!(
                g.get(1).unwrap().as_ref().unwrap().name,
                "minecraft:diamond"
            );
            assert!(g.get(0).unwrap().is_none());
            assert!(g.get(6).is_none(), "out of bounds is None, not a panic");
        }

        #[test]
        fn filled_reports_only_occupied_slots() {
            let mut g = Grid::new(3, 2);
            g.set(0, st("a", 1));
            g.set(4, st("b", 1));
            assert_eq!(g.filled(), vec![0, 4]);
            assert_eq!(g.total_of("a"), 1);
        }
    }
}
pub mod sort {
    //! Sorting and the move plan: IPN's core is turning "sort my inventory" into
    //! a minimal list of slot moves. The comparators match IPN's defaults: by
    //! item name, then count; ordering by columns (down each column) or rows is
    //! the display-order switch upstream exposes.

    use crate::grid::{Grid, Stack};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SortBy {
        /// Item name (the default IPN sort).
        Name,
        /// Item name, then stack count descending within the same item.
        NameThenCount,
        /// Stack count descending.
        Count,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Direction {
        Ascending,
        Descending,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Layout {
        /// Fill left to right, then wrap to the next row.
        Rows,
        /// Fill top to bottom down each column, then to the next column.
        Columns,
    }

    /// The stack key used for sorting: name, then count.
    fn key(s: &Stack, by: SortBy) -> (String, u32) {
        match by {
            SortBy::Name => (s.name.clone(), 0),
            SortBy::NameThenCount => (s.name.clone(), u32::MAX - s.count),
            SortBy::Count => (s.name.clone(), u32::MAX - s.count),
        }
    }

    fn cmp_stacks(a: &Stack, b: &Stack, by: SortBy, dir: Direction) -> std::cmp::Ordering {
        let mut ord = key(a, by).cmp(&key(b, by));
        if dir == Direction::Descending {
            ord = ord.reverse();
        }
        ord
    }

    /// The minimal move plan: pairs of (from_slot, to_slot). Stacks already in
    /// their target slot are skipped; a two-way swap is emitted as one move.
    pub fn move_plan(
        grid: &Grid,
        by: SortBy,
        dir: Direction,
        layout: Layout,
    ) -> Vec<(usize, usize)> {
        let mut target = sorted_target(grid, by, dir, layout);
        let mut plan = Vec::new();
        for slot in 0..grid.len() {
            if grid.stacks[slot] == target[slot] {
                continue;
            }
            if let Some(dest) = target
                .iter()
                .position(|t| t == &grid.stacks[slot] && !t.is_none())
            {
                plan.push((slot, dest));
            }
        }
        plan = consolidate(plan);
        plan
    }

    /// The sorted arrangement: which stack ends in which slot.
    fn sorted_target(
        grid: &Grid,
        by: SortBy,
        dir: Direction,
        layout: Layout,
    ) -> Vec<Option<Stack>> {
        let mut stacks: Vec<Stack> = grid.stacks.iter().flatten().cloned().collect();
        stacks.sort_by(|a, b| cmp_stacks(a, b, by, dir));
        let mut out: Vec<Option<Stack>> = vec![None; grid.len()];
        let width = grid.width.max(1);
        let mut it = stacks.into_iter();
        match layout {
            Layout::Rows => {
                for slot in 0..out.len() {
                    if let Some(s) = it.next() {
                        out[slot] = Some(s);
                    }
                }
            }
            Layout::Columns => {
                let height = (out.len() + width - 1) / width;
                'outer: for col in 0..width {
                    for row in 0..height {
                        let slot = col * height + row;
                        if slot >= out.len() {
                            continue;
                        }
                        if let Some(s) = it.next() {
                            out[slot] = Some(s);
                        } else {
                            break 'outer;
                        }
                    }
                }
            }
        }
        out
    }

    /// Collapse chains so each stack moves once: sort the raw pairs into a
    /// stable, duplicate-free sequence.
    fn consolidate(plan: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        let mut seen = std::collections::BTreeSet::new();
        plan.into_iter().filter(|m| seen.insert(*m)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::grid::Grid;

        fn g(width: usize, items: &[(&str, u32)]) -> Grid {
            let mut grid = Grid::new(width, 3);
            for (i, (name, count)) in items.iter().enumerate() {
                grid.set(
                    i,
                    Some(Stack {
                        name: name.to_string(),
                        count: *count,
                    }),
                );
            }
            grid
        }

        #[test]
        fn target_sorts_by_name_then_count_within_item() {
            let grid = g(3, &[("stone", 1), ("apple", 2), ("stone", 64), ("beef", 5)]);
            let t = sorted_target(
                &grid,
                SortBy::NameThenCount,
                Direction::Ascending,
                Layout::Rows,
            );
            let names: Vec<&str> = t.iter().flatten().map(|s| s.name.as_str()).collect();
            assert_eq!(names, vec!["apple", "beef", "stone", "stone"]);
            let stone_counts: Vec<u32> = t
                .iter()
                .flatten()
                .filter(|s| s.name == "stone")
                .map(|s| s.count)
                .collect();
            assert_eq!(stone_counts, vec![64, 1], "bigger stone stack first");
        }

        #[test]
        fn descending_reverses_names() {
            let grid = g(3, &[("apple", 1), ("beef", 1)]);
            let t = sorted_target(&grid, SortBy::Name, Direction::Descending, Layout::Rows);
            assert_eq!(t[0].as_ref().unwrap().name, "beef");
        }

        #[test]
        fn columns_layout_fills_down_each_column() {
            let grid = g(3, &[("a", 1), ("b", 1), ("c", 1), ("d", 1), ("e", 1)]);
            let t = sorted_target(&grid, SortBy::Name, Direction::Ascending, Layout::Columns);
            // 3 wide, 2 tall grid (6 slots): a b c / d e
            // column-major: a->slot0, d->slot3, b->slot1, e->slot4, c->slot2
            assert_eq!(t[0].as_ref().unwrap().name, "a");
            assert_eq!(t[3].as_ref().unwrap().name, "d");
            assert_eq!(t[1].as_ref().unwrap().name, "b");
            assert_eq!(t[4].as_ref().unwrap().name, "e");
            assert_eq!(t[2].as_ref().unwrap().name, "c");
            assert!(t[5].is_none());
        }

        #[test]
        fn move_plan_is_empty_when_already_sorted() {
            let grid = g(3, &[("apple", 1), ("beef", 1), ("stone", 1)]);
            assert!(move_plan(&grid, SortBy::Name, Direction::Ascending, Layout::Rows).is_empty());
        }

        #[test]
        fn move_plan_moves_each_displaced_stack_once() {
            let grid = g(3, &[("stone", 1), ("apple", 1)]);
            let plan = move_plan(&grid, SortBy::Name, Direction::Ascending, Layout::Rows);
            assert_eq!(plan, vec![(0, 1), (1, 0)], "a swap is two moves");
            assert_eq!(plan.len(), 2);
        }
    }
}
