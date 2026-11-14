//! Upstream `common/PolymorphRecipeManager.java`, translated 1:1. The
//! `PolymorphApi` capability lookups are a seam trait; the no-capability
//! fallback reads the manager's recipe list directly.

use crate::block_entity_recipe_data::AbstractBlockEntityRecipeData;
use crate::player_recipe_data::PlayerRecipeData;
use crate::recipe_cache::{RecipeHolder, RecipeInput, RecipeList};
use crate::recipe_data::LevelHandle;
use crate::recipe_pair::Identifier;

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
    use crate::block_entity_recipe_data::AbstractBlockEntityRecipeData;
    use crate::player_recipe_data::PlayerHandle;
    use crate::recipe_cache::Recipe;
    use crate::recipe_pair::{Identifier, ItemStack, RecipePair};
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
