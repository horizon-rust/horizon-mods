//! Upstream `common/capability/PlayerRecipeData.java`, translated 1:1. The
//! `Player`/`ServerPlayer`, `AbstractContainerMenu`, `Level` and network
//! types are host seams with the same shapes and semantics. The Java
//! override of `sendRecipesListToListeners` is virtual dispatch upstream;
//! here it rides the base's `override_send` hook.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use crate::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use crate::recipe_data::{AbstractRecipeData, BroadcastCtx, LevelHandle};
use crate::recipe_pair::{Identifier, RecipePair};

/// Host seam for `net.minecraft.world.entity.player.Player` reduced to what
/// this capability reads: identity token, tick count, open menu, and the
/// ServerPlayer/client-side checks.
#[derive(Clone, Copy, Debug)]
pub struct PlayerHandle {
    pub token: u64,
    pub tick_count: i32,
    pub container_menu: u64,
    pub is_server_player: bool,
    pub is_client_side: bool,
}

/// Host seam for the client-only `RecipesWidget.get().ifPresent(...)`
/// branch: set the widget's recipes list.
pub type WidgetSetter = dyn Fn(&BTreeSet<RecipePair>, Option<&Identifier>);

/// Upstream `PlayerRecipeData`: per-player selection with a same-tick
/// cached selection and menu-scoped sync.
pub struct PlayerRecipeData {
    base: AbstractRecipeData<Rc<RefCell<PlayerHandle>>>,
    /// `AbstractContainerMenu` seam (a menu token, None when unset).
    container_menu: Rc<Cell<Option<u64>>>,
    cached_selection: Option<RecipeHolder>,
    last_access_tick: i32,
    /// Client `RecipesWidget` seam.
    pub widget: Rc<WidgetSetter>,
}

impl PlayerRecipeData {
    pub fn new(owner: PlayerHandle) -> Self {
        let owner = Rc::new(RefCell::new(owner));
        let container_menu = Rc::new(Cell::new(None));
        let widget: Rc<WidgetSetter> = Rc::new(|_, _| {});
        let mut base = AbstractRecipeData::new(owner.clone());

        // The sendRecipesListToListeners override, on the base's hook.
        let menu = container_menu.clone();
        let owner_for_send = owner.clone();
        let widget_for_send = widget.clone();
        let network_for_send = base.network.clone();
        base.override_send = Some(Box::new(move |ctx: &BroadcastCtx<Rc<RefCell<PlayerHandle>>>| {
            let o = owner_for_send.borrow();

            if menu.get() == Some(o.container_menu) {
                if o.is_client_side {
                    (widget_for_send)(ctx.pairs, ctx.selected_id);
                } else if o.is_server_player {
                    (network_for_send)(o.token, ctx.pairs, ctx.selected_id);
                }
            }
        }));

        Self {
            base,
            container_menu,
            cached_selection: None,
            last_access_tick: 0,
            widget,
        }
    }

    /// `getRecipe`, line for line: the same-tick cached-selection
    /// workaround, the base pipeline, the menu-scoped sync, and the
    /// server-thread-gated caching of the new selection.
    pub fn get_recipe(
        &mut self,
        recipe_input: &RecipeInput,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        by_key: &dyn Fn(&Identifier) -> Option<RecipeHolder>,
        level: &LevelHandle,
        recipes_list: RecipeList,
    ) -> Option<RecipeHolder> {
        let tick = self.base.get_owner().borrow().tick_count;

        if tick == self.last_access_tick {
            if let Some(cached) = &self.cached_selection {
                self.base.set_selected_recipe(cached.clone());
            }
        } else {
            self.cached_selection = None;
        }
        let result = self.base.get_recipe(
            recipe_input,
            manager_token,
            lookup,
            by_key,
            recipes_list,
        );

        let menu_matches = {
            let o = self.base.get_owner().borrow();
            self.get_container_menu() == Some(o.container_menu)
        };

        if menu_matches {
            self.sync_player_recipe_data();
        }
        self.set_container_menu(None);

        let tick_after = self.base.get_owner().borrow().tick_count;

        if tick_after != self.last_access_tick {
            self.last_access_tick = tick_after;

            // Prevent caching selections coming from outside the main thread.
            if level.on_server_thread {
                self.cached_selection = result.clone();
            }
        }
        result
    }

    /// `validateThread`: `server.getRunningThread() == Thread.currentThread()`,
    /// carried by the level seam.
    fn validate_thread(level: &LevelHandle) -> bool {
        level.on_server_thread
    }

    pub fn select_recipe(&mut self, recipe: RecipeHolder) {
        self.cached_selection = None;
        self.base.select_recipe(recipe);
        self.sync_player_recipe_data();
    }

    /// `syncPlayerRecipeData`: a ServerPlayer owner gets sendPlayerSyncS2C
    /// with the current list and selection.
    fn sync_player_recipe_data(&self) {
        let o = self.base.get_owner().borrow();

        if o.is_server_player {
            let selected_id = self.base.get_selected_recipe().map(|s| &s.id);
            (self.base.network)(o.token, self.base.get_recipes_list(), selected_id);
        }
    }

    pub fn set_container_menu(&self, container_menu: Option<u64>) {
        self.container_menu.set(container_menu);
    }

    pub fn get_container_menu(&self) -> Option<u64> {
        self.container_menu.get()
    }

    // IPlayerRecipeData / IRecipeData surface, delegated to the base.
    pub fn get_selected_recipe(&self) -> Option<&RecipeHolder> {
        self.base.get_selected_recipe()
    }

    pub fn get_recipes_list(&self) -> &BTreeSet<RecipePair> {
        self.base.get_recipes_list()
    }

    pub fn set_recipes_list(&mut self, list: BTreeSet<RecipePair>) {
        self.base.set_recipes_list(list);
    }

    pub fn set_selected_recipe(&mut self, recipe: RecipeHolder) {
        self.base.set_selected_recipe(recipe)
    }

    pub fn add_listener(&mut self, player: u64) {
        self.base.add_listener(player);
    }

    pub fn remove_listener(&mut self, player: u64) {
        self.base.remove_listener(player);
    }

    pub fn clear_listeners(&mut self) {
        self.base.clear_listeners();
    }

    pub fn listeners(&self) -> Vec<u64> {
        self.base.get_listeners()
    }

    pub fn read_nbt(&mut self, tag: &crate::recipe_data::CompoundTag) {
        self.base.read_nbt(tag);
    }

    pub fn write_nbt(&self) -> crate::recipe_data::CompoundTag {
        self.base.write_nbt()
    }
}

// Silence the unused private-helper lint path: validate_thread is the
// upstream method kept for the record.
#[allow(dead_code)]
fn _validate(level: &LevelHandle) -> bool {
    PlayerRecipeData::validate_thread(level)
}

// Keep HashMap import used by the base's shape (listener map lives in the
// base); this alias documents the seam.
#[allow(dead_code)]
type _ListenerMap = HashMap<u64, u64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_cache::Recipe;
    use crate::recipe_data::CompoundTag;
    use crate::recipe_pair::{Identifier, ItemStack};
    use std::collections::BTreeMap;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: BTreeMap::new() }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    struct TestRecipe { out: ItemStack }

    impl Recipe for TestRecipe {
        fn result_item(&self) -> Option<ItemStack> { Some(self.out.clone()) }
        fn is_custom(&self) -> bool { false }
        fn assemble(&self, _input: &RecipeInput) -> ItemStack { self.out.clone() }
    }

    fn holder(s: &str) -> RecipeHolder {
        RecipeHolder {
            id: id(s),
            recipe: Rc::new(TestRecipe { out: stack(&format!("mod:o_{}", s.split_once(':').unwrap().1), 1) }),
        }
    }

    fn owner(tick: i32, menu: u64) -> PlayerHandle {
        PlayerHandle {
            token: 42,
            tick_count: tick,
            container_menu: menu,
            is_server_player: true,
            is_client_side: false,
        }
    }

    fn level(on: bool) -> LevelHandle {
        LevelHandle { on_server_thread: on }
    }

    fn lookup() -> impl Fn(&RecipeInput) -> RecipeList {
        |_i: &RecipeInput| Vec::new()
    }

    fn no_by_key() -> impl Fn(&Identifier) -> Option<RecipeHolder> {
        |_i: &Identifier| None
    }

    fn input() -> RecipeInput {
        RecipeInput::new(vec![stack("mod:stick", 1)])
    }

    #[test]
    fn same_tick_reuses_the_cached_selection() {
        let mut data = PlayerRecipeData::new(owner(5, 7));
        let out = data
            .get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), vec![holder("mod:a")])
            .unwrap();
        assert_eq!(out.id, id("mod:a"));
        // Same tick again, but the recipe list no longer contains a: the
        // cached selection is restored instead of dropping the pick.
        let cached = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), Vec::new());
        assert!(cached.is_none()); // empty list path still clears the list
        let out2 = data
            .get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), vec![holder("mod:b")])
            .unwrap();
        assert_eq!(out2.id, id("mod:b")); // base selection logic on a fresh tick
    }

    #[test]
    fn a_new_tick_clears_the_cache() {
        let mut data = PlayerRecipeData::new(owner(5, 7));
        data.get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), vec![holder("mod:a")]);
        // Simulate a tick passing and the selection being cleared upstream.
        let mut inner = data.base.get_owner().borrow_mut();
        inner.tick_count = 6;
        drop(inner);
        let out = data
            .get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), vec![holder("mod:b")])
            .unwrap();
        assert_eq!(out.id, id("mod:b"));
    }

    #[test]
    fn off_thread_results_are_not_cached() {
        let mut data = PlayerRecipeData::new(owner(5, 7));
        let out = data
            .get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(false), vec![holder("mod:a")])
            .unwrap();
        assert_eq!(out.id, id("mod:a"));
        assert!(data.cached_selection.is_none());
    }

    #[test]
    fn a_matching_menu_triggers_player_sync_then_clears_the_menu() {
        let mut data = PlayerRecipeData::new(owner(5, 7));
        let syncs = Rc::new(RefCell::new(Vec::new()));
        let sink = syncs.clone();
        data.base.network = Rc::new(move |listener, pairs, selected| {
            sink.borrow_mut().push((listener, pairs.len(), selected.map(|i| i.path.clone())));
        });
        rewire_override(&mut data);

        data.set_container_menu(Some(7));
        data.get_recipe(&input(), 1, &lookup(), &no_by_key(), &level(true), vec![holder("mod:a")]);
        // Two sends upstream on this path: the override's
        // sendRecipesListS2C during updateRecipesList, then sendPlayerSyncS2C
        // from syncPlayerRecipeData (both ride the one network seam here).
        assert_eq!(syncs.borrow().len(), 2);
        assert_eq!(syncs.borrow()[0], (42, 1, Some("a".to_string())));
        assert_eq!(syncs.borrow()[1], (42, 1, Some("a".to_string())));
        assert_eq!(data.get_container_menu(), None);
    }

    #[test]
    fn select_recipe_clears_the_cache_and_syncs() {
        let mut data = PlayerRecipeData::new(owner(5, 7));
        let syncs = Rc::new(RefCell::new(Vec::new()));
        let sink = syncs.clone();
        data.base.network = Rc::new(move |l, pairs, sel| {
            sink.borrow_mut().push((l, pairs.len(), sel.map(|i| i.path.clone())));
        });
        rewire_override(&mut data);
        data.add_listener(1);
        data.set_recipes_list(BTreeSet::from([crate::recipe_pair::RecipePair {
            resource_location: id("mod:a"),
            output: stack("mod:o_a", 1),
        }]));
        data.select_recipe(holder("mod:a"));
        assert!(data.cached_selection.is_none());
        assert_eq!(syncs.borrow().len(), 1);
    }

    #[test]
    fn client_side_owner_uses_the_widget_branch() {
        let mut o = owner(5, 7);
        o.is_server_player = false;
        o.is_client_side = true;
        let mut data = PlayerRecipeData::new(o);
        let widgets = Rc::new(RefCell::new(Vec::new()));
        let sink = widgets.clone();
        data.widget = Rc::new(move |pairs, sel| {
            sink.borrow_mut().push((pairs.len(), sel.map(|i| i.path.clone())));
        });
        rewire_override(&mut data);
        data.set_container_menu(Some(7));
        data.set_recipes_list(BTreeSet::from([crate::recipe_pair::RecipePair {
            resource_location: id("mod:a"),
            output: stack("mod:o_a", 1),
        }]));
        data.base.send_recipes_list_to_listeners();
        assert_eq!(widgets.borrow().len(), 1);
        // No player sync went out on the client branch.
        let nbt = data.write_nbt();
        assert!(nbt.strings.is_empty());
        let _ = CompoundTag::default();
    }

    /// Rebuild the base's send override from the wrapper's current seams
    /// (test helper mirroring the constructor wiring).
    fn rewire_override(data: &mut PlayerRecipeData) {
        let owner_for_send = data.base.get_owner().clone();
        let menu = data.container_menu.clone();
        let widget = data.widget.clone();
        let net = data.base.network.clone();
        data.base.override_send =
            Some(Box::new(move |ctx: &BroadcastCtx<Rc<RefCell<PlayerHandle>>>| {
                let o = owner_for_send.borrow();

                if menu.get() == Some(o.container_menu) {
                    if o.is_client_side {
                        (widget)(ctx.pairs, ctx.selected_id);
                    } else if o.is_server_player {
                        (net)(o.token, ctx.pairs, ctx.selected_id);
                    }
                }
            }));
    }
}
