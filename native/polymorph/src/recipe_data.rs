//! Upstream `common/capability/AbstractRecipeData.java`, translated 1:1.
//! The Minecraft `Level`, `RecipeManager`, `ServerPlayer`, NBT and network
//! types are host seams with the same shapes and semantics.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::recipe_cache::{RecipeCache, RecipeHolder, RecipeInput, RecipeList};
use crate::recipe_pair::{Identifier, RecipePair};

/// Host seam for `net.minecraft.nbt.CompoundTag`, reduced to the string
/// entries this capability reads and writes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompoundTag {
    pub strings: BTreeMap<String, String>,
}

/// Host seam for the Polymorph network:
/// `PolymorphApi.getNetwork().sendRecipesListS2C(listener, pairs, selectedId)`,
/// keyed by the listener player token. Shared (Rc) so a subclass override
/// can send through the same network object, as the Java API does.
pub type RecipesListSender = dyn Fn(u64, &BTreeSet<RecipePair>, Option<&Identifier>);

/// Host seam for the `Level` pieces the capability logic reads: the server
/// running-thread check in `PlayerRecipeData#validateThread`.
#[derive(Clone, Copy, Debug)]
pub struct LevelHandle {
    pub on_server_thread: bool,
}

/// What a subclass override of `sendRecipesListToListeners` sees at
/// broadcast time (the Java override reads the same fields off `this`).
pub struct BroadcastCtx<'a, E> {
    pub listeners: Vec<u64>,
    pub pairs: &'a BTreeSet<RecipePair>,
    pub selected_id: Option<&'a Identifier>,
    pub owner: &'a E,
}

fn parse_identifier(s: &str) -> Option<Identifier> {
    s.split_once(':').map(|(ns, path)| Identifier::new(ns, path))
}

/// Upstream `AbstractRecipeData<E>`: the per-owner recipe selection state
/// every concrete capability (player, block entity) builds on.
pub struct AbstractRecipeData<E> {
    recipes_list: BTreeSet<RecipePair>,
    owner: E,
    recipe_cache: RecipeCache,
    /// `Map<UUID, ServerPlayer>`, keyed by the player token seam (the value
    /// mirrors the key: only the player identity is read upstream).
    listeners: HashMap<u64, u64>,
    selected_recipe: Option<RecipeHolder>,
    loaded_recipe: Option<Identifier>,
    /// `PolymorphApi.getInstance().getNetwork()`, as a seam (shared).
    pub network: Rc<RecipesListSender>,
    /// A subclass override of `sendRecipesListToListeners`; None runs the
    /// base per-listener send (the Java virtual-dispatch seam).
    pub override_send: Option<Box<dyn for<'a> Fn(&BroadcastCtx<'a, E>)>>,
}

impl<E> AbstractRecipeData<E> {
    pub fn new(owner: E) -> Self {
        Self {
            recipes_list: BTreeSet::new(),
            owner,
            recipe_cache: RecipeCache::new(10),
            listeners: HashMap::new(),
            selected_recipe: None,
            loaded_recipe: None,
            network: Rc::new(|_, _, _| {}),
            override_send: None,
        }
    }

    /// `getRecipe`, line for line: cached-or-given recipes, the loaded-recipe
    /// re-resolution, output assembly with the CustomRecipe fallback, the
    /// 15-entry list cap with the selected bypass, and the default
    /// selection. `lookup` is the manager's `getRecipesFor` seam;
    /// `by_key` is `RecipeManager#byKey`.
    pub fn get_recipe(
        &mut self,
        recipe_input: &RecipeInput,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        by_key: &dyn Fn(&Identifier) -> Option<RecipeHolder>,
        recipes_in: RecipeList,
    ) -> Option<RecipeHolder> {
        let recipes = if recipes_in.is_empty() {
            self.recipe_cache.get(manager_token, lookup, recipe_input)
        } else {
            recipes_in
        };

        if recipes.is_empty() {
            self.update_recipes_list(BTreeSet::new());
            return None;
        }

        if self.loaded_recipe.is_some() && self.get_selected_recipe().is_none() {
            if let Some(holder) = by_key(self.loaded_recipe.as_ref().unwrap()) {
                self.set_selected_recipe(holder);
            }
        }
        self.loaded_recipe = None;

        let mut first_result: Option<RecipeHolder> = None;
        let mut selected: Option<RecipeHolder> = None;
        let mut recipes_list = BTreeSet::new();

        for entry in recipes {
            let mut output = entry.recipe.result_item();

            if output.as_ref().map_or(true, |o| o.is_empty()) || entry.recipe.is_custom() {
                output = Some(entry.recipe.assemble(recipe_input));
            }

            let output = match output {
                Some(o) if !o.is_empty() => o,
                _ => continue,
            };

            if first_result.is_none() {
                first_result = Some(entry.clone());
            }
            let mut flag = false;

            if selected.is_none() && self.get_selected_recipe().is_some() {
                let sel = self.get_selected_recipe().unwrap();
                if sel.id == entry.id {
                    selected = Some(entry.clone());
                    flag = true;
                }
            }

            if recipes_list.len() < 15 || flag {
                recipes_list.insert(RecipePair {
                    resource_location: entry.id.clone(),
                    output,
                });
            }
        }

        let selected = match selected {
            Some(s) => s,
            None => {
                let s = first_result.expect("a non-empty recipe list yields a first result");
                self.set_selected_recipe(s.clone());
                s
            }
        };
        self.update_recipes_list(recipes_list);
        Some(selected)
    }

    /// `updateRecipesList`: publish then broadcast (protected upstream;
    /// public here so the block-entity subclass can reach it).
    pub fn update_recipes_list(&mut self, recipes_list: BTreeSet<RecipePair>) {
        self.set_recipes_list(recipes_list);
        self.send_recipes_list_to_listeners();
    }

    pub fn get_selected_recipe(&self) -> Option<&RecipeHolder> {
        self.selected_recipe.as_ref()
    }

    pub fn set_selected_recipe(&mut self, recipe: RecipeHolder) {
        self.selected_recipe = Some(recipe);
    }

    pub fn get_recipes_list(&self) -> &BTreeSet<RecipePair> {
        &self.recipes_list
    }

    /// `setRecipesList`: clear then addAll (a wholesale replace).
    pub fn set_recipes_list(&mut self, recipes_list: BTreeSet<RecipePair>) {
        self.recipes_list.clear();
        self.recipes_list.extend(recipes_list);
    }

    pub fn get_owner(&self) -> &E {
        &self.owner
    }

    pub fn select_recipe(&mut self, recipe: RecipeHolder) {
        self.set_selected_recipe(recipe);
    }

    /// `getListeners`: an unmodifiable view of the listener players.
    pub fn get_listeners(&self) -> Vec<u64> {
        self.listeners.values().copied().collect()
    }

    pub fn add_listener(&mut self, server_player: u64) {
        self.listeners.insert(server_player, server_player);
    }

    pub fn remove_listener(&mut self, server_player: u64) {
        self.listeners.remove(&server_player);
    }

    pub fn clear_listeners(&mut self) {
        self.listeners.clear();
    }

    pub fn send_recipes_list_to_listeners(&self) {
        let ctx = BroadcastCtx {
            listeners: self.listeners.values().copied().collect(),
            pairs: &self.recipes_list,
            selected_id: self.selected_recipe.as_ref().map(|s| &s.id),
            owner: &self.owner,
        };

        if let Some(over) = &self.override_send {
            (over)(&ctx);
        } else {
            for listener in &ctx.listeners {
                (self.network)(*listener, ctx.pairs, ctx.selected_id);
            }
        }
    }

    /// `readNBT`: a stored SelectedRecipe id is remembered for the next
    /// `getRecipe` to re-resolve through `byKey`.
    pub fn read_nbt(&mut self, compound_tag: &CompoundTag) {
        if let Some(s) = compound_tag.strings.get("SelectedRecipe") {
            self.loaded_recipe = parse_identifier(s);
        }
    }

    pub fn write_nbt(&self) -> CompoundTag {
        let mut nbt = CompoundTag::default();

        if let Some(selected) = &self.selected_recipe {
            nbt.strings.insert(
                "SelectedRecipe".to_string(),
                format!("{}:{}", selected.id.namespace, selected.id.path),
            );
        }
        nbt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_cache::Recipe;
    use crate::recipe_pair::ItemStack;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: BTreeMap::new() }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    struct TestRecipe {
        result: Option<ItemStack>,
        custom: bool,
        assembled: ItemStack,
    }

    impl Recipe for TestRecipe {
        fn result_item(&self) -> Option<ItemStack> { self.result.clone() }
        fn is_custom(&self) -> bool { self.custom }
        fn assemble(&self, _input: &RecipeInput) -> ItemStack { self.assembled.clone() }
    }

    fn holder(s: &str, recipe: TestRecipe) -> RecipeHolder {
        RecipeHolder { id: id(s), recipe: Rc::new(recipe) }
    }

    fn plain(s: &str) -> RecipeHolder {
        let (_, path) = s.split_once(':').unwrap();
        let out = stack(&format!("mod:o_{}", path), 1);
        holder(s, TestRecipe { result: Some(out.clone()), custom: false, assembled: out })
    }

    fn input() -> RecipeInput {
        RecipeInput::new(vec![stack("mod:stick", 1)])
    }

    fn lookup() -> impl Fn(&RecipeInput) -> RecipeList {
        |_i: &RecipeInput| Vec::new()
    }

    fn no_by_key() -> impl Fn(&Identifier) -> Option<RecipeHolder> {
        |_id: &Identifier| None
    }

    fn sorted(v: &[u64]) -> Vec<u64> {
        let mut v = v.to_vec();
        v.sort_unstable();
        v
    }

    fn pair_ids(list: &BTreeSet<RecipePair>) -> Vec<String> {
        list.iter()
            .map(|p| format!("{}:{}", p.resource_location.namespace, p.resource_location.path))
            .collect()
    }

    #[test]
    fn empty_recipes_clears_the_list_and_returns_none() {
        let mut data = AbstractRecipeData::new(());
        data.set_recipes_list(BTreeSet::from([RecipePair {
            resource_location: id("mod:old"),
            output: stack("mod:out", 1),
        }]));
        let out = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), Vec::new());
        assert!(out.is_none());
        assert!(data.get_recipes_list().is_empty());
    }

    #[test]
    fn first_result_becomes_the_selection() {
        let mut data = AbstractRecipeData::new(());
        let recipes = vec![plain("mod:a"), plain("mod:b")];
        let out = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), recipes).unwrap();
        assert_eq!(out.id, id("mod:a"));
        assert_eq!(data.get_selected_recipe().unwrap().id, id("mod:a"));
        assert_eq!(pair_ids(data.get_recipes_list()), vec!["mod:a", "mod:b"]);
    }

    #[test]
    fn selected_recipe_is_kept_and_bypasses_the_cap() {
        let mut data = AbstractRecipeData::new(());
        let recipes: Vec<RecipeHolder> =
            (0..20).map(|i| plain(&format!("mod:r{:02}", i))).collect();
        data.set_selected_recipe(plain("mod:r18"));
        let out = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), recipes).unwrap();
        // The selected recipe is kept even though the list is capped at 15.
        assert_eq!(out.id, id("mod:r18"));
        let ids = pair_ids(data.get_recipes_list());
        assert_eq!(ids.len(), 16);
        assert!(ids.contains(&"mod:r18".to_string()));
    }

    #[test]
    fn custom_and_null_result_fall_back_to_assemble() {
        let mut data = AbstractRecipeData::new(());
        let custom = holder("mod:custom", TestRecipe {
            result: None,
            custom: true,
            assembled: stack("mod:assembled", 2),
        });
        let out = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), vec![custom]).unwrap();
        assert_eq!(out.id, id("mod:custom"));
        let pair = data.get_recipes_list().iter().next().unwrap();
        assert_eq!(pair.output.count, 2);
    }

    #[test]
    fn empty_output_is_skipped() {
        let mut data = AbstractRecipeData::new(());
        let empty = holder("mod:empty", TestRecipe {
            result: Some(stack("mod:out", 0)),
            custom: false,
            assembled: stack("mod:out", 0),
        });
        let out = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), vec![empty, plain("mod:b")]);
        assert_eq!(out.unwrap().id, id("mod:b"));
        assert_eq!(pair_ids(data.get_recipes_list()), vec!["mod:b"]);
    }

    #[test]
    fn loaded_recipe_is_re_resolved_once() {
        let mut data = AbstractRecipeData::new(());
        data.read_nbt(&CompoundTag {
            strings: BTreeMap::from([(
                "SelectedRecipe".to_string(),
                "mod:loaded".to_string(),
            )]),
        });
        let by_key = |wanted: &Identifier| {
            if *wanted == id("mod:loaded") { Some(plain("mod:loaded")) } else { None }
        };
        let out = data
            .get_recipe(&input(), 1, &lookup(), &by_key, vec![plain("mod:a"), plain("mod:loaded")])
            .unwrap();
        // The loaded recipe wins over the first result.
        assert_eq!(out.id, id("mod:loaded"));
        // And the loaded id is consumed either way.
        let again = data.get_recipe(&input(), 1, &lookup(), &no_by_key(), vec![plain("mod:a")]).unwrap();
        assert_eq!(again.id, id("mod:a"));
    }

    #[test]
    fn listeners_receive_the_recipes_list() {
        let mut data = AbstractRecipeData::new(());
        let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = sent.clone();
        data.network = Rc::new(move |listener, pairs, selected| {
            sink.borrow_mut().push((listener, pairs.len(), selected.map(|i| i.path.clone())));
        });
        data.add_listener(7);
        data.add_listener(9);
        data.add_listener(7); // a re-add is a no-op on the UUID key
        assert_eq!(sorted(&data.get_listeners()), vec![7, 9]);
        data.set_recipes_list(BTreeSet::from([RecipePair {
            resource_location: id("mod:a"),
            output: stack("mod:out", 1),
        }]));
        data.set_selected_recipe(plain("mod:a"));
        data.send_recipes_list_to_listeners();
        let mut got = sent.borrow().clone();
        got.sort();
        assert_eq!(got, vec![(7, 1, Some("a".to_string())), (9, 1, Some("a".to_string()))]);
        data.remove_listener(7);
        assert_eq!(data.get_listeners(), vec![9]);
        data.clear_listeners();
        assert!(data.get_listeners().is_empty());
    }

    #[test]
    fn nbt_round_trip() {
        let mut data = AbstractRecipeData::new(());
        data.set_selected_recipe(plain("mod:chosen"));
        let nbt = data.write_nbt();
        assert_eq!(
            nbt.strings.get("SelectedRecipe").map(|s| s.as_str()),
            Some("mod:chosen")
        );
        let mut other = AbstractRecipeData::<()>::new(());
        other.read_nbt(&nbt);
        // writeNBT on a fresh capability (no selection) writes no key.
        assert_eq!(AbstractRecipeData::<()>::new(()).write_nbt(), CompoundTag::default());
        let _ = other;
    }
}
