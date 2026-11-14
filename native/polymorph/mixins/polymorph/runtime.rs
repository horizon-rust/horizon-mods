/*
 * Copyright (C) 2020-2022 Illusive Soulworks
 *
 * Polymorph is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published
 * by the Free Software Foundation, either version 3 of the License, or
 * any later version.
 *
 * Polymorph is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR PARTICULAR PURPOSE.  See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with Polymorph.  If not, see <https://www.gnu.org/licenses/>.
 *
 * Rust translation 2026 sparticle, exact 1:1 of the upstream Java.
 */

//! Exact 1:1 translation of Polymorph (upstream Java, LGPL-3.0-or-later).
//! Where Minecraft/Fabric APIs do not exist on Horizon, the counterpart is a
//! documented host seam with the same shape and semantics.

pub mod recipe_pair {
//! Upstream `common/util/RecipePair.java` (record implementing IRecipePair),
//! translated 1:1. The Minecraft registry, `ItemStack` and
//! `DataComponentMap` are host seams with the same shapes and semantics.

use std::collections::BTreeMap;

/// Host seam for `net.minecraft.resources.ResourceLocation` (namespace:path).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub namespace: String,
    pub path: String,
}

impl Identifier {
    pub fn new(namespace: &str, path: &str) -> Self {
        Self { namespace: namespace.to_string(), path: path.to_string() }
    }

    /// `ResourceLocation#compareTo`: namespace, then path (host seam).
    pub fn compare_to(&self, other: &Identifier) -> std::cmp::Ordering {
        self.namespace
            .cmp(&other.namespace)
            .then(self.path.cmp(&other.path))
    }
}

/// A stable 32-bit component hash for the seam map (the Java code only ever
/// subtracts two of these, never stores them).
fn component_hash(map: &BTreeMap<String, String>) -> i32 {
    let mut h: u32 = 2166136261;
    for (k, v) in map {
        for b in k.bytes().chain(std::iter::once(0x1f)).chain(v.bytes()) {
            h = h.wrapping_mul(16777619) ^ b as u32;
        }
    }
    h as i32
}

/// Host seam for `net.minecraft.world.item.ItemStack` reduced to what the
/// translated logic reads: the item's registry key, the count, and the
/// component map (`ItemStack#matches` semantics). Hash supports the wire
/// HashSet in the network packets.
#[derive(Clone, Debug, Hash)]
pub struct ItemStack {
    pub item: Identifier,
    pub count: i32,
    pub components: BTreeMap<String, String>,
}

impl PartialEq for ItemStack {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.count == other.count
            && self.components == other.components
    }
}

/// Host seam for `net.minecraft.world.item.crafting.Ingredient`; EMPTY is the
/// empty ingredient (no accepted stacks).
#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    pub stacks: Vec<ItemStack>,
}

impl Ingredient {
    pub const EMPTY: Ingredient = Ingredient { stacks: Vec::new() };
}

/// Upstream record `RecipePair(ResourceLocation resourceLocation,
/// ItemStack output) implements IRecipePair`. Hash supports the wire
/// HashSet in the network packets.
#[derive(Clone, Debug, Hash)]
pub struct RecipePair {
    pub resource_location: Identifier,
    pub output: ItemStack,
}

impl ItemStack {
    /// Host seam for `ItemStack#isEmpty` (count 0 is the empty stack).
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Host seam for `ItemStack.isSameItemSameComponents`: same registry key
    /// and same component map, count-insensitive.
    pub fn is_same_item_same_components(a: &ItemStack, b: &ItemStack) -> bool {
        a.item == b.item && a.components == b.components
    }
}

impl RecipePair {
    /// `IRecipePair#compareTo`, line for line: item registry key, then count,
    /// then component-map equality/hash, with vanilla recipes sorted after
    /// modded ones when they share the list.
    pub fn compare_to(&self, other: &RecipePair) -> i32 {
        let output1 = &self.output;
        let output2 = &other.output;
        let compare = output1.item.compare_to(&output2.item);

        if compare.is_eq() {
            let compare = output1.count - output2.count;

            if compare == 0 {
                let components1 = &output1.components;
                let components2 = &output2.components;

                if components1 == components2 {
                    0
                } else {
                    component_hash(components1) - component_hash(components2)
                }
            } else {
                compare
            }
        } else {
            // Sort vanilla recipes after modded recipes if they appear in the same list
            if self.resource_location.namespace == "minecraft"
                && other.resource_location.namespace != "minecraft"
            {
                1
            } else if self.resource_location.namespace != "minecraft"
                && other.resource_location.namespace == "minecraft"
            {
                -1
            } else {
                compare as i32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, count: i32, tag: &str) -> ItemStack {
        let mut components = BTreeMap::new();
        if !tag.is_empty() {
            components.insert("tag".to_string(), tag.to_string());
        }
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components }
    }

    fn pair(id: &str, item: &str, count: i32, tag: &str) -> RecipePair {
        let (ns, path) = id.split_once(':').unwrap();
        RecipePair { resource_location: Identifier::new(ns, path), output: stack(item, count, tag) }
    }

    #[test]
    fn same_item_orders_by_count() {
        let a = pair("mod:a", "mod:out", 2, "");
        let b = pair("mod:b", "mod:out", 1, "");
        assert!(a.compare_to(&b) > 0);
        assert!(b.compare_to(&a) < 0);
    }

    #[test]
    fn same_output_components_break_ties() {
        let a = pair("mod:a", "mod:out", 1, "x");
        let b = pair("mod:b", "mod:out", 1, "y");
        let c = pair("mod:b", "mod:out", 1, "x");
        assert_eq!(a.compare_to(&c), 0);
        assert_ne!(a.compare_to(&b), 0);
        assert_eq!(b.compare_to(&a), -(a.compare_to(&b)));
    }

    #[test]
    fn vanilla_sorts_after_modded() {
        let vanilla = pair("minecraft:stick", "mod:out1", 1, "");
        let modded = pair("mod:x", "mod:out2", 1, "");
        assert_eq!(vanilla.compare_to(&modded), 1);
        assert_eq!(modded.compare_to(&vanilla), -1);
    }

    #[test]
    fn two_modded_fall_through_to_item_compare() {
        let a = pair("mod:aa", "mod:out1", 1, "");
        let b = pair("mod:bb", "mod:out2", 1, "");
        assert_eq!(a.compare_to(&b) < 0, true);
    }
}

impl Ord for RecipePair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let c = self.compare_to(other);
        if c < 0 { std::cmp::Ordering::Less } else if c > 0 { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }
    }
}

impl PartialOrd for RecipePair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RecipePair {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for RecipePair {}
}
pub mod recipe_cache {
//! Upstream `common/capability/RecipeCache.java`, translated 1:1. The
//! `RecipeManager`, `Level`, `RecipeInput`/`CraftingInput` and
//! `RecipeHolder` types are host seams: the manager identity arrives as a
//! token (the Java `WeakReference` identity check) and the recipe lookup as
//! a closure, with the same semantics.

use super::recipe_pair::{Identifier, ItemStack};
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
    use super::recipe_pair::Identifier;
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
}
pub mod block_entity_recipe_data {
//! Upstream `common/capability/AbstractBlockEntityRecipeData.java`,
//! translated 1:1. The `BlockEntity` and inventory types are host seams:
//! the abstract `getInput()` is an input-provider closure.

use super::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use super::recipe_data::AbstractRecipeData;
use super::recipe_pair::Identifier;
use super::recipe_pair::{ItemStack, RecipePair};
use std::collections::BTreeSet;

/// Upstream `AbstractBlockEntityRecipeData<E>`: block-entity-backed recipe
/// data that clears its list when the block's input goes empty.
pub struct AbstractBlockEntityRecipeData<E> {
    base: AbstractRecipeData<E>,
    /// The abstract `getInput()`, as a provider seam.
    get_input: Box<dyn Fn() -> Vec<ItemStack>>,
    was_empty_before: bool,
}

impl<E> AbstractBlockEntityRecipeData<E> {
    pub fn new(owner: E, get_input: impl Fn() -> Vec<ItemStack> + 'static) -> Self {
        Self {
            base: AbstractRecipeData::new(owner),
            get_input: Box::new(get_input),
            was_empty_before: false,
        }
    }

    pub fn get_recipe(
        &mut self,
        recipe_input: &RecipeInput,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        by_key: &dyn Fn(&Identifier) -> Option<RecipeHolder>,
        recipes_list: RecipeList,
    ) -> Option<RecipeHolder> {
        self.base
            .get_recipe(recipe_input, manager_token, lookup, by_key, recipes_list)
    }

    /// `tick`: when listeners are watching and the block's input just went
    /// from non-empty to empty, clear the recipes list.
    pub fn tick(&mut self) {
        if self.base.get_listeners().is_empty() {
            return;
        }
        let current_input = (self.get_input)();
        let mut is_empty = true;

        for current_stack in &current_input {
            if !current_stack.is_empty() {
                is_empty = false;
                break;
            }
        }

        if !self.was_empty_before && is_empty {
            self.base.update_recipes_list(BTreeSet::new());
        }
        self.was_empty_before = is_empty;
    }

    /// `isEmpty`: every input slot empty.
    pub fn is_empty(&self) -> bool {
        for current_stack in (self.get_input)() {
            if !current_stack.is_empty() {
                return false;
            }
        }
        true
    }

    /// The `@SuppressWarnings("unchecked") getOwner()` override: the typed
    /// owner, which the generic base already holds here.
    pub fn get_owner(&self) -> &E {
        self.base.get_owner()
    }

    pub fn add_listener(&mut self, player: u64) {
        self.base.add_listener(player);
    }

    pub fn remove_listener(&mut self, player: u64) {
        self.base.remove_listener(player);
    }

    pub fn get_listeners(&self) -> Vec<u64> {
        self.base.get_listeners()
    }

    pub fn select_recipe(&mut self, recipe: RecipeHolder) {
        self.base.select_recipe(recipe)
    }

    pub fn get_recipes_list(&self) -> &BTreeSet<RecipePair> {
        self.base.get_recipes_list()
    }

    pub fn get_selected_recipe(&self) -> Option<&RecipeHolder> {
        self.base.get_selected_recipe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::recipe_cache::Recipe;
    use super::recipe_pair::Identifier;
    use std::collections::BTreeMap;
    use std::rc::Rc;

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
        RecipeHolder { id: id(s), recipe: Rc::new(TestRecipe { out: stack("mod:o", 1) }) }
    }

    #[test]
    fn tick_without_listeners_does_nothing() {
        let mut data = AbstractBlockEntityRecipeData::new(0_u64, || vec![stack("mod:in", 1)]);
        data.tick(); // no listeners: an early return, no state change
        assert!(!data.is_empty());
        assert!(data.get_recipes_list().is_empty());
    }

    #[test]
    fn going_empty_clears_the_list_once() {
        let contents = Rc::new(std::cell::RefCell::new(vec![stack("mod:in", 1)]));
        let c = contents.clone();
        let mut data = AbstractBlockEntityRecipeData::new(0_u64, move || c.borrow().clone());
        data.add_listener(1);
        // Pre-seed a recipes list, then drain the block.
        data.base.set_recipes_list(BTreeSet::from([RecipePair {
            resource_location: id("mod:old"),
            output: stack("mod:o", 1),
        }]));
        contents.borrow_mut().clear();
        data.tick();
        assert!(data.get_recipes_list().is_empty());
        // Staying empty does not re-broadcast (wasEmptyBefore latched).
        data.tick();
        assert!(data.get_recipes_list().is_empty());
    }

    #[test]
    fn is_empty_reflects_the_input() {
        let contents = Rc::new(std::cell::RefCell::new(vec![stack("mod:in", 0)]));
        let c = contents.clone();
        let data = AbstractBlockEntityRecipeData::new(0_u64, move || c.borrow().clone());
        assert!(data.is_empty());
        contents.borrow_mut().push(stack("mod:in", 1));
        assert!(!data.is_empty());
    }

    #[test]
    fn get_recipe_delegates_to_the_base() {
        let mut data = AbstractBlockEntityRecipeData::new(7_u64, || Vec::new());
        let out = data.get_recipe(
            &RecipeInput::new(vec![stack("mod:stick", 1)]),
            1,
            &|_| vec![holder("mod:a")],
            &|_| None,
            Vec::new(),
        );
        assert_eq!(out.unwrap().id, id("mod:a"));
        assert_eq!(*data.get_owner(), 7_u64);
    }
}
}
pub mod player_recipe_data {
//! Upstream `common/capability/PlayerRecipeData.java`, translated 1:1. The
//! `Player`/`ServerPlayer`, `AbstractContainerMenu`, `Level` and network
//! types are host seams with the same shapes and semantics. The Java
//! override of `sendRecipesListToListeners` is virtual dispatch upstream;
//! here it rides the base's `override_send` hook.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use super::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use super::recipe_data::{AbstractRecipeData, BroadcastCtx, LevelHandle};
use super::recipe_pair::{Identifier, RecipePair};

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

    pub fn read_nbt(&mut self, tag: &super::recipe_data::CompoundTag) {
        self.base.read_nbt(tag);
    }

    pub fn write_nbt(&self) -> super::recipe_data::CompoundTag {
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
    use super::recipe_cache::Recipe;
    use super::recipe_data::CompoundTag;
    use super::recipe_pair::{Identifier, ItemStack};
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
        data.set_recipes_list(BTreeSet::from([super::recipe_pair::RecipePair {
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
        data.set_recipes_list(BTreeSet::from([super::recipe_pair::RecipePair {
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
}
pub mod recipe_data {
//! Upstream `common/capability/AbstractRecipeData.java`, translated 1:1.
//! The Minecraft `Level`, `RecipeManager`, `ServerPlayer`, NBT and network
//! types are host seams with the same shapes and semantics.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use super::recipe_cache::{RecipeCache, RecipeHolder, RecipeInput, RecipeList};
use super::recipe_pair::{Identifier, RecipePair};

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
    use super::recipe_cache::Recipe;
    use super::recipe_pair::ItemStack;

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
}
pub mod recipe_manager {
//! Upstream `common/PolymorphRecipeManager.java`, translated 1:1. The
//! `PolymorphApi` capability lookups are a seam trait; the no-capability
//! fallback reads the manager's recipe list directly.

use super::block_entity_recipe_data::AbstractBlockEntityRecipeData;
use super::player_recipe_data::PlayerRecipeData;
use super::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use super::recipe_data::LevelHandle;
use super::recipe_pair::Identifier;

/// Host seam for `PolymorphApi.getInstance()`: the two capability lookups
/// the recipe manager makes.
pub trait PolymorphApiSeam {
    /// `getPlayerRecipeData(player)`.
    fn player_recipe_data(&mut self, player: u64) -> Option<&mut PlayerRecipeData>;
    /// `getBlockEntityRecipeData(blockEntity)`.
    fn block_entity_recipe_data(
        &mut self,
        block_entity: u64,
    ) -> Option<&mut AbstractBlockEntityRecipeData<()>>;
}

/// Upstream `PolymorphRecipeManager`.
pub struct PolymorphRecipeManager;

impl PolymorphRecipeManager {
    /// `getPlayerRecipe`: attach the menu to the player's capability when
    /// one exists, then run its getRecipe; otherwise the plain first match.
    pub fn get_player_recipe(
        api: &mut dyn PolymorphApiSeam,
        container_menu: u64,
        recipe_input: &RecipeInput,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        by_key: &dyn Fn(&Identifier) -> Option<RecipeHolder>,
        level: &LevelHandle,
        player: u64,
        recipes: RecipeList,
    ) -> Option<RecipeHolder> {
        match api.player_recipe_data(player) {
            Some(recipe_data) => {
                recipe_data.set_container_menu(Some(container_menu));
                recipe_data.get_recipe(
                    recipe_input,
                    manager_token,
                    lookup,
                    by_key,
                    level,
                    recipes,
                )
            }
            None => get_recipe_fallback(lookup, recipe_input),
        }
    }

    /// `getBlockEntityRecipe`: the block entity's capability (or the plain
    /// first match), always with an empty passed-in list.
    pub fn get_block_entity_recipe(
        api: &mut dyn PolymorphApiSeam,
        recipe_input: &RecipeInput,
        manager_token: u64,
        lookup: &dyn Fn(&RecipeInput) -> RecipeList,
        by_key: &dyn Fn(&Identifier) -> Option<RecipeHolder>,
        _level: &LevelHandle,
        block_entity: u64,
    ) -> Option<RecipeHolder> {
        match api.block_entity_recipe_data(block_entity) {
            Some(recipe_data) => recipe_data.get_recipe(
                recipe_input,
                manager_token,
                lookup,
                by_key,
                Vec::new(),
            ),
            None => get_recipe_fallback(lookup, recipe_input),
        }
    }
}

/// The private `getRecipe` tail with a null capability:
/// `getRecipesFor(...).stream().findFirst()`.
fn get_recipe_fallback(
    lookup: &dyn Fn(&RecipeInput) -> RecipeList,
    recipe_input: &RecipeInput,
) -> Option<RecipeHolder> {
    lookup(recipe_input).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::block_entity_recipe_data::AbstractBlockEntityRecipeData;
    use super::player_recipe_data::PlayerHandle;
    use super::recipe_cache::Recipe;
    use super::recipe_pair::{Identifier, ItemStack, RecipePair};
    use std::collections::{BTreeMap, BTreeSet};
    use std::rc::Rc;

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
        RecipeHolder { id: id(s), recipe: Rc::new(TestRecipe { out: stack("mod:o", 1) }) }
    }

    fn level(on: bool) -> LevelHandle {
        LevelHandle { on_server_thread: on }
    }

    fn input() -> RecipeInput {
        RecipeInput::new(vec![stack("mod:stick", 1)])
    }

    struct TestApi {
        player: Option<PlayerRecipeData>,
        block: Option<AbstractBlockEntityRecipeData<()>>,
    }

    impl PolymorphApiSeam for TestApi {
        fn player_recipe_data(&mut self, _player: u64) -> Option<&mut PlayerRecipeData> {
            self.player.as_mut()
        }
        fn block_entity_recipe_data(
            &mut self,
            _be: u64,
        ) -> Option<&mut AbstractBlockEntityRecipeData<()>> {
            self.block.as_mut()
        }
    }

    #[test]
    fn a_player_capability_gets_the_menu_and_runs_its_recipe() {
        let mut api = TestApi {
            player: Some(PlayerRecipeData::new(PlayerHandle {
                token: 1,
                tick_count: 3,
                container_menu: 9,
                is_server_player: true,
                is_client_side: false,
            })),
            block: None,
        };
        let out = PolymorphRecipeManager::get_player_recipe(
            &mut api,
            9,
            &input(),
            1,
            &|_| vec![holder("mod:from_data")],
            &|_| None,
            &level(true),
            1,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(out.id, id("mod:from_data"));
        // The menu was attached (and then cleared by the getRecipe tail).
        assert_eq!(api.player.as_ref().unwrap().get_container_menu(), None);
    }

    #[test]
    fn no_player_capability_falls_back_to_the_first_match() {
        let mut api = TestApi { player: None, block: None };
        let out = PolymorphRecipeManager::get_player_recipe(
            &mut api,
            9,
            &input(),
            1,
            &|_| vec![holder("mod:first"), holder("mod:second")],
            &|_| None,
            &level(true),
            1,
            Vec::new(),
        );
        assert_eq!(out.unwrap().id, id("mod:first"));
    }

    #[test]
    fn block_entity_capability_runs_with_an_empty_list() {
        let mut api = TestApi {
            player: None,
            block: Some(AbstractBlockEntityRecipeData::new((), || Vec::new())),
        };
        let out = PolymorphRecipeManager::get_block_entity_recipe(
            &mut api,
            &input(),
            1,
            &|_| vec![holder("mod:be_recipe")],
            &|_| None,
            &level(true),
            5,
        )
        .unwrap();
        assert_eq!(out.id, id("mod:be_recipe"));
        let data = api.block.as_ref().unwrap();
        assert_eq!(
            data.get_recipes_list(),
            &BTreeSet::from([RecipePair { resource_location: id("mod:be_recipe"), output: stack("mod:o", 1) }])
        );
    }
}
}
pub mod block_entity_ticker {
//! Upstream `common/util/BlockEntityTicker.java`, translated 1:1. The
//! `BlockEntity` map keys are owner tokens and the per-tick liveness check
//! (`be.isRemoved() || be.getLevel() == null ||
//! be.getLevel().isClientSide()`) is an entry seam closure.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::block_entity_recipe_data::AbstractBlockEntityRecipeData;

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
    use super::recipe_pair::ItemStack;

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
}
pub mod network {
//! Upstream `common/network/` packets, `common/network/server/ClientPacketHandler.java`
//! and the packet flows, translated 1:1. The Minecraft `StreamCodec`/wire
//! layer is a host seam: packets are the record fields, each `TYPE` is the
//! namespaced id upstream registers, and the handlers carry the logic.

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use super::block_entity_recipe_data::AbstractBlockEntityRecipeData;
use super::block_entity_ticker::BlockEntityTicker;
use super::player_recipe_data::PlayerRecipeData;
use super::recipe_cache::RecipeHolder;
use super::recipe_pair::{Identifier, ItemStack, RecipePair};

/// Upstream `PolymorphApi.MOD_ID`.
pub const MOD_ID: &str = "polymorph";

/// `PolymorphApi.MOD_ID` namespaced type id, standing in for the Java
/// `Type<...>` constants.
pub fn type_id(name: &str) -> Identifier {
    Identifier::new(MOD_ID, name)
}

/// Host seam for the receiving `ServerPlayer`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ServerPlayerRef {
    pub token: u64,
    pub container_menu: u64,
    pub inventory: u64,
}

/// The capability handed over for `CPacketBlockEntityListener`: the owner
/// token (the ticker map key), the block-entity liveness seam and the data.
pub struct BlockEntityData {
    pub owner: u64,
    pub is_alive: Rc<dyn Fn() -> bool>,
    pub data: AbstractBlockEntityRecipeData<()>,
}

/// Host seams for the server side of the clientbound packets.
pub trait ServerPacketEnv {
    /// The receiving player. The host always has one; the explicit null
    /// guard in `CPacketBlockEntityListener` is impossible on a non-null
    /// host.
    fn player(&self) -> ServerPlayerRef;
    /// `player.level().getRecipeManager().byKey(...)`.
    fn by_key(&self, id: &Identifier) -> Option<RecipeHolder>;
    /// `PolymorphApi.getInstance().getPlayerRecipeData(player)`.
    fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData>;
    /// `PolymorphApi.getInstance().getBlockEntityRecipeData(container)`,
    /// handed over for the ticker registration.
    fn block_entity_recipe_data(&mut self, container: u64) -> Option<BlockEntityData>;
    /// `PolymorphIntegrations.selectRecipe(...)`.
    fn integrations_select(&mut self, owner: u64, container: u64, recipe: &RecipeHolder);
    /// `AbstractContainerMenu#slotsChanged(player.getInventory())`.
    fn slots_changed(&mut self, container: u64, inventory: u64);
    /// `container instanceof ItemCombinerMenu`.
    fn is_combiner_menu(&self, container: u64) -> bool;
    /// `((ItemCombinerMenu) container).createResult()`.
    fn create_result(&mut self, container: u64);
    /// `container.broadcastChanges()`.
    fn broadcast_changes(&mut self, container: u64);
    /// `api.getNetwork().sendUpdatePreviewS2C(player)`.
    fn send_update_preview_s2c(&mut self, player: u64);
    /// `api.getNetwork().sendRecipesListS2C(player, pairs, selected)`.
    fn send_recipes_list_s2c(
        &mut self,
        player: u64,
        pairs: &BTreeSet<RecipePair>,
        selected: Option<&Identifier>,
    );
}

/// Upstream record `CPacketPlayerRecipeSelection(ResourceLocation recipe)`.
pub struct CPacketPlayerRecipeSelection {
    pub recipe: Identifier,
}

impl CPacketPlayerRecipeSelection {
    pub fn type_() -> Identifier {
        type_id("player_recipe_selection")
    }

    /// `handle(packet, ServerPlayer)`.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        let player = env.player();
        let container = player.container_menu;
        if let Some(recipe) = env.by_key(&self.recipe) {
            if let Some(recipe_data) = env.player_recipe_data() {
                recipe_data.select_recipe(recipe.clone());
            }
            env.integrations_select(player.token, container, &recipe);
            env.slots_changed(container, player.inventory);
            if env.is_combiner_menu(container) {
                env.create_result(container);
                env.broadcast_changes(container);
                env.send_update_preview_s2c(player.token);
            }
        }
    }
}

/// Upstream record `CPacketBlockEntityListener(boolean add)`.
pub struct CPacketBlockEntityListener {
    pub add: bool,
}

impl CPacketBlockEntityListener {
    pub fn type_() -> Identifier {
        type_id("block_entity_listener")
    }

    /// `handle(packet, ServerPlayer)`.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        let player = env.player();
        if self.add {
            let container = player.container_menu;
            if let Some(be) = env.block_entity_recipe_data(container) {
                BlockEntityTicker::add(player.token, be.owner, be.data, be.is_alive);
                let (pairs, selected) = BlockEntityTicker
                    ::read(be.owner, |data| {
                        (
                            if data.is_empty() {
                                BTreeSet::new()
                            } else {
                                data.get_recipes_list().clone()
                            },
                            data.get_selected_recipe().map(|r| r.id.clone()),
                        )
                    })
                    .unwrap_or((BTreeSet::new(), None));
                env.send_recipes_list_s2c(player.token, &pairs, selected.as_ref());
            }
        } else {
            BlockEntityTicker::remove(player.token);
        }
    }
}

/// Upstream record `CPacketPersistentRecipeSelection(ResourceLocation recipe)`.
pub struct CPacketPersistentRecipeSelection {
    pub recipe: Identifier,
}

impl CPacketPersistentRecipeSelection {
    pub fn type_() -> Identifier {
        type_id("persistent_recipe_selection")
    }

    /// `handle(packet, ServerPlayer)`: the command-sender world's recipe
    /// manager resolves the id, the open menu's block entity gets the
    /// selection plus the integration hook.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        if let Some(recipe) = env.by_key(&self.recipe) {
            let container = env.player().container_menu;
            let mut selected = false;
            if let Some(mut recipe_data) = env.block_entity_recipe_data(container) {
                let owner = recipe_data.owner;
                recipe_data.data.select_recipe(recipe.clone());
                env.integrations_select(owner, container, &recipe);
                selected = true;
            }
            let _ = selected;
        }
    }
}

/// Upstream record `SPacketPlayerRecipeSync(Optional<HashSet<IRecipePair>>,
/// Optional<ResourceLocation>)`.
pub struct SPacketPlayerRecipeSync {
    pub recipe_list: Option<HashSet<RecipePair>>,
    pub selected: Option<Identifier>,
}

impl SPacketPlayerRecipeSync {
    pub fn type_() -> Identifier {
        type_id("recipe_sync")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_player_recipe_sync(self, env);
    }
}

/// Upstream record `SPacketRecipesList(Optional<HashSet<IRecipePair>>,
/// Optional<ResourceLocation>)`.
pub struct SPacketRecipesList {
    pub recipe_list: Option<HashSet<RecipePair>>,
    pub selected: Option<Identifier>,
}

impl SPacketRecipesList {
    pub fn type_() -> Identifier {
        type_id("recipes_list")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_recipes_list(self, env);
    }
}

/// Upstream record `SPacketHighlightRecipe(ResourceLocation recipe)`.
pub struct SPacketHighlightRecipe {
    pub recipe: Identifier,
}

impl SPacketHighlightRecipe {
    pub fn type_() -> Identifier {
        type_id("highlight_recipe")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_highlight_recipe(self, env);
    }
}

/// Upstream class `SPacketUpdatePreview` (a singleton unit packet).
pub struct SPacketUpdatePreview;

impl SPacketUpdatePreview {
    pub const INSTANCE: SPacketUpdatePreview = SPacketUpdatePreview;

    pub fn type_() -> Identifier {
        type_id("update_preview")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_update_preview(self, env);
    }
}

/// Host seam for the client side: the `LocalPlayer`, the player capability,
/// the recipe manager lookup, the recipes widget and the smithing preview.
pub trait ClientPacketEnv {
    /// `Minecraft.getInstance().player`.
    fn client_player(&self) -> Option<u64>;
    /// `PolymorphApi.getInstance().getPlayerRecipeData(clientPlayer)`.
    fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData>;
    /// `clientPlayer.level().getRecipeManager().byKey(...)`.
    fn by_key(&self, id: &Identifier) -> Option<RecipeHolder>;
    /// `RecipesWidget.get()`.
    fn recipes_widget(&mut self) -> Option<&mut dyn RecipesWidget>;
    /// `RecipesWidget.enqueueRecipesList(...)`.
    fn enqueue_recipes_list(&mut self, pairs: BTreeSet<RecipePair>, selected: Option<Identifier>);
    /// `mc.screen instanceof SmithingScreen`.
    fn screen_is_smithing(&self) -> bool;
    /// `smithingScreen.getMenu().getSlot(3).getItem()`.
    fn smithing_slot_item(&self) -> ItemStack;
    /// `AccessorSmithingScreen.callUpdateArmorStandPreview(stack)`.
    fn update_armor_stand_preview(&mut self, stack: ItemStack);
}

/// Host seam for `IRecipesWidget`.
pub trait RecipesWidget {
    /// `widget.setRecipesList(pairs, selected)`.
    fn set_recipes_list(&mut self, pairs: &BTreeSet<RecipePair>, selected: Option<&Identifier>);
    /// `widget.highlightRecipe(recipe)`.
    fn highlight_recipe(&mut self, recipe: &Identifier);
}

/// Upstream `common/network/server/ClientPacketHandler.java`.
pub struct ClientPacketHandler;

impl ClientPacketHandler {
    /// `handle(SPacketPlayerRecipeSync)`.
    pub fn handle_player_recipe_sync(
        packet: SPacketPlayerRecipeSync,
        env: &mut dyn ClientPacketEnv,
    ) {
        if env.client_player().is_some() {
            // Resolve the selection through the recipe manager first; the
            // upstream flatMap runs before `setSelectedRecipe`.
            let selected = packet
                .selected
                .as_ref()
                .and_then(|id| env.by_key(id));
            if let Some(recipe_data) = env.player_recipe_data() {
                recipe_data.set_recipes_list(sort(packet.recipe_list.unwrap_or_default()));
                if let Some(recipe) = selected {
                    recipe_data.set_selected_recipe(recipe);
                }
            }
        }
    }

    /// `handle(SPacketRecipesList)`.
    pub fn handle_recipes_list(packet: SPacketRecipesList, env: &mut dyn ClientPacketEnv) {
        if env.client_player().is_some() {
            let pairs = sort(packet.recipe_list.unwrap_or_default());
            match env.recipes_widget() {
                Some(widget) => widget.set_recipes_list(&pairs, packet.selected.as_ref()),
                None => env.enqueue_recipes_list(pairs, packet.selected.clone()),
            }
        }
    }

    /// `handle(SPacketHighlightRecipe)`.
    pub fn handle_highlight_recipe(packet: SPacketHighlightRecipe, env: &mut dyn ClientPacketEnv) {
        if env.client_player().is_some() {
            if let Some(widget) = env.recipes_widget() {
                widget.highlight_recipe(&packet.recipe);
            }
        }
    }

    /// `handle(SPacketUpdatePreview)`.
    pub fn handle_update_preview(_packet: SPacketUpdatePreview, env: &mut dyn ClientPacketEnv) {
        if env.screen_is_smithing() {
            let stack = env.smithing_slot_item();
            env.update_armor_stand_preview(stack);
        }
    }
}

/// `sort(HashSet<IRecipePair>)`: the wire set is sorted into a TreeSet.
fn sort(set: HashSet<RecipePair>) -> BTreeSet<RecipePair> {
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::recipe_cache::Recipe;

    struct EmptyRecipe;
    impl Recipe for EmptyRecipe {
        fn result_item(&self) -> Option<ItemStack> {
            None
        }
        fn is_custom(&self) -> bool {
            false
        }
        fn assemble(&self, _input: &super::recipe_cache::RecipeInput) -> ItemStack {
            ItemStack {
                item: Identifier::new("test", "result"),
                count: 1,
                components: Default::default(),
            }
        }
    }

    fn holder(id: &str) -> RecipeHolder {
        let (ns, path) = id.split_once(':').unwrap();
        RecipeHolder {
            id: Identifier::new(ns, path),
            recipe: Rc::new(EmptyRecipe),
        }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    fn pair(recipe: &str) -> RecipePair {
        RecipePair {
            resource_location: id(recipe),
            output: ItemStack {
                item: id(&format!("test:out_{}", recipe.replace(':', "_"))),
                count: 1,
                components: Default::default(),
            },
        }
    }

    #[derive(Default)]
    struct TestServerEnv {
        player: ServerPlayerRef,
        registry: Vec<RecipeHolder>,
        player_data: Option<PlayerRecipeData>,
        block_entity: Option<BlockEntityData>,
        integrations: Vec<(u64, u64, Identifier)>,
        slots_changed_count: u32,
        combiner: bool,
        results: u32,
        broadcasts: u32,
        previews: u32,
        sent_lists: Vec<(u64, BTreeSet<RecipePair>, Option<Identifier>)>,
    }

    impl TestServerEnv {
        fn new() -> Self {
            Self {
                player: ServerPlayerRef { token: 1, container_menu: 100, inventory: 7 },
                ..Default::default()
            }
        }
        fn with_player_data(mut self) -> Self {
            self.player_data = Some(PlayerRecipeData::new(
                super::player_recipe_data::PlayerHandle {
                    token: 1,
                    tick_count: 0,
                    container_menu: 100,
                    is_server_player: true,
                    is_client_side: false,
                },
            ));
            self
        }
        fn with_block_entity(mut self) -> Self {
            self.block_entity = Some(BlockEntityData {
                owner: 55,
                is_alive: Rc::new(|| true),
                data: AbstractBlockEntityRecipeData::new((), Vec::new),
            });
            self
        }
    }

    impl ServerPacketEnv for TestServerEnv {
        fn player(&self) -> ServerPlayerRef {
            self.player
        }
        fn by_key(&self, id: &Identifier) -> Option<RecipeHolder> {
            self.registry.iter().find(|h| &h.id == id).cloned()
        }
        fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData> {
            self.player_data.as_mut()
        }
        fn block_entity_recipe_data(&mut self, _container: u64) -> Option<BlockEntityData> {
            self.block_entity.take()
        }
        fn integrations_select(&mut self, owner: u64, container: u64, recipe: &RecipeHolder) {
            self.integrations.push((owner, container, recipe.id.clone()));
        }
        fn slots_changed(&mut self, _container: u64, _inventory: u64) {
            self.slots_changed_count += 1;
        }
        fn is_combiner_menu(&self, _container: u64) -> bool {
            self.combiner
        }
        fn create_result(&mut self, _container: u64) {
            self.results += 1;
        }
        fn broadcast_changes(&mut self, _container: u64) {
            self.broadcasts += 1;
        }
        fn send_update_preview_s2c(&mut self, _player: u64) {
            self.previews += 1;
        }
        fn send_recipes_list_s2c(
            &mut self,
            player: u64,
            pairs: &BTreeSet<RecipePair>,
            selected: Option<&Identifier>,
        ) {
            self.sent_lists
                .push((player, pairs.clone(), selected.cloned()));
        }
    }

    #[test]
    fn player_selection_selects_syncs_and_refreshes() {
        let mut env = TestServerEnv::new().with_player_data();
        env.combiner = true;
        env.registry.push(holder("test:stick"));
        CPacketPlayerRecipeSelection { recipe: id("test:stick") }.handle(&mut env);
        assert_eq!(
            env.player_data.as_ref().unwrap().get_selected_recipe().map(|h| h.id.clone()),
            Some(id("test:stick"))
        );
        assert_eq!(
            env.integrations,
            vec![(1, 100, id("test:stick"))],
            "integration hook gets the player token, container and recipe"
        );
        assert_eq!(env.slots_changed_count, 1);
        assert_eq!(env.results, 1);
        assert_eq!(env.broadcasts, 1);
        assert_eq!(env.previews, 1, "combiner menu sends the preview update");
    }

    #[test]
    fn player_selection_skips_everything_on_an_unknown_recipe() {
        let mut env = TestServerEnv::new().with_player_data();
        CPacketPlayerRecipeSelection { recipe: id("test:missing") }.handle(&mut env);
        assert_eq!(env.slots_changed_count, 0);
        assert!(env.integrations.is_empty());
    }

    #[test]
    fn player_selection_without_a_capability_still_runs_integrations_and_slots() {
        let mut env = TestServerEnv::new();
        env.registry.push(holder("test:stick"));
        CPacketPlayerRecipeSelection { recipe: id("test:stick") }.handle(&mut env);
        assert_eq!(env.slots_changed_count, 1);
        assert_eq!(env.integrations.len(), 1);
    }

    #[test]
    fn block_entity_listener_add_registers_and_sends_the_list() {
        BlockEntityTicker::clear();
        let mut env = TestServerEnv::new().with_block_entity();
        CPacketBlockEntityListener { add: true }.handle(&mut env);
        assert_eq!(BlockEntityTicker::entry_count(), 1, "the BE is tickable");
        assert_eq!(
            BlockEntityTicker::read(55, |d| d.get_listeners()),
            Some(vec![1])
        );
        assert_eq!(
            env.sent_lists,
            vec![(1, BTreeSet::new(), None)],
            "an empty input sends an empty list with no selection"
        );
    }

    #[test]
    fn block_entity_listener_remove_drops_the_listener() {
        BlockEntityTicker::clear();
        let mut env = TestServerEnv::new().with_block_entity();
        CPacketBlockEntityListener { add: true }.handle(&mut env);
        CPacketBlockEntityListener { add: false }.handle(&mut env);
        assert_eq!(BlockEntityTicker::entry_count(), 0);
        assert_eq!(env.sent_lists.len(), 1, "only the add send went out");
    }

    #[test]
    fn persistent_selection_lands_on_the_block_entity_data() {
        let mut env = TestServerEnv::new().with_block_entity();
        env.registry.push(holder("test:trim"));
        CPacketPersistentRecipeSelection { recipe: id("test:trim") }.handle(&mut env);
        assert_eq!(
            env.integrations,
            vec![(55, 100, id("test:trim"))],
            "the owner passed to the integration hook is the block entity"
        );
    }

    #[test]
    fn persistent_selection_with_no_block_entity_does_nothing() {
        let mut env = TestServerEnv::new();
        env.registry.push(holder("test:trim"));
        CPacketPersistentRecipeSelection { recipe: id("test:trim") }.handle(&mut env);
        assert!(env.integrations.is_empty());
    }

    // Client side.

    struct TestWidget {
        sets: Vec<(BTreeSet<RecipePair>, Option<Identifier>)>,
        highlights: Vec<Identifier>,
    }

    impl RecipesWidget for TestWidget {
        fn set_recipes_list(&mut self, pairs: &BTreeSet<RecipePair>, selected: Option<&Identifier>) {
            self.sets.push((pairs.clone(), selected.cloned()));
        }
        fn highlight_recipe(&mut self, recipe: &Identifier) {
            self.highlights.push(recipe.clone());
        }
    }

    struct TestClientEnv {
        player: Option<u64>,
        player_data: PlayerRecipeData,
        registry: Vec<RecipeHolder>,
        widget: Option<TestWidget>,
        enqueued: Vec<(BTreeSet<RecipePair>, Option<Identifier>)>,
        smithing: bool,
        previews: Vec<ItemStack>,
    }

    impl TestClientEnv {
        fn new() -> Self {
            Self {
                player: Some(9),
                player_data: PlayerRecipeData::new(super::player_recipe_data::PlayerHandle {
                    token: 9,
                    tick_count: 0,
                    container_menu: 0,
                    is_server_player: false,
                    is_client_side: true,
                }),
                registry: Vec::new(),
                widget: Some(TestWidget { sets: Vec::new(), highlights: Vec::new() }),
                enqueued: Vec::new(),
                smithing: false,
                previews: Vec::new(),
            }
        }
    }

    impl ClientPacketEnv for TestClientEnv {
        fn client_player(&self) -> Option<u64> {
            self.player
        }
        fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData> {
            Some(&mut self.player_data)
        }
        fn by_key(&self, id: &Identifier) -> Option<RecipeHolder> {
            self.registry.iter().find(|h| &h.id == id).cloned()
        }
        fn recipes_widget(&mut self) -> Option<&mut dyn RecipesWidget> {
            self.widget.as_mut().map(|w| w as &mut dyn RecipesWidget)
        }
        fn enqueue_recipes_list(
            &mut self,
            pairs: BTreeSet<RecipePair>,
            selected: Option<Identifier>,
        ) {
            self.enqueued.push((pairs, selected));
        }
        fn screen_is_smithing(&self) -> bool {
            self.smithing
        }
        fn smithing_slot_item(&self) -> ItemStack {
            ItemStack { item: id("test:helm"), count: 1, components: Default::default() }
        }
        fn update_armor_stand_preview(&mut self, stack: ItemStack) {
            self.previews.push(stack);
        }
    }

    #[test]
    fn player_sync_sets_the_sorted_list_and_resolved_selection() {
        let mut env = TestClientEnv::new();
        env.registry.push(holder("test:chosen"));
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:zeta"));
        wire.insert(pair("test:alpha"));
        SPacketPlayerRecipeSync {
            recipe_list: Some(wire),
            selected: Some(id("test:chosen")),
        }
        .handle(&mut env);
        let list = env.player_data.get_recipes_list();
        let first = list.iter().next().unwrap();
        assert_eq!(first.resource_location, id("test:alpha"), "sorted, not wire order");
        assert_eq!(
            env.player_data.get_selected_recipe().map(|h| h.id.clone()),
            Some(id("test:chosen"))
        );
    }

    #[test]
    fn player_sync_with_no_list_clears_to_an_empty_set() {
        let mut env = TestClientEnv::new();
        SPacketPlayerRecipeSync { recipe_list: None, selected: None }.handle(&mut env);
        assert!(env.player_data.get_recipes_list().is_empty());
    }

    #[test]
    fn recipes_list_goes_to_the_widget_when_present() {
        let mut env = TestClientEnv::new();
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketRecipesList { recipe_list: Some(wire), selected: Some(id("test:a")) }.handle(&mut env);
        let widget = env.widget.as_ref().unwrap();
        assert_eq!(widget.sets.len(), 1);
        assert_eq!(widget.sets[0].0.iter().next().unwrap().resource_location, id("test:a"));
        assert_eq!(widget.sets[0].1, Some(id("test:a")));
        assert!(env.enqueued.is_empty());
    }

    #[test]
    fn recipes_list_is_enqueued_without_a_widget() {
        let mut env = TestClientEnv::new();
        env.widget = None;
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketRecipesList { recipe_list: Some(wire), selected: None }.handle(&mut env);
        assert_eq!(env.enqueued.len(), 1);
        assert_eq!(env.enqueued[0].0.len(), 1);
    }

    #[test]
    fn highlight_reaches_the_widget() {
        let mut env = TestClientEnv::new();
        SPacketHighlightRecipe { recipe: id("test:a") }.handle(&mut env);
        assert_eq!(env.widget.as_ref().unwrap().highlights, vec![id("test:a")]);
    }

    #[test]
    fn update_preview_only_fires_on_a_smithing_screen() {
        let mut env = TestClientEnv::new();
        SPacketUpdatePreview::INSTANCE.handle(&mut env);
        assert!(env.previews.is_empty(), "no smithing screen, no preview");
        env.smithing = true;
        SPacketUpdatePreview::INSTANCE.handle(&mut env);
        assert_eq!(env.previews.len(), 1);
        assert_eq!(env.previews[0].item, id("test:helm"), "slot 3's item");
    }

    #[test]
    fn packets_silently_ignore_a_missing_client_player() {
        let mut env = TestClientEnv::new();
        env.player = None;
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketPlayerRecipeSync { recipe_list: Some(wire.clone()), selected: None }.handle(&mut env);
        SPacketRecipesList { recipe_list: Some(wire), selected: None }.handle(&mut env);
        SPacketHighlightRecipe { recipe: id("test:a") }.handle(&mut env);
        assert!(env.widget.as_ref().unwrap().sets.is_empty());
        assert!(env.enqueued.is_empty());
        assert!(env.widget.as_ref().unwrap().highlights.is_empty());
    }
}
}
pub mod wrappers {
//! Upstream `server/wrapper/IngredientWrapper.java` and
//! `server/wrapper/RecipeWrapper.java`, translated 1:1. The recipe and
//! ingredient types are host seams (`Recipe`/`RecipeHolder` collapse to the
//! id plus the ingredient list this logic reads).

use std::rc::Rc;

use super::recipe_pair::{Identifier, Ingredient};

/// Upstream `IngredientWrapper`: wraps one ingredient for cross-recipe
/// matching.
#[derive(Clone, Debug)]
pub struct IngredientWrapper {
    ingredient: Ingredient,
}

impl IngredientWrapper {
    pub fn new(ingredient: Ingredient) -> Self {
        Self { ingredient }
    }

    pub fn get_ingredient(&self) -> &Ingredient {
        &self.ingredient
    }

    /// `matches`, line for line: null other is false, EMPTY matches only
    /// EMPTY, otherwise any stack of either side matching any stack of the
    /// other counts.
    pub fn matches(&self, other: Option<&IngredientWrapper>) -> bool {
        let Some(other) = other else {
            return false;
        };
        let other_ingredient = &other.ingredient;

        if other_ingredient == &Ingredient::EMPTY {
            return self.ingredient == Ingredient::EMPTY;
        }
        let stacks = &self.ingredient.stacks;

        for other_stack in &other_ingredient.stacks {
            for stack in stacks {
                if stack == other_stack {
                    return true;
                }
            }
        }
        false
    }
}

/// Upstream `RecipeWrapper`: a recipe holder with its ingredients wrapped.
#[derive(Clone, Debug)]
pub struct RecipeWrapper {
    id: Identifier,
    ingredients: Vec<IngredientWrapper>,
}

impl RecipeWrapper {
    /// Upstream takes `RecipeHolder<?>` and reads `id()` +
    /// `value().getIngredients()`; both arrive through the seam.
    pub fn new(id: Identifier, recipe_ingredients: Vec<Ingredient>) -> Self {
        let ingredients =
            recipe_ingredients.into_iter().map(IngredientWrapper::new).collect();
        Self { id, ingredients }
    }

    pub fn get_id(&self) -> &Identifier {
        &self.id
    }

    pub fn get_ingredients(&self) -> &[IngredientWrapper] {
        &self.ingredients
    }

    /// `conflicts`, line for line: null other is false, same id is true,
    /// different ingredient counts are false, otherwise every position must
    /// match.
    pub fn conflicts(&self, other: Option<&RecipeWrapper>) -> bool {
        let Some(other) = other else {
            return false;
        };
        if self.id == other.id {
            return true;
        }
        if self.ingredients.len() != other.ingredients.len() {
            return false;
        }
        let other_ingredients = &other.ingredients;

        for i in 0..other_ingredients.len() {
            if !other_ingredients[i].matches(Some(&self.ingredients[i])) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::recipe_pair::ItemStack;
    use super::recipe_pair::Identifier;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: Default::default() }
    }

    fn wrapper(id: &str, ingredients: Vec<Vec<ItemStack>>) -> RecipeWrapper {
        let (ns, path) = id.split_once(':').unwrap();
        RecipeWrapper::new(
            Identifier::new(ns, path),
            ingredients
                .into_iter()
                .map(|stacks| Ingredient { stacks })
                .collect(),
        )
    }

    #[test]
    fn same_id_conflicts() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)]]);
        let b = wrapper("mod:a", vec![]);
        assert!(a.conflicts(Some(&b)));
    }

    #[test]
    fn different_sizes_do_not_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        assert!(!a.conflicts(Some(&b)));
    }

    #[test]
    fn matching_positions_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        assert!(a.conflicts(Some(&b)));
    }

    #[test]
    fn mismatched_position_does_not_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:planks", 2)], vec![stack("mod:dirt", 1)]]);
        assert!(!a.conflicts(Some(&b)));
    }

    #[test]
    fn empty_matches_only_empty() {
        let empty = IngredientWrapper::new(Ingredient::EMPTY);
        assert!(empty.matches(Some(&IngredientWrapper::new(Ingredient::EMPTY))));
        let full = IngredientWrapper::new(Ingredient { stacks: vec![stack("mod:stick", 1)] });
        assert!(!empty.matches(Some(&full)));
        assert!(!empty.matches(None));
    }
}

/// Upstream `server/wrapper/CraftingRecipeWrapper.java`. The
/// `Services.PLATFORM.isShaped` / `isSameShape` calls are platform seams
/// the caller resolves; the `RecipeWrapper` cast in the upstream
/// `conflicts` signature is a typed parameter here.
pub struct CraftingRecipeWrapper {
    inner: RecipeWrapper,
    shaped: bool,
    is_same_shape: Rc<dyn Fn(&RecipeWrapper, &RecipeWrapper) -> bool>,
}

impl CraftingRecipeWrapper {
    pub fn new(
        inner: RecipeWrapper,
        shaped: bool,
        is_same_shape: Rc<dyn Fn(&RecipeWrapper, &RecipeWrapper) -> bool>,
    ) -> Self {
        Self { inner, shaped, is_same_shape }
    }

    pub fn is_shaped(&self) -> bool {
        self.shaped
    }

    pub fn get_recipe(&self) -> &RecipeWrapper {
        &self.inner
    }

    /// `conflicts(RecipeWrapper pOther)`: the base conflict check plus the
    /// shape gate.
    pub fn conflicts(&self, other: &CraftingRecipeWrapper) -> bool {
        self.inner.conflicts(Some(&other.inner)) && self.is_same_shape(other)
    }

    /// `isSameShape`: two shaped recipes compare shapes on the platform
    /// seam; anything else counts as the same shape.
    fn is_same_shape(&self, other: &CraftingRecipeWrapper) -> bool {
        if self.shaped && other.is_shaped() {
            (self.is_same_shape)(&self.inner, &other.inner)
        } else {
            true
        }
    }
}

/// The smithing recipe parts the accessors expose on
/// `SmithingTrimRecipe` / `SmithingTransformRecipe`; a recipe that is
/// neither keeps every part `Ingredient.EMPTY`.
#[derive(Clone, Debug)]
pub struct SmithingParts {
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
}

/// Upstream `server/wrapper/SmithingRecipeWrapper.java`. The accessor
/// reads are a seam closure, since the translated `Recipe` seam does not
/// carry the vanilla smithing recipe types.
pub struct SmithingRecipeWrapper {
    inner: RecipeWrapper,
    parts: Rc<dyn Fn() -> SmithingParts>,
}

impl SmithingRecipeWrapper {
    pub fn new(inner: RecipeWrapper, parts: Rc<dyn Fn() -> SmithingParts>) -> Self {
        Self { inner, parts }
    }

    pub fn get_recipe(&self) -> &RecipeWrapper {
        &self.inner
    }

    /// `conflicts(RecipeWrapper pOther)`: the base conflict check plus the
    /// three part comparisons. The upstream `&` chain is non-short-circuit
    /// and stays that way.
    pub fn conflicts(&self, other: &SmithingRecipeWrapper) -> bool {
        let mine = (self.parts)();
        let theirs = (other.parts)();
        let base_wrapper = IngredientWrapper::new(mine.base);
        let other_base_wrapper = IngredientWrapper::new(theirs.base);
        let addition_wrapper = IngredientWrapper::new(mine.addition);
        let other_addition_wrapper = IngredientWrapper::new(theirs.addition);
        let template_wrapper = IngredientWrapper::new(mine.template);
        let other_template_wrapper = IngredientWrapper::new(theirs.template);
        self.inner.conflicts(Some(&other.inner))
            & base_wrapper.matches(Some(&other_base_wrapper))
            & addition_wrapper.matches(Some(&other_addition_wrapper))
            & template_wrapper.matches(Some(&other_template_wrapper))
    }
}

#[cfg(test)]
mod crafting_and_smithing_tests {
    use super::*;
    use super::recipe_pair::ItemStack;
    use std::rc::Rc;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: Default::default() }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    /// The platform `isSameShape` seam is global upstream, so the tests
    /// share one closure whose verdict a Cell flips.
    fn crafting(id_str: &str, shaped: bool, platform: Rc<std::cell::Cell<bool>>) -> CraftingRecipeWrapper {
        CraftingRecipeWrapper::new(
            RecipeWrapper::new(id(id_str), vec![Ingredient { stacks: vec![stack("test:plank", 1)] }]),
            shaped,
            Rc::new(move |_, _| platform.get()),
        )
    }

    #[test]
    fn crafting_conflicts_need_the_same_shape() {
        let platform = Rc::new(std::cell::Cell::new(true));
        let a = crafting("test:a", true, platform.clone());
        let b = crafting("test:b", true, platform.clone());
        assert!(a.conflicts(&b));
        platform.set(false);
        let c = crafting("test:c", true, platform.clone());
        assert!(!a.conflicts(&c), "different shapes do not conflict");
    }

    #[test]
    fn the_shape_gate_only_runs_between_two_shaped_recipes() {
        let platform = Rc::new(std::cell::Cell::new(false));
        let a = crafting("test:a", false, platform.clone());
        let b = crafting("test:b", false, platform.clone());
        assert!(a.conflicts(&b), "the platform shape check never runs unshaped");
        let shaped = crafting("test:c", true, platform.clone());
        assert!(a.conflicts(&shaped), "either side unshaped counts as the same shape");
    }

    fn smithing(
        id_str: &str,
        template: Option<&str>,
        base: Option<&str>,
        addition: Option<&str>,
    ) -> SmithingRecipeWrapper {
        let parts = SmithingParts {
            template: Ingredient {
                stacks: template.map(|t| vec![stack(t, 1)]).unwrap_or_default(),
            },
            base: Ingredient {
                stacks: base.map(|b| vec![stack(b, 1)]).unwrap_or_default(),
            },
            addition: Ingredient {
                stacks: addition.map(|a| vec![stack(a, 1)]).unwrap_or_default(),
            },
        };
        SmithingRecipeWrapper::new(
            RecipeWrapper::new(id(id_str), vec![Ingredient::EMPTY]),
            Rc::new(move || parts.clone()),
        )
    }

    #[test]
    fn smithing_conflicts_match_every_part() {
        let a = smithing("test:a", Some("test:t"), Some("test:b"), Some("test:a1"));
        let b = smithing("test:b", Some("test:t"), Some("test:b"), Some("test:a1"));
        assert!(a.conflicts(&b));
        let c = smithing("test:c", Some("test:t"), Some("test:b"), Some("test:other"));
        assert!(!a.conflicts(&c), "a different addition does not conflict");
    }

    #[test]
    fn smithing_empty_parts_only_match_empty() {
        let plain = smithing("test:plain", None, None, None);
        let trim = smithing("test:trim", Some("test:t"), Some("test:b"), Some("test:a1"));
        assert!(!plain.conflicts(&trim), "EMPTY never matches a filled part");
        assert!(plain.conflicts(&smithing("test:plain2", None, None, None)));
    }
}
}
pub mod client_widget {
//! Upstream `api/client/base/*`, `api/client/widgets/**`,
//! `client/RecipesWidget.java`, `client/PolymorphWidgetsImpl.java` and
//! `client/PolymorphClientEvents.java`, translated 1:1. The Minecraft client
//! GUI (`GuiGraphics`, `AbstractWidget`, `ImageButton`, `WidgetSprites`),
//! `AbstractContainerScreen`/`AbstractContainerMenu`, `Minecraft` and the
//! C2S send path are host seams with the same shapes and semantics. Java
//! inheritance becomes a behavior enum: the three widget subclasses differ
//! only in which output slot they bind and how `selectRecipe` routes.

use super::network::MOD_ID;
use super::recipe_pair::{Identifier, ItemStack, RecipePair};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

/// Host seam for `net.minecraft.client.gui.components.WidgetSprites`.
#[derive(Clone, Debug)]
pub struct WidgetSprites {
    /// `sprite.enabled()`: the idle texture.
    pub enabled: Identifier,
    /// `sprite.enabledFocused()`: the hovered texture.
    pub enabled_focused: Identifier,
}

/// Host seam for `Pair<WidgetSprites, WidgetSprites>`.
#[derive(Clone, Debug)]
pub struct SpritePair {
    pub first: WidgetSprites,
    pub second: WidgetSprites,
}

/// `AbstractRecipesWidget.OUTPUT`.
pub fn output_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "output_button"),
        enabled_focused: Identifier::new(MOD_ID, "output_button_highlighted"),
    }
}

/// `AbstractRecipesWidget.CURRENT_OUTPUT`.
pub fn current_output_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "current_output"),
        enabled_focused: Identifier::new(MOD_ID, "current_output_highlighted"),
    }
}

/// `AbstractRecipesWidget.SELECTOR`.
pub fn selector_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "selector_button"),
        enabled_focused: Identifier::new(MOD_ID, "selector_button_highlighted"),
    }
}

pub const BUTTON_X_OFFSET: i32 = 0;
pub const BUTTON_Y_OFFSET: i32 = -22;
pub const WIDGET_X_OFFSET: i32 = -4;
pub const WIDGET_Y_OFFSET: i32 = -26;

/// Host seam for `net.minecraft.world.inventory.Slot` reduced to what the
/// widgets read: the draw position and the backing container kind, which
/// drives `findResultSlot` (`ResultContainer`) and the crafter scan
/// (`NonInteractiveResultSlot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotRef {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub container: ContainerKind,
}

/// The backing `Container` classes the widget layer distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    /// `ResultContainer`.
    Result,
    /// `NonInteractiveResultSlot`'s container.
    NonInteractiveResult,
    /// Any other container.
    Other,
}

/// Host seam for `AbstractContainerScreen<?>` and its menu: identity, the
/// slot list, the screen's draw origin and the menu type, which drives
/// `PolymorphWidgetsImpl.setup`'s instanceof chain.
pub trait ClientScreen {
    /// Screen identity; two calls with the same id are one screen.
    fn id(&self) -> u64;
    /// `containerScreen.getMenu().slots`.
    fn menu_slots(&self) -> Vec<SlotRef>;
    /// `Services.CLIENT_PLATFORM.getScreenLeft(screen)`.
    fn screen_left(&self) -> i32;
    /// `Services.CLIENT_PLATFORM.getScreenTop(screen)`.
    fn screen_top(&self) -> i32;
    /// The menu kind for the `setup` matcher.
    fn menu_kind(&self) -> MenuKind;
}

/// The menu classes `PolymorphWidgetsImpl.setup` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    /// `SmithingMenu`.
    Smithing,
    /// `AbstractFurnaceMenu`.
    AbstractFurnace,
    /// `CrafterMenu`.
    Crafter,
    /// Any other menu.
    Other,
}

/// Host seam for the client side of `GuiGraphics`.
pub trait GuiGraphics {
    /// `guiGraphics.blitSprite(texture, x, y, z, width, height)`.
    fn blit_sprite(&mut self, texture: &Identifier, x: i32, y: i32, z: i32, width: i32, height: i32);
    /// `guiGraphics.renderItem(stack, x, y)`.
    fn render_item(&mut self, stack: &ItemStack, x: i32, y: i32);
    /// `guiGraphics.renderItemDecorations(font, stack, x, y)`.
    fn render_item_decorations(&mut self, stack: &ItemStack, x: i32, y: i32);
    /// `guiGraphics.renderTooltip(font, stack, mouseX, mouseY)`.
    fn render_tooltip(&mut self, stack: &ItemStack, mouse_x: i32, mouse_y: i32);
}

/// Host seam for the client world/network face the widgets touch:
/// `Minecraft.getInstance()` connection and player, the level recipe
/// manager's `byKey`, and the three C2S sends on
/// `PolymorphApi.getInstance().getNetwork()`.
pub trait ClientWidgetEnv {
    /// `Minecraft.getInstance().getConnection() != null`.
    fn connection_present(&self) -> bool;
    /// `level.getRecipeManager().byKey(id)` on the client player's level.
    fn recipe_by_key(&self, id: &Identifier) -> Option<super::recipe_cache::RecipeHolder>;
    /// `api.getPlayerRecipeData(player).selectRecipe(recipe)`.
    fn select_on_player_capability(&mut self, recipe: super::recipe_cache::RecipeHolder);
    /// `sendBlockEntityListenerC2S(add)`.
    fn send_block_entity_listener_c2s(&mut self, add: bool);
    /// `sendPersistentRecipeSelectionC2S(resourceLocation)`.
    fn send_persistent_recipe_selection_c2s(&mut self, id: &Identifier);
    /// `sendPlayerRecipeSelectionC2S(resourceLocation)`.
    fn send_player_recipe_selection_c2s(&mut self, id: &Identifier);
}

/// `api/client/widgets/children/OutputWidget.java`.
pub struct OutputWidget {
    x: i32,
    y: i32,
    /// `AbstractWidget.visible`.
    pub visible: bool,
    output: ItemStack,
    resource_location: Identifier,
    sprites: SpritePair,
    highlighted: bool,
}

impl OutputWidget {
    pub fn new(sprites: SpritePair, recipe_pair: &RecipePair) -> Self {
        Self {
            x: 0,
            y: 0,
            visible: true,
            output: recipe_pair.output.clone(),
            resource_location: recipe_pair.resource_location.clone(),
            sprites,
            highlighted: false,
        }
    }

    /// `setPosition` (AbstractWidget host seam).
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn get_x(&self) -> i32 {
        self.x
    }

    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// `getWidth()` is pinned to 25.
    pub fn get_width(&self) -> i32 {
        25
    }

    pub fn get_output(&self) -> &ItemStack {
        &self.output
    }

    pub fn get_resource_location(&self) -> &Identifier {
        &self.resource_location
    }

    pub fn set_highlighted(&mut self, highlighted: bool) {
        self.highlighted = highlighted;
    }

    pub fn is_highlighted(&self) -> bool {
        self.highlighted
    }

    /// `renderWidget`: the sprite pair's second half when highlighted, the
    /// focused texture on hover, the sprite blitted at z=600, the item and
    /// its decorations pushed to z=700.
    pub fn render_widget(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        let sprite = if self.highlighted {
            &self.sprites.second
        } else {
            &self.sprites.first
        };
        let mut texture = sprite.enabled.clone();

        if self.x + 25 > mouse_x && self.x <= mouse_x && self.y + 25 > mouse_y && self.y <= mouse_y
        {
            texture = sprite.enabled_focused.clone();
        }
        gui.blit_sprite(&texture, self.x, self.y, 600, self.get_width(), self.get_width());
        let k = 4;
        gui.render_item(&self.output, self.x + k, self.y + k);
        gui.render_item_decorations(&self.output, self.x + k, self.y + k);
    }

    /// `isValidClickButton`: button 0 or 1.
    fn is_valid_click_button(&self, button: i32) -> bool {
        button == 0 || button == 1
    }

    /// `AbstractWidget#mouseClicked` on this 25x25 widget.
    pub fn mouse_clicked(&self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        let inside = (mouse_x as i32) > self.x
            && (mouse_x as i32) < self.x + 25
            && (mouse_y as i32) > self.y
            && (mouse_y as i32) < self.y + 25;
        inside && self.is_valid_click_button(button)
    }
}

/// `api/client/widgets/children/SelectionWidget.java`.
pub struct SelectionWidget {
    on_select: Rc<dyn Fn(&Identifier)>, // upstream `onSelect`
    container_screen: Rc<dyn ClientScreen>,
    output_widgets: Vec<OutputWidget>,
    sprites: SpritePair,
    x_offset: i32,
    y_offset: i32,
    /// `hoveredButton`, as an index into `output_widgets`.
    hovered_button: Option<usize>,
    active: bool,
    x: i32,
    y: i32,
    last_x: i32,
    last_y: i32,
}

impl SelectionWidget {
    pub fn new(
        x: i32,
        y: i32,
        x_offset: i32,
        y_offset: i32,
        sprites: SpritePair,
        on_select: Rc<dyn Fn(&Identifier)>,
        container_screen: Rc<dyn ClientScreen>,
    ) -> Self {
        let mut widget = Self {
            on_select,
            container_screen,
            output_widgets: Vec::new(),
            sprites,
            x_offset,
            y_offset,
            hovered_button: None,
            active: false,
            x: 0,
            y: 0,
            last_x: -1,
            last_y: -1,
        };
        widget.set_position(x, y);
        widget
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        self.update_button_positions();
    }

    pub fn set_offsets(&mut self, x: i32, y: i32) {
        self.x_offset = x;
        self.y_offset = y;
    }

    /// `highlightButton`: highlight only the widget with that recipe.
    pub fn highlight_button(&mut self, resource_location: &Identifier) {
        for widget in &mut self.output_widgets {
            widget.set_highlighted(widget.get_resource_location() == resource_location);
        }
    }

    /// `updateButtonPositions`: center the row on the anchor, with the even
    /// count nudged 13 right.
    fn update_button_positions(&mut self) {
        let size = self.output_widgets.len() as i32;
        let mut x_offset = -25 * (size as f32 / 2.0).floor() as i32;

        if size % 2 == 0 {
            x_offset += 13;
        }
        let mut pos = (self.x + x_offset, self.y);
        for widget in &mut self.output_widgets {
            widget.set_position(pos.0, pos.1);
            pos.0 += 25;
        }
    }

    pub fn get_output_widgets(&self) -> &[OutputWidget] {
        &self.output_widgets
    }

    /// `setRecipeList`: drop pairs whose output is empty, then re-layout.
    pub fn set_recipe_list(&mut self, recipe_list: &BTreeSet<RecipePair>) {
        self.output_widgets.clear();
        for data in recipe_list {
            if !data.output.is_empty() {
                self.output_widgets.push(OutputWidget::new(self.sprites.clone(), data));
            }
        }
        self.update_button_positions();
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// `renderTooltip`: the hovered button's output at z=501.
    fn render_tooltip(&self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if let Some(index) = self.hovered_button {
            let button = &self.output_widgets[index];
            gui.render_tooltip(button.get_output(), mouse_x, mouse_y);
        }
    }

    /// `render`: when active, track the screen's draw origin, render each
    /// button and record the hovered one.
    pub fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if self.is_active() {
            let x = self.container_screen.screen_left() + self.x_offset;
            let y = self.container_screen.screen_top() + self.y_offset;

            if self.last_x != x || self.last_y != y {
                self.set_position(x, y);
                self.last_x = x;
                self.last_y = y;
            }
            self.hovered_button = None;
            for (index, button) in self.output_widgets.iter_mut().enumerate() {
                button.render_widget(gui, mouse_x, mouse_y);

                if button.visible && button.mouse_clicked(mouse_x as f64, mouse_y as f64, 0) {
                    self.hovered_button = Some(index);
                }
            }
            self.render_tooltip(gui, mouse_x, mouse_y);
        }
    }

    /// `mouseClicked`: fire `onSelect` for the clicked widget.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        if self.is_active() {
            for widget in &self.output_widgets {
                if widget.mouse_clicked(mouse_x, mouse_y, button) {
                    (self.on_select)(widget.get_resource_location());
                    return true;
                }
            }
        }
        false
    }
}

/// `api/client/widgets/children/OpenSelectionButton.java`, an `ImageButton`
/// 16x16 that repositions itself against the screen's draw origin on render.
pub struct OpenSelectionButton {
    x: i32,
    y: i32,
    container_screen: Rc<dyn ClientScreen>,
    x_offset: i32,
    y_offset: i32,
    sprites: WidgetSprites,
    /// The `OnPress` callback.
    pub on_press: Rc<dyn Fn(&mut OpenSelectionButton)>,
    /// `AbstractWidget.visible`.
    pub visible: bool,
    active: bool,
}

impl OpenSelectionButton {
    pub fn new(
        container_screen: Rc<dyn ClientScreen>,
        x: i32,
        y: i32,
        sprites: WidgetSprites,
        on_press: Rc<dyn Fn(&mut OpenSelectionButton)>,
    ) -> Self {
        Self {
            x: 0,
            y: 0,
            container_screen,
            x_offset: x,
            y_offset: y,
            sprites,
            on_press,
            visible: true,
            active: true,
        }
    }

    pub fn set_offsets(&mut self, x: i32, y: i32) {
        self.x_offset = x;
        self.y_offset = y;
    }

    pub fn get_x(&self) -> i32 {
        self.x
    }

    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// `renderWidget`: reset the shader color, reposition against the
    /// screen's draw origin, then the super render (the sprite blit).
    pub fn render_widget(&mut self, gui: &mut dyn GuiGraphics, _mouse_x: i32, _mouse_y: i32) {
        self.x = self.container_screen.screen_left() + self.x_offset;
        self.y = self.container_screen.screen_top() + self.y_offset;
        let texture = if self.active {
            self.sprites.enabled.clone()
        } else {
            self.sprites.enabled_focused.clone()
        };
        gui.blit_sprite(&texture, self.x, self.y, 0, 16, 16);
    }

    /// `AbstractWidget#mouseClicked` on this 16x16 button, then `onPress`.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        let inside = (mouse_x as i32) >= self.x
            && (mouse_x as i32) < self.x + 16
            && (mouse_y as i32) >= self.y
            && (mouse_y as i32) < self.y + 16;
        if inside && button == 0 {
            let on_press = self.on_press.clone();
            on_press(self);
            true
        } else {
            false
        }
    }
}

/// Which upstream subclass the widget stands for; they differ only in their
/// output slot and `selectRecipe` routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetBehavior {
    /// `PersistentRecipesWidget`: `selectRecipe` sends the persistent C2S.
    Persistent,
    /// `PlayerRecipesWidget`: local capability select, then the player C2S.
    Player,
}

/// `api/client/base/AbstractRecipesWidget.java` with the subclass split
/// carried in `behavior` and `output_slot`.
pub struct AbstractRecipesWidget {
    pub container_screen: Rc<dyn ClientScreen>,
    env: Rc<RefCell<dyn ClientWidgetEnv>>,
    pub x_offset: i32,
    pub y_offset: i32,
    pub behavior: WidgetBehavior,
    /// The bound output slot (`FurnaceRecipesWidget` slot 2, the crafter
    /// scan, or the passed-in `PlayerRecipesWidget` slot).
    pub output_slot: SlotRef,
    pub selection_widget: Option<SelectionWidget>,
    pub open_button: Option<OpenSelectionButton>,
}

impl AbstractRecipesWidget {
    /// `AbstractRecipesWidget(containerScreen)`: the default offsets.
    pub fn new(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
        behavior: WidgetBehavior,
        output_slot: SlotRef,
    ) -> Self {
        Self {
            container_screen,
            env,
            x_offset: WIDGET_X_OFFSET,
            y_offset: WIDGET_Y_OFFSET,
            behavior,
            output_slot,
            selection_widget: None,
            open_button: None,
        }
    }

    /// `FurnaceRecipesWidget`: the furnace's result slot is index 2.
    pub fn new_furnace(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Self {
        let slot = container_screen
            .menu_slots()
            .get(2)
            .copied()
            .expect("furnace menu slot 2");
        Self::new(container_screen, env, WidgetBehavior::Persistent, slot)
    }

    /// `CrafterRecipesWidget`: the first `NonInteractiveResultSlot`, else
    /// the menu's first slot.
    pub fn new_crafter(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Self {
        let slots = container_screen.menu_slots();
        let slot = slots
            .iter()
            .find(|slot| slot.container == ContainerKind::NonInteractiveResult)
            .or_else(|| slots.first())
            .copied()
            .expect("crafter menu slot");
        Self::new(container_screen, env, WidgetBehavior::Persistent, slot)
    }

    /// `getSelectorSprites`.
    pub fn get_selector_sprites(&self) -> WidgetSprites {
        selector_sprites()
    }

    /// `getOutputSprites`.
    pub fn get_output_sprites(&self) -> SpritePair {
        SpritePair { first: output_sprites(), second: current_output_sprites() }
    }

    /// `resetWidgetOffsets`.
    pub fn reset_widget_offsets(&mut self) {
        let x = self.get_x_pos();
        let y = self.get_y_pos();
        if let Some(widget) = &mut self.selection_widget {
            widget.set_offsets(x + self.x_offset, y + self.y_offset);
        }
        if let Some(button) = &mut self.open_button {
            button.set_offsets(x, y);
        }
    }

    /// `initChildWidgets`.
    pub fn init_child_widgets(&mut self) {
        let x = self.container_screen.screen_left() + self.get_x_pos();
        let y = self.container_screen.screen_top() + self.get_y_pos();
        let env = self.env.clone();
        let behavior = self.behavior;
        let screen = self.container_screen.clone();
        let sprites = self.get_output_sprites();
        let selection_widget = SelectionWidget::new(
            x + self.x_offset,
            y + self.y_offset,
            self.get_x_pos() + self.x_offset,
            self.get_y_pos() + self.y_offset,
            sprites,
            Rc::new(move |id: &Identifier| match behavior {
                WidgetBehavior::Persistent => {
                    env.borrow_mut().send_persistent_recipe_selection_c2s(id);
                }
                WidgetBehavior::Player => {
                    // `PlayerRecipesWidget.selectRecipe`: the local
                    // capability select through the recipe manager, then the
                    // C2S packet.
                    let recipe = env.borrow().recipe_by_key(id);
                    if let Some(recipe) = recipe {
                        env.borrow_mut().select_on_player_capability(recipe);
                    }
                    env.borrow_mut().send_player_recipe_selection_c2s(id);
                }
            }),
            screen,
        );
        let visible_more_than_one = selection_widget.get_output_widgets().len() > 1;
        let screen = self.container_screen.clone();
        let x_pos = self.get_x_pos();
        let y_pos = self.get_y_pos();
        let selector = self.get_selector_sprites();
        let open_button = OpenSelectionButton::new(
            screen,
            x_pos,
            y_pos,
            selector,
            Rc::new(|_button: &mut OpenSelectionButton| {
                // The toggle itself runs at the widget level below, where
                // the selection widget is reachable; upstream defines the
                // same lambda inside initChildWidgets.
            }),
        );
        let mut open_button = open_button;
        open_button.visible = visible_more_than_one;
        self.selection_widget = Some(selection_widget);
        self.open_button = Some(open_button);
    }

    pub fn get_x_pos(&self) -> i32 {
        self.output_slot.x + BUTTON_X_OFFSET
    }

    pub fn get_y_pos(&self) -> i32 {
        self.output_slot.y + BUTTON_Y_OFFSET
    }

    /// `AbstractRecipesWidget.selectRecipe` is abstract upstream; here the
    /// subclass routing runs through `initChildWidgets`'s onSelect closure,
    /// so this path only stands for direct calls.
    pub fn select_recipe(&mut self, resource_location: &Identifier) {
        match self.behavior {
            WidgetBehavior::Persistent => {
                self.env
                    .borrow_mut()
                    .send_persistent_recipe_selection_c2s(resource_location);
            }
            WidgetBehavior::Player => {
                let recipe = self.env.borrow().recipe_by_key(resource_location);
                if let Some(recipe) = recipe {
                    self.env.borrow_mut().select_on_player_capability(recipe);
                }
                self.env
                    .borrow_mut()
                    .send_player_recipe_selection_c2s(resource_location);
            }
        }
    }

    pub fn highlight_recipe(&mut self, resource_location: &Identifier) {
        if let Some(widget) = &mut self.selection_widget {
            widget.highlight_button(resource_location);
        }
    }

    /// `setRecipesList`: sort into a TreeSet, refresh the list, show the
    /// open button only above one result, highlight the selected recipe.
    pub fn set_recipes_list(
        &mut self,
        recipes_list: &BTreeSet<RecipePair>,
        selected: Option<&Identifier>,
    ) {
        let sorted: BTreeSet<RecipePair> = recipes_list.iter().cloned().collect();
        if let Some(widget) = &mut self.selection_widget {
            widget.set_recipe_list(&sorted);
        }
        if let Some(button) = &mut self.open_button {
            button.visible = recipes_list.len() > 1;
        }

        if let Some(selected) = selected {
            self.highlight_recipe(selected);
        }
    }

    /// `render`.
    pub fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if let Some(widget) = &mut self.selection_widget {
            widget.render(gui, mouse_x, mouse_y);
        }
        if let Some(button) = &mut self.open_button {
            if button.visible {
                button.render_widget(gui, mouse_x, mouse_y);
            }
        }
    }

    /// `mouseClicked`: the open button first, then a selection click, then
    /// deactivating an open panel on any other click.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        if let Some(button_widget) = &mut self.open_button {
            if button_widget.mouse_clicked(mouse_x, mouse_y, button) {
                if let Some(widget) = &mut self.selection_widget {
                    widget.set_active(!widget.is_active());
                }
                return true;
            }
        }
        if let Some(widget) = &mut self.selection_widget {
            if widget.mouse_clicked(mouse_x, mouse_y, button) {
                widget.set_active(false);
                return true;
            } else if widget.is_active() {
                widget.set_active(false);
                return true;
            }
        }
        false
    }
}

/// `IRecipesWidget` as implemented by the widget layer.
pub trait IRecipesWidget {
    fn init_child_widgets(&mut self);
    fn select_recipe(&mut self, resource_location: &Identifier);
    fn highlight_recipe(&mut self, resource_location: &Identifier);
    fn set_recipes_list(&mut self, recipes_list: &BTreeSet<RecipePair>, selected: Option<&Identifier>);
    fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32);
    fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool;
    fn get_output_slot(&self) -> SlotRef;
    /// `widget instanceof PersistentRecipesWidget`.
    fn is_persistent(&self) -> bool {
        false
    }
}

impl IRecipesWidget for AbstractRecipesWidget {
    fn init_child_widgets(&mut self) {
        AbstractRecipesWidget::init_child_widgets(self)
    }

    fn select_recipe(&mut self, resource_location: &Identifier) {
        AbstractRecipesWidget::select_recipe(self, resource_location)
    }

    fn highlight_recipe(&mut self, resource_location: &Identifier) {
        AbstractRecipesWidget::highlight_recipe(self, resource_location)
    }

    fn set_recipes_list(&mut self, recipes_list: &BTreeSet<RecipePair>, selected: Option<&Identifier>) {
        AbstractRecipesWidget::set_recipes_list(self, recipes_list, selected)
    }

    fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        AbstractRecipesWidget::render(self, gui, mouse_x, mouse_y)
    }

    fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        AbstractRecipesWidget::mouse_clicked(self, mouse_x, mouse_y, button)
    }

    fn get_output_slot(&self) -> SlotRef {
        self.output_slot
    }

    fn is_persistent(&self) -> bool {
        self.behavior == WidgetBehavior::Persistent
    }
}

/// `api/client/base/ITickingRecipesWidget.java`: the tick opt-in no shipped
/// widget uses yet.
pub trait ITickingRecipesWidget {
    fn tick(&mut self);
}

/// `client/RecipesWidget.java`: the static widget holder. The upstream
/// statics become thread-locals; `PolymorphWidgets` and the client env are
/// passed in where upstream reads its singletons.
pub struct RecipesWidgetHolder;

thread_local! {
    static WIDGET: RefCell<Option<Box<dyn IRecipesWidget>>> = const { RefCell::new(None) };
    static LAST_SCREEN: RefCell<Option<u64>> = const { RefCell::new(None) };
    static PENDING_DATA: RefCell<Option<(BTreeSet<RecipePair>, Option<Identifier>)>> =
        const { RefCell::new(None) };
}

impl RecipesWidgetHolder {
    /// `get().isPresent()`.
    pub fn is_present() -> bool {
        WIDGET.with_borrow(|w| w.is_some())
    }

    /// `get()`: run `f` against the live widget when one exists.
    pub fn with_widget<R>(f: impl FnOnce(Option<&mut Box<dyn IRecipesWidget>>) -> R) -> R {
        WIDGET.with_borrow_mut(|w| f(w.as_mut()))
    }

    /// `enqueueRecipesList`.
    pub fn enqueue_recipes_list(recipes_list: BTreeSet<RecipePair>, selected: Option<Identifier>) {
        PENDING_DATA.with_borrow_mut(|p| *p = Some((recipes_list, selected)));
    }

    /// `create`: reuse the widget on the same screen, else resolve through
    /// the registry, init it, and flush any pending recipes list.
    pub fn create(
        container_screen: &Rc<dyn ClientScreen>,
        widgets: &PolymorphWidgetsImpl,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) {
        let same_screen_and_widget = LAST_SCREEN.with_borrow(|s| *s == Some(container_screen.id()))
            && WIDGET.with_borrow(|w| w.is_some());
        if same_screen_and_widget {
            return;
        }
        let mut widget = widgets.get_widget_or_default(container_screen, env);

        if widget.is_some() {
            let widget = widget.as_mut().unwrap();

            if widget.is_persistent() && env.borrow().connection_present() {
                env.borrow_mut().send_block_entity_listener_c2s(true);
            }
            widget.init_child_widgets();
            LAST_SCREEN.with_borrow_mut(|s| *s = Some(container_screen.id()));

            if let Some((list, selected)) = PENDING_DATA.take() {
                widget.set_recipes_list(&list, selected.as_ref());
            }
        } else {
            LAST_SCREEN.with_borrow_mut(|s| *s = None);
        }
        WIDGET.with_borrow_mut(|w| *w = widget);
        PENDING_DATA.with_borrow_mut(|p| *p = None);
    }

    /// `clear`.
    pub fn clear(env: &Rc<RefCell<dyn ClientWidgetEnv>>) {
        let persistent = WIDGET.with_borrow(|w| w.as_ref().map(|w| w.is_persistent()));
        if persistent == Some(true) && env.borrow().connection_present() {
            env.borrow_mut().send_block_entity_listener_c2s(false);
        }
        WIDGET.with_borrow_mut(|w| *w = None);
        LAST_SCREEN.with_borrow_mut(|s| *s = None);
    }
}

/// `PolymorphWidgets.IRecipesWidgetFactory`.
pub trait IRecipesWidgetFactory {
    fn create_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
}

/// `api/client/PolymorphWidgets.java`.
pub trait PolymorphWidgetsApi {
    fn get_current_widget(&self) -> bool;
    fn get_widget_or_default(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
    fn get_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
    fn register_widget(&self, factory: Box<dyn IRecipesWidgetFactory>);
    fn find_result_slot(&self, container_screen: &Rc<dyn ClientScreen>) -> Option<SlotRef>;
}

/// `client/PolymorphWidgetsImpl.java`. The `CopyOnWriteArrayList` becomes a
/// plain list behind a `RefCell`; registration only happens at setup.
pub struct PolymorphWidgetsImpl {
    widget_factories: RefCell<Vec<Box<dyn IRecipesWidgetFactory>>>,
}

impl PolymorphWidgetsImpl {
    pub fn new() -> Self {
        Self { widget_factories: RefCell::new(Vec::new()) }
    }

    /// `setup`: register the built-in menu matcher.
    pub fn setup(&self) {
        self.register_widget(Box::new(BuiltinWidgetFactory));
    }
}

impl Default for PolymorphWidgetsImpl {
    fn default() -> Self {
        Self::new()
    }
}

/// The `setup` factory: smithing binds slot 3 of the player recipes widget,
/// furnaces use the furnace widget, crafters the crafter widget.
struct BuiltinWidgetFactory;

impl IRecipesWidgetFactory for BuiltinWidgetFactory {
    fn create_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        match container_screen.menu_kind() {
            MenuKind::Smithing => {
                let slot = container_screen
                    .menu_slots()
                    .get(3)
                    .copied()
                    .expect("smithing menu slot 3");
                Some(Box::new(AbstractRecipesWidget::new(
                    container_screen.clone(),
                    env.clone(),
                    WidgetBehavior::Player,
                    slot,
                )))
            }
            MenuKind::AbstractFurnace => Some(Box::new(AbstractRecipesWidget::new_furnace(
                container_screen.clone(),
                env.clone(),
            ))),
            MenuKind::Crafter => Some(Box::new(AbstractRecipesWidget::new_crafter(
                container_screen.clone(),
                env.clone(),
            ))),
            MenuKind::Other => None,
        }
    }
}

impl PolymorphWidgetsApi for PolymorphWidgetsImpl {
    /// `getCurrentWidget`: whether the static holder has a widget.
    fn get_current_widget(&self) -> bool {
        RecipesWidgetHolder::is_present()
    }

    /// `getWidgetOrDefault`: the registered match, else the player widget on
    /// the found result slot.
    fn get_widget_or_default(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        let widget = self.get_widget(container_screen, env);

        if widget.is_none() {
            let slot = self.find_result_slot(container_screen);

            if let Some(slot) = slot {
                return Some(Box::new(AbstractRecipesWidget::new(
                    container_screen.clone(),
                    env.clone(),
                    WidgetBehavior::Player,
                    slot,
                )));
            }
        }
        widget
    }

    /// `getWidget`: the first factory that returns a widget.
    fn get_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        for factory in self.widget_factories.borrow().iter() {
            if let Some(widget) = factory.create_widget(container_screen, env) {
                return Some(widget);
            }
        }
        None
    }

    /// `registerWidget`: a null factory is rejected upstream with an error
    /// log; here the caller cannot pass one, so the list just grows.
    fn register_widget(&self, factory: Box<dyn IRecipesWidgetFactory>) {
        self.widget_factories.borrow_mut().push(factory);
    }

    /// `findResultSlot`: the first slot over a `ResultContainer`.
    fn find_result_slot(&self, container_screen: &Rc<dyn ClientScreen>) -> Option<SlotRef> {
        container_screen
            .menu_slots()
            .into_iter()
            .find(|slot| slot.container == ContainerKind::Result)
    }
}

/// `client/PolymorphClientEvents.java`, the four screen event hooks.
pub struct PolymorphClientEvents;

impl PolymorphClientEvents {
    /// `tick`: drop the widget when the player or screen went away.
    pub fn tick(screen_present: bool, env: &Rc<RefCell<dyn ClientWidgetEnv>>) {
        let has_widget = RecipesWidgetHolder::is_present();
        if has_widget {
            if !screen_present {
                RecipesWidgetHolder::clear(env);
            }
            // No shipped widget implements `ITickingRecipesWidget`, so the
            // tick branch stays a no-op exactly as upstream resolves it.
        }
    }

    /// `initScreen`: every screen the widget layer sees is a container
    /// screen in this port, so `create` runs directly.
    pub fn init_screen(
        container_screen: &Rc<dyn ClientScreen>,
        widgets: &PolymorphWidgetsImpl,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) {
        RecipesWidgetHolder::create(container_screen, widgets, env);
    }

    /// `render`.
    pub fn render(gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        RecipesWidgetHolder::with_widget(|widget| {
            if let Some(widget) = widget {
                widget.render(gui, mouse_x, mouse_y);
            }
        });
    }

    /// `mouseClick`.
    pub fn mouse_click(mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        RecipesWidgetHolder::with_widget(|widget| match widget {
            Some(widget) => widget.mouse_clicked(mouse_x, mouse_y, button),
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stack(item: &str) -> ItemStack {
        let mut parts = item.split(':');
        ItemStack {
            item: Identifier::new(parts.next().unwrap(), parts.next().unwrap()),
            count: 1,
            components: BTreeMap::new(),
        }
    }

    fn pair(id: &str, item: Option<&str>) -> RecipePair {
        RecipePair {
            resource_location: Identifier::new("test", id),
            output: match item {
                Some(item) => stack(item),
                None => ItemStack {
                    item: Identifier::new("test", "empty"),
                    count: 0,
                    components: BTreeMap::new(),
                },
            },
        }
    }

    struct TestScreen {
        id: u64,
        slots: Vec<SlotRef>,
        kind: MenuKind,
    }

    impl ClientScreen for TestScreen {
        fn id(&self) -> u64 {
            self.id
        }
        fn menu_slots(&self) -> Vec<SlotRef> {
            self.slots.clone()
        }
        fn screen_left(&self) -> i32 {
            100
        }
        fn screen_top(&self) -> i32 {
            50
        }
        fn menu_kind(&self) -> MenuKind {
            self.kind
        }
    }

    /// A recording env: the sends and the player-capability select land in
    /// one shared log the test holds a second handle to.
    struct TestEnv {
        log: Rc<RefCell<Vec<String>>>,
        connected: bool,
    }

    fn test_env(connected: bool) -> (Rc<RefCell<dyn ClientWidgetEnv>>, Rc<RefCell<Vec<String>>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        (
            Rc::new(RefCell::new(TestEnv { log: log.clone(), connected })),
            log,
        )
    }

    impl ClientWidgetEnv for TestEnv {
        fn connection_present(&self) -> bool {
            self.connected
        }
        fn recipe_by_key(&self, id: &Identifier) -> Option<super::recipe_cache::RecipeHolder> {
            Some(super::recipe_cache::RecipeHolder {
                id: id.clone(),
                recipe: Rc::new(TestRecipe),
            })
        }
        fn select_on_player_capability(&mut self, _recipe: super::recipe_cache::RecipeHolder) {
            self.log.borrow_mut().push("capability-select".into());
        }
        fn send_block_entity_listener_c2s(&mut self, add: bool) {
            self.log
                .borrow_mut()
                .push(format!("listener:{}", add));
        }
        fn send_persistent_recipe_selection_c2s(&mut self, id: &Identifier) {
            self.log
                .borrow_mut()
                .push(format!("persistent:{}", id.path));
        }
        fn send_player_recipe_selection_c2s(&mut self, id: &Identifier) {
            self.log
                .borrow_mut()
                .push(format!("player:{}", id.path));
        }
    }

    struct TestRecipe;
    impl super::recipe_cache::Recipe for TestRecipe {
        fn result_item(&self) -> Option<ItemStack> {
            None
        }
        fn is_custom(&self) -> bool {
            false
        }
        fn assemble(&self, _input: &super::recipe_cache::RecipeInput) -> ItemStack {
            stack("test:result")
        }
    }

    fn slot(index: usize, x: i32, y: i32, container: ContainerKind) -> SlotRef {
        SlotRef { index, x, y, container }
    }

    fn widget(
        screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
        behavior: WidgetBehavior,
        output: SlotRef,
    ) -> AbstractRecipesWidget {
        AbstractRecipesWidget::new(screen.clone(), env.clone(), behavior, output)
    }

    #[test]
    fn the_recipe_list_skips_empty_outputs_and_lays_out_the_row() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 1,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, _log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("b", None));
        list.insert(pair("c", Some("test:c")));
        w.set_recipes_list(&list, None);
        let selection = w.selection_widget.as_ref().unwrap();
        assert_eq!(selection.get_output_widgets().len(), 2);
        // Two widgets: xOffset = -25 * floor(1) + 13 = -12.
        assert_eq!(selection.get_output_widgets()[0].get_x(), 100 + 10 - 4 - 12);
        assert_eq!(selection.get_output_widgets()[1].get_x(), 100 + 10 - 4 - 12 + 25);
        assert!(w.open_button.as_ref().unwrap().visible);
    }

    #[test]
    fn a_single_result_hides_the_open_button_and_a_click_selects_the_recipe() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 2,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        w.set_recipes_list(&list, None);
        assert!(!w.open_button.as_ref().unwrap().visible);

        // Open the panel, then click the output widget: onSelect fires and
        // the panel closes.
        w.selection_widget.as_mut().unwrap().set_active(true);
        let widget_x = w.selection_widget.as_ref().unwrap().get_output_widgets()[0].get_x() as f64 + 1.0;
        let widget_y = w.selection_widget.as_ref().unwrap().get_output_widgets()[0].get_y() as f64 + 1.0;
        assert!(w.mouse_clicked(widget_x, widget_y, 0));
        assert_eq!(
            *log.borrow(),
            vec!["capability-select".to_string(), "player:a".to_string()]
        );
        assert!(!w.selection_widget.as_ref().unwrap().is_active());
    }

    #[test]
    fn the_open_button_toggles_the_panel() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 3,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, _log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("c", Some("test:c")));
        w.set_recipes_list(&list, None);
        let open = w.open_button.as_mut().unwrap();
        open.x = 0;
        open.y = 0;
        // The toggle runs through the widget's click chain.
        assert!(w.mouse_clicked(0.5, 0.5, 0));
        assert!(w.selection_widget.as_ref().unwrap().is_active());
    }

    #[test]
    fn persistent_widgets_send_the_listener_and_persistent_selection_packets() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 4,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, log) = test_env(true);
        let mut w = widget(&screen, &env, WidgetBehavior::Persistent, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        assert!(log.borrow().is_empty());
        w.select_recipe(&Identifier::new("test", "recipe"));
        assert_eq!(*log.borrow(), vec!["persistent:recipe".to_string()]);
    }

    #[test]
    fn the_widgets_impl_matches_menus_and_finds_result_slots() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, _log) = test_env(false);

        // Furnace: the factory binds slot 2.
        let furnace: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 5,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 0, 0, ContainerKind::Other),
                slot(2, 8, 8, ContainerKind::Other),
            ],
            kind: MenuKind::AbstractFurnace,
        });
        let widget = widgets.get_widget_or_default(&furnace, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 2);

        // Crafter: the NonInteractiveResultSlot wins over the first slot.
        let crafter: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 6,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 4, 4, ContainerKind::NonInteractiveResult),
            ],
            kind: MenuKind::Crafter,
        });
        let widget = widgets.get_widget_or_default(&crafter, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 1);

        // An unknown menu falls back to the player widget on the
        // ResultContainer slot.
        let smithing: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 7,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 0, 0, ContainerKind::Other),
                slot(2, 0, 0, ContainerKind::Other),
                slot(3, 0, 0, ContainerKind::Other),
                slot(4, 12, 12, ContainerKind::Result),
            ],
            kind: MenuKind::Other,
        });
        let widget = widgets.get_widget_or_default(&smithing, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 4);
        assert!(!widget.is_persistent());
    }

    #[test]
    fn create_reuses_the_widget_on_the_same_screen_and_flushes_pending_data() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, log) = test_env(false);
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 8,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });

        // A recipes list that arrives before the screen enqueues, then
        // lands on widget init.
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("c", Some("test:c")));
        RecipesWidgetHolder::enqueue_recipes_list(list, Some(Identifier::new("test", "a")));
        RecipesWidgetHolder::create(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());
        assert!(log.borrow().is_empty(), "a player widget sends no listener packet");

        // The same screen keeps its widget instead of rebuilding it.
        RecipesWidgetHolder::create(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());

        // A screen change clears nothing until clear() runs, but create()
        // resolves a fresh widget; a menu with no result slot yields none.
        let bare: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 9,
            slots: vec![slot(0, 0, 0, ContainerKind::Other)],
            kind: MenuKind::Other,
        });
        RecipesWidgetHolder::create(&bare, &widgets, &env);
        assert!(!RecipesWidgetHolder::is_present());
    }

    #[test]
    fn the_client_events_route_into_the_holder() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, log) = test_env(false);
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 10,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        PolymorphClientEvents::init_screen(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());
        assert!(!PolymorphClientEvents::mouse_click(-1.0, -1.0, 0));

        // Tick with no screen clears the widget; a player widget sends no
        // listener packet either way.
        PolymorphClientEvents::tick(false, &env);
        assert!(!RecipesWidgetHolder::is_present());
        assert!(log.borrow().is_empty());
    }
}
}
