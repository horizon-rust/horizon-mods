//! Upstream `common/capability/AbstractBlockEntityRecipeData.java`,
//! translated 1:1. The `BlockEntity` and inventory types are host seams:
//! the abstract `getInput()` is an input-provider closure.

use crate::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use crate::recipe_data::AbstractRecipeData;
use crate::recipe_pair::Identifier;
use crate::recipe_pair::{ItemStack, RecipePair};
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
    use crate::recipe_cache::Recipe;
    use crate::recipe_pair::Identifier;
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
