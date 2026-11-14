//! Upstream `common/capability/RecipeCache.java`, translated 1:1. The
//! `RecipeManager`, `Level`, `RecipeInput`/`CraftingInput` and
//! `RecipeHolder` types are host seams: the manager identity arrives as a
//! token (the Java `WeakReference` identity check) and the recipe lookup as
//! a closure, with the same semantics.

use crate::recipe_pair::{Identifier, ItemStack};
use std::rc::Rc;

/// Host seam for a Minecraft `Recipe<T>`, reduced to what the translated
/// logic reads: its output item, whether it is a `CustomRecipe` (which
/// always falls back to `assemble`), and `assemble` itself.
pub trait Recipe {
    /// `Recipe#getResultItem(registryAccess)`, None when the host gives null.
    fn result_item(&self) -> Option<ItemStack>;
    /// `recipe instanceof CustomRecipe`.
    fn is_custom(&self) -> bool;
    /// `Recipe#assemble(input, registryAccess)`.
    fn assemble(&self, input: &RecipeInput) -> ItemStack;
}

/// Host seam for `RecipeHolder<T>`: the recipe's id plus its (shared)
/// recipe value.
#[derive(Clone)]
pub struct RecipeHolder {
    pub id: Identifier,
    pub recipe: Rc<dyn Recipe>,
}

/// Opaque recipe-holder list the cache carries through.
pub type RecipeList = Vec<RecipeHolder>;

/// Host seam for `net.minecraft.world.item.crafting.RecipeInput`
/// (a `CraftingInput` additionally carries its width and height).
#[derive(Clone, Debug)]
pub struct RecipeInput {
    pub items: Vec<ItemStack>,
    pub crafting: Option<(usize, usize)>,
}

impl RecipeInput {
    pub fn new(items: Vec<ItemStack>) -> Self {
        Self { items, crafting: None }
    }

    pub fn crafting(items: Vec<ItemStack>, width: usize, height: usize) -> Self {
        Self { items, crafting: Some((width, height)) }
    }

    /// `RecipeInput#size()`.
    pub fn size(&self) -> usize {
        self.items.len()
    }

    /// `RecipeInput#isEmpty()`: every slot empty.
    pub fn is_empty(&self) -> bool {
        self.items.iter().all(|s| s.count == 0 && s.components.is_empty())
    }

    /// `RecipeInput#getItem(i)`.
    pub fn get_item(&self, i: usize) -> &ItemStack {
        &self.items[i]
    }
}

/// An LRU cache entry: the per-slot key (`count` normalized to 1), the size,
/// and the recipes computed for it.
struct Entry {
    key: Vec<ItemStack>,
    size: usize,
    recipes: RecipeList,
    /// `CraftingEntry`'s width/height gate; None on a plain `Entry`.
    crafting: Option<(usize, usize)>,
}

impl Entry {
    /// `Entry#matches`: same size, every slot `isSameItemSameComponents`
    /// (count-insensitive by design, matching the upstream key).
    fn matches(&self, input: &RecipeInput) -> bool {
        if self.size == input.size() {
            for i in 0..self.key.len() {
                if !ItemStack::is_same_item_same_components(&self.key[i], input.get_item(i)) {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// `CraftingEntry#matches`: only a `CraftingInput` of the same width and
    /// height gets past to the slot check.
    fn matches_crafting(&self, input: &RecipeInput) -> bool {
        match (self.crafting, input.crafting) {
            (Some((w, h)), Some((iw, ih))) if w == iw && h == ih => self.matches(input),
            _ => false,
        }
    }
}

/// Upstream `RecipeCache`: a fixed-size LRU over recipe lookups, invalidated
/// whenever the recipe manager changes.
pub struct RecipeCache {
    entries: Vec<Option<Entry>>,
    capacity: usize,
    /// `WeakReference<RecipeManager>` identity, as a token seam.
    cached_manager: Option<u64>,
}

impl RecipeCache {
    pub fn new(size: usize) -> Self {
        Self { entries: Vec::new(), capacity: size, cached_manager: None }
    }

    /// `get`, line for line: empty input is no recipes, a changed manager
    /// drops the cache, a hit moves its entry to the front, a miss computes
    /// and inserts.
    pub fn get(
        &mut self,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        recipe_input: &RecipeInput,
    ) -> RecipeList {
        if recipe_input.is_empty() {
            return Vec::new();
        }
        self.validate_recipe_manager(manager_token);

        for i in 0..self.entries.len() {
            if let Some(entry) = &self.entries[i] {
                let hit = match entry.crafting {
                    Some(_) => entry.matches_crafting(recipe_input),
                    None => entry.matches(recipe_input),
                };
                if hit {
                    self.move_entry_to_front(i);
                    return self.entries[0].as_ref().unwrap().recipes.clone();
                }
            }
        }
        self.compute(lookup, recipe_input)
    }

    /// `validateRecipeManager`: a different manager clears every entry.
    fn validate_recipe_manager(&mut self, manager_token: u64) {
        if self.cached_manager != Some(manager_token) {
            self.cached_manager = Some(manager_token);
            self.entries.clear();
        }
    }

    /// `compute`: the manager's `getRecipesFor` (the lookup seam), then the
    /// result is cached.
    fn compute(&mut self, lookup: &dyn Fn(&RecipeInput) -> RecipeList, recipe_input: &RecipeInput) -> RecipeList {
        let list = lookup(recipe_input);
        self.insert(recipe_input, list.clone());
        list
    }

    /// `moveEntryToFront`: the arraycopy shift, expressed as remove-at-index
    /// and insert-at-zero.
    fn move_entry_to_front(&mut self, index: usize) {
        if index > 0 {
            let entry = self.entries.remove(index);
            self.entries.insert(0, entry);
        }
    }

    /// `insert`: the key is every input item copied with count 1, entries
    /// shift right one slot (the oldest drops off), and a `CraftingInput`
    /// makes a `CraftingEntry`.
    fn insert(&mut self, recipe_input: &RecipeInput, recipes: RecipeList) {
        let list: Vec<ItemStack> = (0..recipe_input.size())
            .map(|i| {
                let mut item = recipe_input.get_item(i).clone();
                item.count = 1;
                item
            })
            .collect();
        let entry = Entry {
            key: list,
            size: recipe_input.size(),
            recipes,
            crafting: recipe_input.crafting,
        };
        self.entries.insert(0, Some(entry));
        self.entries.truncate(self.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_pair::Identifier;
    use std::collections::BTreeMap;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: BTreeMap::new() }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    struct TestRecipe { output: ItemStack }

    impl Recipe for TestRecipe {
        fn result_item(&self) -> Option<ItemStack> { Some(self.output.clone()) }
        fn is_custom(&self) -> bool { false }
        fn assemble(&self, _input: &RecipeInput) -> ItemStack { self.output.clone() }
    }

    fn holder(s: &str) -> RecipeHolder {
        RecipeHolder { id: id(s), recipe: Rc::new(TestRecipe { output: stack("mod:out", 1) }) }
    }

    fn lookup_of(entries: Vec<(&str, Vec<&str>)>) -> impl Fn(&RecipeInput) -> RecipeList {
        move |input: &RecipeInput| {
            let key = format!("{}", input.get_item(0).item.path);
            entries
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, ids)| ids.iter().map(|s| holder(s)).collect())
                .unwrap_or_default()
        }
    }

    fn ids(list: &RecipeList) -> Vec<String> {
        list.iter().map(|h| format!("{}:{}", h.id.namespace, h.id.path)).collect()
    }

    #[test]
    fn empty_input_is_no_recipes() {
        let mut cache = RecipeCache::new(4);
        let input = RecipeInput::new(vec![ItemStack {
            item: id("mod:stick"),
            count: 0,
            components: BTreeMap::new(),
        }]);
        let out = cache.get(1, &|_| Vec::new(), &input);
        assert!(out.is_empty());
    }

    #[test]
    fn hit_returns_cached_recipes() {
        let mut cache = RecipeCache::new(4);
        let input = RecipeInput::new(vec![stack("mod:stick", 5)]);
        let lookup = lookup_of(vec![("stick", vec!["mod:recipe_a"])]);
        assert_eq!(ids(&cache.get(1, &lookup, &input)), vec!["mod:recipe_a"]);
        // A second lookup closure that would return something else must not
        // be consulted: the cached list comes back.
        let never = |_i: &RecipeInput| vec![holder("mod:wrong")];
        assert_eq!(ids(&cache.get(1, &never, &input)), vec!["mod:recipe_a"]);
    }

    #[test]
    fn count_is_ignored_in_the_key() {
        let mut cache = RecipeCache::new(4);
        let input = RecipeInput::new(vec![stack("mod:stick", 1)]);
        let lookup = lookup_of(vec![("stick", vec!["mod:recipe_a"])]);
        cache.get(1, &lookup, &input);
        let bigger = RecipeInput::new(vec![stack("mod:stick", 64)]);
        assert_eq!(ids(&cache.get(1, &lookup, &bigger)), vec!["mod:recipe_a"]);
    }

    #[test]
    fn manager_change_clears_entries() {
        let mut cache = RecipeCache::new(4);
        let input = RecipeInput::new(vec![stack("mod:stick", 1)]);
        let lookup = lookup_of(vec![("stick", vec!["mod:recipe_a"])]);
        cache.get(1, &lookup, &input);
        let other = |_i: &RecipeInput| vec![holder("mod:recipe_b")];
        assert_eq!(ids(&cache.get(2, &other, &input)), vec!["mod:recipe_b"]);
    }

    #[test]
    fn lru_evicts_the_oldest() {
        let mut cache = RecipeCache::new(2);
        let lookup = |_i: &RecipeInput| vec![holder("mod:recipe")];
        let a = RecipeInput::new(vec![stack("mod:a", 1)]);
        let b = RecipeInput::new(vec![stack("mod:b", 1)]);
        let c = RecipeInput::new(vec![stack("mod:c", 1)]);
        cache.get(1, &lookup, &a);
        cache.get(1, &lookup, &b);
        // Touching a moves it to the front, so the next insert evicts b.
        cache.get(1, &lookup, &a);
        cache.get(1, &lookup, &c);
        // Now a and c are cached; b is a miss again.
        let hit = cache.get(1, &lookup, &b);
        assert_eq!(ids(&hit), vec!["mod:recipe"]);
        assert_eq!(cache.entries.len(), 2);
    }

    #[test]
    fn crafting_entry_gates_on_width_and_height() {
        let mut cache = RecipeCache::new(4);
        let input2x2 = RecipeInput::crafting(vec![stack("mod:stick", 1); 4], 2, 2);
        let lookup = |_i: &RecipeInput| vec![holder("mod:recipe")];
        cache.get(1, &lookup, &input2x2);
        // Same slots but a 3x3 shape must not hit the 2x2 entry.
        let input3x3 = RecipeInput::crafting(vec![stack("mod:stick", 1); 9], 3, 3);
        let before = cache.entries.len();
        cache.get(1, &lookup, &input3x3);
        assert_eq!(cache.entries.len(), before + 1);
    }
}
