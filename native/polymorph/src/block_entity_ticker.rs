//! Upstream `common/util/BlockEntityTicker.java`, translated 1:1. The
//! `BlockEntity` map keys are owner tokens and the per-tick liveness check
//! (`be.isRemoved() || be.getLevel() == null ||
//! be.getLevel().isClientSide()`) is an entry seam closure.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::block_entity_recipe_data::AbstractBlockEntityRecipeData;

/// The per-entry seam: `true` while the block entity is still ticking,
/// i.e. not removed, its level present and server-side.
pub type IsAlive = Rc<dyn Fn() -> bool>;

struct TickerEntry {
    is_alive: IsAlive,
    data: RefCell<AbstractBlockEntityRecipeData<()>>,
}

thread_local! {
    static TICKABLE_BLOCKS: RefCell<HashMap<u64, TickerEntry>> =
        RefCell::new(HashMap::new());
}

/// Upstream static holder `BlockEntityTicker`.
pub struct BlockEntityTicker;

impl BlockEntityTicker {
    /// `tick`: live entries tick, dead ones are collected then removed.
    pub fn tick() {
        TICKABLE_BLOCKS.with(|blocks| {
            let mut blocks = blocks.borrow_mut();
            let mut to_remove = Vec::new();
            for (be, entry) in blocks.iter() {
                if (entry.is_alive)() {
                    entry.data.borrow_mut().tick();
                } else {
                    to_remove.push(*be);
                }
            }
            for be in to_remove {
                blocks.remove(&be);
            }
        });
    }

    /// `add(ServerPlayer, IBlockEntityRecipeData)`: keyed by the owner the
    /// caller resolves from `recipeData.getOwner()`; an existing entry gets
    /// the listener, otherwise the passed data keeps the listener and is
    /// stored.
    pub fn add(
        server_player: u64,
        owner: u64,
        recipe_data: AbstractBlockEntityRecipeData<()>,
        is_alive: IsAlive,
    ) {
        let mut data = recipe_data;
        let existing = TICKABLE_BLOCKS.with(|blocks| {
            let mut blocks = blocks.borrow_mut();
            match blocks.get_mut(&owner) {
                Some(entry) => {
                    entry.data.borrow_mut().add_listener(server_player);
                    true
                }
                None => false,
            }
        });
        if !existing {
            data.add_listener(server_player);
            TICKABLE_BLOCKS.with(|blocks| {
                blocks.borrow_mut().insert(
                    owner,
                    TickerEntry {
                        is_alive,
                        data: RefCell::new(data),
                    },
                );
            });
        }
    }

    /// `remove(ServerPlayer)`: every entry drops the listener; entries left
    /// without listeners are removed.
    pub fn remove(server_player: u64) {
        TICKABLE_BLOCKS.with(|blocks| {
            let mut blocks = blocks.borrow_mut();
            let mut to_remove = Vec::new();
            for (be, entry) in blocks.iter_mut() {
                entry.data.borrow_mut().remove_listener(server_player);
                if entry.data.borrow().get_listeners().is_empty() {
                    to_remove.push(*be);
                }
            }
            for be in to_remove {
                blocks.remove(&be);
            }
        });
    }

    /// `clear`.
    pub fn clear() {
        TICKABLE_BLOCKS.with(|blocks| blocks.borrow_mut().clear());
    }

    /// A read borrow of an owner's entry. Upstream reads the same
    /// capability object it passed into `add`, which is the map's entry
    /// once stored.
    pub fn read<T>(
        owner: u64,
        f: impl FnOnce(&AbstractBlockEntityRecipeData<()>) -> T,
    ) -> Option<T> {
        TICKABLE_BLOCKS.with(|blocks| {
            blocks
                .borrow()
                .get(&owner)
                .map(|entry| f(&entry.data.borrow()))
        })
    }

    /// Number of live entries (test surface for the map's size).
    pub fn entry_count() -> usize {
        TICKABLE_BLOCKS.with(|blocks| blocks.borrow().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use crate::recipe_pair::ItemStack;

    fn empty_input() -> Vec<ItemStack> {
        Vec::new()
    }

    fn data_with_listener() -> AbstractBlockEntityRecipeData<()> {
        AbstractBlockEntityRecipeData::new((), empty_input)
    }

    fn alive() -> IsAlive {
        Rc::new(|| true)
    }

    #[test]
    fn add_registers_the_owner_and_listener() {
        BlockEntityTicker::clear();
        BlockEntityTicker::add(7, 42, data_with_listener(), alive());
        assert_eq!(BlockEntityTicker::entry_count(), 1);
        assert_eq!(
            BlockEntityTicker::read(42, |d| d.get_listeners()),
            Some(vec![7])
        );
    }

    #[test]
    fn second_add_reuses_the_existing_entry() {
        BlockEntityTicker::clear();
        BlockEntityTicker::add(1, 42, data_with_listener(), alive());
        // The second add's data is discarded; the map's entry gains the listener.
        BlockEntityTicker::add(2, 42, data_with_listener(), alive());
        assert_eq!(BlockEntityTicker::entry_count(), 1);
        // The base keeps listeners in a HashMap, so the order is the
        // map's, not the adds'.
        let mut listeners = BlockEntityTicker::read(42, |d| d.get_listeners()).unwrap();
        listeners.sort_unstable();
        assert_eq!(listeners, vec![1, 2]);
    }

    #[test]
    fn tick_runs_live_entries_and_drops_dead_ones() {
        BlockEntityTicker::clear();
        let ticked = Rc::new(Cell::new(0u32));
        let t = ticked.clone();
        let mut data = AbstractBlockEntityRecipeData::new((), move || {
            t.set(t.get() + 1);
            Vec::new()
        });
        data.add_listener(5);
        BlockEntityTicker::add(5, 9, data, alive());
        BlockEntityTicker::add(5, 10, data_with_listener(), Rc::new(|| false));
        BlockEntityTicker::tick();
        assert_eq!(ticked.get(), 1, "the dead entry must not tick");
        assert_eq!(BlockEntityTicker::entry_count(), 1, "the dead entry is removed");
    }

    #[test]
    fn remove_drops_entries_left_without_listeners() {
        BlockEntityTicker::clear();
        BlockEntityTicker::add(1, 3, data_with_listener(), alive());
        BlockEntityTicker::add(1, 4, data_with_listener(), alive());
        BlockEntityTicker::add(2, 4, data_with_listener(), alive());
        BlockEntityTicker::remove(1);
        assert_eq!(BlockEntityTicker::entry_count(), 1, "entry 3 had only player 1");
        assert_eq!(
            BlockEntityTicker::read(4, |d| d.get_listeners()),
            Some(vec![2])
        );
        BlockEntityTicker::remove(2);
        assert_eq!(BlockEntityTicker::entry_count(), 0);
    }

    #[test]
    fn clear_empties_the_map() {
        BlockEntityTicker::clear();
        BlockEntityTicker::add(1, 3, data_with_listener(), alive());
        BlockEntityTicker::clear();
        assert_eq!(BlockEntityTicker::entry_count(), 0);
    }
}
